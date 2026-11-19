# Emotion render

Perpetual Open MIT License. (c) 2026 Galion Labs.

Maps one 80 ms target onto the cloning controls that already exist. It does not synthesise audio.

The inventory is `configs/emotions-v2.toml` (24 Cowen 2019 categories, both tracks). Acoustic signs are cited in `src/direction.rs` from Juslin & Laukka 2003, Banse & Scherer 1996, Hughes et al. 2010, and Boiten 1998. Cowen et al. 2019 names the categories and the prosody tier. A missing row stays unknown. Step sizes live in `configs/symphony/emotion-render.toml`. `unknown` does not become a number.

Neutral is the speaker baseline: pitch shift 0, rate 1, energy 1, no extra pause. Chatterbox exaggeration, cfg_weight and temperature stay at the worker's existing literals unless a step size is configured. Those three knobs are not parameters in the papers above.

Prosody applies the speech signs. Burst keeps the category and does not apply speech signs: a wordless sound is not a shift against this speaker's speech. Any of the 24 can be the prosody of a word, including embarrassment. The text is passed through.

## Worker JSON

`clone_request_patch` emits `exaggeration`, `cfg_weight`, `temperature`, and `prosody.pitch_st`, `range`, `rate`, `energy`, `pre_pause_ms`.

The Nano worker applies `prosody.rate` only (clamped to 0.8..=1.2) and logs the other prosody fields. It does not read exaggeration, cfg_weight or temperature off the request. Wiring that is the pipeline's job.

## Sibling interfaces

- Spec and labels: `configs/emotions-v2.toml`, `configs/symphony/emotion-labels-v2.toml` (PR 189).
- Decoder `TickState` JSON (`t_ms`, `arousal`, `valence`, `dominance`, `intensity`, `blend`) is accepted. That decoder's fixed exaggeration/cfg/speed formulas are not used. `speed` here is the rate multiplier.
- Evals (PR 190) do not call this crate. Direction words match the label file.
