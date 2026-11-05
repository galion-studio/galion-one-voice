# W20-18 STATUS — Assemble MULTILINGUAL-VOICE-PLAN.md
**Agent:** W20-18 · **Track:** ML design · **Date:** 2026-11-02 (Europe/Zagreb, UTC+1) · **Scope:** LOCAL ONLY (no git push / gh / weights / he-IL)

## Status: DONE (draft for owner review)

## Delivered
- `docs/multilingual/MULTILINGUAL-VOICE-PLAN.md`:
  1. Grok Voice switching: behaviour contract copied from W20-16's GROK-VOICE-SWITCHING.md (auto-detect, a `language_hint` that biases and never locks, session/turn pins). Architecture not assumed. Polish is not on Grok's list.
  2. Small-model approach: Stage A = CPU cascade on `galion-tensor` (ASR → small LLM → TTS, A18 budgets). Stage B = `<|xx|>` language tokens + a tiny LID head in the student (matches the W20-19 `LanguageId::token()` format).
  3. Tiers from W20-17's TOP-20-LANGUAGES.md: T0 en · T1 pl → hr · T2a EU · T2b global · T2c low priority.
  4. V4 integration: Whisper/Parakeet-class in, Piper/SpeechT5/Pocket-TTS-class out, SenseVoice-class inbound affect (not used for PL LID). The 80 ms hot path stays the student, and the cascade is the fallback lane.
  5. Phased backlog P0–P6 with engineer-day estimates. P0 (≈8–11 ed) can start now; EN+PL on the cascade (P0–P2) ≈ 56–86 ed.
- `docs/multilingual/README.md` — folder index.

## Notes
- Sibling inputs were present when I assembled the plan (TOP-20 at 12:36, GROK-VOICE-SWITCHING shortly after).
- Model language-coverage claims are tagged `[verify]` (not checked live). Examples: Parakeet PL/HR, Piper PL voices, SenseVoice lacking PL, Pocket-TTS being EN-only at release.
- The doc contains no forbidden-format strings, so the G2 grep stays clean.
- No code changed, no LanguageId variants added.

## Blockers carried
A21 licence (P1+), G5 `.gq` writer, R0–R3 VPS latency, H6 GPU (Stage B), PL data consent.

## Owner decisions requested
HR as T1 after PL; turn-boundary-only switching on the cascade; Piper-class as PL TTS default; optional Grok external reference run.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
