# F-breath — lungs, glottis, mouth, nose

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM foundations, S20 |
| Crate | `horbm/crates/horbm-breath` (`horbm-breath`), own workspace member. Not in the root workspace (`Cargo.toml` excludes `horbm`). |
| Clock | 80 ms. `FRAME_MS` matches `LOCKS.toml` `clock.frame_ms`. |
| Written | 2026-11-24. CPU only. No audio file, no weights, no network. |

`LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not added.

## What a frame is

`plan_speech` and `plan_quiet` return one `Frame` per 80 ms tick. The voice can read:

| Field | Meaning |
|---|---|
| `phase` | `Inhale`, `Exhale`, or `Hold` (a boundary, not mid-phrase speech) |
| `speaking` | True only on a speech exhale |
| `subglottal_cm_h2o` | Model pressure. Positive drives air out. Speech is clamped to 3..=16. Quiet expiration stays in 0.2..=2. Inspiration is a small negative dip. |
| `airflow_ml_s` | Same sign as the pressure. Millilitres per second, not a spirometer trace. |
| `open_quotient` | Fant's quotient while speaking. An abduction fraction (folds open) on inhale, hold, and quiet breathing. |
| `breathiness` | Rises with Fant Rd while speaking. 1 when the frame is air, not voice. |
| `nasal_coupling` | 0 oral, 1 nasal. The caller sets the talk target. |
| `mouth_openness` | 0 closed, 1 open. The caller sets the talk target. |

`Track::to_bytes` is little-endian, magic `HB01`, schema `horbm-breath/1`. The same plan is the same bytes. A bad prefix is refused. Nothing in the bytes is a waveform.

## Three inputs, kept apart

- `VoiceBaseline` is the steady speaker: quiet period, I:E, modal speech pressure, modal Rd, resting nasal leak, resting mouth. `VoiceBaseline::ADULT` is the default. Planning does not write it.
- `Inside` is the body for this plan: `energy` (0 adds nothing), `fatigue` (0 rested, 1 spent), `health` (1 well, 0 unwell, not a diagnosis). Names match the MnE `InnerFactors` fields. This crate does not depend on that crate.
- `EmotionMoment` is plain `f32`s: `arousal` (−1..=1, 0 still), `amusement`, `disgust`, `fear` (each 0..=1). There is no `emotion-core` crate on `main`, and this crate does not name one. Valence and dominance are not inputs. No breath row was read for them.

A pause is a run of frames the caller already counted. This crate does not tokenize text. A pause shorter than the inspiration is lengthened so the whole inspiration sits in the pause. The next talk starts on the exhale. A trailing pause is an end-expiratory hold.

## What moves the numbers

Signs follow the directions in `DIRECTIONS`. The coefficients are model scales, chosen so a resting adult stays inside the envelopes. They are not cells copied out of a paper. Fatigue has no published magnitude; the sign here is that fatigue lowers speech pressure. Low health does the same and raises Rd (breathier, more open). Energy raises speech pressure. Positive arousal and fear shorten the breath group. Amusement shortens inspiration. Disgust inserts a hold between the inhale and the exhale. Negative arousal does not invent a slower breath.

The open quotient is Fant and Kruckenberg's Rd map (the same algebra as `tools/symphony/throat`). Rd 1 is about 65%. This crate does not sample the Liljencrants–Fant waveform.

## Crates that were not linked

`NOT_LINKED` in the crate records the same list:

- `organpool` 0.2 is MIT OR Apache-2.0, and it runs lungs on wall-clock threads. That is not an 80 ms speech clock, and it is not deterministic.
- `phoneme-synth` 0.3 is MIT OR Apache-2.0, and it renders PCM. Open quotient is not a field of its `Voice`. This crate does not render samples.
- `svara` 2 is GPL-3.0-only. `synthie-trombone` 0.2 is EUPL-1.2 and wants rustc 1.88.
- `galion-lungs` plans a phrase and also writes a breath waveform. The waveform stays out of this crate.

No path, no `.exe`, no `std::fs`, no `cfg(target_os)` branch. Linux is the CI target. The same source is what a later cross build would compile.

The conductor read-back is still its own stand-in. This crate does not replace it.

## Tests

Synthetic plans only.

```text
cargo +1.85.0 test --manifest-path horbm/Cargo.toml -p horbm-breath
cargo +1.85.0 clippy --manifest-path horbm/Cargo.toml -p horbm-breath --all-targets -- -D warnings
bash scripts/check-locks.sh
```

Quiet cycles stay at 10..=20 breaths/min with expiration longer than inspiration. A pause between two talks begins after an exhale and ends before the next exhale. Fatigue lowers speech pressure. Two calls with the same input encode to the same bytes.
