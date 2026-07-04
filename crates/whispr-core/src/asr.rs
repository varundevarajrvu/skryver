//! ASR engine wrapper. M1: Moonshine (offline, English) via sherpa-onnx.
//! Parakeet TDT (accuracy option) joins in M3 behind the same `Engine` interface.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use sherpa_rs::moonshine::{MoonshineConfig, MoonshineRecognizer};

pub const SAMPLE_RATE: u32 = 16_000;

pub struct Engine {
    rec: MoonshineRecognizer,
}

impl Engine {
    /// Load a Moonshine model from a sherpa-onnx model directory
    /// (e.g. `sherpa-onnx-moonshine-base-en-int8/`).
    pub fn load_moonshine(dir: &Path, num_threads: i32) -> Result<Self> {
        let file = |name: &str| -> Result<String> {
            let p = dir.join(name);
            if !p.exists() {
                bail!("missing model file: {}", p.display());
            }
            Ok(p.to_string_lossy().into_owned())
        };
        let t0 = Instant::now();
        let rec = MoonshineRecognizer::new(MoonshineConfig {
            preprocessor: file("preprocess.onnx")?,
            encoder: file("encode.int8.onnx")?,
            uncached_decoder: file("uncached_decode.int8.onnx")?,
            cached_decoder: file("cached_decode.int8.onnx")?,
            tokens: file("tokens.txt")?,
            num_threads: Some(num_threads),
            provider: Some("cpu".into()),
            debug: false,
        })
        .map_err(|e| anyhow::anyhow!("failed to create Moonshine recognizer: {e}"))?;
        eprintln!("[asr] moonshine loaded in {:.2}s", t0.elapsed().as_secs_f32());
        Ok(Self { rec })
    }

    /// Transcribe 16 kHz mono f32 samples.
    pub fn transcribe(&mut self, samples: &[f32]) -> String {
        self.rec.transcribe(SAMPLE_RATE, samples).text.trim().to_string()
    }
}

/// Locate the default model dir: `WHISPR_MODEL_DIR` env var, or walk up from the
/// current dir looking for `bench/models/sherpa-onnx-moonshine-base-en-int8`.
pub fn default_model_dir() -> Result<PathBuf> {
    if let Ok(d) = std::env::var("WHISPR_MODEL_DIR") {
        return Ok(PathBuf::from(d));
    }
    let mut dir = std::env::current_dir().context("cwd")?;
    loop {
        let candidate = dir.join("bench/models/sherpa-onnx-moonshine-base-en-int8");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            bail!(
                "model dir not found; set WHISPR_MODEL_DIR or run from the repo \
                 (expected bench/models/sherpa-onnx-moonshine-base-en-int8)"
            );
        }
    }
}
