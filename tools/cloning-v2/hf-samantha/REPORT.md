# Samantha HF restoration — report (2026-11-05)

Scope: five reference-prep variants (A–E) → zero-shot cbx voices `own-samantha-v2-hf-*` → scored on the same 8 lines (t01–t08) as the baseline `own-samantha` renders (own-clone-analysis/wav/samantha). Every number below comes from `scores8/<variant>/*.json` (made by `scripts/score8.sh` and `scripts/summarize8.py`). Older 12-line and partial scores in `scores/` are kept but were not used for this table.

## Reference prep (input to cloning)
| ref | f99 Hz | % energy >4 kHz | LUFS |
|---|---|---|---|
| original reference.wav | 2672 | 0.39 | -20.6 |
| A-clean (denoise+declick+loudness) | 3129 | 0.70 | -21.8 |
| B-shelf (+5 dB shelf @2.8k, exciter 0.14) | 6445 | 2.47 | -23.8 |
| C-bwe (spectral-mirror BWE + shelf + exciter) | 4301 | 1.13 | -21.3 |
| D-orig44-bwe (BWE from 44.1k original) | 4102 | 1.03 | -21.1 |
| E-mild (milder HF lift, see refs/E-mild_meta.json) | 4395 | 1.36 | -22.9 |

Any energy above about 2.7 kHz is **estimated**, not recovered from the recording.

## Clone results (8 lines, fp32)
| variant | f99 Hz | %E>4k | ECAPA | WER | MOS | emo-sim | ang/neu (of 8) | words/s | F0 med Hz | artic syl/s | fric centroid Hz | clipped/clicks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| baseline | 2895 | 0.46 | 0.707 | 0.049 | 3.68 | 0.469 | 3/3 | 4.06 | 204 | 5.64 | 4948 | 2/11 |
| a-clean | 3179 | 0.60 | 0.685 | 0.017 | 3.96 | 0.433 | 4/3 | 4.04 | 196 | 5.66 | 5117 | 12/31 |
| b-shelf | 4518 | 1.82 | 0.683 | 0.017 | 3.78 | 0.403 | 4/4 | 4.06 | 208 | 5.80 | 4969 | 0/24 |
| c-bwe | 2870 | 0.39 | 0.600 | 0.038 | 4.28 | 0.412 | 7/0 | 3.98 | 205 | 5.54 | 4485 | 13/20 |
| d-orig44 | 2956 | 0.45 | 0.638 | 0.041 | 3.88 | 0.384 | 7/0 | 4.02 | 202 | 5.63 | 4516 | 11/20 |
| e-mild | 3795 | 1.06 | 0.700 | 0.034 | 3.87 | 0.368 | 6/2 | 3.98 | 205 | 5.56 | 4664 | 2/15 |

How to read the table: f99 and %E>4k are averaged per line (galion-hygiene). ECAPA is measured against the original reference (ceiling 0.828). WER uses Vosk-small. MOS is SQUIM. emo-sim and ang/neu come from galion-emotion against the reference, whose label is neu. words/s = reference words ÷ speech span. The remaining columns come from galion-artic.

## Verdict
- **Best: E-mild (`own-samantha-v2-hf-e-mild`).** It is the only variant that adds HF while staying inside the identity budget: ECAPA 0.700 vs 0.707 (−0.007). f99 goes from 2895 to 3795 Hz and energy above 4 kHz from 0.46% to 1.06%. WER falls from 0.049 to 0.034 and MOS rises from 3.68 to 3.87.
- **Cost:** emotion similarity drops from 0.469 to 0.368, and 6 of 8 lines are tagged "ang" (baseline 3/8). Fricative centroid also drops from 4948 to 4664 Hz. E sounds brighter or edgier to the detector, not calmer, so it fails the persona's "calm, warm" target on emotion. Listen before adopting it.
- B-shelf has the most HF (f99 4518 Hz) but ECAPA 0.683 (−0.024), so it is rejected. A-clean (0.685) is rejected too.
- C and D (BWE) lose identity badly (0.600 and 0.638). They also gain no HF in the output (f99 about 2.9k), and all 8 lines are tagged angry. Rejected.
- **Pace:** none of the variants fixes it. On this metric every clone, baseline included, runs at about 4.0 words/s, against about 2.82 for the reference (persona figure). The interim note that "hf-b-shelf runs fast at 4.09 w/s" is real, but the baseline is equally fast (4.06), so pace comes from cbx and not from the HF prep.
- Not beaten by anyone on emotion: the baseline has the best emo-sim.

## Rust vs Python
- **Rust:** synthesis (cbx, Chatterbox Turbo ONNX, fp32, CPU, 2 intra-threads) and all scoring: galion-ecapa, galion-hygiene (f99, HF, clicks, LUFS), galion-mos (SQUIM ONNX), galion-wer (Vosk), galion-emotion, galion-artic.
- **Python:** reference enhancement only (`scripts/enhance_refs.py`: numpy, scipy, librosa, noisereduce, pyloudnorm), plus the small table summarizer `scripts/summarize8.py`. Porting the enhancer to Rust is still open.

## Limits and caveats
- Renders used `bakeoff-t648/src/cbx` (the binary already wired into synth_missing.sh, with its default greedy decoding), not `pipeline/cbx-v2` with an explicit `--temperature 0`. All variants and the baseline share that binary, so the comparison is consistent. Re-rendering E with cbx-v2 `--temperature 0` is a cheap follow-up.
- Only 8 lines and a single take per line, so ECAPA sd is about 0.03 and a difference of −0.007 is within noise.
- There was no listening test. The emotion labels may be detector noise on TTS (persona note).
- The HF is synthesized. The real fix is to re-record with a full-band mic chain.
- Extra renders t09–t12 exist for C, D, and E (synth_missing.sh renders 12 lines) but were not scored.
- Nothing was deleted, committed, or trained. Originals, own-samantha, and the .bak files were not touched.
