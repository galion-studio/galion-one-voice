# cbx-v2 pipeline fork (quality bottlenecks + expressive controls)

**Work root:** `cloning-v2/pipeline/` (does not modify `bakeoff-t648/src/cbx/`).  
**Binary:** `cloning-v2/pipeline/cbx-v2/target/release/cbx`  
**Voices:** `own-samantha-v2-pipe`, `own-jarvis-v2-pipe` (never overwrite `own-samantha` / `own-jarvis` or `*.bak`).

## Fixes shipped
| Bottleneck | Fix |
|---|---|
| Linear resample | rubato `SincFixedIn` Blackman-Harris sinc (len 256) |
| s16 clamp, no limiter | float true-peak ≤ −1 dBTP + declick (jump>0.35) before s16 |
| Hardcoded argmax / no seed | `--seed`, `--temperature`, `--top-p`, `--repetition-penalty` |
| No pace / pitch knobs | `--pace` (OLA; rate&lt;1 slows/lengthens), `--pitch-semitones` (mild resample; **hurts ECAPA if ≥1 st — prefer 0**) |
| fp16 footgun | `speak` default dtype pinned to **fp32** |

## Exaggeration — honest limit
Turbo ONNX `conditional_decoder` inputs: `speech_tokens`, `speaker_embeddings`, `speaker_features` only.  
**No exaggeration/CFG/emotion input.** `--exaggeration` warns only. Emotion-control experiments live in `cloning-v2/emotion-fix/` (other worker).

## PerTh
Rust path never called perth (see `docs/PERTH.md`). Nothing removed. Apply Python `perth` post-hoc if required.

## Sampling caveat
Non-zero `--temperature` can miss STOP and run toward `--max-new-tokens` on this vocab. Prefer temp=0 for batch; cap max_new_tokens for temp experiments.

## Recommended premium flags
Hygiene-preserving (ECAPA flat):  
`--dtype fp32 --repetition-penalty 1.0 --temperature 0 --pace 1.0 --pitch-semitones=0`

Rate match toward refs (small ECAPA cost):  
Sam `--pace 0.85` · Jar `--pace 0.82` · keep pitch at 0.

## Samples
`/workspace/samples/v2/pipeline/{samantha,jarvis}/tXX.wav` (hygiene+pace post on analysis baselines; full TTS blocked by box OOM under 10-worker load).


## Experimental shared expression-diverse conditioning (off by default)
New-voice entry point: `bin/cbx-add-voice`. Explicit `--emotion-blend` or per-voice
`emotion_blend.enabled=true` selects a diverse <=6s reference while retaining original
identity tensors. Legacy cbx executable is not replaced. See [EMOTION-BLEND.md](EMOTION-BLEND.md).
