//! The dictation pipeline thread: hotkey → capture → ASR → route → paste.
//! Mirrors skryver-cli but settings-aware and reloadable: hotkey/dict changes
//! apply instantly; engine/LLM-mode changes arrive as a reload signal.

use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};
use skryver_core::{asr, audio, hotkey, inject, llm, postproc};

use crate::{HistoryEntry, Shared};

const HISTORY_CAP: usize = 100;

pub fn run(app: AppHandle, shared: Arc<Shared>, ctl_rx: mpsc::Receiver<()>) {
    loop {
        if let Err(e) = run_once(&app, &shared, &ctl_rx) {
            set_status(&app, &shared, &format!("error: {e:#}"));
            // Wait for a settings change (reload signal) before retrying.
            if ctl_rx.recv().is_err() {
                return;
            }
        }
    }
}

/// One pipeline "generation": loads engine per current settings, serves takes
/// until a reload signal arrives, then returns Ok to be restarted.
fn run_once(app: &AppHandle, shared: &Arc<Shared>, ctl_rx: &mpsc::Receiver<()>) -> anyhow::Result<()> {
    let (engine_kind, llm_mode) = {
        let s = shared.settings.read().unwrap();
        (s.engine.parse::<asr::EngineKind>()?, s.llm_mode.clone())
    };

    set_status(app, shared, "loading model…");
    let root = asr::default_models_root()?;
    let mut engine = asr::Engine::load(engine_kind, &root, 4)?;

    let formatter = if llm_mode != "off" {
        set_status(app, shared, "starting AI cleanup…");
        let server = llm::find_server_exe(&root);
        let gguf = root.join("qwen2.5-1.5b-instruct-q4_k_m.gguf");
        match server {
            Some(server) if gguf.exists() => Some(llm::Formatter::spawn(&server, &gguf, 8)?),
            _ => {
                set_status(app, shared, "AI cleanup unavailable (model/runtime missing)");
                None
            }
        }
    } else {
        None
    };

    eprintln!(
        "[pipe] engine={:?} llm_mode={} formatter={}",
        engine_kind,
        llm_mode,
        formatter.is_some()
    );

    let rec = audio::Recorder::open()?;
    set_status(app, shared, "ready");

    loop {
        // Reload requested?
        if ctl_rx.try_recv().is_ok() {
            return Ok(());
        }
        let vk = shared.settings.read().unwrap().hotkey_vk;
        let key = hotkey::HoldKey::new(vk);
        if !shared.enabled.load(Ordering::Relaxed) || !key.is_down() {
            std::thread::sleep(Duration::from_millis(8));
            continue;
        }

        rec.start();
        set_status(app, shared, "recording…");
        key.wait_up();
        let t_release = Instant::now();
        let samples = rec.stop();
        if (samples.len() as f32 / asr::SAMPLE_RATE as f32) < 0.3 {
            set_status(app, shared, "ready");
            continue;
        }

        set_status(app, shared, "transcribing…");
        let (dict_rules, llm_mode) = {
            let s = shared.settings.read().unwrap();
            (s.dict.clone(), s.llm_mode.clone())
        };
        let dict = postproc::Dictionary::from_rules(dict_rules);
        let text = dict.apply(&engine.transcribe(&samples));
        eprintln!("[take] asr: {:?} (len={})", text, text.len());
        if text.is_empty() {
            set_status(app, shared, "ready");
            continue;
        }

        let nr = postproc::needs_rephrase(&text);
        let use_llm = formatter.is_some() && (llm_mode == "always" || (llm_mode == "auto" && nr));
        eprintln!(
            "[take] llm_mode={} needs_rephrase={} -> use_llm={}",
            llm_mode, nr, use_llm
        );
        let (text, path) = if use_llm {
            set_status(app, shared, "polishing…");
            let pre = text.clone();
            let post = formatter.as_ref().unwrap().format(&text);
            eprintln!("[take] llm in={:?} out={:?}", pre, post);
            (post, "ai")
        } else {
            (postproc::bulletize(&text), "fast")
        };
        eprintln!("[take] FINAL path={} text={:?}", path, text);

        if let Err(e) = inject::paste_text(&text) {
            set_status(app, shared, &format!("paste failed: {e}"));
        } else {
            set_status(app, shared, "ready");
        }

        let entry = HistoryEntry {
            time: clock_hms(),
            path: path.into(),
            text,
            ms: t_release.elapsed().as_millis() as u64,
        };
        {
            let mut h = shared.history.lock().unwrap();
            h.push_front(entry.clone());
            h.truncate(HISTORY_CAP);
        }
        let _ = app.emit("take", &entry);
    }
}

fn set_status(app: &AppHandle, shared: &Arc<Shared>, status: &str) {
    *shared.status.lock().unwrap() = status.to_string();
    let _ = app.emit("status", status);
}

fn clock_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Local offset: IST (UTC+5:30) hardcoding avoided — show UTC-agnostic wall
    // clock via modulo; good enough for a session-relative history list.
    let day = secs % 86_400;
    format!("{:02}:{:02}:{:02}", day / 3600, (day % 3600) / 60, day % 60)
}
