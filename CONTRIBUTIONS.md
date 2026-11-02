# CONTRIBUTIONS · Galion Voice
Galion Voice — Perpetual Open MIT License · © 2026 Maciej Grajczyk · 2026-11-02 (Europe/Zagreb, UTC+1)

Based only on files in this tree (LEDGER.md, README.md, LICENSE, docs/agent-v1-demo/*, docs/agent-wave20/*, docs/agent-wave21/*). The tree is not a git repo, so there is no commit history to attribute from. Agent IDs are lane IDs from the docs, not people.

## 1. Owner
- **Maciej Grajczyk** — owner, copyright holder (LICENSE, README), decision-maker (docs/agent-v1-demo/FRXDEL-ANSWERS.md).

## 2. Main components

| Component | State | Source / contributor (per docs) |
|---|---|---|
| V1 one-dot demo, `http://127.0.0.1:7878/` | present | V1-01…V1-04, FP-01…FP-04 (docs/agent-v1-demo) |
| README.md | present | needs review (author lane not recorded) |
| LICENSE | present — MIT text, © 2026 Maciej Grajczyk; docs call it "Perpetual Open MIT License (draft)" | needs review (final licence wording) |
| CREDENTIALS.md | **not present** in this tree | planned/open — needs review |
| CONTRIBUTIONS.md | present (this file) | — |
| Speech path (A21 = Piper VITS, CPU) | present | FP-03 |
| Microphone path | **open** — no microphone; synthetic "you" band used | FP-SUMMARY |
| Stubs (ASR / LLM / Mimi) | **open** — still stubbed; Mimi kernels on synthetic tensors (W20-15, W21-02) | needs review |
| UX fixes | present | FP-04 (FP-04-UX.md), FP-01 deletions |
| Deploy dry-run | present as scripts/checklist only; VPS not reached (SSH denied) | A22, W20-13 (R0-R3-DRY-RUN.md) |
| Ratification docs | **open** — LOCKS ratification + espeak-ng GPL-3 counsel pending | A21 (A21-BLOCKERS.md, A21-COUNSEL-MEMO.md, W20-12) |

## 3. Agent lanes
- **V4 ledger (LEDGER.md):** A1 Locks · A2 OS Voice image spec · A3 VoiceBackend slot · A4 SIMD/int8 GEMV · A5 conv/attention/KV · A6 Mimi stub · A7 .gq reader · A8 student KD plan · A9 AffectStore/FiLM · A10 Rika + G6 + heartbeat · A11 live console · A12 one-dot render · A13 presence harness · A14 canned demo golden · A15 probe · A16 gates + runner · A17 Hermes sideband types · A18 cascade lane · A19 branding (not in wave) · A20 ledger/integration · A21 licence/weights memo (draft, needs counsel) · A22 deploy posture (dry-run).
- **Wave 20 (W20-01…W20-20):** stack, Eureka, Rika, emotion, render, presence, console, student, Hermes, cascade, gates, A21 memo, VPS dry-run, .gq/G5, Mimi kernels, ML research ×2, multilingual plan, galion-ml-voice EN crate + demo.
- **Wave 21 (W21-01…W21-05):** live demo binary · Mimi wiring · EN cascade lane · one-dot window + console · G2 scrub + LIVE-DEMO.md.
- **V1 demo (V1-01…V1-04):** screen · wiring · speech · runbook (`scripts/run-v1-demo.sh`).
- **First-principles pass (FP-01…FP-04):** delete/simplify · backend wiring to one dot · TTS/audio (A21 built) · E2E test + UX + runbook.

## 4. External / borrowed sources (high level)
- **Piper** TTS, voice `en_US-ljspeech-medium` — docs say MIT; voice-data licence needs review.
- **espeak-ng** (Piper phonemizer dependency) — GPL-3 flagged for counsel; needs review.
- **flite** — fallback TTS; licence needs review.
- Audio players used at runtime (paplay / pw-play / aplay / ffplay) — system tools, not vendored.
- Designs reference Parakeet, Pocket-TTS, SpeechT5, SenseVoice — design notes only; needs review.
- No CREDENTIALS.md exists yet to list these formally — needs review.

## 5. Needs review (7)
README author lane · final licence wording · CREDENTIALS.md missing · stub status · Piper voice-data licence · espeak-ng GPL-3 · flite + referenced models' licences.

— Maciej Grajczyk
