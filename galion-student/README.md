# galion-student

This directory holds the plan scaffolding for the Galion Voice distilled student: PersonaPlex-7B distilled into a 1.0B model (S1), with 1.6B (S2) and 0.5B (S0) as alternates.

**Status:** config stubs only. Nothing has been trained and no weights are included.

- **License:** the code and configs here are Perpetual Open MIT. Weights ship separately, with their own `LICENSE-WEIGHTS`.
- **Weights:** exported as safetensors with a quant metadata table (primary path only; no foreign pack formats).
- **Quantization:** Q8 per-channel FFN and attention. int4 is allowed only for embeddings and the LM head.
- **GPL code:** `rlx-*` is a reference only. No GPL code is copied in.
- **Data:** no NC data. That excludes Meta Seamless Interaction and the original Emilia. The loader rejects any shard tagged NC or unknown.

The full plan is in `galion-voice-20/B7-student-plan.md`.

| File | Purpose |
|---|---|
| `configs/student.toml` | Architecture for the S0, S1, and S2 variants |
| `configs/distill.toml` | Stages 0–5: prune, KD, duplex KD, persona KD, Q8 QAT |
| `configs/data.toml` | Shard manifests and the license allowlist |
| `configs/eval.toml` | Gates in the style of Full-Duplex-Bench v1/v1.5, plus CPU RTF gates |
| `configs/openmind-slot.toml` | The Obsidian Open Mind v1 model slot contract |
