# W20-19 STATUS — galion-ml-voice English-only scaffold

**Agent:** W20-19 · **Track:** EN proto · **Date:** 2026-11-02 · **Scope:** LOCAL ONLY (no git push / gh / weights / he-IL)

## Status: DONE (local, tests green)

## Delivered
- `crates/galion-ml-voice/` (zero external deps, `#![forbid(unsafe_code)]`, `publish = false`)
  - `src/lib.rs`
    - `LanguageId` — `#[non_exhaustive]`, **`En` only**; `ALL`, `code()`, `token()` → `"<|en|>"`, `from_code()`, `from_token()`, `Display`
    - `with_lang_token(lang, text)` / `split_lang_token(text)` helpers
    - `Audio`, `Transcript`, `VoiceError`
    - Stub traits: `AsrEngine`, `TtsEngine`, `LangRouter` (object-safe)
    - Comment: more languages come later per the multilingual plan (docs/multilingual/; EN + PL priority, then top-20). No Polish or other language modules added.
  - `src/stub.rs` — synthetic fixtures + deterministic stubs: `fixture_audio()` (24 kHz, 240 samples/char), `FIXTURE_EN_TEXT`, `StubAsr`, `StubTts`, `EnglishOnlyRouter`
  - `tests/scaffold.rs` — 7 tests: EN-only set, `<|en|>` round-trip (rejects `<|pl|>`), prefix helpers, fixture shape, TTS→ASR round-trip via router, empty-input errors, trait object safety
- Workspace `Cargo.toml`: appended `"crates/galion-ml-voice"` to `members` (other agents concurrently added `galion-cascade`, `galion-student-backend`; preserved).

## Verification
- `cargo test -p galion-ml-voice --offline` → 7 passed, 0 failed
- clippy not installed on box (not run); `cargo build` warning-free

## Not done / handoff
- No real ASR/TTS/LID backends, no weights (by design).
- W20-20: demo/tests + PROTOTYPE.md wiring notes can build on `StubTts`/`StubAsr`/`EnglishOnlyRouter`.
- Adding languages: add `LanguageId` variants + tokens only after multilingual plan sign-off.

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
