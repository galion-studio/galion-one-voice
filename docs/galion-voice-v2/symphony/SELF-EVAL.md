# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A section is done when its gate is recorded here.

## jarvis-texture

Command: `scripts/eval/jarvis-texture.sh`

The hub is a separate layer. Off copies the clean-speech asset. On adds
the layer. The band check requires the on mix to clear the baseline by
`0.70` of the layer's click, alarm, and ambience energy. A stripped mix
is not accepted as off. A forbidden clean stage fails even when the mix
is right.

| Check | Result | Why |
|---|---|---|
| Toggle ABI and band check, synthetic baseline | PASS | `cargo test -p galion-ml-voice --lib jarvis_texture`. Both rates, 48 kHz and 24 kHz. Asset samples unchanged. |
| Owner `jarvis.wav` band check | BLOCKED | The wav is gitignored and not in this tree. Rights in `VOICE-CLONING.md` stay closed. This gate does not open the file. |

No FAIL on the synthetic gate. `LOCKS.toml` was not edited.

— Maciej Grajczyk
