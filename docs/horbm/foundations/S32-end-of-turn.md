# S32 — end of turn from ear prosody

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S32, end-of-turn from ear prosody |
| Written | 2026-11-24. No weight download, no voice file, no paid API, no merge. |
| Base | PR #284, branch `cursor/horbm-s27-ear-prosody-ba7e`. This change stacks on that branch. It does not retarget `main`. |
| Persona | Samantha only. The miss line stays `I don't know.` Aria is not wired. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. |

## What it decides

The ear already measures pitch, energy, speaking rate, and the trailing pause on each 80 ms frame. This layer adds the latest ASR partial and answers one question: has the user finished the turn?

The answer is a three-way message, `TurnMsg`:

| Message | Meaning |
|---|---|
| `PauseOnly` | Sensing is off. The conductor and the voice loop keep the pause rule they already had. |
| `Hold` | The user still has the floor. Do not eager-reply. |
| `Yield { question }` | The user has finished. `question` is a rising terminal contour. |

`galion-conductor` and `horbm-voice` are separate workspaces, so each keeps a copy of those three variants. The host maps one onto the other. The conductor takes the message on `tick_turn`. The voice loop takes it on `tick_turn`. `tick` is still pause-only. Barge-in does not read the message: overlap while Samantha is speaking still arms silence on the frame that contains the 200 ms lock, reported as 160 ms.

A yield on a quiet listening frame runs the brain and releases the clause on that same frame. Pause-only still speculates on the first quiet frame and releases the clause on the second. A hold does neither, including when the recognizer has already emitted a final (`um`).

## Why this is not a VAD crate

`voice-stack/vad` (`galion-sam-vad`) already ends a turn from silence plus the trailing word. It links `ort` and a Silero weight, it is its own workspace, and its `rust-version` is 1.92. `LOCKS.toml` still lists `ort` under `formats.forbidden_runtimes`. This change does not take that crate, and it does not download or commit a model.

Pitch stays `pitch-detection` 0.3.0 `YINDetector` (MIT OR Apache-2.0), which S27 already calls. The end-of-turn rules are glue on the scalars that detector and the ear already return. Thresholds live in `crates/horbm-ear/eot.toml` and are compiled in with `include_str!`. No path is opened at runtime. There is no `.exe`, no `cfg(unix)` in the new code, and no machine address.

The rules, from that file:

- A filler (`um`, `uh`, …) or an unfinished word (`and`, `the`, …) holds until `hold_pause_ms` (720). A falling contour does not override that. The word list is the one in `voice-stack/vad/src/turn.rs`.
- A strong fall (at least 4 semitones across the voiced tail) plus an energy drop, a finished word, and 80 ms of pause yields a statement.
- A milder fall (at least 1.2 semitones) with the same energy drop waits 160 ms. A sharp drop in speaking rate holds that milder fall, so a hesitant tail is not treated as an ending.
- A rise of at least 1.5 semitones, a finished word, and 80 ms of pause yields a question.
- While the frame is still voiced, the message is `Hold`. The yield waits until the pause.
- `FrameOut::Denied` (which is what `Ear::deny`, `EarConfig { sense: false }`, and `GALION_EAR_SENSE` set to `0`, `off`, or `deny` produce) clears the contour and returns `PauseOnly`.

`GALION_HORBM_RESEARCH_DIR` is not read and was not set.

## Measurements

Host: Linux, rustc 1.85.0. The clock is the virtual 80 ms frame. Signals are sines and exact prosody cues built in the tests. No audio file is committed or read.

Decision latency is `(first yield frame − last voiced frame) × 80 ms`. That is the time after the frame that held the last word. Nine finished turns: four strong falls and two rising questions through YIN, three mild falls as exact cues so pitch error cannot move the 160 ms bucket.

| Measure | Result |
|---|---|
| Decision latency p50 | 80 ms |
| Decision latency p95 | 160 ms |
| False cut-ins on that set | 0 |
| False cut-ins on `um...`, on a falling `and`, and on a flat finished word with a short pause | 0 |

Percentile is linear interpolation: rank `p/100 × (n − 1)` on the sorted sample, n = 9.

Eager-to-voice, same host, same frame clock:

| Path | Result |
|---|---|
| Conductor, prosody yield: end-of-turn stamp to clause stamp | 0 ms (same frame) |
| Conductor, pause-only: end-of-turn stamp to clause stamp | 80 ms |
| Conductor, silence start to clause | 80 ms with the yield, 160 ms pause-only. The clause is one frame earlier. |
| Voice loop: yield to first mouth frame | 80 ms. The mouth still waits one frame, the same gap an ASR final uses. |
| Voice loop: last word to first mouth frame | 80 ms when the yield is forwarded. 320 ms when the loop waits for a final scripted 240 ms after the word. The reply audio starts 240 ms earlier. The 240 ms is a scripted lag, not a live recognizer. |
| Barge-in, conductor and voice loop, with yield and hold messages arriving during the overlap | 160 ms. Lock remains 200 ms. |

## Tests

`cargo test -p horbm-ear` covers the scorer, the synthetic contours, the off switch, and the S27 frame tests. `cargo test --manifest-path crates/galion-conductor/Cargo.toml` covers the join, the pause-only gates, and barge-in. `cargo test --manifest-path horbm/Cargo.toml -p horbm-voice` covers the voice loop. CI stays the existing Ubuntu `gates` workflow. No Windows workflow was added. The new code does not use an OS-specific API; the runtime and CI this was measured on are Linux.
