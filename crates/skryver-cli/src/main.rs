//! skryver M1 headless CLI: hold F9, speak, release — text is pasted into the
//! focused app. `--wav <file>` transcribes a file instead (no mic; smoke test).
//!
//! ASR + paste run on a worker thread so the hotkey stays responsive while a
//! previous take is still transcribing; takes are processed in order.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result};
use skryver_core::{asr, audio, hotkey, inject, llm, postproc};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut models_root: Option<PathBuf> = None;
    let mut wav: Option<PathBuf> = None;
    let mut engine_kind = asr::EngineKind::Moonshine;
    let mut threads: i32 = 4; // M0: 4 threads beats 8 on the 1334U (E-core contention)
    let mut dict_path: Option<PathBuf> = None;
    let mut save_takes: Option<PathBuf> = None;
    let mut use_llm = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--models" => models_root = it.next().map(PathBuf::from),
            "--engine" => {
                engine_kind = it.next().context("--engine needs a value")?.parse()?;
            }
            "--wav" => wav = it.next().map(PathBuf::from),
            "--threads" => threads = it.next().and_then(|s| s.parse().ok()).unwrap_or(4),
            "--dict" => dict_path = it.next().map(PathBuf::from),
            "--save-takes" => save_takes = it.next().map(PathBuf::from),
            "--llm" => use_llm = true,
            "--help" | "-h" => {
                println!(
                    "skryver-cli [--engine moonshine|parakeet] [--models DIR] [--threads N] \
                     [--dict FILE] [--save-takes DIR] [--llm] [--wav FILE]"
                );
                return Ok(());
            }
            other => eprintln!("ignoring unknown arg: {other}"),
        }
    }

    // Dictionary: --dict, else skryver.dict.txt next to cwd if present.
    let dict = match &dict_path {
        Some(p) => postproc::Dictionary::load(p)?,
        None => {
            let default = PathBuf::from("skryver.dict.txt");
            if default.exists() {
                postproc::Dictionary::load(&default)?
            } else {
                postproc::Dictionary::empty()
            }
        }
    };
    if !dict.is_empty() {
        eprintln!("[dict] {} rule(s) loaded", dict.len());
    }
    if let Some(dir) = &save_takes {
        std::fs::create_dir_all(dir).context("create takes dir")?;
    }

    let root = match models_root {
        Some(d) => d,
        None => asr::default_models_root()?,
    };
    let mut engine = asr::Engine::load(engine_kind, &root, threads)?;

    let formatter = if use_llm {
        let server = llm::find_server_exe(&root)
            .context("llama-server.exe not found (expected <exe_dir>/llama/ when packaged, or tools/llama/ near models root in dev)")?;
        let gguf = root.join("qwen2.5-1.5b-instruct-q4_k_m.gguf");
        anyhow::ensure!(gguf.exists(), "LLM model missing: {}", gguf.display());
        Some(llm::Formatter::spawn(&server, &gguf, threads as usize)?)
    } else {
        None
    };

    if let Some(path) = wav {
        return transcribe_wav(&mut engine, &dict, formatter.as_ref(), &path);
    }

    let rec = audio::Recorder::open()?;
    let key = hotkey::HoldKey::new(hotkey::VK_F9);
    let stop = AtomicBool::new(false);

    // Worker: transcribe + paste off the hotkey loop, in take order.
    // One hotkey, automatic routing: clean speech pastes instantly; messy takes
    // (fillers, stutters, run-on lists) go through the LLM when it's available.
    let has_llm = formatter.is_some();
    let (tx, rx) = mpsc::channel::<(Vec<f32>, Instant)>();
    let worker = std::thread::spawn(move || {
        let mut take_no = 0u32;
        for (samples, t_release) in rx {
            let audio_s = samples.len() as f32 / asr::SAMPLE_RATE as f32;
            let text = dict.apply(&engine.transcribe(&samples));
            let asr_ms = t_release.elapsed().as_millis();
            let text = match &formatter {
                Some(f) if !text.is_empty() && postproc::needs_rephrase(&text) => {
                    eprintln!("[llm] rephrasing…");
                    f.format(&text)
                }
                _ => postproc::bulletize(&text),
            };
            if let Some(dir) = &save_takes {
                take_no += 1;
                let path = dir.join(format!("take{take_no:03}.wav"));
                if let Err(e) = write_wav(&path, &samples) {
                    eprintln!("[takes] save failed: {e}");
                } else {
                    eprintln!("[takes] {} — \"{text}\"", path.display());
                }
            }
            if text.is_empty() {
                eprintln!("[asr] (no speech detected) — {asr_ms} ms");
                continue;
            }
            if let Err(e) = inject::paste_text(&text) {
                eprintln!("[err] paste failed: {e} — transcript: \"{text}\"");
                continue;
            }
            let total_ms = t_release.elapsed().as_millis();
            eprintln!("[ok] {audio_s:.1}s audio → asr {asr_ms} ms, total {total_ms} ms");
            eprintln!("     \"{text}\"");
        }
    });

    println!(
        "skryver ready ({engine_kind:?}) — hold F9 to dictate{}. Ctrl+C to quit.",
        if has_llm { " (auto AI cleanup on messy takes)" } else { "" }
    );
    loop {
        if !key.wait_down(&stop) {
            break;
        }
        rec.start();
        eprintln!("[rec] ● recording…");
        key.wait_up();
        let t_release = Instant::now();
        let samples = rec.stop();
        if (samples.len() as f32 / asr::SAMPLE_RATE as f32) < 0.3 {
            eprintln!("[rec] too short, ignored");
            continue;
        }
        if tx.send((samples, t_release)).is_err() {
            break;
        }
    }
    drop(tx);
    let _ = worker.join();
    Ok(())
}

fn write_wav(path: &std::path::Path, samples: &[f32]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: asr::SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)?;
    }
    w.finalize()?;
    Ok(())
}

fn transcribe_wav(
    engine: &mut asr::Engine,
    dict: &postproc::Dictionary,
    formatter: Option<&llm::Formatter>,
    path: &PathBuf,
) -> Result<()> {
    let mut reader = hound::WavReader::open(path).context("open wav")?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.sample_rate == asr::SAMPLE_RATE && spec.channels == 1,
        "expected 16 kHz mono wav, got {} Hz {} ch",
        spec.sample_rate,
        spec.channels
    );
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|s| s.map(|v| v as f32 / 32768.0))
            .collect::<Result<_, _>>()?,
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
    };
    let t0 = Instant::now();
    let mut text = dict.apply(&engine.transcribe(&samples));
    text = match formatter {
        Some(f) => f.format(&text),
        None => postproc::bulletize(&text),
    };
    println!("{text}");
    eprintln!(
        "[wav] {:.1}s audio in {} ms",
        samples.len() as f32 / asr::SAMPLE_RATE as f32,
        t0.elapsed().as_millis()
    );
    Ok(())
}
