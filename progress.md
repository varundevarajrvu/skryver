# whispr — progress

> Orchestration state file. Read this + `tasks.json` + `git log` before resuming work.

**Project:** whispr — fully-local, open-source voice dictation (alternative to Wispr Flow).
**Repo root:** `C:\Users\varun\brain\raw\whispr`

## Decisions (and their status)

| Decision | Value | Status |
|---|---|---|
| Target platform | **Windows-first, portable core** (macOS port later as thin layer) | ✅ **User confirmed 2026-07-04.** |
| Repo location/name | `brain\raw\whispr` (renamed from user-created `Wishpr`) | ✅ **User confirmed 2026-07-04.** Note: OpenWhispr/open-wispr OSS name collision flagged in findings.md — accepted risk. |
| ASR stack (working rec) | Silero VAD → sherpa-onnx → Parakeet TDT 0.6B int8; whisper.cpp fallback; Rust + Tauri v2 shell | Pending M0 benchmark on actual hardware; see `implementation_plan.md` |
| UI theme | "Paperback": warm paper backgrounds, gridlines, literary serif type, light/dark toggle, no Inter/Roboto/Arial | Per spec |
| Landing page | Separate deliverable, own directory/branch, 3D animated, paperback theme | Per spec |

## Phase status

- **Phase 1 — Research: DONE** (3 raw reports in `research/`, synthesized into `findings.md`)
- **Phase 2 — Planning: DONE** (plan approved by user 2026-07-04)
- **Phase 3 — Implementation: IN PROGRESS** — M0 done (see `bench/results.md`); next: M1 headless pipeline
- **Phase 4 — Landing page: not started**
- **Final deliverables:** product pushed to GitHub · landing page · demo video · LinkedIn post

## Session log

- **2026-07-03 (session 1):** Repo initialized. Research agents dispatched. Raw reports saved to `research/` and committed.
- **2026-07-04 (session 2):** User confirmed both open decisions (Windows-first, keep name `whispr`). Synthesized `findings.md` inline (subagent unnecessary — reports already read). Drafted `implementation_plan.md` with M0–M6 milestones. **User approved the plan.** Ran M0: installed Rust 1.96.1 + MSVC Build Tools (winget), benchmarked 4 ASR models via sherpa-onnx Python wheels on the i5-1334U. **Engine decision: Moonshine base-en int8 @ 4 threads default (568 ms on 5 s utterance — beats 700 ms target); Parakeet TDT 0.6B int8 as accuracy option; whisper disqualified as primary (fixed 30 s window ⇒ 3.2–5.1 s latency).** Full data: `bench/results.md`. Models cached in `bench/models/` (gitignored).

## Handoff notes / what remains

**Next: M1 — headless core pipeline** (Rust workspace: `whispr-core` crate + thin CLI;
hotkey → cpal capture → Silero VAD → sherpa-onnx (Moonshine) → clipboard-paste injection with
snapshot/restore). Key crates to evaluate: `sherpa-rs` (or FFI to sherpa-onnx C API directly),
`cpal`, `enigo`/Win32 SendInput, `global-hotkey`. M1 exit: dictate into Notepad/VS Code with
logged latency; real-microphone accuracy sanity check (M0 used TTS audio — no WER signal yet).
Environment: Rust 1.96.1 + MSVC ready; `gh auth login` still pending (needed by M5).

Known environment gaps: `gh auth login` not yet done (needed to push to GitHub in Phase 3+), Docker first-launch pending (likely not needed).
