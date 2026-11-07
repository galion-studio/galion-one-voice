# cbx-v2 diff summary (vs bakeoff-t648/src/cbx)

**Binary:** `/workspace/swarm/voice-v2-sprint/cloning-v2/pipeline/cbx-v2/target/release/cbx`  
**Package:** `chatterbox-rs 0.2.0-pipe`  
**Upstream tree untouched.**

## Files changed
| File | Change |
|---|---|
| `Cargo.toml` | +rubato 0.16.2, +fastrand 2.3; version 0.2.0-pipe |
| `src/audio.rs` | rewrite: rubato sinc resample; true-peak ≤−1 dBTP; declick; OLA pace; mild pitch_semitones; `prepare_reference_for_encode` |
| `src/chatterbox.rs` | `SamplingConfig` {seed, temperature, top_p, repetition_penalty}; `sample_token`; encode uses prepare_reference |
| `src/lib.rs` | Speak defaults **fp32**; flags `--seed --temperature --top-p --pace --pitch-semitones --exaggeration --raw-pcm`; post-process on write |

## Not changed (honest limits)
- ONNX graph: no exaggeration/CFG input → `--exaggeration` warns only (emotion-fix worker owns further experiments in `cloning-v2/emotion-fix/`).
- PerTh: not present in Rust; documented in `docs/PERTH.md`; not removed anything.
- Original recordings / `own-samantha` / `own-jarvis` / `*.bak`: untouched.
