# galion-feeling

Perpetual Open MIT License. (c) 2026 Galion Labs.

The feeling core for Mouth and Expressions. One open continuous state, one
80 ms tick. Valence, arousal, dominance, approach, and any further axis an
intent frame names. The 24 Cowen names are optional pins, not the state.

```text
intent frames  ->  feeling  ->  controls tick   (PR #192, galion-emotion-render)
                           ->  lungs moment    (PR #195)
```

The intent wire is the local stand-in in `mne-contract/` (`galion-mne-contract/0`).
Agent S1 owns the real contract. This copy is deleted when that lands.

Knobs: `configs/symphony/feeling.toml`. A pin axis is a cited pole. A half-life
is config. `unknown` is not applied. Prose: `docs/galion-voice-v2/symphony/FEELING.md`.

```shell
cargo test --manifest-path tools/symphony/feeling/Cargo.toml --workspace
```

`galion-feeling` reads intent JSON on stdin and writes the run on stdout.
No audio.
