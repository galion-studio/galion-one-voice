# Cloning V2 — shared Rust reference-prep (`refprep`)

**Time zone:** Europe/Zagreb (CET).  
**Finished:** 2026-11-05 ~22:23 CET.  
**Constraints:** originals / existing voices untouched; no commits/pushes/deletes; nothing to laptop; `nice -n 10`, 2 threads; max 1 render at a time; numbers from Rust scorers only.

## Tool path + default config

| Item | Path / value |
|---|---|
| Crate | `/workspace/swarm/voice-v2-sprint/cloning-v2/refprep/` |
| Binary | `target/release/refprep` (`prep`, `default-config`, `measure`) |
| Default config | `configs/default.toml` (one shared file for every voice) |
| Voices under test | `own-jarvis-v2-refprep`, `own-samantha-v2-refprep` |
| Render | `pipeline/cbx-v2/target/release/cbx`, temperature **0**, `emotion-fix/test12.tsv`, n=12 |
| Scorers | `refprep/score.sh` → `galion-{ecapa,hygiene,mos,wer,emotion}` |

**Default pipeline (`configs/default.toml`):**

1. **Gate** — stationary spectral denoise; skip if frame SNR ≥ 45 dB (data-driven).  
2. **Declick** — threshold 0.35.  
3. **Trim** — 0.10 s pad.  
4. **Select** — `ecapa_consistent`: phrase split on 0.20 s pauses, keep 1.5–6.0 s segments, drop if cos < median−max(mad_k·1.4826·MAD, 0.10); min keep 10 s; 15 ms xfade.  
5. **HF shelf** — only when input f99 < 3500 Hz (E-mild: 2800 Hz / +2.5 dB / Q 0.707 / exciter 0.08).  
6. **Loudness** — −23 LUFS, true-peak ≤ −1 dBTP.  
7. Sample rate **24 kHz**.

Per-voice behaviour is measurement-driven only (gate skip, HF trigger, which phrases survive ECAPA consistency).

### Prep meta (this run)

| Voice | Gate | HF shelf | Phrases kept | Out f99 Hz | Out %E>4k | Out dur s |
|---|---|---|---:|---:|---:|---:|
| Jarvis | skipped (SNR 64.3 ≥ 45) | skipped (f99 7254 ≥ 3500) | 3/3 | 7266 | 6.04 | 15.09 |
| Samantha | applied (SNR 35.7) | applied (f99 2660 < 3500) | 3/4 (dropped first phrase) | 3762 | 0.92 | 13.97 |

## Scoreboard (n=12, vs original `clip-store-own/*/reference.wav`)

| tag | n | ECAPA ↑ | WER ↓ | MOS ↑ | emotion dist ↓ | SER labels | f99 Hz | %E>4k | clicks/clipped |
|---|---|---|---|---|---|---|---|---|---|
| baseline-samantha | 12 | 0.707±0.032 | 0.050 | 3.73 | 0.506 | ang 7, hap 2, neu 3 | 2813 | 0.44 | 32/9 |
| **refprep-samantha** | 12 | **0.674±0.031** | 0.026 | 3.94 | 0.478 | ang 6, hap 2, neu 4 | 3438 | 0.79 | 1/0 |
| baseline-jarvis | 12 | 0.733±0.056 | 0.019 | 4.14 | 0.511 | neu 12 | 6023 | 2.97 | 39/4 |
| **refprep-jarvis** | 12 | **0.746±0.035** | 0.032 | 4.14 | 0.596 | ang 1, hap 1, neu 10 | 6242 | 2.86 | 0/0 |

Δ ECAPA: Samantha **−0.033**; Jarvis **+0.013**.  
Emotion distance: Samantha improved (−0.028); Jarvis **regressed** (+0.085; all-neu → mixed labels).

## Recommendation

**Do not keep / promote as the default shared cloning-V2 reference stage.**

Keep rule was: ECAPA must not drop > 0.01 **and** no emotion regression.

| Voice | ECAPA rule | Emotion rule | Verdict |
|---|---|---|---|
| Samantha | fail (−0.033 > 0.01) | OK (dist ↓, labels similar) | reject |
| Jarvis | OK (+0.013) | fail (dist 0.511→0.596; neu purity lost) | reject |
| Shared default | — | — | **reject** (both voices must pass) |

Collateral positives (cleaner audio, Samantha MOS/WER/f99 up, Jarvis clicks gone) are real but do not override the keep rule. Samantha’s HF shelf raised clone f99 (~2813→3438) at the cost of speaker identity. Next levers belong outside this shared gate/shelf/select defaults (or voice-specific overrides), not a blind default swap.

## Artifacts

- Refs: `refs/{jarvis,samantha}-refprep.wav`  
- Meta: `meta/{jarvis,samantha}-refprep.json`  
- Wavs: `wav/own-*-v2-refprep/t01..t12.wav` (renders finished earlier; not re-done)  
- Scores: `scores/{baseline,refprep}-{samantha,jarvis}/*.json`  
- Table: `scores/SUMMARY.md` (from `summarize.py`)
