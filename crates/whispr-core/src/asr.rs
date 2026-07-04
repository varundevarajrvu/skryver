//! ASR engines via sherpa-onnx. Two options per the M0 benchmark:
//! - Moonshine base int8 — lowest latency (default)
//! - Parakeet TDT 0.6B v2 int8 — better accuracy (accents/jargon), ~2x slower

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use sherpa_rs::moonshine::{MoonshineConfig, MoonshineRecognizer};
use sherpa_rs::transducer::{TransducerConfig, TransducerRecognizer};

pub const SAMPLE_RATE: u32 = 16_000;

pub const MOONSHINE_DIR: &str = "sherpa-onnx-moonshine-base-en-int8";
pub const PARAKEET_DIR: &str = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineKind {
    Moonshine,
    Parakeet,
}

impl std::str::FromStr for EngineKind {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "moonshine" => Ok(Self::Moonshine),
            "parakeet" => Ok(Self::Parakeet),
            other => bail!("unknown engine '{other}' (expected: moonshine, parakeet)"),
        }
    }
}

pub enum Engine {
    Moonshine(MoonshineRecognizer),
    Parakeet(TransducerRecognizer),
}

impl Engine {
    /// `models_root` is the directory containing the sherpa-onnx model folders.
    pub fn load(kind: EngineKind, models_root: &Path, num_threads: i32) -> Result<Self> {
        let t0 = Instant::now();
        let engine = match kind {
            EngineKind::Moonshine => {
                let dir = models_root.join(MOONSHINE_DIR);
                let file = |name: &str| model_file(&dir, name);
                Self::Moonshine(
                    MoonshineRecognizer::new(MoonshineConfig {
                        preprocessor: file("preprocess.onnx")?,
                        encoder: file("encode.int8.onnx")?,
                        uncached_decoder: file("uncached_decode.int8.onnx")?,
                        cached_decoder: file("cached_decode.int8.onnx")?,
                        tokens: file("tokens.txt")?,
                        num_threads: Some(num_threads),
                        provider: Some("cpu".into()),
                        debug: false,
                    })
                    .map_err(|e| anyhow::anyhow!("create Moonshine recognizer: {e}"))?,
                )
            }
            EngineKind::Parakeet => {
                let dir = models_root.join(PARAKEET_DIR);
                let file = |name: &str| model_file(&dir, name);
                Self::Parakeet(
                    TransducerRecognizer::new(TransducerConfig {
                        encoder: file("encoder.int8.onnx")?,
                        decoder: file("decoder.int8.onnx")?,
                        joiner: file("joiner.int8.onnx")?,
                        tokens: file("tokens.txt")?,
                        num_threads,
                        sample_rate: SAMPLE_RATE as i32,
                        feature_dim: 80,
                        model_type: "nemo_transducer".into(),
                        debug: false,
                        ..Default::default()
                    })
                    .map_err(|e| anyhow::anyhow!("create Parakeet recognizer: {e}"))?,
                )
            }
        };
        eprintln!("[asr] {kind:?} loaded in {:.2}s", t0.elapsed().as_secs_f32());
        Ok(engine)
    }

    /// Transcribe 16 kHz mono f32 samples.
    pub fn transcribe(&mut self, samples: &[f32]) -> String {
        match self {
            Self::Moonshine(rec) => rec.transcribe(SAMPLE_RATE, samples).text.trim().to_string(),
            Self::Parakeet(rec) => rec.transcribe(SAMPLE_RATE, samples).trim().to_string(),
        }
    }
}

fn model_file(dir: &Path, name: &str) -> Result<String> {
    let p = dir.join(name);
    if !p.exists() {
        bail!("missing model file: {}", p.display());
    }
    Ok(p.to_string_lossy().into_owned())
}

/// Locate the models root: `WHISPR_MODELS` env var, or walk up from the current
/// dir looking for `bench/models`.
pub fn default_models_root() -> Result<PathBuf> {
    if let Ok(d) = std::env::var("WHISPR_MODELS") {
        return Ok(PathBuf::from(d));
    }
    let mut dir = std::env::current_dir().context("cwd")?;
    loop {
        let candidate = dir.join("bench/models");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            bail!("models root not found; set WHISPR_MODELS or run from the repo");
        }
    }
}
