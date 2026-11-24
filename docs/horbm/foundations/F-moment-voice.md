# F — moment on the voice

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S30. Samantha's voice carries the emotion-core moment. |
| Written | 2026-11-24. No voice audio, no recording, no weight, no network call. |
| Base | `main` after the emotion core, the native mouth, and the inner life. |
| Crate | `horbm-voice` module `moment_voice`. The brain gains `Brain::seat`. |
| Locks | `LOCKS.toml` was not edited. `affect_to_voice_max_frames` stays 2 and `affect_to_voice_max_ms` stays 160. The glue copies those numbers. |
| SOUL | `SOUL.md` is not in this tree. Nothing was added. |

The emotion core already keeps a baseline apart from the 80 ms moment. The native mouth already speaks on that same clock. This layer is the map between them.

`VoiceBaseline` is who is speaking: persona id, voice id, rest point, design pitch centre. `MomentVoice::tick` does not assign it. The controls are the difference of the moment from that rest. On the baseline, Samantha is the e0 line already in `galion-sam-prosody` (`length_scale` 1, energy 1, pitch offset 0). Pitch on the packet is a sentence offset. The centre in hertz stays the baseline's centre.

Aria is parked (`ARIA_PARKED`). So is every persona id other than `samantha`, including the emotion-core stand-in. A parked packet does not move with the moment and does not take Samantha's noise scales. `HorbmTurn` seats Samantha. `HorbmTurn::seat` is how a host parks Aria.

## What the mouth takes

Each frame emits `length_scale`, `noise_scale`, `noise_w`, sentence silence, pitch offset, energy, pre-pause, and a breath place. The gains are the Samantha slopes in `galion-sam-prosody`, recentred so a zero delta is `Pad::RESTING`. Approach, the fourth axis, adds a small pace and energy lean and then the same clamps.

The warm Piper process reads the three scales at spawn. Changing them would kill it. Pace is a sample stretch on the sentence, which is the later rate change the speech path already uses. Pitch, energy, pause, and breath are applied to the sentence buffer. `noise_scale` and `noise_w` stay on the packet for that spawn. They are not turned into dither. Inhale is silence before the body. Exhale is silence after it. There is no bracket token.

A new target is the midpoint on the next frame and the target on the frame after that. Two frames, 160 ms. A second target starts its own ramp from wherever the voice already is, so one frame never crosses the whole gap. Breath place switches when the ramp arrives, not halfway through it.

The inner life is not read here. It already moves the moment through `IntentMsg`. Empathy mood is not stacked on top of the moment: while Samantha is steering, the Piper mouth asks the synth for a resting pad and shapes the moment itself. A parked mouth keeps the old pad path.

## Checks

No model. The synthetic render is a generated tone. CI stays on the Linux workflow that already builds this crate.

```shell
cargo test --manifest-path horbm/Cargo.toml -p horbm-voice --features piper
cargo test --manifest-path horbm/Cargo.toml -p horbm --test moment
bash scripts/check-locks.sh
```
