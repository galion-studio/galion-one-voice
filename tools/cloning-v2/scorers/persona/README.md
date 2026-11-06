# Persona scorer (opt-in)

Sibling of `scorers/universal/`. The old scorer stays the default.

Start at [INTERFACE.md](INTERFACE.md). Crying detector notes and probe false-positive counts: [CRYING.md](CRYING.md). Persona files: `pipeline/personas/own-samantha.toml` and `own-jarvis.toml`.

Love/warmth loss vs a detector clip is `pipeline/expression` (Rust) with a Python cross-check in `py/expression_diag.py`. The published Samantha gate (merge-v2 0.6869450087862239, coherent 0.6713695158531341) is `insufficient_evidence` until a clone cosine or a prosody delta is supplied. Jarvis is unchanged.
