# galion-mouth

Perpetual Open MIT License. (c) 2026 Galion Labs.

Mouth targets on the 80 ms tick: formant shifts, lips, jaw, tongue, precision, speaking rate, coarticulation, and Rhubarb viseme timing. Design: [docs/galion-voice-v2/symphony/MOUTH.md](../../../docs/galion-voice-v2/symphony/MOUTH.md).

```bash
cargo test --manifest-path tools/symphony/mouth/Cargo.toml
cargo run --manifest-path tools/symphony/mouth/Cargo.toml -- plan \
  --intent intent.json --feeling feeling.json \
  --scales configs/symphony/mouth.toml --out plan.json
cargo run --manifest-path tools/symphony/mouth/Cargo.toml -- read --card articulation.json
# Optional, only if galion-articulation is already installed. Nothing is downloaded.
GALION_ARTICULATION=../articulation/target/release/galion-articulation \
  cargo run --manifest-path tools/symphony/mouth/Cargo.toml -- read --wav clip.wav
```

`intent.json` is a local stub until `galion-mne-contract` (agent S1) lands. Phones are caller-supplied. A missing `intensity` is 0, which adds no feeling. Step sizes in `configs/symphony/mouth.toml` are `unknown` until a source gives a number.

The cloner packet is `clone_mouth`. Visemes for an avatar are `avatar.cues` (phone intervals) and `avatar.ticks` (one Rhubarb letter per 80 ms).
