# F — inner life

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S15, inner life. Roles: cares and wants, expectation and surprise, intent exchange, tests and this note. |
| Written | 2026-11-24. No voice audio, no recording, no weight, no network call. |
| Crate | `crates/galion-emotion-core` module `inner`. HORBM calls it from `Brain::inner_turn`. |
| Base | Stacked on PR #263 (`cursor/emotion-core-moment-0010`). Trunk is `main` after that PR. |
| Locks | Listed below. `SOUL.md` is not in this tree. Nothing was added for it. |

The inner life is what the speaker cares about, wants, and expects. It is small fixed state, updated once per dialogue turn from the heard line and the emotion point. The point is the moment on valence, arousal, dominance, and approach. It is not a class id. `CARE_SLOTS` (4) and `WANT_SLOTS` (4) are buffer sizes. A care is an open token, a strength, and a preferred point. A want is an open token, a strength, and whether the cue has been met. This layer does not call `project_universal` and it does not store a Cowen name.

Persona and memory stay the stand-in files. `standins/inner-life.txt` is only the seed. The cue is the last word (`heard`, `asked`, `speaking`). A label longer than 32 bytes is clipped. The parser does not move the moment.

## Expectation and surprise

The expected point starts on the speaker baseline. A turn measures the gap before it updates. The point gap is the mean absolute difference on the four axes. The word gap is 1 when the line has a word and none of them is the expected token. Surprise is `hold * (point gap + 0.50 * word gap)`, clamped to 0..1. `hold` starts at 1. After the measure, the expected point becomes the observed point. A lexical valence cue then shifts valence by 0.125. The cues are `thanks`, `good`, `yes`, `glad` against `no`, `not`, `stop`, `wrong`. Those eight words are not an emotion inventory. The strongest care then pulls the expected point 0.125 of the way toward the point it prefers. That preferred point is seeded from the baseline and a turn does not move it.

A surprise at or above `SURPRISE_MIN` (0.0625) emits a GEMI intent with the open verb `surprise`. The pull is the point error. When the words missed and the point did not move, the pull is an arousal startle of 0.50. `EmotionCore::step` is the port that applies that pull. The step does not assign `VoiceBaseline`. This layer does not hold a baseline.

## Intent

The message is `IntentMsg`, 72 bytes, magic `GEMI`, version 1. There is no second format. Outbound source is `Own`. Inbound bytes are `IntentMsg::decode`. A fraction of an inbound pull (`INBOUND_FOLD` 0.25, times strength) is added to the expected point. That updates the working expectation. It does not write the kept weights.

## Learning

The turn updates live strengths, `met`, and the working expectation. It does not write the kept copy. `learn` copies live cares, wants, and the expectation into that copy only when `LOCKS.toml` `[consent] learn` is `allow`. Any other value, and a missing key, are deny. `at_rest` and `from_standin` read the lock text compiled in from `LOCKS.toml`. The turn does not open the file. Learning is not a fifth EmotionWire bit. The wire stays `sense`, `adapt`, `remember`, `share`.

`TURN_ALLOC_BUDGET` is 0. The turn does not call libm. `EmotionCore::step` is still the frame path, and its own budget is unchanged.

## Lock changes

- `LOCKS.toml` `[consent]`: added `learn = "deny"`. Not added to `bits`. `default`, `sense`, `adapt`, `remember`, and `share` stay `deny`. No other key was changed.
- `scripts/check-locks.sh`: not changed. It still requires those four bits to be deny. This crate treats a missing `learn` key as deny. `allow` is the only value that permits `learn`.
- `SOUL.md`: not in this checkout. No edit.

```shell
cargo test -p galion-emotion-core
cargo test --manifest-path horbm/Cargo.toml --workspace
bash scripts/check-locks.sh
```
