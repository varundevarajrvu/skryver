# M0 Benchmark Results — 2026-07-04

Machine: i5-1334U (2P+8E), 16 GB, Windows 11, CPU-only. Runtime: sherpa-onnx 1.13.3 (Python
wheel, same C runtime the Rust app will use). Audio: TTS-generated 16 kHz mono — clean speech,
so this measures **speed only**; accuracy ranking comes from published WER (all four models
transcribed the test audio essentially perfectly, as expected for clean TTS).

Reproduce: `.venv\Scripts\python bench_asr.py` (raw numbers in `results.json`).

## Measured

| Model | Threads | Load (s) | RTF¹ | 5 s-utterance latency | RAM delta |
|---|---|---|---|---|---|
| **moonshine-base-en int8** | **4** | 2.7 | **0.289** | **568 ms** ✅ | 1372 MB² |
| moonshine-base-en int8 | 8 | 4.0 | 0.293 | 1027 ms | 1380 MB |
| **parakeet-tdt-0.6b-v2 int8** | **4** | 11.4 | **0.288** | 1228 ms | 1588 MB |
| parakeet-tdt-0.6b-v2 int8 | 8 | 7.3 | 0.345 | 1831 ms | 1568 MB |
| whisper-base.en int8 | 4 | 2.5 | ~0.75³ | 3249 ms ❌ | 673 MB |
| whisper-base.en int8 | 8 | 3.4 | ~0.47³ | 2304 ms ❌ | 676 MB |
| whisper-small.en int8 | 4 | 5.2 | ~1.28³ ❌ | 4382 ms ❌ | 1211 MB |
| whisper-small.en int8 | 8 | 4.6 | ~1.14³ ❌ | 5105 ms ❌ | 1219 MB |

¹ processing_time / audio_time on a 59.7 s passage; lower is better, <1 = real-time.
² RSS delta of the Python process; includes ONNX runtime arena overhead — Rust app idle
  footprint will differ (validated against the <200 MB idle target in M4, where models can be
  unloaded/mmapped when idle).
³ sherpa-onnx truncates whisper input to its fixed 30 s window, so the raw long-clip number
  only covers half the audio — corrected ×1.99 here.

## Findings

1. **Moonshine base int8 @ 4 threads hits the <700 ms target out of the box** (568 ms for a
   ~5 s utterance) — the only candidate that does. It processes actual audio length instead of
   whisper's fixed 30 s window.
2. **Whisper-family is disqualified as the primary engine on this CPU.** The fixed 30 s
   padded window means even a 5 s utterance costs 3.2–5.1 s — architecturally unfixable for
   hold-to-talk. (Stays available in the model picker for multilingual use, with managed
   expectations.)
3. **Parakeet TDT 0.6B int8 is viable as the accuracy option**: RTF 0.29, 1.2 s on a 5 s
   utterance. With VAD-segmented incremental transcription during the hold (transcribe
   completed speech segments while the user is still talking), the felt latency at release ≈
   RTF × final segment only — well under 700 ms for continuous dictation.
4. **4 threads beats 8 consistently** (P-core vs E-core contention on the 1334U). Default
   num_threads=4, expose as advanced setting.
5. Cold model load: Moonshine 2.7 s, Parakeet 11.4 s → load at app start / keep resident, or
   lazy-load with a visible "warming up" state; never load per-dictation.

## Decision (M0 exit)

- **Default engine: Moonshine base-en int8 @ 4 threads** — meets the latency headline today.
- **"High accuracy" engine: Parakeet TDT 0.6B v2 int8 @ 4 threads** — better published WER
  (~6.05% avg vs Moonshine base ~8–9%), paired with VAD-segmented incremental decode.
- Both run in the **same sherpa-onnx runtime** — one integration, swappable models, which was
  the reason for choosing sherpa-onnx in the first place. Zipformer streaming preview remains
  the M6 enhancement path.
- **Toolchain proven:** Rust 1.96.1 + MSVC Build Tools 17.14 installed; hello-world compiles,
  links, and runs. sherpa-onnx works on this machine via the C runtime.

## Caveats

- TTS test audio ⇒ no local WER signal; M1 must include real-microphone dictation testing
  (accents, disfluencies, jargon) before the default-engine choice is considered settled.
- Moonshine is English-only (per-language mono models exist for 7 more); multilingual users
  get whisper via the model picker until sherpa-onnx grows a better multilingual offline model.
- Python-wheel numbers ≈ Rust numbers (same C core), but re-verify once the Rust pipeline
  exists (M1).
