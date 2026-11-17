# Sound out

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-17.

This is the stage after the voice. Code: `tools/symphony/sound-out`. It does not replace the Chatterbox worker, Piper, the lungs planner, or the emotion-render patch. It calls them.

## What runs

One trait, `ClonerBackend`. Three adapters:

| Adapter | What it does in CI | Real weights |
|---|---|---|
| Chatterbox | Spawns `scripts/clone/chatterbox_worker.py` with `GALION_CLONE_STUB_PCM=1`. The request line includes `exaggeration`, `cfg_weight`, `temperature`, and `prosody.pitch_st`, `rate`, `energy`, `pre_pause_ms`. The worker applies them. Device on that process stays `cpu`, which is the landing it already refuses to leave. | `ChatterboxBackend::real` only if the pinned safetensors are already on disk. It does not fetch them. The `real-backends` feature test checks that gate and does not synthesize. |
| Piper | A vowel. `length_scale` is `1/rate`, pitch moves F0, energy moves amplitude, `pre_pause_ms` is leading silence. Same knobs the mouth already applies around Piper. Realtime on CPU. 80 ms frames. | `PiperBackend::real_if_present` only when `GALION_PIPER_RUNTIME`, `GALION_PIPER_MODEL` and `GALION_PIPER_CONFIG` already point at files. |
| Moshi | 80 ms frames of a quiet tone. No weights, no network. The real model is not reported as realtime on CPU. | Not implemented. |

Speech is placed on the lungs timeline (`galion-lungs`, PR #195). The inhale is known before the cloner returns, so those frames can leave first. Bursts are samples passed in by the nose layer. This crate places them. It does not invent the category's sound. Room gain and distance gain are the pipeline defaults from PR #194. They are not a cited distance curve. A positive `noise_db` adds a deterministic bed that never exceeds −24 dBFS. It is not dB SPL and it is not the Garnier Lombard slope.

Device pick matches PR #194: CPU unless CUDA is visible; an explicit CPU wins; ROCm is recorded and the clone stays on CPU.

## What is stubbed

- `galion-mne-contract` (agent S1). `contract::MomentFixture` is a local stand-in. TODO(S1) is on that module.
- Moshi weights and a real Moshi process.
- Piper ONNX, unless the three paths above are already set.
- Chatterbox Nano, unless the pinned pack is already on disk. CI uses the protocol stub.
- Burst synthesis. Agent S5 owns it. `synthetic_burst` is a test noise, not a laugh.

`cargo test --manifest-path tools/symphony/sound-out/Cargo.toml` is synthetic. It does not download weights and it does not write a recording into git.
