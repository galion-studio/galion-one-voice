# galion-emotion — Rust-native emotion scorer report

**Binary:** `/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/emotion/target/release/galion-emotion`  
**Built:** 2026-11-05 20:41 Europe/Zagreb  
**Schema:** `galion-emotion/1`  
**Source:** `src/{main,audio,models,proxies}.rs`  
**No rebuild this pass** — existing `results/` and `logs/` were complete; nothing was redownloaded.

---

## What it scores

For each WAV (mono; resampled to 16 kHz for models/proxies that need it):

1. **Categorical SER** — ONNX `superb_er(.int8).onnx` (SUPERB / wav2vec2-base-superb-er). Softmax over **`neu`, `hap`, `ang`, `sad`**, plus `top` / `top_conf`.
2. **Dimensional A/V/D** — ONNX `msp_dim(.int8).onnx` (audeering wav2vec2-large-robust-12-ft-emotion-msp-dim). Continuous **arousal, valence, dominance** (≈0..1). Output order from the model is arousal / dominance / valence; JSON fields are named accordingly.
3. **Acoustic emotion proxies** (pure Rust, no ONNX) — pitch (`f0_mean_hz`, `f0_median_hz`, `f0_sd_st`, `f0_range_st_p5_p95`, `f0_range_st_minmax`, `voiced_frac`), energy (`energy_dr_db`, `energy_sd_db`, `rms_dbfs`), timing/rate/pauses, voice quality (**`jitter_local_pct`, `shimmer_local_pct` / `_db`**, `glottal_cycles`), spectral balance (`spectral_tilt_db_oct`, `alpha_ratio_db`, `hammarberg_db`).
4. **Clone-vs-ref** (when `--ref` is set) — SER JS divergence, AVD Euclidean delta/distance, proxy deltas, and a clipped composite `emotion_distance` / `emotion_similarity`.

Default ORT intra-op threads: **2** (`--threads`). `--int8` selects `*.int8.onnx`.

---

## Artifacts used

| File | Role |
|------|------|
| `results/baseline_samantha.json` / `baseline_jarvis.json` | Ref + 12 own-clone-analysis lines each (fp32) |
| `results/rust_all_fp32.json` / `rust_all_int8.json` | Same 26 paths in `results/files.txt` |
| `results/py_all_bench.json` | Python ONNX parity bench (soxr_hq), n=26 |
| `results/compare_rust_vs_python.txt` | SER/AVD + proxy vs older Py/Praat notes |
| `results/compare_proxies_vs_praat.txt` | Rust proxies vs Praat/parselmouth 0.4.7 |
| `results/rust_proxies.json` / `praat2pass.json` | Proxy cross-check inputs |
| `logs/*.time` | `/usr/bin/time`-style `real` / `user` / `sys` |

Related (read-only context): `../../emotion-fix/scores/{sam,jar}_old_emo.json` match these baselines’ SER counts / mean AVD; scoreboard `own-*-v2-emo-*.json` entries still show `"emotion": "pending"` / no wav outputs — not used as numbers here.

---

## Rust vs Python agreement (n=26, fp32)

From `results/compare_rust_vs_python.txt` (Rust `rust_all_fp32` vs this-run Python):

| Metric | Value |
|--------|-------|
| SER top-label agreement | **26/26** |
| max \|Δprob\| (per-file max over 4 classes) | mean **0.0153**, max **0.0389** |
| \|Δarousal\| | mean 0.0018, max **0.0035** |
| \|Δvalence\| | mean 0.0008, max **0.0021** |
| \|Δdominance\| | mean 0.0016, max **0.0030** |

vs an *earlier* Python REPORT label set: Rust (and this Python run) agree with that set on **25/26** tops — the one flip is `samantha/t01.wav` (**hap** now vs **ang** in the old REPORT). That is why the plan’s “8/12 ang” figure is one higher than the current fp32 baseline.

Proxies vs Praat/numpy (`compare_proxies_vs_praat.txt`, n=26): energy_dr_db mean\|d\| **0.161**, r=**0.995**; f0_mean/median strong (r≥0.991); **jitter_local_pct** weakly aligned (r=**−0.063**, mean\|d\| 0.720); shimmer r=0.634.

---

## fp32 vs int8 agreement (n=26)

Computed from `rust_all_fp32.json` vs `rust_all_int8.json`:

| Metric | Value |
|--------|-------|
| SER top-label agreement | **19/26** (7 flips) |
| max \|Δprob\| | mean **0.2226**, max **0.7414** |
| \|Δarousal\| | mean 0.0246, max 0.1645 |
| \|Δvalence\| | mean 0.0368, max 0.1740 |
| \|Δdominance\| | mean 0.0220, max 0.1646 |

Disagreements (fp32 → int8): `samantha/reference` neu→ang; `jarvis/t01,t03,t04` neu→hap; `samantha/t03` hap→ang; `samantha/t05,t07` neu→ang. **Prefer fp32 for label-sensitive baselines**; int8 is for throughput only.

---

## Speed and CPU

Shell times (`logs/*.time`, TIMEFORMAT `real %R user %U sys %S`). Same 26 files, `--threads 2`, `nice -n 10`. Wall times are load-sensitive (box: 8 cores; load was high during earlier runs).

| Run | real (s) | user (s) | sys (s) | JSON wall_s / cpu_s |
|-----|----------|----------|---------|---------------------|
| Rust fp32 (`rust_all_fp32`) | **87.767** | 61.551 | 6.876 | 87.56 / 68.28 |
| Rust int8 (`rust_all_int8`) | **36.586** | 34.102 | 0.727 | 36.46 / 34.73 |
| Python bench (`py_all_bench`, soxr_hq) | **114.826** | 71.642 | 9.032 | 108.83 / 77.66 |
| Python earlier soxr pass (`py_all_soxr`) | 880.900 | 115.390 | 87.368 | (older / colder; not used for speedup) |

**Speedup ratios (shell real wall, 26 files):**

- Python / Rust fp32 ≈ **1.31×** (Rust ~24% wall faster)
- Python / Rust int8 ≈ **3.14×**
- Rust fp32 / Rust int8 ≈ **2.40×** (int8 ~2.4× wall faster than fp32)

User-CPU ratios: Py/fp32 ≈ **1.16×**, Py/int8 ≈ **2.10×**, fp32/int8 ≈ **1.80×**.

Baseline-only (ref + 12 clones each, fp32): Samantha real **188.191** / user 44.779 (JSON wall 187.8 s, cpu 60.06 s — long ref WAV); Jarvis real **78.511** / user 33.749 (JSON wall 78.3 s, cpu 36.54 s).

Model sizes on disk: `superb_er.onnx` ~361 MiB / `.int8` ~116 MiB; `msp_dim.onnx` ~631 MiB / `.int8` ~195 MiB.

---

## Baseline emotion — Samantha

Sources: `results/baseline_samantha.json` (fp32, threads=2). Ref: `clip-store-own/samantha/reference.wav`. Clones: `own-clone-analysis/wav/samantha/t01..t12.wav`.

| | SER top | Arousal | Valence | Dominance |
|--|---------|---------|---------|-----------|
| **Reference** | **neu** (conf 0.349; probs neu 0.349, sad 0.318, hap 0.214, ang 0.119) | 0.499 | 0.553 | 0.569 |
| **Clone mean (n=12)** | — | **0.516** | **0.537** | **0.593** |
| Clone − ref (summary `mean_avd_delta`) | — | +0.017 | −0.016 | +0.024 |

**Clone top-label share:** **`ang` 7/12**, `neu` 3/12, `hap` 2/12. Mean P(ang)=**0.554**, P(neu)=0.337, P(hap)=0.099, P(sad)=0.011.  
Label match vs ref: **0.25** (3/12). Mean SER JS div **0.332**. Mean emotion_similarity **0.494**.

Per-line tops: t01 hap, t02 ang, t03 hap, t04 neu, t05 neu, t06 ang, t07 neu, t08–t12 ang.

**Correction vs earlier “8/12 ang” claim** (plan / persona docs): current Rust fp32 + matching Python bench + `emotion-fix/scores/sam_old_emo.json` all give **7/12 ang**. The missing “8th” is t01, formerly labeled ang in an older REPORT, now **hap** (Rust/Python agree; max\|dprob\| vs Py 0.0204).

Clone proxy means (vs ref): f0_mean **205.5** Hz (ref 190.1); f0_range_st_p5_p95 **10.70** (ref 9.11); energy_dr_db **27.72** (ref 28.77); jitter **1.38%** (ref 1.41); shimmer **8.14%** (ref 7.34). Summary `mean_proxy_delta`: f0_median_st **+2.04**, articulation_rate_ratio **1.11**, pause_ratio **−0.125**.

---

## Baseline emotion — Jarvis

Sources: `results/baseline_jarvis.json`. Ref: `clip-store-own/jarvis/reference.wav`. Clones: `own-clone-analysis/wav/jarvis/t01..t12.wav`.

| | SER top | Arousal | Valence | Dominance |
|--|---------|---------|---------|-----------|
| **Reference** | **neu** (conf 0.848; probs neu 0.848, hap 0.140, ang 0.009, sad 0.003) | 0.567 | 0.698 | 0.679 |
| **Clone mean (n=12)** | — | **0.508** | **0.499** | **0.599** |
| Clone − ref (`mean_avd_delta`) | — | **−0.059** | **−0.199** | **−0.080** |

**Clone top-label share:** **`neu` 12/12** — confirms the earlier Python finding. Mean P(neu)=**0.775**, P(ang)=0.133, P(hap)=0.091, P(sad)=0.0015.  
Label match vs ref: **1.0**. Mean SER JS div **0.082**. Mean emotion_similarity **0.489**. Mean AVD distance **0.230** (driven largely by valence drop).

Clone proxy means: f0_mean **144.4** Hz (ref 134.7); f0_range_st_p5_p95 **11.80** (ref 10.38); energy_dr **24.84** (ref 22.87); jitter **1.31%**; shimmer **8.93%** (ref 8.03).

---

## Limits

- **Four SER classes only**; no fine affect (warmth, sarcasm, etc.). SUPERB-ER / MSP-Podcast models are **not trained on TTS** — “ang” on Samantha clones may be detector bias on bright/fast synthetic speech, not intentional anger.
- **int8 is not label-safe** here (19/26 tops; max \|Δprob\| 0.74). Use fp32 for product / scoreboard emotion gates.
- **Jitter** proxy vs Praat correlation is near zero; treat jitter/shimmer as rough, not clinical.
- Wall times include **model load** (~1.7–5.8 s) and are **sensitive to box load**; prefer user-CPU or internal `timing.cpu_s` for comparisons.
- Audio longer than 30 s is **chunk-averaged**; very short tails (&lt;0.1 s) skipped.
- Original recordings (`Samantha.wav`, `Jarvis.wav`, `clip-store-own/*/original.wav` & `reference.wav`) are **read-only** inputs; this scorer does not modify them.
- Scoreboard `own-*-v2-emo-*` slots still pending generation — this report covers the scorer + own-clone-analysis baselines only.

---

## Quick reuse

```bash
cd /workspace/swarm/voice-v2-sprint/cloning-v2/scorers/emotion
nice -n 10 target/release/galion-emotion score --threads 2 \
  --ref /workspace/swarm/voice-v2-sprint/clip-store-own/samantha/reference.wav \
  /workspace/swarm/voice-v2-sprint/own-clone-analysis/wav/samantha/t*.wav \
  --out results/baseline_samantha.json
# faster / smaller (expect label drift): add --int8
```
