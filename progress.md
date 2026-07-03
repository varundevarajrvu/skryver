# whispr — progress

> Orchestration state file. Read this + `tasks.json` + `git log` before resuming work.

**Project:** whispr — fully-local, open-source voice dictation (alternative to Wispr Flow).
**Repo root:** `C:\Users\varun\brain\raw\whispr`

## Decisions (and their status)

| Decision | Value | Status |
|---|---|---|
| Target platform | **Windows-first, portable core** (macOS port later as thin layer) | ⚠️ Default chosen by orchestrator — user was AFK when asked. Spec mentioned "MacBook M1 Pro" but dev/test machine is Windows 11 (i5-1334U, 16 GB, no dGPU). **Confirm with user.** |
| Repo location/name | `brain\raw\whispr` (renamed from user-created `Wishpr`) | ⚠️ Same — confirm. |
| UI theme | "Paperback": warm paper backgrounds, gridlines, literary serif type, light/dark toggle, no Inter/Roboto/Arial | Per spec |
| Landing page | Separate deliverable, own directory/branch, 3D animated, paperback theme | Per spec |

## Phase status

- **Phase 1 — Research: IN PROGRESS** (3 parallel research subagents running: Wispr Flow architecture / complaints+competitors / local ASR stacks → Opus synthesis → `findings.md`)
- **Phase 2 — Planning: not started** (`implementation_plan.md`; requires user approval before Phase 3)
- **Phase 3 — Implementation: not started** (gated on plan approval)
- **Phase 4 — Landing page: not started**
- **Final deliverables:** product pushed to GitHub · landing page · demo video · LinkedIn post

## Session log

- **2026-07-03 (session 1):** Repo initialized. Research agents dispatched. Awaiting their reports; raw reports will be saved under `research/`, synthesized into `findings.md`.

## Handoff notes / what remains

Waiting on 3 research subagents. Next actions: save raw reports to `research/`, run Opus synthesis → `findings.md`, commit, then draft `implementation_plan.md` and stop for user approval.

Known environment gaps: `gh auth login` not yet done (needed to push to GitHub in Phase 3+), Docker first-launch pending (likely not needed).
