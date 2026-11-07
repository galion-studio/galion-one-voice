# Similarity V2 — speaker-similarity gap (Samantha / Jarvis)

**Agent work root:** `cloning-v2/similarity/` only.  
**Time zone:** Europe/Zagreb (CET).  
**Constraints honored:** originals read-only; `own-samantha` / `own-jarvis` / `*.bak` untouched; no Piper; no laptop; `nice -n 10`, cbx `--intra-threads 2`, cargo `-j 2`.

## Rust tooling (`simsel-rs`)

Binary: `cloning-v2/similarity/simsel-rs/target/release/simsel`

| Command | Role |
|---|---|
| `prep-cleaned` | trim + declick + peak/loudness norm (TP ≤ −1 dBFS), mono 24 kHz |
| `prep-multiclip` | overlapping windows / phrases from **same** original, crossfade |
| `prep-all` | both personas → `refs/*-rs-*.wav` |
| `best-of-n` | N cbx synths; select by **0.65×(1−emotion_distance)+0.35×ECAPA** |
| `score-pair` / `scorers-status` / `wait-scorers` | wire to `cloning-v2/scorers/{ecapa,emotion,…}` |

## Rust vs Python timings

| Task | Rust | Python | Speedup |
|---|---:|---:|---:|
| Ref prep-all (2× cleaned+multi+phrases) | **625 ms** | 8814 ms | **14.1×** |
| ECAPA 1 file (`galion-ecapa` vs `ref_score.py`) | **4451 ms** | 25963 ms | **5.8×** |
| Cosine agreement (same file) | **0.6878** | **0.6878** | exact |
| Emotion 1 file (`galion-emotion`) | ~16257 ms | — | — |

ECAPA `READY` present. Emotion binary used via soft-discover (`target/release/galion-emotion`) until its READY lands. Python kept only as cross-check/fallback.

## Voices created (new only)

| Voice | Ref pack |
|---|---|
| `own-samantha-v2-sim-cleaned` / `own-jarvis-v2-sim-cleaned` | trim+declick+LN |
| `own-samantha-v2-sim-multi` / `own-jarvis-v2-sim-multi` | multiclip-long+LN |
| `own-*-v2-sim-rs-*` | same via **simsel** Rust prep |

## Configs tried (analysis levers only)

1. Reference prep: cleaned + true-peak ≤ −1 dBTP, mono 24 kHz  
2. Multi-clip pack from segments of the **same** original  
3. A/B `repetition_penalty` **1.0 vs 1.2** (cbx has no seed/temperature on bakeoff binary)  
4. Best-of-N selection (emotion-primary composite) — implemented; demo interrupted by box OOM (rc 137 under 10-worker load)

## Before / after ECAPA (vs **original** reference; galion-ecapa) — FINAL

Baselines (fp32, n=12, `refprep/score.sh`): Sam **0.707±0.032** WER 0.050 MOS 3.73; Jar **0.733±0.056** WER 0.019 MOS 4.14.

Full Rust scoreboard (`scores/final/`, n=12 except noted; `summarize.py`):

| Config | n | ECAPA ↑ | Δ | WER ↓ | MOS ↑ | emo dist ↓ | SER labels | Call |
|---|---:|---:|---:|---:|---:|---:|---|---|
| **baseline own-samantha** | 12 | 0.707±0.032 | — | 0.050 | 3.73 | 0.506 | ang 7, hap 2, neu 3 | — |
| sam-cleaned rp1.2 | 12 | 0.697±0.036 | −0.010 | 0.046 | 3.69 | 0.524 | ang 8, hap 1, neu 3 | no win (≤1 sd) |
| sam-cleaned rp1.0 | 12 | 0.397±0.270 | −0.310 | 0.387 | 3.64 | 0.619 | ang 7, hap 4, neu 1 | **broken** (long/corrupt renders) |
| sam-multi rp1.2 | 12 | 0.718±0.035 | **+0.011** | 0.049 | 3.75 | 0.489 | ang 9, neu 3 | small gain, within ~1 sd |
| **baseline own-jarvis** | 12 | 0.733±0.056 | — | 0.019 | 4.14 | 0.511 | neu 12 | — |
| jar-cleaned rp1.2 | 12 | 0.736±0.055 | +0.003 | 0.018 | 4.17 | 0.562 | hap 1, neu 11 | flat vs baseline |
| jar-multi rp1.2 | 12 | 0.737±0.049 | +0.004 | 0.026 | 4.17 | 0.541 | hap 1, neu 11 | flat vs baseline |
| jar-cleaned rp1.0 | 11 | 0.246±0.304 | −0.487 | 0.467 | 3.61 | 0.628 | hap 1, neu 10 | **broken** (t09 FAIL; many near-zero ECAPA) |

**Final call (2026-11-05 22:22 CET):** **No clear win.** Early provisional Jarvis gains (cleaned 0.756 / multi 0.754 at n≪12) **did not hold** at n=12 (+0.003 / +0.004). Samantha multi-clip is the only modest positive (+0.011 ECAPA, slightly better emotion distance) but not beyond ~1 baseline sd. `repetition_penalty=1.0` configs are unusable (broken synths). Priority grid + t08 refill finished; full `score.sh` into `scores/final/`.

### Final best voices (provisional → final)
- **Jarvis:** neither cleaned nor multi beats baseline beyond noise at n=12. Keep shipping baseline `own-jarvis` until a stronger lever (HF/richer takes / best-of-N) lands.  
- **Samantha:** `own-samantha-v2-sim-multi` (rp 1.2) is the least-bad picker candidate (+0.011 ECAPA); cleaned-only still slightly below baseline. Closing toward ceiling 0.83 still needs HF restore / richer takes — prep alone insufficient.

## Samples (same sentence old vs new)

`/workspace/samples/v2/samantha/old-t01-baseline.wav`  
`/workspace/samples/v2/samantha/new-t01-sam-cleaned-rp1.2.wav`  
`/workspace/samples/v2/samantha/new-t01-sam-multi-rp1.2.wav`  
`/workspace/samples/v2/jarvis/old-t01-baseline.wav`  
`/workspace/samples/v2/jarvis/new-t01-jar-multi-rp1.2.wav`

## Limited by original recordings

1. **Samantha HF bandwidth** (~0.4% energy >4 kHz) — largest identity/MOS ceiling driver; clones cannot invent presence.  
2. **Short single takes** (~16–17 s) — multi-clip is overlapping windows of one scripted monologue, not phonetic diversity.  
3. **Jarvis short half-vs-half ceiling ~0.725** — mean already near/above; further ECAPA gains need longer/richer own refs.  
4. **Turbo ONNX** — no exaggeration/CFG; bakeoff cbx has no seed/temp (pipeline fork may). Decoder noise → run variance; best-of-N is the right lever once scorers stay READY under load.  
5. **Box contention** — 10 workers caused synth OOM kills (rc 137) and 60–250 s/line; priority grid + refill finished; final scores in `scores/final/`.

## Artifacts

- Refs: `refs/*-cleaned-ln.wav`, `*-multiclip-long-ln.wav`, `*-rs-*.wav`, `prep_report*.json`  
- Scores: `scores/final/<cfg>/{ecapa,wer,mos,hygiene,emotion}.json`, root `*_ecapa.json` / `*_emotion.json` (auto-score), `summary_table.json` (final)  
- Synth: `wav/<cfg>/tXX.wav`, `logs/synth_*.log`  
- Binary: `simsel-rs/target/release/simsel`
