# Pipeline worker report — cbx-v2 quality bottlenecks

**When:** 2026-11-05 evening Europe/Zagreb (CET)  
**Scope:** `cloning-v2/pipeline/` only. Upstream `bakeoff-t648/src/cbx/` untouched. Originals read-only.  
**Coordination:** emotion-fix owns exaggeration experiments; scorers/* own ECAPA/MOS/WER/SER; this worker = Rust pipeline + hygiene.

## Code / binary
| | |
|---|---|
| Tree | `/workspace/swarm/voice-v2-sprint/cloning-v2/pipeline/cbx-v2/` |
| Binary | `.../cbx-v2/target/release/cbx` |
| Voices | `own-samantha-v2-pipe`, `own-jarvis-v2-pipe` (from originals via sinc+TP-limit encode) |
| Diff | see `DIFF_SUMMARY.md`, `PIPELINE.md`, `docs/PERTH.md` |

### Diff highlights
- `audio.rs`: rubato sinc resample; true-peak ≤ −1 dBTP; declick; OLA `--pace` (fixed: rate&lt;1 lengthens); mild `--pitch-semitones`
- `chatterbox.rs`: `SamplingConfig` + `sample_token` (temp/top_p/seed); encode uses `prepare_reference_for_encode`
- `lib.rs`: speak default **fp32**; new CLI flags; `--exaggeration` warns (ONNX has no such input)

## Before / after (12-line analysis set)

Measured with `galion-ecapa` (READY) + local hygiene script mirroring `audio.rs`.  
**Full re-TTS via cbx-v2 was OOM-killed** under 10-worker load (~2 GB/fp32 session × many cbx). Numbers below apply the new **write-path post** to the existing analysis WAVs (same decoder content) — valid for limiter/declick/pace; encode-path sinc + LM flags need a quieter box to score end-to-end.

### Samantha
| Metric | Baseline (analysis) | Hygiene only (TP+declick) | Hygiene + pace 0.85 |
|---|---|---|---|
| ECAPA mean | **0.707** | **0.707** | 0.695 |
| Peak dBFS mean | −1.13 (4 @ 0 dBFS) | −1.60 | −5.30 |
| Clipping samples | 9 / 6* | **0** | **0** |
| Clicks (Δ&gt;0.35) | 32 | 5 | **0** |
| Duration mean | 5.17 s | ~same | 6.06 s (slower) |
| MOS / WER / SER | 3.73 / 0.05 / ang-heavy | not re-run (scorer workers) | not re-run |

\*clip counts differ slightly vs report totals depending on ≥0.999 vs exact 0 dBFS definition.

### Jarvis
| Metric | Baseline | Hygiene only | Hygiene + pace 0.82 |
|---|---|---|---|
| ECAPA mean | **0.733** | **0.733** | 0.699 |
| Peak dBFS mean | −2.05 | −2.18 | −6.23 |
| Clipping samples | 4 / 3* | **0** | **0** |
| Clicks | 39 | **0** | **0** |
| Duration mean | 5.43 s | ~same | 6.55 s |

### Pitch post-hack (documented, not recommended)
Applying −1.0 / −1.5 st via resample **destroyed** ECAPA (Sam 0.51, Jar 0.63). Keep `--pitch-semitones=0` for premium path; F0 gap must be closed via better refs / emotion-ctrl / non-Turbo if needed.

## PerTh
Investigated: Python `perth` installed; **no Rust call site**; ONNX Random* = decoder noise. Documented in `docs/PERTH.md`. Nothing removed.

## Limits (honest)
1. Samantha reference HF hole (~0.4% energy &gt;4 kHz) — pipeline cannot invent presence (hf-samantha worker).
2. Turbo ONNX has **no exaggeration/CFG** input — emotion-fix worker’s problem space.
3. Non-zero temperature can miss STOP → use temp=0 for batch.
4. Full end-to-end TTS scoring of cbx-v2 blocked this session by **OOM** (10 parallel workers). Binary + voices ready; re-synth when quieter.
5. Mild OLA pace costs ~0.01–0.03 ECAPA; hygiene-only is ECAPA-neutral and clears clips/clicks.

## Sample paths
- `/workspace/samples/v2/pipeline/samantha/t01..t12.wav` (hygiene+pace)
- `/workspace/samples/v2/pipeline/samantha/tXX-hygiene-only.wav`
- `/workspace/samples/v2/pipeline/jarvis/...` (same)
- Scores: `cloning-v2/pipeline/scores/`
