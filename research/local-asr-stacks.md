# Fully-Local Speech Recognition Stacks for Real-Time Dictation — Research Report

> Raw report from research subagent r3 (Phase 1). Feeds into `findings.md`.

Target hardware:
- **(A)** Windows 11, Intel i5-1334U (10 cores: 2P+8E, Raptor Lake-U, AVX2/AVX-VNNI), 16GB RAM, Intel UHD iGPU, no CUDA
- **(B)** MacBook M1 Pro, 16GB RAM

Note on RTF convention: sources are inconsistent. Standard **RTF** = processing_time / audio_time (lower is faster, <1 = faster than real time). Some community benchmarks report **RTFx** or an inverted "speed factor" (higher is faster). Convention flagged per figure.

---

## 1. whisper.cpp

- **License: MIT.** Repo: [ggml-org/whisper.cpp](https://github.com/ggml-org/whisper.cpp)
- **Disk/RAM per model** (official table): tiny 75MiB/~273MB, base 142MiB/~388MB, small 466MiB/~852MB, medium 1.5GiB/~2.1GB, large 2.9GiB/~3.9GB. large-v3-turbo ~1.6GB disk (809M params, distilled decoder 32→4 layers) ([Whisper Notes](https://whispernotes.app/blog/introducing-whisper-large-v3-turbo)).
- **Quantization:** Q4_0/Q4_1, Q5_0/Q5_1, Q8_0, F16. Community benchmarking on a legacy x86 CPU (i5-460M, 2010, no AVX) found **Q4_0 fastest across all sizes; Q5_0/Q5_1 were 3–5.5x slower than Q4_0 on CPU** (unpack overhead); Q8_0 fast but strictly worse than Q4_0 ([Discussion #3752](https://github.com/ggml-org/whisper.cpp/discussions/3752)). RTF table from that discussion (their convention: **higher = faster**):

  | Model | Q4_0 | Q5_0 | Q8_0 | F16 |
  |---|---|---|---|---|
  | tiny | 1.58x | 0.53x | 1.53x | 0.43x |
  | base | 0.86x | 0.23x | 0.82x | 0.18x |
  | small | 0.28x | 0.06x | 0.24x | 0.04x |
  | medium | 0.096x | 0.018x | 0.083x | 0.014x |
  | large-v3-turbo | 0.077x | N/A | 0.062x | 0.009x |

  That's a 2010-era 2-core CPU — the i5-1334U (10 cores, AVX2) should beat it by roughly an order of magnitude (extrapolated, not measured). Modern x86 laptops: whisper.cpp base ~15x real-time ([promptquorum](https://www.promptquorum.com/power-local-llm/local-whisper-stt-comparison-2026), aggregator, treat cautiously). For large-v3: FP16 can't hit real-time even at 16 threads (RTF=1.26), but **INT4 large-v3 reaches RTF=0.91 at 8 threads / 0.57 at 16 threads** ([arXiv 2503.09905](https://arxiv.org/pdf/2503.09905)).
- **Streaming:** built-in `whisper-stream` example (~0.5s windows, SDL2) — basic sliding-window, visible re-transcription artifacts at chunk boundaries; OK for MVP, not a proper incremental algorithm.
- **Metal (Apple Silicon):** full GPU inference; ~30–60% speedup over CPU-only, gap widens with model size ([promptquorum Metal benchmark](https://www.promptquorum.com/local-llms/apple-silicon-whisper-metal-benchmark)). On M1, large-v3 "barely usable for live transcription" even with Metal; tiny/base/small exceed 10x real-time on M2 Pro+ ([justvoice.ai](https://justvoice.ai/blog/whisper-benchmark-apple-silicon-m3-m4)).
- **Vulkan/OpenVINO for Intel iGPU:** both supported. OpenVINO accelerates only the **encoder**; a user reported large-v3-turbo in ~3s via OpenVINO+iGPU ([Discussion #2650](https://github.com/ggml-org/whisper.cpp/discussions/2650)). Vulkan gave 3-4x RTF improvement on AMD 680M / Intel Arc iGPUs ([Phoronix](https://www.phoronix.com/news/Whisper-cpp-1.8.3-12x-Perf)). BUT SYCL beat Vulkan 3x on some Intel setups, and **weak non-Arc Intel UHD iGPUs may lack compute features and silently fall back to CPU** ([Discussion #2996](https://github.com/ggml-org/whisper.cpp/discussions/2996), [#2662](https://github.com/ggml-org/whisper.cpp/discussions/2662)) — **the i5-1334U's UHD iGPU benefit is uncertain; benchmark, don't assume.**
- **Bindings:** Rust (whisper-rs), Python (whispercpp), Node (official), Go, Java, .NET, Swift, etc.
- **Apple-specific alternative: WhisperKit (Argmax, MIT)** — Swift/CoreML, auto-dispatch across ANE/GPU/CPU; 2.2% WER with large-v3-turbo at real-time streaming latency on Neural Engine; ~15–30x real-time for large-v3 via CoreML vs ~1x plain PyTorch on M1 ([whisperkit-coreml](https://huggingface.co/argmaxinc/whisperkit-coreml), [MacParakeet](https://macparakeet.com/blog/whisper-to-parakeet-neural-engine/)).

## 2. faster-whisper (CTranslate2)

- **License: MIT** (both) ([LICENSE](https://github.com/SYSTRAN/faster-whisper/blob/master/LICENSE)).
- **int8 CPU:** "Up to 4x faster than openai/whisper for the same accuracy, using less memory" ([README](https://github.com/SYSTRAN/faster-whisper)); int8 CPU inference ~1/4 baseline time ([localaimaster](https://localaimaster.com/blog/faster-whisper-guide)).
- **Comparative RTF (community, modern x86, higher=faster):** base int8 CPU ~20x real-time vs whisper.cpp base ~15x; tiny int8 ~32x ([promptquorum](https://www.promptquorum.com/power-local-llm/local-whisper-stt-comparison-2026), aggregator estimate). faster-whisper likely edges whisper.cpp on raw x86 int8 throughput for small/base.
- **Memory (approx, CPU):** tiny ~273MB, base ~1GB, small ~2GB, large-v3 ~10GB float → ~3GB int8. 16GB comfortable for tiny→medium int8.
- **Streaming wrappers:**
  - **whisper_streaming** (ufal) — "local agreement" incremental decoding, **3.3s latency** on long-form speech ([ufal/whisper_streaming](https://github.com/ufal/whisper_streaming)) — the most legitimate real-streaming wrapper for Whisper-family.
  - **WhisperLive** (Collabora) — client/server; Faster-Whisper, TensorRT, OpenVINO backends ([collabora/WhisperLive](https://github.com/collabora/WhisperLive)).
  - **wyoming-faster-whisper** (rhasspy) — Wyoming-protocol service shim ([repo](https://github.com/rhasspy/wyoming-faster-whisper)).

## 3. Moonshine (Useful Sensors)

- **License: MIT** core + weights ([moonshine-ai/moonshine](https://github.com/moonshine-ai/moonshine), [RealtimeSTT licenses table](https://github.com/KoljaB/RealtimeSTT/blob/master/docs/licenses.md)).
- **Sizes:** tiny (26–34M), base (58M), **small-streaming (123M), medium-streaming (245M)**.
- **Architecture:** Moonshine v2 "ergodic streaming encoder" — 50Hz frontend + sliding-window position-free Transformer engineered for latency-critical streaming; caches encoder output and decoder state so continuous audio isn't redundantly recomputed ([arXiv 2602.12241](https://arxiv.org/html/2602.12241v1)). The key architectural difference vs whisper-family chunk-and-rerun.
- **Latency/accuracy:** medium-streaming 6.65% WER vs Whisper large-v3 7.44% at ~1/6 params; measured end-to-end latency **~107ms vs ~11,286ms for Whisper large-v3 on the same MacBook** ([README](https://github.com/moonshine-ai/moonshine)) — strongest argument for Moonshine if English-first is acceptable.
- **Languages:** mono-lingual models per language (EN + ES/ZH/JA/KO/VI/UK/AR); streaming story most mature for English.
- **ONNX:** ships via ONNXRuntime, memory-mappable `.ort` format; tiny at 237ms on Raspberry Pi 5 — trivially light for the i5-1334U.
- **sherpa-onnx integration:** Moonshine Streaming is a first-class model in sherpa-onnx.

## 4. NVIDIA Parakeet / Canary (NeMo)

- **Leaderboard:** Canary-Qwen-2.5B tops Open ASR Leaderboard at 5.63% WER ([Northflank](https://northflank.com/blog/best-open-source-speech-to-text-stt-model-in-2026-benchmarks)). **Parakeet CTC 1.1B: RTFx 2793.75 vs Whisper large-v3's 68.56** (~40x faster) at 6.68% vs 6.43% WER ([HF leaderboard blog](https://huggingface.co/blog/open-asr-leaderboard)). Parakeet TDT 0.6B v2 ranks 10th on WER.
- **License:** **Parakeet: CC-BY-4.0** (commercial OK). Original Canary: CC BY-NC 4.0 (weights, non-commercial); **Canary-1B-v2 moved to CC-BY-4.0** ([NVIDIA blog](https://developer.nvidia.com/blog/new-standard-for-speech-recognition-and-translation-from-the-nvidia-nemo-canary-model/)).
- **CPU without CUDA:** feasible via ONNX — sherpa-onnx has pre-converted Parakeet models (e.g. parakeet-tdt-0.6b-v3), pure CPU cross-platform ([sherpa NeMo docs](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html)); **onnx-asr** package supports Parakeet v2 (EN)/v3 (multilingual), CPU/DirectML/CoreML backends, no PyTorch dependency ([PyPI](https://pypi.org/project/onnx-asr/)).
- **Apple Silicon:** VoiceInk ships Parakeet via **FluidAudio** (CoreML/ANE) — likely the best Parakeet path on M1 Pro.
- **Caveat:** offline/non-streaming by default in current ONNX conversions; true streaming needs a separate architecture (see sherpa Zipformer).

## 5. sherpa-onnx / Vosk

- **sherpa-onnx** — **Apache-2.0** ([LICENSE](https://github.com/k2-fsa/sherpa-onnx/blob/master/LICENSE)). Unified ONNX framework (Next-gen Kaldi): streaming zipformer/transducer ASR, TTS, VAD, diarization; 12 language bindings; CPU-only by default.
  - **Streaming Zipformer transducer**: purpose-built frame-synchronous streaming — ~43.7ms avg latency at RTF≈0.022 on M3 Max; "stays abreast of real-time audio on 1–2 modest CPU cores" ([zipformer docs](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/online-transducer/zipformer-transducer-models.html)). Strongest concrete low-latency streaming evidence in this survey (Apple Silicon numbers, extrapolate cautiously to Intel).
  - int8 streaming Zipformer models <200MB; embedded-class hardware still real-time → very comfortable headroom on the i5-1334U.
  - Hosts Whisper, Moonshine, Parakeet, Paraformer under one runtime.
- **Vosk** — **Apache-2.0**. Native streaming, CPU-only, compact; "won't match transformer-based accuracy," noticeably worse in noise ([sinologic](https://www.sinologic.net/en/2026-05/vosk-vs-whisper-local-the-ultimate-2026-guide-to-self-hosted-speech-recognition-stt.html)). Largely superseded by sherpa-onnx streaming Zipformer for new projects.

## 6. VAD

- **Silero VAD (v5/v6)** — **MIT**, no telemetry ([snakers4/silero-vad](https://github.com/snakers4/silero-vad)). 30+ms chunk in <1ms on one CPU thread; v5 ~2MB, ~3x faster inference than v4; ONNX up to 4-5x faster in some conditions. v6 exists; detailed benchmarks not surfaced — verify against releases page.
- **ten-vad (TEN Framework)** — claims higher precision + lower false-positive rate than WebRTC and Silero; RTF 0.015 on AMD Ryzen; **306KB** footprint; claims lower transition-detection latency than Silero ("several hundred ms" lag on speech→silence transitions) ([TEN-framework/ten-vad](https://github.com/TEN-framework/ten-vad), [communeify](https://www.communeify.com/en/blog/ten-vad-webrtc-killer-opensource-ai-voice-detection/)). **Verify license before shipping.**
- **WebRTC VAD** — oldest/simplest, less accurate than Silero at matched FPR ([picovoice](https://picovoice.ai/blog/best-voice-activity-detection-vad/)).

## 7. What real dictation apps use

- **SuperWhisper** — whisper.cpp/CoreML-derivative on-device + optional cloud LLMs.
- **VoiceInk** (MIT, OSS) — **whisper.cpp locally** + **Parakeet via FluidAudio** (CoreML) as fast mode; custom models must be whisper.cpp `.bin` format ([Beingpax/VoiceInk](https://github.com/Beingpax/VoiceInk)).
- **Handy** — MIT, cross-platform, whisper-wrapper category.
- **OpenWhispr** — local Whisper AND local Parakeet + cloud BYOK, cross-platform ([OpenWhispr/openwhispr](https://github.com/OpenWhispr/openwhispr)).
- Ecosystem pattern: whisper.cpp as default/safe choice; Parakeet-via-CoreML as the "faster on Apple Silicon" upgrade path.

## 8. Local LLM post-processing (llama.cpp)

- Proxy data: Llama2-7B Q4 on M2-Ultra: 32 tok/s at 8 cores ([llama.cpp #4167](https://github.com/ggml-org/llama.cpp/discussions/4167)); TinyLlama-1.1B on Jetson Orin AGX: 13.3 tok/s Q4_K ([arXiv 2403.12844](https://arxiv.org/pdf/2403.12844)). A 0.5–1.5B model at Q4/Q5 on the i5-1334U or M1 Pro should plausibly hit tens-to-100+ tok/s (**estimate, not measured**); general guidance "20-100 tok/s on consumer CPU" ([sitepoint](https://www.sitepoint.com/breaking-the-speed-limit-strategies-for-17k-tokens-sec-local-inference/)).
- **RAM:** 0.5–1.5B Q4/Q5 GGUF ≈ 300MB–1.2GB — fits comfortably alongside ASR+VAD in 16GB (if ASR is tiny→medium, not large-v3 fp16).
- **Is it needed?** Whisper-family output already has decent punctuation/casing. For v1, **rule-based or ASR-native punctuation may suffice**; small local LLM (Qwen2.5-0.5B/1.5B Apache-2.0; Llama 3.2 1B / Gemma 3 1B have custom licenses with conditions) as an **optional** cleanup toggle, not a mandatory stage.

---

## Comparison Table

| Stack | Params | Quantized size | Approx RAM | Est. RTF on i5-1334U | Streaming | Languages | License |
|---|---|---|---|---|---|---|---|
| whisper.cpp tiny | 39M | ~75MB (Q4) | ~270MB | Very fast (est.) | Chunked (basic) | 99 | MIT |
| whisper.cpp base | 74M | ~140MB (Q4) | ~390MB | Comfortably real-time (est.) | Chunked (basic) | 99 | MIT |
| whisper.cpp small/medium | 244M/769M | 460MB/1.5GB | 850MB/2.1GB | small: real-time-ish; medium: borderline w/o iGPU (est.) | Chunked (basic) | 99 | MIT |
| whisper.cpp large-v3-turbo | 809M | ~1.6GB | ~2-3GB | Likely real-time on 10 cores Q4/Q5 (est., unconfirmed) | Chunked (basic) | 99 | MIT |
| faster-whisper int8 | = Whisper | similar/smaller | 20-30% less (est.) | Faster than whisper.cpp at matched size on x86 (est.) | whisper_streaming wrapper (3.3s latency) | 99 | MIT |
| Moonshine tiny/base/streaming | 27–245M | tens of MB–~250MB | <500MB | Very fast; sub-200ms latency demonstrated | **Native low-latency streaming (best-in-class)** | EN (+7 mono) | MIT |
| Parakeet TDT 0.6B (onnx) | 600M | ~600MB–1.2GB int8 | ~1.5GB | Comfortably fast on 10 cores (not benchmarked on this CPU) | Offline by default | EN (v2) / multi (v3) | CC-BY-4.0 |
| sherpa-onnx streaming Zipformer | 20–120M | <200MB int8 | <500MB | Very fast; designed for 1-2 CPU cores | **True frame-synchronous streaming** | Dozens | Apache-2.0 |
| Vosk | varies | 50MB–1.8GB | varies | Real-time | Native streaming | ~20 | Apache-2.0 |
| Silero VAD | tiny | ~2MB | negligible | <1ms/chunk | N/A | agnostic | MIT |
| ten-vad | tiny | 306KB | negligible | RTF 0.015 (AMD Ryzen) | N/A | agnostic | **verify license** |
| Qwen2.5/Llama3.2/Gemma3 0.5–1.5B | 0.5–1.5B | 300MB–1.2GB Q4/Q5 | similar | Tens–100+ tok/s (est.) | N/A (LLM) | multi | Apache-2.0 (Qwen) / custom (Meta, Google) |

---

## Recommended stack candidates

### Target A — Windows 11, i5-1334U, 16GB, UHD iGPU, no CUDA

1. **Safe/mature baseline:** Silero VAD → faster-whisper int8 (`base`/`small`) + `whisper_streaming` local-agreement wrapper → optional Qwen2.5-0.5B-Instruct via llama.cpp for cleanup. Best-evidenced x86 CPU path, MIT/Apache throughout. Skip iGPU acceleration (uncertain on UHD; benchmark before depending on it).
2. **Lowest-latency, English-first:** ten-vad or Silero → **Moonshine base/small-streaming (ONNX)** as primary engine — architecturally built for streaming, best measured latency in the survey; no LLM pass initially.
3. **Best accuracy/latency architecture, more engineering:** Silero VAD → sherpa-onnx **streaming Zipformer** for live low-latency preview + **Parakeet TDT 0.6B** as higher-accuracy final re-transcription of completed utterances → Qwen2.5-1.5B cleanup. The most "correct" real-time dictation architecture (fast streaming preview + accurate finalization); most moving parts.

### Target B — MacBook M1 Pro, 16GB

1. **Native Apple path:** WhisperKit (CoreML/ANE) large-v3-turbo — real-time streaming at 2.2% WER; ANE offload frees CPU/GPU for VAD + LLM.
2. **Cross-platform code-reuse path:** Silero VAD → whisper.cpp with Metal (base/small/large-v3-turbo) — the only engine with mature support on both targets; same MIT C++ core as target A.
3. **Fastest Parakeet path:** Silero VAD → Parakeet via FluidAudio (CoreML/ANE, what VoiceInk ships) → llama.cpp cleanup. Apple-only engine; no code sharing with target A.

---

## Sources

(See inline links throughout; principal repos/docs:)
- https://github.com/ggml-org/whisper.cpp · discussions #3752, #2996, #2662, #2650
- https://www.phoronix.com/news/Whisper-cpp-1.8.3-12x-Perf
- https://arxiv.org/pdf/2503.09905 (Whisper quantization study)
- https://github.com/SYSTRAN/faster-whisper · https://github.com/ufal/whisper_streaming · https://github.com/collabora/WhisperLive · https://github.com/rhasspy/wyoming-faster-whisper
- https://github.com/moonshine-ai/moonshine · https://arxiv.org/html/2602.12241v1 (Moonshine v2)
- https://huggingface.co/blog/open-asr-leaderboard · https://github.com/huggingface/open_asr_leaderboard
- https://northflank.com/blog/best-open-source-speech-to-text-stt-model-in-2026-benchmarks
- https://developer.nvidia.com/blog/new-standard-for-speech-recognition-and-translation-from-the-nvidia-nemo-canary-model/
- https://k2-fsa.github.io/sherpa/onnx/ (zipformer + NeMo transducer docs) · https://github.com/k2-fsa/sherpa-onnx
- https://pypi.org/project/onnx-asr/
- https://www.sinologic.net/en/2026-05/vosk-vs-whisper-local-the-ultimate-2026-guide-to-self-hosted-speech-recognition-stt.html
- https://towardsdatascience.com/vosk-for-efficient-enterprise-grade-speech-recognition-an-evaluation-and-implementation-guide-87a599217a6c/
- https://github.com/snakers4/silero-vad · https://github.com/TEN-framework/ten-vad · https://picovoice.ai/blog/best-voice-activity-detection-vad/
- https://github.com/KoljaB/RealtimeSTT/blob/master/docs/licenses.md
- https://github.com/Beingpax/VoiceInk · https://tryvoiceink.com/docs/custom-local-whisper-models
- https://github.com/OpenWhispr/openwhispr · https://github.com/cjpais/handy
- https://huggingface.co/argmaxinc/whisperkit-coreml · https://macparakeet.com/blog/whisper-to-parakeet-neural-engine/
- https://www.promptquorum.com/power-local-llm/local-whisper-stt-comparison-2026 · https://www.promptquorum.com/local-llms/apple-silicon-whisper-metal-benchmark
- https://justvoice.ai/blog/whisper-benchmark-apple-silicon-m3-m4 · https://whispernotes.app/blog/introducing-whisper-large-v3-turbo
- https://github.com/ggml-org/llama.cpp/discussions/4167 · https://arxiv.org/pdf/2403.12844 · https://www.sitepoint.com/breaking-the-speed-limit-strategies-for-17k-tokens-sec-local-inference/

**Caveat on evidence quality:** Several numeric benchmarks come from aggregator/blog sites (promptquorum, getvoibe, localaimaster) rather than first-party reproducible benchmarks — flagged as estimates above. Load-bearing numbers to re-verify empirically before committing: (1) whisper.cpp/faster-whisper RTF on the actual i5-1334U, (2) whether Vulkan/OpenVINO gives any real speedup on Intel UHD (non-Arc) iGPU, (3) Silero VAD v6 delta over v5.
