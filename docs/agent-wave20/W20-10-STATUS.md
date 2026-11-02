# W20-10 STATUS — Sector 11 Cascade (EN lane stub)
2026-11-02 (Europe/Zagreb, CET). LOCAL ONLY — no push/gh/VPS/weights; EN only.

## Done
- New crate `crates/galion-cascade` (zero deps, `cargo --offline`), added to workspace members.
  - Traits: `Asr` (Whisper/Parakeet slot), `EmotionIn` (SenseVoice-Small ref), `Llm` (small LLM, .gq only — no legacy container formats), `Tts` (Piper/SpeechT5/Pocket-TTS slot).
  - Deterministic mocks + `Cascade::turn`; `Lang::En` only; A18 budget constants; feature `cascade` (non-default).
  - 5 unit tests pass (`cargo test --offline -p galion-cascade`).
- Design doc `docs/cascade/EN-CASCADE-LANE.md` (pipeline, budgets, candidate model/licence table).

## Open / blockers
- Model picks + licences → A21 table (W20-12); owner K15/K16/E9 confirmations.
- No real backends, no latency measurement yet; hr/pl routing deferred.
