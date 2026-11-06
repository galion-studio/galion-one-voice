# Cloning V2 — ten parallel workstreams (index)

See `CLONING-V2-PLAN.md` §0A / §3 for full detail. **Emotion is P0 #1.**

| # | Directory | Crate | Priority |
|---|---|---|---|
| 1 | `cloning-v2/plan/` | docs | Maintain plan |
| 2 | `cloning-v2/hf-samantha/` | `refprep-rs` | P0 (Sam HF) |
| 3 | `cloning-v2/similarity/` | `simsel-rs` | P0 |
| 4 | `cloning-v2/pipeline/` | `cbx-v2` | P0 |
| 5 | `cloning-v2/emotion-score/` | **`galion-emotion`** | **P0 #1** |
| 6 | `cloning-v2/emotion-fix/` | **`cbx-emo`** | **P0 #1** |
| 7 | `cloning-v2/ecapa/` | `galion-ecapa` | P0 |
| 8 | `cloning-v2/articulation/` | `galion-artic` | P0 (mouth) |
| 9 | `cloning-v2/mos-wer/` | `galion-hygiene` | P0 |
| 10 | `cloning-v2/scoreboard/` | `galion-board` | P0 (promote gate) |

Hard rules: originals read-only; V1 clones are backups; nothing to laptop; no Piper own-voice training; Rust hot path only.
