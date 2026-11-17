# galion-throat

Perpetual Open MIT License. (c) 2026 Galion Labs.

Glottal source for one utterance. Per 80 ms tick: pitch versus the speaker baseline, phonation, open quotient, spectral tilt, jitter and shimmer. Feeling supplies published signs. The lungs tick supplies subglottal pressure, `f0_st` and `loudness_db`. Unknown step sizes are not applied.

The pulse is the Liljencrants–Fant model. Design and citations: [docs/galion-voice-v2/symphony/THROAT.md](../../../docs/galion-voice-v2/symphony/THROAT.md). Magnitudes: [configs/symphony/throat.toml](../../../configs/symphony/throat.toml).

```text
cargo test --manifest-path tools/symphony/throat/Cargo.toml
```

```text
galion-throat render \
  --feeling feeling.json \
  --lungs lungs.json \
  --baseline-hz 180 \
  --config configs/symphony/throat.toml \
  --out controls.json \
  --source source.wav
```

`--source` is optional. The wav is a synthetic LF derivative, peak-scaled, and is not a recording. Do not commit it.
