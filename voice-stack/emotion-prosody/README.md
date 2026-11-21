<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# S07 emotion prosody

Package `galion-sam-prosody`. Directory `voice-stack/emotion-prosody/`, the sector path in Swarm 16. It maps a live HORBM `Pad` onto Samantha's speaking parameters and slews that voice on the 80 ms tick. It does not synthesise audio, load a reference clip, or call a model.

The crate is its own Cargo workspace. It is not a member of the root voice workspace. The repository `LICENSE` covers it. This directory does not carry a second copy.

`rustc` 1.85 or newer (`rust-version` in `Cargo.toml`).

## What a tick returns

`ProsodyClock::tick` moves the applied mood one step toward the last target, then returns:

| Field | Rest (`Pad::RESTING`) | Bound |
|---|---|---|
| Chatterbox `exaggeration` | 0.52 | 0.48 ..= 0.65, this sector's choice |
| Piper `length_scale` | 1.00 | Samantha `length_bounds` |
| Piper `noise_scale` | 0.667 | 0.20 ..= 0.90 |
| Piper `noise_w` | 0.80 | 0.20 ..= 1.00 |
| `sentence_silence_s` | 0.20 | 0.08 ..= 0.45 |
| `pre_pause_ms` | 0 | 0 ..= 250 |
| pitch, semitones | 0 | Samantha `pitch_bound_st` (±3) |
| energy | 1.00 | 0.60 ..= 1.20 |
| worker `rate` | 1.00 | 0.80 ..= 1.20 |

`Pad::RESTING` is arousal 0.3, valence 0.25, dominance 0.5. That triple is kept. `EmotionFrame::neutral` shares the arousal and the dominance and uses valence 0. This crate does not copy a `Pad` into an `EmotionFrame`.

`horbm` does not document a `Pad` range in this repository. `ASSUMED_AROUSAL` (0..=1), `ASSUMED_VALENCE` (−1..=1), and `ASSUMED_DOMINANCE` (0..=1) are this crate's clamp. The valence numbers match the emotion-frame axis. A sample below 0 is negative on that assumption. It is not a value `horbm` has defined.

Higher arousal shortens `length_scale` (faster) and raises noise. Valence moves pitch. Dominance softens energy and adds a little air when it sits below rest. Energy never goes under 0.60, the floor `ProsodyControl::clamped` and the Samantha profile already use. There is no whisper token and no whisper gain. `propensity` is not a pitch curve. `ProsodyMap::map_with_caller` ignores the caller.

`rate` is `1 / length_scale`. `worker_rate` is that value clamped the way `speaking_rate_of` clamps it (0.8 ..= 1.2). `piper_length_scale` is `length_scale · 2^(pitch_st/12)`. `prosody_control()` returns a `galion_emotion::ProsodyControl` with `e0` warmth 0.62 and pitch range 1.

The 0.48..=0.65 exaggeration band is this sector's choice. `scripts/clone/chatterbox_worker.py` clamps exaggeration to 0..=2 and does not name those endpoints. The s711 plan text is not in this checkout.

## Where the numbers come from

* `SAMANTHA` in `galion-emotion/src/persona_mood.rs`: length, noise, pitch, energy, pause ceiling, and the hard bounds. The consts in this crate are those fields.
* Samantha `e0` / `e1` in `docs/galion-voice-v2/personas/voice/samantha.toml`: pitch 0, rate 1, loudness 1, pre-pause 0, `e1` pre-pause 250. A test reads that file.
* Sentence-silence slope 0.25 from `docs/galion-voice-v2/PERSONA-MOOD.md`.
* Deltas are from `Pad::RESTING`, not from the persona rest arousal of 0.35, so a brain at rest speaks the `e0` line. The persona formula's `e` term is the user's empathy weight. This sector does not invent `e` from the brain's own valence.
* The 80 ms tick and the 0.240 s approach time are `crates/galion-heartbeat` (`FRAME_MS = 80`) and `tools/symphony/voice-v2/src/fluid.rs` (`TAU_S`). One tick moves about 28% of the remaining gap.

`emotion-v2/emotions.toml` is the design inventory of named emotions. This sector does not pick one of those labels.

## Tags

The persona allow-list is `[laugh]` and `[chuckle]`, with `unknown_tags = "off"`. `on_piper` is false for every tag. `on_clone` is true only when the token is on that list.

* A giggle mood (bright, playful, held for 4 ticks / 320 ms, then a 50-tick / 4 s cooldown) puts `[chuckle]` on the packet. `[giggle]` is not emitted.
* An inhale mood (warm and unhurried, held for 4 ticks, then 40 ticks / 3.2 s) sets `breath` and lengthens the pre-pause. It does not carry `[inhale]`.
* Rest, a single hot tick, and a collapsed mood emit neither.
* The reply string is never rewritten. Piper still must not be handed a bracket tag.

## HORBM seam

`ReplyEngine::turn` returns a `TurnOut<Intent, Arm>`. `Intent` and `Arm` stay generic. `FakeReplyEngine` is the test stand-in. Only `mood` is copied onto the clock.

## Run

Linux, from the repository root:

```
cd voice-stack/emotion-prosody
cargo +1.85.0 test
cargo +1.85.0 run --release -- --ticks 1
cargo +1.85.0 run --release -- --ticks 12 --target 0.72,0.80,0.55
cargo +1.85.0 run --release -- --json --ticks 1
cargo +1.85.0 run --release -- --bench 100000
```

Windows, from the same checkout. If the repo is at `G:\galion`:

```
cd /d G:\galion\voice-stack\emotion-prosody
cargo test
cargo run --release -- --ticks 1
cargo run --release -- --ticks 12 --target 0.72,0.80,0.55
cargo run --release -- --bench 100000
```

The crate does not read `GALION_SAMANTHA_REF` and does not open `G:\galion`. `--target` and `--from` are `arousal,valence,dominance`. A bad pad exits 2.

## Measured on this host

Host: 4 vCPU KVM guest, `Intel(R) Xeon(R) Processor` (family 6, model 207), `rustc 1.85.0`. This is not the owner's Windows laptop. Nothing here was timed on that machine, and no audio was rendered.

`cargo +1.85.0 test --offline --test bounds -- --nocapture tick_mean_is_under_the_80ms_bar` (debug):

```
measure_ticks n=100000 elapsed_us=10766 mean_ns=107.7
```

Same test, `--release`:

```
measure_ticks n=100000 elapsed_us=1309 mean_ns=13.1
```

`cargo +1.85.0 run --offline --release --quiet -- --bench 100000`:

```
bench ticks=100000 elapsed_us=1405 mean_ns=14.1 tick_budget_us=80000
```

`cargo +1.85.0 run --offline --quiet -- --bench 100000` (debug):

```
bench ticks=100000 elapsed_us=10745 mean_ns=107.5 tick_budget_us=80000
```

The 80 ms bar is 80,000 µs per tick. The release binary's mean for this stage was 14.1 ns. That is the arithmetic of one mood step, not time-to-first-audio.

`cargo +1.85.0 run --offline --release --quiet -- --ticks 1` at rest:

```
t=0 ms=0 a=0.300 v=0.250 d=0.500 ex=0.5200 ls=1.0000 ns=0.6670 nw=0.8000 sil=0.2000 pause_ms=0.0 pitch_st=0.000 energy=1.0000 rate=1.0000 worker_rate=1.0000 tag=- breath=no
```

`cargo +1.85.0 run --offline --release --quiet -- --ticks 12 --target 0.72,0.80,0.55` (first tick, the chuckle tick, and the last tick of that run):

```
t=0 ms=0 a=0.419 v=0.406 d=0.514 ex=0.5498 ls=0.9524 ns=0.7105 nw=0.8357 sil=0.1702 pause_ms=0.0 pitch_st=0.511 energy=1.0581 rate=1.0500 worker_rate=1.0500 tag=- breath=no
t=5 ms=400 a=0.663 v=0.726 d=0.543 ex=0.6108 ls=0.8547 ns=0.7997 nw=0.9089 sil=0.1092 pause_ms=0.0 pitch_st=1.560 energy=1.1785 rate=1.1700 worker_rate=1.1700 tag=[chuckle] breath=no
t=11 ms=880 a=0.712 v=0.790 d=0.549 ex=0.6231 ls=0.8351 ns=0.8177 nw=0.9237 sil=0.0969 pause_ms=0.0 pitch_st=1.771 energy=1.2000 rate=1.1975 worker_rate=1.1975 tag=- breath=no
```

The numeric columns of those three lines are the same run as before the tag change. The token on tick 5 is `[chuckle]`. `cargo +1.85.0 test --offline`: 22 unit tests, 7 bounds tests, 1 allocation test, 0 failed. `cargo +1.85.0 clippy --offline --all-targets -- -D warnings` is run on this tree before the commit.

## Not in this crate

* The real `horbm` crate. The fake engine does not retrieve cards.
* Wiring into the heartbeat process, the Piper spawn, or `chatterbox_worker.py`.
* `[inhale]` and `[giggle]` on any packet. They are not on the persona allow-list.
* A listening check, or a timing run on the owner's machine.
