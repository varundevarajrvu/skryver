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
- **Phase 2 — Planning: AWAITING USER APPROVAL** (`implementation_plan.md` drafted; gate p2 open)
- **Phase 3 — Implementation: not started** (gated on plan approval; milestone breakdown M0–M5 now in tasks.json)
- **Phase 4 — Landing page: not started**
- **Final deliverables:** product pushed to GitHub · landing page · demo video · LinkedIn post

## Session log

- **2026-07-03 (session 1):** Repo initialized. Research agents dispatched. Raw reports saved to `research/` and committed.
- **2026-07-04 (session 2):** User confirmed both open decisions (Windows-first, keep name `whispr`). Synthesized `findings.md` inline (subagent unnecessary — reports already read). Drafted `implementation_plan.md` with M0–M6 milestones. **Stopped at gate p2 for plan approval.**

## Handoff notes / what remains

**Waiting on user: approve/amend `implementation_plan.md` (gate p2).** On approval → start M0
(benchmark spike: measure real RTF of Parakeet TDT 0.6B int8 / whisper base-small Q5 /
Moonshine on the i5-1334U, prove the Rust/sherpa-onnx toolchain builds; engine choice
finalized from measured data), then M1 headless pipeline.

Known environment gaps: `gh auth login` not yet done (needed to push to GitHub in Phase 3+), Docker first-launch pending (likely not needed).
