# galion-nose

Perpetual Open MIT License. (c) 2026 Galion Labs.

Nose and wordless sounds for one utterance. Velum opening and a nasal murmur on the 80 ms tick. Laughs, sighs, moans, gasps, hums and "mm" on their own track.

Magnitudes are `configs/symphony/nose.toml`. Directions and the papers are in `docs/galion-voice-v2/symphony/NOSE.md`.

## Reused, not rewritten

- The 24 names are `configs/emotions-v2.toml` (draft PR 189). This crate does not edit them.
- Burst detection stays in `galion-articulation` (draft PR 191). `galion-nose read` parses the laughter-worker JSON and the decode burst track. It does not run the detector.
- The 165 Hz band cut is that crate's `speaker_baseline` prior, used only to pick which Bachorowski column a laugh F0 comes from.
- Unvoiced noise uses the heartbeat LCG, seed 13 (`crates/galion-heartbeat/src/presence.rs`).
- The intent object is a local stub. `TODO(S1)`: replace `src/contract.rs` with `galion-mne-contract`.

A reference burst is taken from the speaker's own samples at runtime. No burst audio is committed. No model is downloaded.

## Run

From the repository root:

```bash
cargo run --manifest-path tools/symphony/nose/Cargo.toml -- plan \
  --text "That was perfect." --burst laugh --f0 120 --category amusement --out /tmp/nose.json
cargo run --manifest-path tools/symphony/nose/Cargo.toml -- render \
  --plan /tmp/nose.json --sr 24000 --out-nose /tmp/murmur.wav --out-bursts /tmp/bursts.wav
cargo run --manifest-path tools/symphony/nose/Cargo.toml -- read \
  --json tools/symphony/nose/fixtures/laughter-worker-gap.json
```

`--reference-laugh` (and the same flag for sigh, moan, gasp, hum, mm) reads a WAV at runtime and does not copy it into git.

```bash
cargo test --manifest-path tools/symphony/nose/Cargo.toml
```

Tests are synthetic. They do not measure a person and they do not download a model.

## What is stubbed

- `galion-mne-contract` (agent S1). The schema here is `galion-mne-intent-stub/0`.
- Feeling does not move the velum until `feeling.velum_per_intensity` is set. No per-category nasalance was read.
- A1–P0 is a direction word. The dB slope ships at 0.
- The oral antiresonance is a control. Speech is not filtered.
- Sigh, moan, gasp, hum and "mm" have no published acoustic target. Their durations are config.
- The laughter worker in PR 191 writes a gap unless a local checkout already exists. This layer does not fill that gap.
- No cloner is called. CPU synthesis is the sound, or the caller's own reference.
