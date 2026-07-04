//! Microphone capture via cpal (WASAPI on Windows).
//!
//! The input stream stays open for the life of the `Recorder`; a gate flag
//! controls whether callbacks append to the buffer. This keeps key-release
//! latency free of device-open cost. Captured audio is downmixed to mono at
//! the device rate, then resampled to 16 kHz when a take is finished.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::asr::SAMPLE_RATE;

pub struct Recorder {
    _stream: cpal::Stream,
    buf: Arc<Mutex<Vec<f32>>>,
    recording: Arc<AtomicBool>,
    device_rate: u32,
    channels: u16,
}

impl Recorder {
    pub fn open() -> Result<Self> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device (is a microphone connected?)"))?;
        let name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "<unknown>".into());
        let config = device.default_input_config().context("query input config")?;
        let device_rate = config.sample_rate();
        let channels = config.channels();
        eprintln!("[audio] device: {name} ({device_rate} Hz, {channels} ch, {:?})", config.sample_format());

        let buf = Arc::new(Mutex::new(Vec::<f32>::new()));
        let recording = Arc::new(AtomicBool::new(false));
        let (b, r) = (buf.clone(), recording.clone());
        let err_fn = |e| eprintln!("[audio] stream error: {e}");

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                config.into(),
                move |data: &[f32], _: &_| {
                    if r.load(Ordering::Relaxed) {
                        b.lock().unwrap().extend_from_slice(data);
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                config.into(),
                move |data: &[i16], _: &_| {
                    if r.load(Ordering::Relaxed) {
                        b.lock().unwrap().extend(data.iter().map(|&s| s as f32 / 32768.0));
                    }
                },
                err_fn,
                None,
            )?,
            other => return Err(anyhow!("unsupported sample format: {other:?}")),
        };
        stream.play().context("start input stream")?;

        Ok(Self { _stream: stream, buf, recording, device_rate, channels })
    }

    pub fn start(&self) {
        self.buf.lock().unwrap().clear();
        self.recording.store(true, Ordering::Relaxed);
    }

    /// Stop capturing and return the take as normalized 16 kHz mono samples.
    pub fn stop(&self) -> Vec<f32> {
        self.recording.store(false, Ordering::Relaxed);
        let raw = std::mem::take(&mut *self.buf.lock().unwrap());
        let mono = downmix(&raw, self.channels as usize);
        let mut out = resample(&mono, self.device_rate, SAMPLE_RATE);
        normalize(&mut out);
        out
    }
}

/// Remove DC offset and peak-normalize to 0.9 (skipped for near-silence so we
/// don't amplify noise floors into phantom speech).
fn normalize(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let mut peak = 0f32;
    for s in samples.iter_mut() {
        *s -= mean;
        peak = peak.max(s.abs());
    }
    if peak > 0.01 {
        let gain = 0.9 / peak;
        for s in samples.iter_mut() {
            *s *= gain;
        }
    }
}

fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Downsample with an anti-aliasing low-pass. Without the filter, energy above
/// the target Nyquist (8 kHz) folds back into the speech band and audibly hurts
/// ASR on consonants — measured as real-world misrecognitions in M1 testing.
fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let filtered = if from > to { lowpass(input, from, to) } else { input.to_vec() };
    let ratio = from as f64 / to as f64;
    let out_len = (filtered.len() as f64 / ratio) as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = filtered[idx.min(filtered.len() - 1)];
            let b = filtered[(idx + 1).min(filtered.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

/// 63-tap windowed-sinc FIR low-pass at 0.9x the target Nyquist.
fn lowpass(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    const TAPS: usize = 63;
    let cutoff = 0.9 * (to as f32 / 2.0) / from as f32; // normalized (0..0.5)
    let mid = (TAPS / 2) as isize;
    let mut kernel = [0f32; TAPS];
    let mut sum = 0f32;
    for (i, k) in kernel.iter_mut().enumerate() {
        let n = i as isize - mid;
        let sinc = if n == 0 {
            2.0 * cutoff
        } else {
            (2.0 * std::f32::consts::PI * cutoff * n as f32).sin() / (std::f32::consts::PI * n as f32)
        };
        // Hamming window
        let w = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (TAPS - 1) as f32).cos();
        *k = sinc * w;
        sum += *k;
    }
    for k in kernel.iter_mut() {
        *k /= sum;
    }
    let n = input.len();
    (0..n)
        .map(|i| {
            let mut acc = 0f32;
            for (j, k) in kernel.iter().enumerate() {
                let idx = i as isize + (j as isize - mid);
                if idx >= 0 && (idx as usize) < n {
                    acc += input[idx as usize] * k;
                }
            }
            acc
        })
        .collect()
}
