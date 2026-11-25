# S61 — cascade locks

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S61, galion-cascade fallback lane |
| Written | 2026-11-25. No voice audio committed, no weights, no paid API, no merge. |
| Base | `main` `3c6751141f510f4c6538cd1cb07bafa2f63a98b5` |
| Gaps | `docs/horbm/foundations/F1-cascade-audit.md`: `ONSET_P50_BUDGET_MS` was 800 against `LOCKS.toml` `onset_p50_max_ms` 300, and a failed turn's error did not carry the partial stamp sheet. |
| This change | `crates/galion-cascade` and this note. No new dependency. `Cargo.lock` is unchanged. |
| Not edited | `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, root workspace members, `.github/workflows/*`, `crates/galion-console`, `horbm/crates/horbm`, `horbm-soak`, `galion-gates`, `galion-tensor`. |
| Consent | `remember=deny`, `learn=deny`, `learn_in_band=false`. Not edited. Ear prosody stays on (`GALION_EAR_SENSE` is the off switch). Samantha stays the persona. Aria stays parked. |

## What it decides

`LOCKS.toml` `[gates]` is the source of truth for the two ceilings the stub lane publishes.

| Constant | Lock key | Value |
|---|---|---|
| `ONSET_P50_BUDGET_MS` | `onset_p50_max_ms` | 300 |
| `BARGE_IN_BUDGET_MS` | `barge_in_max_ms` | 200 |

Both live in `crates/galion-cascade/src/lib.rs`. `budgets_match_locks_toml` reads `LOCKS.toml` with `include_str!` (test only; the library binary does not embed the file), takes the integer in `[gates]` before any `#` comment, and checks each constant. The old onset constant was 800. The lock is the stricter number. This change does not edit the lock. A measured miss does not raise the constant.

The parser ignores other tables, so a comment or a different section that mentions the same name does not count. `barge_in_max_ms` was already 200. It stays 200.

A dead research gateway is not sent to the rule table. `HermesLlm` still answers with `UNREACHABLE` or `PATIENT`. `PhoneLane` still builds `HermesLlm` when `GALION_LLM` is unset. `GALION_LLM=canned` is still the only way onto `CannedLlm`. This change does not add a fallback.

## Measured onset

The stub lane is `mock_cascade`: `MockAsr`, `MockEmotion`, `MockLlm`, `MockTts`. Input is 1,600 samples of the constant `100` (100 ms at 16 kHz). Mock ASR returns its fixed line. Mock TTS returns silence, 10 ms of zeros per character of the reply. `turn` returns that buffer at the end, so the turn time is the time to audio.

Method, same index as `galion_presence::percentile`: discard 16 warmup turns, time 49 turns with `std::time::Instant`, sort, nearest-rank p50 (`ceil(0.50 * 49) - 1`, index 24). The profile is `cargo test` (debug, unoptimized). The test requires p50 > 0, so a stuck clock cannot pass. If p50 is over 300 ms the test fails and the message states the gap in microseconds. It does not rewrite the constant.

Host: Linux 6.12.113+, 4 cores, `Intel(R) Xeon(R) Processor` at 2400 MHz. `rustc` 1.85.0.

Six runs of `stub_lane_onset_p50_against_the_lock`. p50 was 40, 41, 42, 43, 47, and 52 µs. The lock is 300 ms, which is 300,000 µs. Every run is under the lock. This is a pass of the stub. It is not a Piper onset and it is not a Flite onset.

The 24 kHz English stub (`en_cascade`: `StubAsr`, `MockLlm`, `StubTts`, no call to the speech stack) was timed the same way. Four runs: 170, 181, 182, and 252 µs. Also under the lock. `run_en_turn` still calls the speech stack after that stub. That call was not part of this timing.

This session did not time A21 Piper or libflite against 300 ms. `full_turn_emits_24k_speech_wav` checks that a WAV came back. F1 already says a green full-turn test is not a latency result. That is still true. The spoken path is not claimed here.

## Error sheet

A lane that has opened a turn copies `Domain04Stamps` onto the error before it returns. `PhoneError::partial_sheet` is that copy. `Display` prints the existing message, then a newline, then `log_line()`. `stamp_log_clean` accepts the line: fixed keys, `absent` or an unsigned integer. A transcript word or a `RIFF` header fails the test.

`galion-console` `turn_worker` builds the published error as `format!("{kind}: {e}")`. This swarm does not edit that file. The format string already includes `Display`, so `TurnReply.error` carries the sheet. `TurnReply.stages` on the error path is still whatever the console assigns (`String::new()`, or a seam or research token). The stamps are on the error, so formatting the error no longer drops them.

Message variants hold the sheet in `FailNote`. `Empty` and `Superseded` stay unit variants so the console's existing matches keep compiling. The worker does not publish those two (`None`). Their sheet stays on `PhoneLane::stamps`. A direct `PhoneTurnAsr` error, a WAV parse that never opened a turn, and `UnknownLlm` from config have no sheet. Nothing is invented to fill one.

Checked shapes:

| Failure | Sheet on the error | What the line holds |
|---|---|---|
| Stub ASR refuses real speech | yes | `t_eou_decided` set, ASR and later times absent, `pcm_in_samples` is the count, `heard_chars` 0. The message still says no transcript was invented. The line has no guessed words and no `RIFF`. |
| Speech off after a stub fixture (`hello galion`) | yes | `heard_chars=12`, `reply_chars=0`, `pcm_out_samples=0`. The words are not on the line. |
| `speak_fixed` with no backend | yes | `reply_chars` is the character count. The reply words are not on the line. |
| Empty PCM | no (unit variant) | The lane sheet still has `t_eou_decided` and `pcm_in_samples=0`. Display has no `domain04` line. |

The console tests `speech_off_publishes_tone_and_error_label` and the other `turn_worker` tests still pass. They were not edited.

## Linux-only

The product runtime stays Galion-Ubuntu. This change adds no `std::os::unix`, no hard-coded `.exe`, and no hard-coded path. The stamp clock is `std::time::Instant`. The measurement above is this Linux host, debug profile.

Unix speech workers are unchanged from S52: Piper, Flite, and the Vosk parent run on Unix; other targets get `SpeechHostKind::Stub` and `PhoneError::NoSpeech`. That stub does not spawn and does not invent a transcript.

## Tests

`rustc` 1.85.0.

| Command | Result |
|---|---|
| `cargo test -p galion-cascade --lib` | 97 passed |
| `cargo test -p galion-cascade --features cascade --lib` | 135 passed |
| `cargo test -p galion-console --features phone-turn --lib turn_worker` | 10 passed |
| `cargo test --workspace` | 1067 passed, 5 ignored, 0 failed |
| `cargo test --manifest-path horbm/Cargo.toml --workspace` | 267 passed, 2 ignored, 0 failed |
| `cargo clippy -p galion-cascade --all-targets -- -D warnings` | clean |
| `cargo clippy -p galion-cascade --features cascade --all-targets -- -D warnings` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |

`cargo test --workspace` uses default features, so it runs the lock check and the stub-lane timing. It does not compile `phone.rs`. The `--features cascade` line does. The console line is the worker that formats `{kind}: {e}`.
