# galion-lungs

Perpetual Open MIT License. (c) 2026 Galion Labs.

Physiology track for one utterance: plan the inhalations, step relative lung volume and subglottal pressure, and render an audible breath. Breath is not an emotion. The 24 categories are inputs, read from `configs/emotions-v2.toml`.

Magnitudes are `configs/symphony/lungs.toml`. Directions and the papers are in `docs/galion-voice-v2/symphony/LUNGS.md`.

## Reused, not rewritten

- Breath cues, `apply_cues`, and `splice_inhales` match the features agent's `tools/symphony/articulation/src/breath.rs` (agent bc-56b9f22c, draft PR 191). `measure` stays in that crate. This tool reads the JSON it writes.
- Inhale noise is the heartbeat breath: white noise, LCG seed 13, neutral RMS 0.003 (`crates/galion-heartbeat/src/presence.rs`).
- The −55 dBFS floor is `galion-presence`'s default breath gate.

A reference inhale is taken from the speaker's own samples at runtime. No breath audio is committed.

## Run

From the repository root:

```bash
cargo run --manifest-path tools/symphony/lungs/Cargo.toml -- plan \
  --text "Hello there. This one is longer." --category sadness --arousal 0.2 --out /tmp/lungs.json
cargo run --manifest-path tools/symphony/lungs/Cargo.toml -- render \
  --plan /tmp/lungs.json --sr 24000 --out /tmp/breaths.wav
```

`--reference` needs `--lungs` pointing at the features agent's `lungs` object or breath-worker cues. The WAV is read and not copied into git.

```bash
cargo test --manifest-path tools/symphony/lungs/Cargo.toml
```

Tests are synthetic. They do not measure a person and they do not download a model.
