# Skryver

**Private, 100% offline voice dictation for Windows.**

Hold a hotkey, speak, and Skryver types cleaned-up text into whatever app you're
in. Speech recognition and grammar cleanup both run on your own CPU — no
internet, no account, no audio or text ever leaves your machine.

---

## Features

- **Push-to-talk dictation** — hold `F9` (configurable) anywhere, speak, release.
  Your words are transcribed and pasted into the focused text field.
- **On-device AI cleanup** — a local LLM fixes grammar, punctuation, and filler
  (e.g. *"send it to John, sorry, I mean Jane"* → *"send it to Jane"*). Toggle
  off for instant raw dictation.
- **Two speech engines** — *High accuracy* (Parakeet) or *Fastest* (Moonshine).
- **Personal dictionary** — teach it names/jargon it keeps mis-hearing.
- **Fully offline & private** — nothing is uploaded; there's no telemetry.
- **Lives in the system tray** — a settings window plus a warm, paper-styled UI.

## How it works

Skryver is a Rust workspace with three crates:

| Crate | Role |
| --- | --- |
| `skryver-core` | The engine: audio capture, ASR, hotkey, text injection, LLM post-processing, dictionary. |
| `skryver-cli` | A headless command-line dictation tool over `skryver-core`. |
| `skryver-app` | The [Tauri](https://tauri.app) tray app + settings UI (the shipping product). |

Under the hood:

- **Speech-to-text** via [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx)
  running ONNX models (Parakeet / Moonshine) on the CPU.
- **Grammar cleanup** via a local [llama.cpp](https://github.com/ggerganov/llama.cpp)
  server running a small quantized instruct model (Qwen2.5-1.5B).
- **Text injection** pastes the result into the active window.

## Requirements

- Windows 10 / 11 (64-bit)
- A microphone
- ~2 GB free disk for the bundled models/runtime

## Usage

1. Launch the app (`skryver.exe`). First launch takes ~30–60s to load models.
   An icon appears in the system tray.
2. Click into any text box.
3. **Hold `F9`**, speak, then **release**. Your text is typed in.
4. Right-click the tray icon → **Settings** to change the hotkey, theme, AI
   cleanup mode, engine, or personal dictionary.

## Building from source

The models and the llama.cpp runtime are **not** in this repo (they're large and
gitignored). You'll need to supply them locally under `bench/models/` and
`tools/llama/` — see `scripts/package.ps1` for the exact layout the app expects.

```sh
cargo build --release -p skryver-app
```

The binary is emitted at `target/release/skryver.exe`. To assemble a portable,
self-contained folder (exe + DLLs + models + llama runtime):

```powershell
./scripts/package.ps1
```

## Privacy

Everything runs locally. Skryver makes no network requests for dictation — the
speech model, the language model, and text injection all execute on your CPU.

## License

[MIT](LICENSE) © 2026 Varun Devaraj
