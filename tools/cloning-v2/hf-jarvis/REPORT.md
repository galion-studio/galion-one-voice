# Jarvis HF restoration — E-mild report (2026-11-05 Europe/Zagreb)

Scope: same E-mild reference-prep as Samantha (`shelf 2.5 dB @ 2.8 kHz` + `exciter mix 0.08` + denoise+declick+loudness) → zero-shot cbx voice `own-jarvis-v2-hf-e-mild` → scored on the same 8 lines (t01–t08) as the baseline `own-jarvis` renders (`own-clone-analysis/wav/jarvis`). Every number below comes from `scores8/<variant>/*.json` (made by `scripts/score8.sh` and `scripts/summarize8.py`). Originals / `own-jarvis` / `*.bak` were not touched.

## Reference prep (input to cloning)
| ref | f99 Hz | % energy >4 kHz | LUFS |
|---|---|---|---|
| reference.wav | 7254 | 5.96 | -22.7 |
| jarvis-ref-E-mild.wav | 8133 | 15.63 | -22.52 |

Python enhancer bandwidth (librosa STFT, see `refs/E-mild_meta.json`): in f99=7265.6 / %E>4k=6.01 → out f99=8144.5 / %E>4k=15.691.

Honest note: Jarvis’s original reference already had **f99 ≈ 7.25 kHz** and **~6% energy >4 kHz** (Samantha’s was 2.7 kHz / 0.4%). E-mild still pushed energy above 4 kHz from ~6% to ~15.6% and f99 from 7254 to 8133 Hz (galion-hygiene). This is not a “bandwidth recovery” case; it is a further HF boost on an already full-band ref.

## Clone results (8 lines, fp32)
| variant | f99 Hz | %E>4k | ECAPA | WER | MOS | emo-sim | ang/neu (of 8) | words/s | F0 med Hz | artic syl/s | fric centroid Hz | clipped/clicks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| baseline | 6009 | 3.08 | 0.748 | 0.007 | 4.14 | 0.504 | 0/8 | 3.71 | 140 | 5.87 | 5258 | 4/15 |
| e-mild | 7589 | 7.72 | 0.722 | 0.028 | 4.24 | 0.479 | 0/7 | 3.71 | 138 | 5.60 | 5260 | 1/321 |

e-mild also has **1× hap** (t01); baseline has 0 hap. Earlier 12-line baseline figures for context: ECAPA 0.733, WER 0.018, MOS 4.14 (ref 4.55), all lines neu. This 8-line rescore of the same own-clone-analysis wavs: ECAPA 0.748, WER 0.007, MOS 4.14, all neu.

## Per-line table (baseline vs E-mild)
| line | bl label | bl ang | bl neu | em label | em ang | em neu | bl ECAPA | em ECAPA | bl f99 | em f99 | bl MOS | em MOS | bl LUFS | em LUFS | label Δ |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| t01 | neu | 0.102 | 0.497 | hap | 0.142 | 0.304 | 0.793 | 0.756 | 6094 | 7371 | 3.85 | 4.04 | -20.38 | -21.22 | neu→hap |
| t02 | neu | 0.051 | 0.939 | neu | 0.092 | 0.902 | 0.771 | 0.686 | 5332 | 6106 | 4.12 | 4.28 | -21.23 | -21.92 | — |
| t03 | neu | 0.238 | 0.560 | neu | 0.096 | 0.668 | 0.715 | 0.728 | 6926 | 8098 | 4.01 | 4.34 | -21.23 | -21.26 | — |
| t04 | neu | 0.012 | 0.926 | neu | 0.025 | 0.860 | 0.729 | 0.736 | 6106 | 8367 | 4.26 | 4.33 | -20.64 | -21.29 | — |
| t05 | neu | 0.132 | 0.852 | neu | 0.156 | 0.797 | 0.799 | 0.758 | 6047 | 7980 | 4.17 | 4.08 | -20.91 | -20.65 | — |
| t06 | neu | 0.120 | 0.657 | neu | 0.023 | 0.582 | 0.712 | 0.746 | 6258 | 7945 | 4.40 | 4.17 | -20.59 | -21.02 | — |
| t07 | neu | 0.115 | 0.857 | neu | 0.193 | 0.746 | 0.716 | 0.599 | 6152 | 7418 | 4.05 | 4.31 | -21.37 | -21.57 | — |
| t08 | neu | 0.080 | 0.885 | neu | 0.086 | 0.865 | 0.750 | 0.766 | 5156 | 7430 | 4.29 | 4.35 | -20.62 | -21.48 | — |

### Label changes
- **t01: neu→hap** — baseline ang=0.102/neu=0.497; E-mild ang=0.142/neu=0.304/hap=0.550.

## Verdict / recommendation
- ECAPA: **0.748 → 0.722** (Δ **−0.026**). Keep rule was ECAPA drop ≤ ~0.01; this fails (−0.026). t07 alone collapses to 0.599.
- Emotion: **no angry regression** (baseline 0/8 ang, E-mild 0/8 ang). emo-sim 0.504 → 0.479. One label change: t01 neu→hap. Unlike Samantha E-mild (3→6 angry), Jarvis does not get “angrier” to the detector.
- WER: 0.007 → 0.028 (worse). MOS: 4.14 → 4.24 (slightly better). Clone f99 6009 → 7589; %E>4k 3.08 → 7.72.
- Hygiene cost: clicks **15 → 321** across 8 lines (exciter/shelf artifact on an already bright ref).
- Pace unchanged (~3.71 words/s).

- **Recommendation: Do not keep** `own-jarvis-v2-hf-e-mild` for production. Jarvis already had full-band HF; E-mild buys more brightness and a tiny MOS bump at the cost of identity (−0.026 ECAPA), higher WER, and a click explosion. Leave `own-jarvis` as the reference voice.

## Listening copies
- `/workspace/tmp/listen/jarvis-baseline-t02.wav` / `jarvis-e-mild-t02.wav`
- `/workspace/tmp/listen/jarvis-baseline-t01.wav` / `jarvis-e-mild-t01.wav` (only label change: neu→hap)

## Method notes
- Synth: `bakeoff-t648/src/cbx` fp32 CPU `--intra-threads 2`, voice `own-jarvis-v2-hf-e-mild` from `refs/jarvis-ref-E-mild.wav`.
- Baseline wavs: existing `own-clone-analysis/wav/jarvis/t01–t08.wav` (same set Samantha’s baseline used from `.../samantha`).
- Scorers: galion-ecapa / hygiene / mos (NMR=samantha ref) / wer / emotion (fp32 default) / artic under `cloning-v2/scorers/`.
- Voice registered only as `own-jarvis-v2-hf-e-mild`; `own-jarvis` and `*.bak` SHA256 unchanged (`logs/voices_before.sha256` == `voices_after.sha256`).
- No git commits, no deletes, no Piper, nothing to laptop.
