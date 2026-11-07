# galion-artic baselines (2026-11-05 ~20:40 CET)

## Same-text: reference vs clone rendering of the reference's own words (compare)
| | MCD-DTW dB | dur ratio | vowel |ΔF1| % | |ΔF2| % | |ΔF3| % | F0 offset (cents) | log-F0 corr | artic rate ref→clone | trimmed VSA Bark² ref→clone | fric centroid ref→clone Hz | VOT ms | pause ratio | score |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Samantha ← own-samantha | 6.05 | 0.87 | 10.0 | 6.0 | 6.2 | +123 | 0.45 | 5.41→5.12 | 13.3→14.1 | 4147→4780 | 51→45 | 0.23→0.14 | 79.3 |
| Samantha ← own-samantha-v2-hf-a-clean | 6.08 | 0.88 | 11.0 | 6.9 | 5.8 | +78 | 0.59 | 5.41→5.51 | 13.3→17.3 | 4147→4938 | 51→34 | 0.23→0.14 | 74.4 |
| Jarvis ← own-jarvis | 5.68 | 0.94 | 14.2 | 11.5 | 8.0 | +208 | 0.52 | 5.18→5.60 | 18.3→30.0 | 5449→5416 | 53→63 | 0.12→0.08 | 76.5 |
Anchors: same recording (original vs reference.wav cut) 0.10 dB / 99; different speaker+text 11.6 dB / 47.
One stochastic cbx render per voice → treat ±0.2 dB MCD / ±3 score as noise until re-rendered several times.

## Profile: reference vs 12 test lines (own-clone-analysis), clone mean ± sd
| metric | Samantha ref | Samantha clones | Jarvis ref | Jarvis clones |
|---|---|---|---|---|
| F0 median Hz | 176 | 202 ± 14 | 129 | 142 ± 11 |
| F0 range p5–p95 st | 9.5 | 11.5 ± 1.9 | 9.8 | 11.2 ± 1.5 |
| vowel F1 / F2 median Hz | 464 / 1750 | 495 / 1716 | 445 / 1277 | 451 / 1408 |
| trimmed VSA Bark² | 13.3 | 10.4 ± 4.7 | 18.3 | 14.9 ± 5.6 |
| articulation rate syl/s | 5.41 | 5.38 ± 0.91 | 5.18 | 5.56 ± 0.70 |
| speech rate syl/s | 4.14 | 4.98 ± 0.91 | 4.53 | 5.46 ± 0.83 |
| pause ratio | 0.23 | 0.07 ± 0.08 | 0.12 | 0.02 ± 0.04 |
| pauses / min | 24.9 | 10.3 ± 10.3 | 11.8 | 2.9 ± 4.5 |
| fricative centroid Hz | 4147 | 4818 ± 833 | 5449 | 5257 ± 582 |
| sibilant centroid Hz | 5044 | 5249 ± 751 | 5591 | 5703 ± 180 |
| plosive bursts / s | 1.31 | 2.21 ± 0.49 | 1.35 | 2.22 ± 0.52 |
| burst level dB re p99 | −14.5 | −15.9 ± 3.1 | −15.2 | −11.6 ± 3.2 |
| VOT proxy ms | 51 | 50 ± 16 | 53 | 71 ± 19 |
| HF tilt 4k dB | −32.8 | −34.0 ± 1.6 | −33.7 | −32.8 ± 2.9 |
| dynamic range dB | 20.6 | 19.9 ± 1.5 | 19.0 | 19.2 ± 1.0 |
Note: the 12 test lines are different text from the reference, so these are distribution comparisons only (scripted TTS lines naturally pause less).

## Clone-vs-clone same text (own-samantha t01–t05 → v2-hf-a-clean t01–t05)
MCD 4.32–4.78 dB, vowel |ΔF1| 3.7–11.6 %, |ΔF2| 3.0–9.4 %, score 80–91.
