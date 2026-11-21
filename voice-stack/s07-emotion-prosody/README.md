<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# S07 emotion prosody

Maps a live HORBM mood point onto Samantha's speaking parameters, and slews that voice on the 80 ms heartbeat. It does not synthesise audio, load a reference clip, or call a model.

The crate is its own Cargo workspace. It is not a member of the root voice workspace.

## What a tick returns

`ProsodyClock::tick` moves the applied mood one step toward the last target, then returns:

| Field | Rest (`Pad::RESTING`) | Bound |
|---|---|---|
| Chatterbox `exaggeration` | 0.52 | 0.48 ..= 0.65 |
| Piper `length_scale` | 1.00 | 0.80 ..= 1.30 |
| Piper `noise_scale` | 0.667 | 0.20 ..= 0.90 |
| Piper `noise_w` | 0.80 | 0.20 ..= 1.00 |
| `sentence_silence_s` | 0.20 | 0.08 ..= 0.45 |
| `pre_pause_ms` | 0 | 0 ..= 250 |
| pitch, semitones | 0 | −3 ..= 3 |
| energy | 1.00 | 0.60 ..= 1.20 |
| worker `rate` | 1.00 | 0.80 ..= 1.20 |

`Pad::RESTING` is arousal 0.3, valence 0.25, dominance 0.5. Higher arousal shortens `length_scale` (faster) and raises noise. Valence moves pitch. Dominance softens energy and adds a little air when it sits below rest. Energy never goes under 0.60, which is the floor `scripts/clone/chatterbox_worker.py` already applies. There is no whisper token and no whisper gain.

`rate` is `1 / length_scale`. `worker_rate` is that value clamped the way `speaking_rate_of` clamps it (0.8 ..= 1.2). `piper_length_scale` is `length_scale · 2^(pitch_st/12)`, the pitch-compensated spawn argument from `docs/galion-voice-v2/PERSONA-MOOD.md`.

## Where the numbers come from

* Samantha `e0` in `docs/galion-voice-v2/personas/voice/samantha.toml`: pitch 0, rate 1, loudness 1, pre-pause 0. `e1` pre-pause is 250 ms. Tags are `on_clone = true`, `on_piper = false`. A test reads that file and checks these constants.
* Samantha column of `docs/galion-voice-v2/PERSONA-MOOD.md` and `SAMANTHA` in `galion-emotion/src/persona_mood.rs`: pace −0.40 per unit arousal, pitch 1.6 per unit valence and 2.2 per unit arousal, energy 0.45, noise 0.30, noise_w 0.30, sentence silence slope 0.25, and the hard bounds above.
* Deltas are from `Pad::RESTING`, not from the older persona rest arousal of 0.35, so a brain at rest speaks the `e0` line. The persona formula's `e` term is the user's empathy weight. This sector does not invent `e` from the brain's own valence. Pre-pause uses the 250 ms `e1` gap as a ceiling, scaled by how far arousal and dominance sit below rest.
* The 80 ms tick and the 0.240 s approach time are `crates/galion-heartbeat` (`FRAME_MS = 80`) and `tools/symphony/voice-v2/src/fluid.rs` (`TAU_S`). One tick moves about 28% of the remaining gap. A late caller still gets one step, not a catch-up jump.
* Exaggeration stays in 0.48 ..= 0.65. That band is the s711 sector contract. The plan text is not in this checkout. The Chatterbox worker's own clamp is the wider 0 ..= 2.

`emotion-v2/emotions.toml` is the design inventory of named emotions. This sector does not pick one of those labels. The voice follows the live Pad.

## Tags

`[inhale]` and `[giggle]` are hints on the speaking packet. They are not written into the reply string. The mapper does not read the reply.

* Giggle opens only when valence, arousal, and dominance stay in a bright playful window for 4 ticks (320 ms), then waits 50 ticks (4 s).
* Inhale opens only for a warm, unhurried mood (arousal at or under 0.28, valence still positive) for 4 ticks, then waits 40 ticks (3.2 s).
* Rest, a single hot tick, and a collapsed low-dominance mood emit neither.
* The two gates do not overlap. Neither token is `[whisper]` or `[whispering]`.
* `on_piper` is false. Piper must not be given the token.
* `samantha.toml` `[tags].allowed` is `[laugh]` and `[chuckle]`, with `unknown_tags = "off"`. `[inhale]` and `[giggle]` are not on that list. A strict reader of the persona file drops them. `Tag::persona_allows` reports that.

## HORBM seam

`ReplyEngine::turn` returns a `TurnOut` with the fields the real `horbm::Brain` will return (`reply`, `intent`, `arm`, `propensity`, `evidence`, `sources`, `violations`, `lookup_us`, `took_us`, `mood`). `FakeReplyEngine` is the test stand-in. The real crate is not linked. Only `mood` is copied onto the clock.

## Run

Rust 2021. No dependencies, so the build does not download crates. The crate does not read `GALION_SAMANTHA_REF` and does not open `G:\galion`.

Linux, from the repository root:

```
cd voice-stack/s07-emotion-prosody
cargo test
cargo run --release -- --ticks 1
cargo run --release -- --ticks 12 --target 0.72,0.80,0.55
cargo run --release -- --json --ticks 1
cargo run --release -- --bench 100000
```

Windows, from the same checkout. If the repo is at `G:\galion`:

```
cd /d G:\galion\voice-stack\s07-emotion-prosody
cargo test
cargo run --release -- --ticks 1
cargo run --release -- --ticks 12 --target 0.72,0.80,0.55
cargo run --release -- --bench 100000
```

`--target` and `--from` are `arousal,valence,dominance`. A bad pad exits 2 and prints the usage line.

## Measured on this host

Host: 4 vCPU KVM guest, `Intel(R) Xeon(R) Processor` (family 6, model 207), `rustc 1.83.0`. This is not the owner's Windows laptop. Nothing here was timed on that machine, and no audio was rendered.

`cargo test --offline --test bounds -- --nocapture tick_mean_is_under_the_80ms_bar` (debug):

```
measure_ticks n=100000 elapsed_us=11043 mean_ns=110.4
```

Same test, `--release`:

```
measure_ticks n=100000 elapsed_us=1314 mean_ns=13.1
```

`cargo run --offline --release --quiet -- --bench 100000`:

```
bench ticks=100000 elapsed_us=1387 mean_ns=13.9 tick_budget_us=80000
```

`cargo run --offline --quiet -- --bench 100000` (debug):

```
bench ticks=100000 elapsed_us=10436 mean_ns=104.4 tick_budget_us=80000
```

The 80 ms bar is 80,000 µs per tick. The release binary's mean for this stage was 13.9 ns. That is the arithmetic of one mood step, not time-to-first-audio.

`cargo run --offline --release --quiet -- --ticks 1` at rest:

```
t=0 ms=0 a=0.300 v=0.250 d=0.500 ex=0.5200 ls=1.0000 ns=0.6670 nw=0.8000 sil=0.2000 pause_ms=0.0 pitch_st=0.000 energy=1.0000 rate=1.0000 worker_rate=1.0000 tag=-
```

`cargo run --offline --release --quiet -- --ticks 12 --target 0.72,0.80,0.55` (first, middle, and the giggle tick):

```
t=0 ms=0   a=0.419 v=0.406 d=0.514 ex=0.5498 ls=0.9524 ns=0.7105 nw=0.8357 sil=0.1702 pause_ms=0.0 pitch_st=0.511 energy=1.0581 rate=1.0500 worker_rate=1.0500 tag=-
t=5 ms=400 a=0.663 v=0.726 d=0.543 ex=0.6108 ls=0.8547 ns=0.7997 nw=0.9089 sil=0.1092 pause_ms=0.0 pitch_st=1.560 energy=1.1785 rate=1.1700 worker_rate=1.1700 tag=[giggle]
t=11 ms=880 a=0.712 v=0.790 d=0.549 ex=0.6231 ls=0.8351 ns=0.8177 nw=0.9237 sil=0.0969 pause_ms=0.0 pitch_st=1.771 energy=1.2000 rate=1.1975 worker_rate=1.1975 tag=-
```

`cargo test --offline` after that run: 19 unit tests, 7 bounds tests, 1 allocation test, 0 failed. `cargo clippy --offline --all-targets -- -D warnings` finished with no warnings.

## Not in this crate

* The real `horbm` crate and `horbm-memory`. The fake engine does not retrieve cards.
* Wiring into the heartbeat process, the Piper spawn, or `chatterbox_worker.py`. The JSON line uses field names those programs already read (`exaggeration`, `prosody.rate`, `energy`, `pitch_st`, `pre_pause_ms`) plus the Piper scales the worker does not apply.
* Inserting `[inhale]` or `[giggle]` into text. They are not on the persona allow-list.
* Any audio, including a listening check. Reference clips stay out of git.
