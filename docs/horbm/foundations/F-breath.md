# F — breath

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S45, lungs for MnE V2. Volume, phrase pauses, pressure, procedural inhale. |
| Crate | `horbm/crates/horbm-breath`. Member of the `horbm` workspace. |
| Clock | 80 ms. One `Lungs::tick` is one frame. |
| Persona | Samantha. `Lungs::open` refuses any other id. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not added. |

Emotion stays an open point. This crate does not read a category name. The baseline voice (persona bytes, voice id, pitch centre) is not an input. `Drive::from_core` reads the moment's arousal and the inside factors energy, fatigue, and health. The core's own lung-volume slot is the stand-in cycle; the lungs here own the volume track and do not copy that slot.

## Two passes, both bands

The duplex tick is both bands on one call. Breath uses that same call.

1. **Plan.** `plan_pauses` walks the reply once and writes pause slots into a caller buffer. A sentence end or a line break closes a group. A comma, semicolon, or colon closes one only once the group is already long enough. A word that would pass the group budget starts a new group. That budget is the air. Higher arousal, and energy above rest, shorten it. The last group has no trailing pause.
2. **Tick.** `Lungs::tick` steps volume, phase, flow, and pressure. `speaking` is the me band: inhale, one hold frame, then speech, and a new inhale when the budget or a planned break is reached. `speaking == false` is the you band on that frame: a short release, then a quiet inhale back toward rest. No speech air is spent.

`Lungs::frame` is both passes. Pass 1 runs only when a reply is passed, and the same text does not reset the phrase clock.

## What a frame carries

- **Pauses.** `Pause::at_byte` is the byte where the mouth should break the reply. `PauseWhy::Air` is the budget. `PauseWhy::Boundary` is the punctuation.
- **Envelope.** During speech, `pressure` is relative subglottal drive and `loudness_gain` is the mouth's amplitude multiplier. Off speech the gain is 0. The decibel figure is an offset from the model's own reference, not dB SPL. The gain is a first-order map of that offset (`DB_TO_LIN`).
- **Inhale.** `Lungs::write_inhale` fills a caller buffer. The generator is the heartbeat LCG, seed 13, scaled to RMS 0.003, with a parabola window so the block starts and ends at 0. No recording is stored. A frame that is not an inhale writes nothing.

## Glottis (S41)

`GlottalAir` is the whole seam: `pressure` and `flow` for one 80 ms frame. Flow is +1 inhale through −1 exhale. Pressure during speech is the relative subglottal value. While inhaling, releasing, holding, or resting it is `OPEN_PRESSURE` (0.05), not a phonation drive. S41's glottis reads that struct and owns formants, open quotient, and F0. This crate does not link `tools/symphony/articulation` or any other S41 crate.

`horbm-duplex` still has `SpeakingBreath` for its own virtual-clock tests. The end-to-end smoke does not. It calls `Lungs::frame` on the breath port, so the report says `breath=real` and prints the tick time. Ear prosody is unchanged: on unless `GALION_EAR_SENSE` is `0`, `off`, or `deny`.

## Knobs

Directions follow `docs/galion-voice-v2/symphony/LUNGS.md` and the coefficients in `configs/symphony/lungs.toml`. Magnitudes are those knobs, not re-measured papers. Rest volume is the emotion core's `VOLUME_REST` (0.55), so a resting tick matches the inside factor. Category rows (amusement, disgust, fear) are not applied. Fatigue's published magnitude is still none; the toml coefficients are wired and labelled as such. Energy raises the rate through `ENERGY_RATE_K`, zeroed at `ENERGY_REST`. That slope is not in the toml. `galion-lungs` is not linked: it plans a whole timeline, allocates, and reads category names.

`stamp_intent` copies phase, volume, and flow into an `IntentMsg` and sets the breath mask. It does not write the baseline.

## Budget

`TICK_ALLOC_BUDGET` is 0. `tests/zero_alloc.rs` counts a thousand ticks, plans, and inhale writes. Release, rustc 1.85.0, 200 batches of 100 calls, nearest-rank, this machine:

```text
breath tick p50_us=0.027 p99_us=0.028 p50_ns=27 p99_ns=28 batches=200 batch=100
breath inhale p50_us=2.330 p99_us=2.911 p50_ns=2330 p99_ns=2911 batches=200 batch=100
```

Both sit under the 80 ms frame. The release smoke on this host, prosody on, printed:

```text
breath=real
breath_ns=30
breath_p50_us=0.030
breath_p99_us=0.031
breath_pauses=0
breath_group_ms=3360
```

The prosody-off run printed `breath_ns=26`, `breath_p50_us=0.026`, `breath_p99_us=0.027`. Pauses are 0 because "The kettle is on the hob." fits in the 3360 ms group. The smoke calls `Duplex::set_aec(false)`: there is no speaker, so the script is not a microphone echo. Ear prosody is left on.

```shell
cargo test --manifest-path horbm/Cargo.toml -p horbm-breath
cargo clippy --manifest-path horbm/Cargo.toml -p horbm-breath --all-targets --no-deps -- -D warnings
bash scripts/horbm-e2e-smoke.sh
```
