//! whispr M1 headless CLI: hold F9, speak, release — text is pasted into the
//! focused app. `--wav <file>` transcribes a file instead (no mic; smoke test).

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context, Result};
use whispr_core::{asr, audio, hotkey, inject};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut model_dir: Option<PathBuf> = None;
    let mut wav: Option<PathBuf> = None;
    let mut threads: i32 = 4; // M0: 4 threads beats 8 on the 1334U (E-core contention)
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--model-dir" => model_dir = it.next().map(PathBuf::from),
            "--wav" => wav = it.next().map(PathBuf::from),
            "--threads" => threads = it.next().and_then(|s| s.parse().ok()).unwrap_or(4),
            "--help" | "-h" => {
                println!("whispr-cli [--model-dir DIR] [--threads N] [--wav FILE]");
                return Ok(());
            }
            other => eprintln!("ignoring unknown arg: {other}"),
        }
    }

    let dir = match model_dir {
        Some(d) => d,
        None => asr::default_model_dir()?,
    };
    eprintln!("[asr] loading model from {}", dir.display());
    let mut engine = asr::Engine::load_moonshine(&dir, threads)?;

    if let Some(path) = wav {
        return transcribe_wav(&mut engine, &path);
    }

    let rec = audio::Recorder::open()?;
    let key = hotkey::HoldKey::new(hotkey::VK_F9);
    let stop = AtomicBool::new(false);
    println!("whispr ready — hold F9 to dictate, release to paste. Ctrl+C to quit.");

    loop {
        if !key.wait_down(&stop) {
            return Ok(());
        }
        rec.start();
        eprintln!("[rec] ● recording…");
        key.wait_up();
        let t_release = Instant::now();
        let samples = rec.stop();
        let audio_s = samples.len() as f32 / asr::SAMPLE_RATE as f32;
        if audio_s < 0.3 {
            eprintln!("[rec] too short ({audio_s:.2}s), ignored");
            continue;
        }
        let text = engine.transcribe(&samples);
        let asr_ms = t_release.elapsed().as_millis();
        if text.is_empty() {
            eprintln!("[asr] (no speech detected) — {asr_ms} ms");
            continue;
        }
        inject::paste_text(&text)?;
        let total_ms = t_release.elapsed().as_millis();
        eprintln!("[ok] {audio_s:.1}s audio → asr {asr_ms} ms, total {total_ms} ms");
        eprintln!("     \"{text}\"");
    }
}

fn transcribe_wav(engine: &mut asr::Engine, path: &PathBuf) -> Result<()> {
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
    let text = engine.transcribe(&samples);
    println!("{text}");
    eprintln!(
        "[wav] {:.1}s audio in {} ms",
        samples.len() as f32 / asr::SAMPLE_RATE as f32,
        t0.elapsed().as_millis()
    );
    Ok(())
}
