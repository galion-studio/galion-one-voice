# W21-03 · Sector 11 Cascade + EN ml-voice · STATUS
*2026-11-02, 14:25 CET (Europe/Zagreb, UTC+1). LOCAL ONLY: no push, no `gh`, no VPS, no weights, no training. ENGLISH ONLY (he-IL / pl / others rejected).*

**State: DONE — EN demo lane runnable, offline, tests green.**

## Live-demo hook (stable contract)
```bash
scripts/run-en-cascade-demo.sh            # build + run both lanes, verify WAVs
scripts/run-en-cascade-demo.sh --test     # + 46 offline tests first
scripts/run-en-cascade-demo.sh --play     # + play en-cascade.wav (paplay/aplay/afplay if present)
scripts/run-en-cascade-demo.sh --text "good morning" --hint en-GB --emotion happy
```
Outputs under `${GALION_DEMO_OUT:-target/galion-demo}` (paths stable for `scripts/run-live-demo.sh`, W21-01):

| file | content |
|---|---|
| `en-cascade.wav` | **audible** earcon of the reply, 24 kHz mono PCM16, one 80 ms frame per char (1920 samples), silent lead-in/out frame, peak ≈ 0.3 FS — feed straight into the heartbeat |
| `en-cascade-fixture.wav` | exact StubTts signal (decodes back to the reply text; loop check) |
| `en-cascade.json` | turn summary (`route_source`, `asr`, `emotion`, `reply`, `frames`, `audible_ms`, wav paths) |
| `en-roundtrip.wav` | galion-ml-voice `roundtrip` example output |

Last stdout line is `GALION_EN_CASCADE_WAV=<abs path>` so a caller can `eval`/grep it. Exit codes: 0 ok, 2 unsupported language (e.g. `--pin he-IL`) or empty input, 64 bad flags.

Suggested call from `run-live-demo.sh` (W21-01 owns that file; not edited here):
```bash
EN_WAV=$(scripts/run-en-cascade-demo.sh | sed -n 's/^GALION_EN_CASCADE_WAV=//p')
```

## What landed
| item | files | tests |
|---|---|---|
| Cascade route (plan P0.2 contract, EN impl): `CascadeRoute`, `route_en`, `Lid` trait + `EnLid` stub. Precedence **pin > hint > LID**; unsupported pin = error; unsupported hint = ignored → LID | `crates/galion-cascade/src/route.rs` (always compiled, zero deps) | 8 |
| EN demo lane (feature `cascade`): `MlAsr`/`MlTts` adapters onto galion-ml-voice `AsrEngine`/`TtsEngine`, `FixedEmotion`, `run_en_turn[_with]`, `earcon`, `wav_bytes` | `crates/galion-cascade/src/en_lane.rs` | 10 |
| CLI example | `crates/galion-cascade/examples/en_cascade.rs` (`required-features = ["cascade"]`) | — |
| Manifest | `crates/galion-cascade/Cargo.toml`: `cascade = ["dep:galion-ml-voice"]`, optional path dep; default build still zero-dep | — |
| Sideband P0.1 types: `LangTag`, `LanguageHint`, `LangPin`, `LangPrefs` (precedence), `LangEvent` | `crates/galion-sideband/src/lang.rs` (+1 `pub mod lang;` line in lib.rs), `tests/lang.rs` | 5 |
| Script | `scripts/run-en-cascade-demo.sh` | — |
| Docs | this file; `docs/multilingual/PROTOTYPE.md` live-demo hook section | — |

Untouched: galion-ml-voice `src/`, its tests, and the `roundtrip` example (already produces a WAV; the script points `--out` at the stable dir).

## Test counts (cargo 1.85.1, `--offline`)
| command | result |
|---|---|
| `cargo test --offline -p galion-cascade` | 13 passed (5 existing + 8 route) |
| `cargo test --offline -p galion-cascade --features cascade` | 23 passed (+10 en_lane) |
| `cargo test --offline -p galion-ml-voice` | 14 passed (7 scaffold + 7 roundtrip) |
| `cargo test --offline -p galion-sideband` | 9 passed (3 client + 5 lang + 1 types) |
| **total via `--test`** | **46 passed, 0 failed** |

`cargo build --offline --workspace --all-targets`: clean, no warnings at 14:20 CET. `scripts/check-no-forbidden.sh crates/galion-sideband`: OK.

## Design notes
- The canned mic signal is a DC code (not speech), so RMS `MockEmotion` would read it as Angry; the lane uses `FixedEmotion` (default Neutral, `--emotion` to demo the feeling slot).
- The lane runs at 24 kHz end to end (Mimi-class bus). Cascade's 16 kHz `SAMPLE_RATE_HZ` slot resampler is still a next step.
- The stub reply is `"[Neutral] you said: hello galion"` (MockLlm). The audible WAV is an earcon (pentatonic tone per alphanumeric char), not synthetic speech. No TTS weights exist.

## Not done / next
1. Wire `run-live-demo.sh` (W21-01) to call the script or read `target/galion-demo/en-cascade.wav`.
2. 16↔24 kHz resampler adapter; hysteresis (2 windows) in `CascadeRoute` once a second language exists.
3. Plan P0.3–P0.5 (vocab token range, switch-latency console field, `presence.cascade.json`) belong to A8/A11/A13 lanes.
4. Real ASR/TTS/LLM remain blocked on A21 (licence clearance). No Polish code.

— Maciej Grajczyk
