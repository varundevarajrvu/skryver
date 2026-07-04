# whispr — Implementation Plan

> Phase 2 deliverable. Builds on `findings.md`. **Status: awaiting user approval (gate p2).**
> No implementation happens until this plan is approved.

## What we're building (one paragraph)

A fully-local, open-source voice dictation app for Windows (portable core for a later macOS
port). Hold a hotkey, speak, release — polished text appears in whatever app has focus, in
under ~700 ms, with zero network traffic, no account, no subscription, and a tiny footprint.
Paperback-themed UI (warm paper, gridlines, literary serif, light/dark).

## Architecture

```
┌─────────────────────────── whispr (Tauri v2 app) ───────────────────────────┐
│  ┌──────────────┐   ┌───────────────────────── whispr-core (Rust crate) ──┐ │
│  │ Tray + UI    │   │                                                      │ │
│  │ (paperback   │◄──┤  hotkey ──► audio capture ──► VAD trim ──► ASR ──►  │ │
│  │  webview)    │   │  (global)      (cpal)        (Silero)   (sherpa-    │ │
│  └──────────────┘   │                                          onnx)      │ │
│                     │        ──► post-process ──► inject                   │ │
│                     │            (rules + dict)   (clipboard paste w/      │ │
│                     │                              restore + fallbacks)    │ │
│                     └──────────────────────────────────────────────────────┘ │
│  Platform-specific code isolated behind traits: Hotkey / AudioIn / Injector  │
└──────────────────────────────────────────────────────────────────────────────┘
```

- **Language/shell:** Rust core + Tauri v2 (single lean binary, tray app, webview settings UI).
- **ASR:** sherpa-onnx runtime (Apache-2.0). Primary model **Parakeet TDT 0.6B int8**
  (CC-BY-4.0) transcribing on utterance-end; whisper.cpp-format models as multilingual
  fallback; streaming Zipformer live-preview later (M6).
- **VAD:** Silero VAD (MIT) — trims silence, enables auto-stop in hands-free mode.
- **Post-processing v1:** deterministic rules (filler strip, spacing, terminal-aware mode) +
  user dictionary. Local-LLM cleanup is a later optional toggle, off by default.
- **Injection:** clipboard paste (snapshot → set → Ctrl+V → restore), Shift+Insert for
  terminals, "paste last transcript" rescue hotkey, transcription history so nothing is lost.
- **Models:** downloaded on first run to `%LOCALAPPDATA%\whispr\models` with checksums;
  fully offline afterward.

## Milestones

| # | Milestone | Deliverable / exit criteria |
|---|---|---|
| **M0** | **Benchmark spike** (de-risks everything) | Script that measures real RTF + latency on this i5-1334U for: Parakeet TDT 0.6B int8, whisper base/small Q5, Moonshine base. **Engine choice finalized from measured data.** Also verifies the Rust/sherpa-onnx toolchain builds on this machine. |
| **M1** | **Core pipeline, headless** | CLI/tray-less prototype: hold hotkey → speak → release → text pasted into the focused app. Latency logged. This is the "it works!" demo moment. |
| **M2** | **Tray app + paperback UI** | Tauri tray app: settings window (hotkey config, model picker + downloader, mic picker, history view), light/dark paperback theme. |
| **M3** | **Polish layer** | Rule-based cleanup, custom dictionary CRUD, terminal-aware mode, non-QWERTY-safe hotkeys, hands-free toggle mode with VAD auto-stop. |
| **M4** | **Robustness + dead zones** | Tested against: VS Code, terminals (elevated + normal), browsers, WSL, Notepad. Injection fallback UX. Footprint check vs targets (<200 MB idle). Error states (no mic, model missing). |
| **M5** | **Ship** | README + screenshots, MIT license, GitHub repo pushed (`gh auth login` needed), CI build, installer/portable exe. |
| **M6** | **Post-ship enhancements** (separate approval) | Streaming live-preview (Zipformer), optional local-LLM cleanup toggle, macOS port, Linux port. |

Phase 4 (3D animated paperback landing page, own directory/branch), demo video, and LinkedIn
post follow M5 as already tracked in `tasks.json`.

## Decisions locked vs. deferred

**Locked (per user):** Windows-first portable core · name `whispr` · paperback theme ·
free/OSS/no-cloud positioning.

**Deferred to M0 data:** exact primary model (Parakeet vs whisper-small vs Moonshine) ·
whether iGPU acceleration is worth enabling (assume no).

**Out of scope for v1:** screen-context awareness, per-app tone styles, accounts/sync of any
kind, mobile.

## Risks (from findings.md §5, with plan hooks)

1. Extrapolated benchmarks → **M0 exists precisely to kill this risk first.**
2. Windows toolchain friction (CMake/clang for sherpa-rs) → M0 proves the build early;
   prebuilt sherpa-onnx libs as fallback.
3. Injection fragility → M4 tests documented dead zones; history view guarantees no lost text.
4. 16 GB RAM budget → int8 models + no mandatory LLM keep total working set ~1–2 GB.
