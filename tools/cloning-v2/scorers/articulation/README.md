# galion-artic — Rust-native acoustic articulation / mouth-expression scorer

Built 2026-11-05 evening (Europe/Zagreb, CET). Pure Rust (hound, rustfft, realfft, rubato, serde_json; no Python at runtime).
Replaces the planned Python `cloning-v2/pipeline/articulation_score.py` (not touched). Audio-only: everything is measured acoustically.

Binary: `/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/articulation/bin/galion-artic` (3.0 MB, static Rust, sha256 b47045d8…)
Source: `galion-artic/` (`cargo build --release -j 2 --offline`)

## Usage
```
galion-artic profile [opts] FILE.wav...                       # per-file profiles + distribution across files
galion-artic compare [opts] --ref REF.wav --clone C.wav [--clone C2.wav ...]   # same-text pairs
  -j N (threads, default 2)  --ceiling HZ (formant ceiling; auto 5500 if median F0>160 else 5000)
  --fmin/--fmax (F0 range, 65..600)  --min-pause S (0.3)  --pyin-beta-mean M (0.2; 0.1 strict .. 0.3 lenient)
  --frames (per-frame t/f0/f0_yin/int_db/f1..f3)  --out FILE  --compact
```
Any WAV (int/float, any rate, stereo mixed to mono). JSON to stdout or `--out`.

## What it measures (10 ms frame grid)
| Block | Method |
|---|---|
| F0 | pYIN (Beta prior over 100 YIN thresholds, ≤6 candidates/frame, Viterbi with 2 st/10 ms Gaussian transition + V/UV switch 0.01) and plain YIN (abs. threshold 0.15). FFT difference function, 1 kHz zero-phase LP, octave-up correction. 16 kHz. |
| Formants F1–F3 | Praat `To Formant (burg)` clone: resample to 2×ceiling, pre-emphasis from 50 Hz, Gaussian window 50 ms physical (25 ms effective), Burg LPC order 10, Aberth polynomial roots, 50 Hz < F < ceiling−50. Per voiced frame and per vowel segment. |
| Syllable nuclei / vowels | de Jong & Wempe-style: intensity peaks above (p99−25 dB), ≥2 dB dip between peaks, voiced. Vowel segment = voiced frames within 3 dB of peak (±100 ms); segment formants = median. |
| Vowel space | Convex hull of per-vowel (F2,F1): raw Hz², 5–95 % trimmed Hz², raw/trimmed Bark², F1/F2 ranges, Bark dispersion. |
| Rate | articulation rate = syllables / phonation time (span minus pauses ≥0.3 s); speech rate = syllables / span. |
| Pauses | internal silences ≥0.3 s: count, per-min, total, ratio, duration distribution, list, mean run (syllables between pauses). |
| Consonants | 24 kHz, band 0.5–11 kHz. Fricatives = speech frames, unvoiced, >50 % energy above 2.5 kHz → spectral moments (centroid, SD, skewness, excess kurtosis), sibilant (centroid>4 kHz) centroid. Plosives = abrupt ≥15 dB pre-emphasised onset after ≥25 dB-down closure (2.5 ms hop) → burst level re p99, burst centroid, VOT proxy (burst → next pYIN-voiced frame), long-lag fraction. HF tilt = median voiced 4–11 kHz vs 0–4 kHz energy (dB). |
| MCD | 25 mel-cepstra (40 mel bands, 16 kHz, 25 ms Hamming, cepstral 1/N scaling → SPTK-like dB), DTW over speech frames only, MCD on c1..c24 = (10/ln10)·√2·‖Δc‖ averaged on path. |
| compare extras | DTW-aligned vowel formant deltas (each ref vowel → median clone formants on mapped voiced frames): signed/abs Hz, abs %; aligned F0 offset (cents), MAD/RMSE after offset, log-F0 corr, V/UV agreement; 30 scalar metric deltas/ratios. |
| score (heuristic) | `articulation_score_0_100` = mean of: s_mcd = clamp(1−(MCD−4)/8), s_formant = exp(−mean(|ΔF1|%,|ΔF2|%)/20), s_vsa = min ratio of trimmed Bark² hulls, s_rate = min ratio of articulation rates, s_consonant = exp(−|Δ fricative centroid|/1500 Hz). Use sub-metrics for decisions; the composite is a convenience. |

## Accuracy vs praat-parselmouth 0.4.x (same frame times; `crosscheck/crosscheck.py`, results/crosscheck_praat.json)
Formants (Praat-voiced, non-silent frames), median |Δ| / fraction within 10 %:
- Samantha original: F1 0.72 % / 0.96, F2 0.79 % / 0.90, F3 0.74 % / 0.94
- Jarvis original:   F1 1.58 % / 0.88, F2 0.90 % / 0.91, F3 2.23 % / 0.82
- 7 clone files: F1 0.2–1.0 %, F2 0.2–0.9 %, F3 0.1–1.1 % median; 88–98 % within 10 %
- Synthetic 5-formant vowels (ground truth): Rust == Praat to ±2 Hz on /a/,/u/,/o/; both LPC trackers mis-split F2/F3 on high-F0 /i/ (220 Hz) and /æ/ F3 — known LPC limitation, same in Praat.
F0 (pYIN vs Praat ac, both-voiced frames): median |Δ| 8.6–16 cents, gross error (>20 %) 0–1.2 %, median F0 within 0.4 % (Samantha) / 1.2 % (Jarvis original). Voicing agreement 0.89–0.93 on Samantha, 0.64–0.82 on Jarvis (pYIN is stricter on the weakly periodic/creaky Jarvis voice; `--pyin-beta-mean 0.3` raises Jarvis agreement to 0.78–0.86 at GPE ≤4.8 %).
Syllable nuclei vs text syllable counts (26 files): count ratio 1.017, mean |err| 9.7 %, r = 0.979.
MCD-DTW vs independent numpy implementation: 6.048 vs 6.046 dB. Sanity: original vs its own reference.wav cut = 0.10 dB / score 99; Samantha vs Jarvis (different voice + text) = 11.6 dB / score 47.

## Speed (box shared by 10 workers, load avg 19–56; user-CPU time)
- profile, 26 files = 160.5 s audio: Rust 0.93 s CPU single-thread (≈172× real time; 0.72 s wall at -j 2) vs Python/parselmouth+numpy equivalent 8.7–9.5 s CPU → ≈ 9.8× less CPU.
- compare (17 s ref + 15 s clone, incl. DTW): Rust 0.21 s CPU vs Python 2.2–2.3 s → ≈ 10.6×. (Python side already does its heavy lifting in Praat's C++; Rust additionally runs pYIN+Viterbi, bursts, moments.)
- Peak RSS small (DTW matrix 2×n·m f32 ≈ 10 MB for 17 s pairs).

## Layout
`bin/galion-artic` · `galion-artic/` source · `results/` baselines · `crosscheck/` Praat + syllable + synthetic checks · `bench/artic_py.py` Python reference · `work/selfclone/` clone renderings of the references' own words (vosk transcript → cbx fp32, voices own-samantha, own-samantha-v2-hf-a-clean, own-jarvis; one stochastic render each) · `venv/` parselmouth (cross-check only) · `run_baseline.sh`.
see results/BASELINES.md for baseline tables
