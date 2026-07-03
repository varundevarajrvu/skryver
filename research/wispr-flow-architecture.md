# Wispr Flow — Technical Architecture Research

> Raw report from research subagent r1 (Phase 1). Feeds into `findings.md`.

## 1. Capture Pipeline

- **Activation model**: Hotkey-triggered, not always-on streaming. Flow runs as a background app/menu-bar (Windows: Electron; macOS: native-ish) and activates on a user-configurable hotkey. Two modes: hold-to-talk (press and hold, release to finalize) and a hands-free/continuous mode. [Use Flow hands-free](https://docs.wisprflow.ai/articles/6391241694-use-flow-hands-free) · [Hotkey shortcuts](https://docs.wisprflow.ai/articles/2612050838-supported-unsupported-keyboard-hotkey-shortcuts)
- Marketing copy states speech "converts to text as you speak, with no noticeable delay," implying **streaming ASR during the hold**, with the LLM cleanup pass firing on hotkey release. **Unconfirmed**: exact VAD algorithm/library — no public source describes a specific VAD model; inferred from behavior that end-of-utterance is keyed off hotkey release rather than (or in addition to) silence detection.
- Audio appears to stream to the backend continuously while the hotkey is held — the security docs state "audio is streamed to the backend and not persisted locally" [Security & Compliance FAQ](https://docs.wisprflow.ai/articles/3467817258-security-and-compliance-faq).
- "Context Awareness" is a separate, optional capture channel: it grabs on-screen text/screenshot content (not audio) to help formatting match the destination app. This was originally **default-on and sent periodic screenshots to cloud infra**, discovered via a user's network-traffic inspection in 2025 — sparked major backlash after Wispr initially banned the reporting user; CTO Sahaj Garg later issued a public apology and the feature was made opt-in. [ModelPiper writeup](https://modelpiper.com/blog/wispr-flow-privacy-incident) · [embertype account](https://embertype.com/blog/the-day-wispr-flow-banned-a-user/) · [Voibe "Is Wispr Flow Safe"](https://www.getvoibe.com/resources/is-wispr-flow-safe/)

## 2. Speech Recognition Backend

- **Cloud-only, no offline/on-device fallback.** Company's own docs: "Transcription always happens in the cloud to provide the best speed and accuracy" — confirmed no offline mode exists. [Security & Compliance FAQ](https://docs.wisprflow.ai/articles/3467817258-security-and-compliance-faq)
- **Model provenance is contested/mixed across sources**:
  - Secondary sources (blog teardowns) describe the ASR stage as "Whisper or a faster reimplementation of it." [Forasoft engineering walkthrough](https://www.forasoft.com/learn/ai-for-video-engineering/articles-ai/whisper-flow-app-engineering-walkthrough)
  - However, Wispr's own careers/research materials say the company is **building custom ASR foundation models to compete with Whisper and Apple's on-device transcription**, explicitly researching "context-conditioned ASR models (conditioned on speaker qualities, surrounding context, and individual history)" and a novel "sub-audible speech" problem (users speaking quietly) that "no ASR systems have been trained to solve." [Wispr Flow technical-challenges post](https://wisprflow.ai/post/technical-challenges) · [Applied Methods careers summary](https://www.appliedmethods.ai/companies/wispr-flow)
  - **Best-supported inference**: Wispr likely started on/fine-tuned Whisper-class open models and is actively moving toward proprietary ASR models trained on their own dictation corpus (they cite "1 billion words/month" of usage data as of the technical-challenges post) — labeled **inference**, not confirmed by a single authoritative source stating current production model identity.
- ML Scientist job listing requires **PhD + top-tier ML publication record (ICML/NeurIPS/ICLR/ICASSP)**, tasked with "training speech models" and using "fine-tuning and RL techniques to improve LLMs" and scaling personalization — corroborates in-house model training, not pure third-party API wrapping. [Search-derived job posting summary, ZipRecruiter/Ashby listings]
- 104 languages supported per Wikipedia (~40% English, ~60% other languages of actual usage). [Wikipedia: Wispr Flow](https://en.wikipedia.org/wiki/Wispr_Flow)

## 3. Post-Processing / "AI Edits"

- Explicit two-stage pipeline confirmed by both secondary teardown and Wispr's own post: (1) raw ASR transcript with fillers/false starts intact → (2) an **LLM pass** ("the kind of model behind a chatbot") that strips filler words, fixes punctuation/capitalization, repairs false starts, and matches tone/formatting to the destination app. [Forasoft walkthrough](https://www.forasoft.com/learn/ai-for-video-engineering/articles-ai/whisper-flow-app-engineering-walkthrough) · [technical-challenges post](https://wisprflow.ai/post/technical-challenges)
- **Where the LLM runs**: cloud-side — privacy policy confirms Customer Content ("Personal Information") is shared with **third-party AI/LLM providers**, with contractual "generally deleted within 30 days" retention, i.e., not self-hosted-only, not on-device. [Privacy Policy](https://wisprflow.ai/privacy-policy)
- **Personalization/learning loop**: Wispr's technical post describes token-level personalized formatting (user preference for dashes vs. commas, capitalization exceptions), and a stated goal to "capture edits on a user's device, determine whether edits should not be repeated... learn a local RL policy to align with a user's particular style preference" — suggesting some **on-device edit-capture/signal generation**, feeding a cloud-trained personalization model. [technical-challenges post](https://wisprflow.ai/post/technical-challenges)
- **Personal Dictionary**: user-corrected spellings auto-added; manual entries for names/jargon; replacement rules; syncs across devices via account (cloud), always syncs regardless of Cloud Sync/Privacy Mode settings for dictionary/snippets. [Teach Flow your words](https://docs.wisprflow.ai/articles/4052411709-teach-flow-your-words-with-the-dictionary) · [Data Controls](https://wisprflow.ai/data-controls)
- **Tone matching ("Flow Styles")**: per-app tone profiles (formal for docs/email, casual for messages); users can supply writing samples to bias output style. [Flow Styles setup](https://docs.wisprflow.ai/articles/2368263928-how-to-setup-flow-styles)
- User complaints: LLM cleanup sometimes **over-edits** — "improving" what was actually said, especially first-person voice/unconventional phrasing (Trustpilot-sourced complaints, 2.7/5 rating there vs 4.5/5 on G2). [HN discussion](https://news.ycombinator.com/item?id=41696153)

## 4. Text Injection Mechanism

**macOS** (per one credible secondary teardown, not an official Wispr doc — treat as best-available but unverified against source code):
- **Three-tier fallback**: (1) direct AX (Accessibility API) text insertion → (2) simulated CGEvent Cmd+V → (3) AppleScript-based paste. Clipboard is snapshotted before and restored after; pasted content is marked "concealed" so most third-party clipboard managers won't log it. [Search-derived summary, no single primary URL confirmed reachable]
- Requires macOS Accessibility permission to function at all; official docs confirm this is required to "insert spoken words into other apps." [Keyboard/Screen Reader Accessibility doc](https://docs.wisprflow.ai/articles/3941699399-keyboard-and-screen-reader-accessibility-in-wispr-flow)

**Windows**:
- Primary mechanism is **simulated Ctrl+V** (system paste shortcut), with **Shift+Insert** as a fallback specifically in IDE/terminal contexts. [Cursor/VS Code IDE doc](https://docs.wisprflow.ai/articles/6434410694-use-flow-with-cursor-vs-code-and-other-ides)
- **Privilege-level limitation (confirmed, official doc)**: "Windows security blocks paste between apps at different privilege levels" — if the target terminal runs elevated, Flow itself must also run as Administrator. [WSL/Linux/Terminal doc](https://docs.wisprflow.ai/articles/6478598909-using-flow-with-linux-wsl-and-terminal-applications)
- A live regression is documented in a public GitHub issue: **simulated Ctrl+V paste broke on Windows in Wispr Flow v2.1.83**, breaking injection into Claude Code's prompt — evidence the injection is a naive simulated-keystroke/paste approach rather than a robust UI Automation text-insertion API, since it's fragile to target-app updates. [anthropics/claude-code issue #38620](https://github.com/anthropics/claude-code/issues/38620)

**Cross-platform known limitations (official docs)**:
- **WSL, Linux VMs, SSH sessions, tmux/screen**: no direct paste support at all — transcription succeeds but text must be manually pasted via a "Paste last transcript" hotkey (Mac: Cmd+Ctrl+V; Windows: Shift+Alt+Z). [Linux/WSL/Terminal doc](https://docs.wisprflow.ai/articles/6478598909-using-flow-with-linux-wsl-and-terminal-applications)
- **Electron editors** (Cursor, VS Code, Windsurf): explicitly supported via accessibility-API context reading for variable/file names, but VS Code Insiders lacks this; "Flow reads IDE context via accessibility APIs and cannot function without this permission." [IDE doc](https://docs.wisprflow.ai/articles/6434410694-use-flow-with-cursor-vs-code-and-other-ides)
- Terminals apply Flow's auto-formatting (capitalization/spacing/punctuation) even to shell commands, which can corrupt command syntax — a known footgun called out in Wispr's own docs. [same doc]
- General reviews report Flow **freezing target apps** like VS Code/Notepad++ during dictation/injection on Windows (Electron overhead-related). [Spokenly review](https://spokenly.app/blog/wispr-flow-review)

## 5. Latency Optimizations

All figures below are from **Wispr's own engineering blog post**:
- Target: **full transcription + LLM formatting within 700ms** of the user stopping speaking.
- Budget breakdown: **ASR inference <200ms**, **LLM inference <200ms**, **network budget 200ms** (designed to tolerate "spotty internet connections" from anywhere in the world).
- Streaming implied: architecture is described as ASR → LLM sequential pipeline completing inside the 700ms window, consistent with streaming/incremental ASR decoding rather than upload-then-batch-transcribe.
- Scale claims: "users today dictate 1 billion words a month," ~10x growth expected within six months at post time, requiring 99.99% uptime.
[Technical challenges and breakthroughs behind Flow](https://wisprflow.ai/post/technical-challenges)
- **Unconfirmed**: specific chunking size/interval for streaming audio; no source gives concrete ms-level chunk windows.

## 6. Privacy / Data Model

- **Two independent toggles** define the privacy posture (official):
  - **Privacy Mode**: on/off switch for whether dictation data (audio, transcripts, edits) is usable for model training/evaluation. Was default-off pre-backlash; now opt-in.
  - **Cloud Sync**: controls server-side persistence of transcripts. When off, "audio and transcripts are processed in real time and discarded after the request completes."
  - **Zero Data Retention (ZDR)** = Privacy Mode ON + Cloud Sync OFF — default for Enterprise/HIPAA customers. "All dictation-pipeline artifacts are kept off Wispr's servers — the audio, any associated screen context, the speech-to-text output, the formatted result, and any downstream variants."
  [Security & Compliance FAQ](https://docs.wisprflow.ai/articles/3467817258-security-and-compliance-faq) · [Data Controls](https://wisprflow.ai/data-controls) · [Privacy Mode & Data Retention](https://docs.wisprflow.ai/articles/6274675613-privacy-mode-data-retention)
- **Not end-to-end encrypted** — official admission: "the backend must decrypt audio to perform transcription," though under ZDR the decrypted content isn't persisted. Encryption: TLS 1.2+ in transit, AES-256 at rest via cloud HSM-backed (FIPS 140-2) key service. [Security & Compliance FAQ]
- **Third-party AI/LLM subprocessors**: full list gated behind NDA (DPA Annex 2); Wispr states it maintains **zero-retention agreements with third-party AI providers** in Privacy Mode, and generally that shared data is "deleted within 30 days, subject to the provider's applicable retention practices" absent Privacy Mode. [Privacy Policy](https://wisprflow.ai/privacy-policy)
- **Compliance certifications** (as of research date, per official Trust Center summary): SOC 2 Type I (April 2026, auditor A-LIGN, clean opinion, scope = Security); ISO 27001:2022 Stage 1 complete (April 2026), Stage 2 scheduled June 2026; HIPAA BAA available; SOC 2 Type II in progress. **Not held**: FedRAMP, PCI DSS, SOC 1/3. [Security & Compliance FAQ]
- **Major compliance controversy (March 2026)**: Wispr's *prior* SOC 2 Type II and ISO 27001 certs were issued via **Delve**, a compliance-automation auditor accused of mass-producing near-identical boilerplate reports (99.8% shared text across 494 analyzed reports, allegedly pre-populated conclusions). Wispr was a named affected customer and had to re-engage a new auditor (A-LIGN) and switch to Drata, reissuing certs. **This materially undermines confidence in Wispr's pre-2026 compliance claims.** [getvoibe.com Delve/safety writeup](https://www.getvoibe.com/resources/is-wispr-flow-safe/)
- Enterprise controls: SAML2/OIDC SSO, Admin Portal to enforce org-wide Privacy Mode/Cloud Sync, three local-storage retention policies (normal / delete-after-24h / never-store), HIPAA BAA. No on-prem deployment offered. [Security & Compliance FAQ]
- Google Workspace data (Calendar/Gmail/Contacts) explicitly carved out — never used for generalized AI/ML training even under opt-in training programs. [Privacy Policy](https://wisprflow.ai/privacy-policy)

## 7. Platform Footprint

- **Windows build is Electron-based** (multiple independent reviews). Reported idle footprint: **~800MB RAM, ~8% CPU continuously**, 8–10 second cold start — unusually heavy for a tray dictation utility. Attributed to persistent cloud connections, background context-monitoring, and always-ready hotkey listening. [Spokenly review](https://spokenly.app/blog/wispr-flow-review) (secondary source; treat exact numbers as user-reported, not vendor-confirmed)
- macOS build appears comparatively lighter/more native per reviews, though "the gap to native Mac builds remains noticeable." **Unconfirmed** whether macOS uses Electron, Catalyst, or native Swift/AppKit — no primary source states the mac client's UI framework explicitly.
- **Network dependence is absolute**: no offline transcription mode on any platform; losing connectivity means dictation stops working entirely. [Security & Compliance FAQ]
- Platforms: macOS, Windows, iOS (third-party keyboard), Android (added 2026). [Wikipedia](https://en.wikipedia.org/wiki/Wispr_Flow)
- Company scale: 100,000+ DAU, 270+ Fortune 500 customers, $81M total raised, ~$3.8M revenue Jul 2024–Jul 2025, 80% six-month retention, 19% paid conversion. [Wikipedia](https://en.wikipedia.org/wiki/Wispr_Flow) · [Applied Methods](https://www.appliedmethods.ai/companies/wispr-flow)

---

## Key Takeaways (for a fully-local competitor)

1. **Zero on-device ASR fallback exists today** — Wispr is 100% cloud-dependent for transcription; a local-first competitor's biggest structural differentiator is "works offline / no network round-trip."
2. **Latency bar to beat**: <700ms total (ASR <200ms + LLM <200ms + network <200ms). A local pipeline that beats this without any network hop is achievable and a strong headline claim.
3. **Text injection is fragile by nature (simulated paste/keystrokes)** — confirmed real-world breakage (Claude Code GitHub issue) and known dead zones: WSL, SSH, tmux/screen, elevated-privilege terminals. A robust competitor needs a more resilient injection strategy (native OS text-insertion APIs, per-app strategy, graceful clipboard-fallback UX) and should explicitly test these dead zones.
4. **The LLM cleanup stage is the actual product differentiator**, not raw ASR — filler removal, tone/style matching per destination app, personal dictionary, edit-learning loop are what users pay for. A local competitor needs a capable local post-processing stage, not just good ASR.
5. **Screen-context capture is a privacy landmine** — Wispr's biggest PR crisis (2025 screenshot-uploading scandal). Any screen-reading must be opt-in and loudly disclosed from day one — a trust differentiator.
6. **Compliance certifications can be theater** (Delve scandal) — genuinely verifiable local processing beats claimed compliance; Wispr explicitly lacks E2E encryption since servers must decrypt audio.
7. **Electron-driven resource bloat** (~800MB RAM / 8% CPU idle reported on Windows) is a known pain point — a lean app is a tangible, provable advantage.
8. **Wispr is moving toward custom ASR models + RL personalization** — differentiate on privacy/control/latency rather than trying to out-accuracy them long-term.

## Sources

- https://wisprflow.ai/post/technical-challenges (primary — latency budget, architecture, personalization)
- https://wisprflow.ai/privacy-policy · https://wisprflow.ai/data-controls (primary)
- https://docs.wisprflow.ai/articles/6274675613-privacy-mode-data-retention (primary)
- https://docs.wisprflow.ai/articles/3467817258-security-and-compliance-faq (primary)
- https://docs.wisprflow.ai/articles/6434410694-use-flow-with-cursor-vs-code-and-other-ides (primary)
- https://docs.wisprflow.ai/articles/6478598909-using-flow-with-linux-wsl-and-terminal-applications (primary)
- https://docs.wisprflow.ai/articles/3941699399-keyboard-and-screen-reader-accessibility-in-wispr-flow (primary)
- https://docs.wisprflow.ai/articles/2612050838-supported-unsupported-keyboard-hotkey-shortcuts (primary)
- https://docs.wisprflow.ai/articles/6391241694-use-flow-hands-free (primary)
- https://docs.wisprflow.ai/articles/4052411709-teach-flow-your-words-with-the-dictionary (primary)
- https://docs.wisprflow.ai/articles/2368263928-how-to-setup-flow-styles (primary)
- https://wisprflow.ai/research (primary, low technical detail)
- https://en.wikipedia.org/wiki/Wispr_Flow (secondary, well-sourced)
- https://www.forasoft.com/learn/ai-for-video-engineering/articles-ai/whisper-flow-app-engineering-walkthrough (secondary teardown)
- https://www.getvoibe.com/resources/is-wispr-flow-safe/ (secondary — Delve scandal, screen access risk)
- https://modelpiper.com/blog/wispr-flow-privacy-incident (secondary — 2025 screenshot incident)
- https://embertype.com/blog/the-day-wispr-flow-banned-a-user/ (secondary — banned-user account)
- https://github.com/anthropics/claude-code/issues/38620 (primary evidence — injection regression)
- https://spokenly.app/blog/wispr-flow-review (secondary — resource usage, user-reported)
- https://news.ycombinator.com/item?id=41696153 (HN thread)
- https://www.appliedmethods.ai/companies/wispr-flow (secondary — careers/company scale)

**Caveats**: The macOS three-tier paste description and the "800MB RAM/8% CPU" figures come from secondary aggregator sites (search snippets only, not independently re-fetched) — moderately confidently sourced but not vendor-primary. The exact current production ASR model (in-house vs. Whisper-derived) is genuinely ambiguous and labeled as inference above.
