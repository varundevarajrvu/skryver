# whispr — Phase 1 Research Findings (Synthesis)

> Synthesized from the three raw reports in `research/`. Citations live there; this file is the
> decision-oriented digest that `implementation_plan.md` builds on.
>
> - `research/wispr-flow-architecture.md` — how Wispr Flow actually works
> - `research/complaints-and-competitors.md` — what users hate, who else is in the space
> - `research/local-asr-stacks.md` — what we can run locally on the target hardware

**Confirmed decisions (user, 2026-07-04):** Windows-first with a portable core; repo/product name stays `whispr`.

---

## 1. The product thesis, sharpened by evidence

Wispr Flow is a **cloud-only** dictation product with a genuinely good UX. Every one of its
structural weaknesses is a direct consequence of the cloud architecture, and every one is
documented, not hypothetical:

| Wispr Flow weakness | Evidence | whispr's answer |
|---|---|---|
| Privacy scandal (screenshots uploaded by default, user banned for reporting it, CTO apology) | HN thread, 2025–2026 incident | **Nothing ever leaves the machine.** Not a toggle — an architecture. |
| 75+ outages since Dec 2025; no offline mode at all | StatusGator + official status page | No server exists to go down. Works on a plane. |
| ~800 MB RAM / ~8% CPU idle (Electron); freezes VS Code | Multiple independent reviews | Lean native app; target **< 200 MB RAM idle, ~0% CPU idle**. |
| $15/mo subscription; free tier ≈ 15 min of dictation/week ("word-count anxiety") | Official pricing docs | Free, open-source, unmetered. |
| Mandatory account; auth outages lock users out | Official docs + status page | No account, no login, no telemetry. |
| Compliance theater (prior SOC2 via discredited auditor Delve) | March 2026 investigation | Auditability by source code, not by certificate. |
| No Linux support (unofficial port proves demand) | GitHub port, forum threads | Portable core makes Linux a follow-on target, not a rewrite. |

**Latency bar to beat:** Wispr's own engineering target is ~700 ms end-to-end (ASR <200 ms +
LLM <200 ms + network 200 ms). A local pipeline has no network hop, so **sub-700 ms on
utterance-end is the headline claim to design for** — and the local ASR research says it's
achievable on our hardware with the right model choice.

**What actually retains Wispr users** is not raw ASR — it's the polish layer: filler-word
removal, punctuation, per-app tone, personal dictionary. A local competitor that ships raw
Whisper output will feel worse even if accuracy is identical. Post-processing is a
first-class requirement, not a stretch goal.

## 2. Competitive positioning

The niche is real and currently vacant: **open-source + fully-local + Windows-first + live
dictation** (not file transcription).

- Mac has strong local options (SuperWhisper, VoiceInk, MacWhisper) — Windows does not.
- Existing cross-platform OSS (Handy, Vibe, OpenWhispr) are either early/unpolished or focused
  on file transcription rather than hold-to-talk dictation into any app.
- **Naming caution:** `OpenWhispr` and `open-wispr` already exist as OSS projects. The name
  `whispr` is kept per user decision, but expect SEO/branding collision; a distinctive tagline
  and logo matter more than usual. Re-check before any trademark/domain spend.

## 3. Technical stack — what the evidence supports

Target hardware reality check: i5-1334U (10 cores, AVX2), 16 GB RAM, **no usable GPU** (Intel
UHD non-Arc iGPU may silently fall back to CPU in whisper.cpp Vulkan/OpenVINO — benchmark,
don't assume). Everything below is CPU-first.

### ASR engine candidates (all runnable locally, permissive licenses)

| Candidate | Why in | Why cautious |
|---|---|---|
| **sherpa-onnx** (Apache-2.0) runtime hosting **Parakeet TDT 0.6B int8** (CC-BY-4.0) | Parakeet ≈ Whisper-large accuracy at ~40× the speed; sherpa-onnx is one Apache-2.0 runtime that also hosts Moonshine/Zipformer/Whisper, so we can swap models without changing the app | Parakeet ONNX is offline (per-utterance), not streaming — fine for hold-to-talk, needs a streaming companion for live preview |
| **sherpa-onnx streaming Zipformer** (int8, <200 MB) | True frame-synchronous streaming, ~44 ms latency class, runs on 1–2 CPU cores — ideal for live preview text | English-centric checkpoints; accuracy below Parakeet — use as preview, not final |
| **Moonshine base/small-streaming** (MIT) | Best measured streaming latency in the survey (~107 ms vs ~11 s for Whisper large on the same machine) | English-first; per-language mono models |
| **whisper.cpp / faster-whisper** (MIT) | Most mature ecosystem, 99 languages, the "safe default" every competitor ships | Chunked pseudo-streaming only; on CPU, small+ sizes get borderline for real-time |

**Working recommendation** (to be validated by an M0 benchmark on the actual machine):
`Silero VAD (MIT) → sherpa-onnx runtime → Parakeet TDT 0.6B int8 on utterance-end` for final
text, with `streaming Zipformer` live preview as a later enhancement. whisper.cpp stays the
multilingual fallback path. **Load-bearing numbers that MUST be re-measured locally before
committing:** actual RTF of Parakeet int8 / whisper base-small / Moonshine on the i5-1334U.

### Post-processing

Start **rule-based** (filler-word strip, spacing/punctuation normalization, custom-dictionary
replacements) — Whisper/Parakeet already emit decent punctuation. A small local LLM pass
(Qwen2.5-0.5B/1.5B-Instruct, Apache-2.0, ~300 MB–1.2 GB Q4 via llama.cpp) is an **optional
toggle** later, never a mandatory stage — Wispr's own users complain about LLM over-editing.

### Text injection (Windows) — where competitors are fragile

Wispr's injection is simulated Ctrl+V and it breaks (documented regression against Claude
Code's prompt; dead zones in WSL/SSH/tmux; blocked across privilege levels). Plan:
clipboard-paste primary (with clipboard snapshot/restore), Shift+Insert fallback for
terminals, a "paste last transcript" rescue hotkey, and explicit testing of the documented
dead zones. Elevated-window limitation is a Windows security boundary — document it, don't
fight it.

### App shell

Rust core + **Tauri v2** shell (the Handy/Vibe pattern): system tray, global hotkey,
paperback-themed settings window, single lean binary. This directly attacks the 800 MB
Electron complaint and keeps the core portable for the macOS port (platform-specific bits
isolated behind traits: audio capture, hotkey, injection).

## 4. Product requirements distilled from complaint data

Must-have (each maps to a named, sourced complaint):
1. Hold-to-talk global hotkey + toggle mode; **non-QWERTY-safe** hotkey handling.
2. Works 100% offline after first model download; no account, no telemetry.
3. Custom dictionary (names/jargon) applied deterministically — not LLM-mediated.
4. Idle footprint dramatically under Wispr's (~measurable: <200 MB RAM, ~0% CPU).
5. Injection fallback UX when paste fails (never silently lose a transcription — keep history).
6. Terminal-aware mode: no auto-capitalization/punctuation when target is a terminal.

Explicitly deferred: screen-context awareness (Wispr's scandal feature — if ever added, opt-in
and loudly disclosed), per-app tone profiles, mobile.

## 5. Key risks

1. **Benchmark risk** — all RTF numbers for this exact CPU are extrapolated. M0 must measure
   before the engine choice is final. (Mitigation: sherpa-onnx makes models swappable.)
2. **Windows build toolchain** — whisper-rs/sherpa-rs need CMake/clang; first build on this
   machine may need setup. (Mitigation: sherpa-onnx ships prebuilt libs.)
3. **Injection fragility** — inherent to the OS; mitigated by fallbacks + history, not solved.
4. **Name collision** (OpenWhispr/open-wispr) — branding/SEO risk, accepted for now.
5. **iGPU acceleration is a maybe** — treat any Vulkan/OpenVINO speedup as a bonus, never a
   dependency.
