# W20-20: EN proto demo/tests + PROTOTYPE.md (status)
*2026-11-02, 12:38 CET (Europe/Zagreb, UTC+1). LOCAL ONLY. ENGLISH-ONLY.*

## Result: DONE
- **Dependency:** the W20-19 crate `crates/galion-ml-voice` appeared during this run (I raced it and my fallback scaffold was skipped because the directory already existed). I built on it **without modifying** W20-19's files (`Cargo.toml`, `src/lib.rs`, `src/stub.rs`, `tests/scaffold.rs`). The crate is already in the workspace `members`.
- **Added:**
  - `crates/galion-ml-voice/examples/common/demo.rs`: round trip (StubAsr → echo → StubTts) + PCM16 WAV writer, zero deps
  - `crates/galion-ml-voice/examples/roundtrip.rs`: CLI (`--out/--lang/--text`), writes a 24 kHz mono PCM16 WAV
  - `crates/galion-ml-voice/tests/roundtrip_wav.rs`: 7 tests
  - `docs/multilingual/PROTOTYPE.md`: run instructions + V4 cascade wiring notes (galion-cascade 16 kHz i16 ↔ ml-voice 24 kHz f32 adapters, LanguageId canonical, 80 ms = 1920-sample frames)
- **Verified (offline):** `cargo test --offline -p galion-ml-voice` → 7 + 7 pass, 0 warnings. `cargo build --offline --workspace` OK. Example output: `target/galion-ml-voice/roundtrip-en.wav`, 10 604 bytes, Python `wave` reports 24000 Hz / 1 ch / 16-bit / 5280 frames (220 ms). `--lang he-IL` → exit 2 "unsupported language".

## Confirmations
- Nothing pushed: `/workspace/galion-voice-code` is **not a git repo** (`git status` → "not a git repository"), so no commit, push, or `gh` happened.
- No weights, downloads, or network were used. No he-IL. Only English is accepted.

## Open / handoff
- An adapter layer in galion-cascade (behind the `cascade` feature) plus a 16↔24 kHz resampler. See PROTOTYPE.md "Next steps".
- `LanguageId::from_code` accepts only `en`. Cascade accepts `en-US`/`en-GB`, so the two need aligning (W20-19's file; not changed here).

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
