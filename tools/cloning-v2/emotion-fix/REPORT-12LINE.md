# Cloning V2: full 12-line emotion and shared-pause validation

Updated 2026-11-05T21:54:05.835869+01:00 — Europe/Zagreb.

## Recommendation

**No shared production config is recommended.** Shared punctuation pauses remain default off: they increase measured pauses, preserve identity, but worsen emotion distance for both voices. Original voices remain untouched. Both Jarvis milder references failed to improve emotion on 12 lines, despite the warmer crop’s promising four-line development score. No promotion.

## Samantha: reused completed 12-line validation

Source: `VALIDATE12.md`, `analysis/validate12.json`, `runs/sd_r2_t012`, `runs/sd_r2_up12`. No duplicate Samantha cohorts remain in this worker’s queue.

| Cohort | n | Emotion distance ↓ | ECAPA ↑ | Persona distance ↓ | Mean WER ↓ | Predicted MOS ↑ |
|---|---:|---:|---:|---:|---:|---:|
| sam_base12 | 12 | 0.506203 | 0.7072 | 6.1220 | 0.05025 | 3.7285 |
| sd_r2_t012 | 12 | 0.467498 | 0.7082 | 6.4378 | 0.01767 | 4.1045 |
| sd_r2_up12 | 12 | 0.482812 | 0.7032 | 4.7594 | 0.06833 | 4.1018 |

At t0 Samantha r2 improves emotion 0.506203 → 0.467498, ECAPA 0.7072 → 0.7082 (passes identity), but persona-prosody distance worsens 6.1220 → 6.4378. The t0.8 result improves emotion to 0.482812, ECAPA 0.7032 (−0.0040), but is one seed only and **not** a matched t0.8 baseline or three-seed stability study. This worker was explicitly steered to drop duplicate Samantha cohorts, so no three-seed job is claimed running here.

## Jarvis milder references and full paired pause results

| Cohort | n | Emotion distance ↓ | ECAPA ↑ | Pause ratio | Valence | WER ↓ | Predicted MOS ↑ | Clicks | Clipped samples |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| jar_mild_halfnorm_legacy_t0_s0 | 12 | 0.512074 | 0.7227 | 0.06149 | 0.49380 | 0.02470 | 4.0111 | 2 | 0 |
| jar_mild_warm10_legacy_t0_s0 | 12 | 0.511825 | 0.7304 | 0.05717 | 0.49699 | 0.02887 | 4.1391 | 8 | 2 |
| jd_old_paired_pause_off | 12 | 0.511399 | 0.7327 | 0.05726 | 0.49886 | 0.01852 | 4.1418 | 39 | 4 |
| jd_old_paired_pause_on | 12 | 0.512718 | 0.7302 | 0.06783 | 0.49876 | 0.01852 | 4.1279 | 39 | 4 |
| sd_old_paired_pause_off | 12 | 0.506203 | 0.7072 | 0.07368 | 0.53692 | 0.05026 | 3.7285 | 32 | 9 |
| sd_old_paired_pause_on | 12 | 0.514982 | 0.7072 | 0.09463 | 0.53939 | 0.04233 | 3.8073 | 32 | 9 |

Every table row has 12 t01–t12 WAVs scored with Rust galion-emotion fp32 (--ref), ECAPA (--no-ceiling), articulation profile, WER (test12 transcripts), MOS (other persona NMR), and hygiene. JSONs and audit logs live at `validation12/<cohort>/`. MOS is model-predicted, not human listening. WER is the scorer’s mean_raw (small rounding differences from Samantha’s reused per-file averages).

### Jarvis definitions and comparison guard

`own-jarvis-v2-emo-m1`: half-strength loudness normalization to −24.85 LUFS, no rotation. `own-jarvis-v2-emo-m2`: first 10 s of the original reference, favoring its warmer start, no loudness change. Both were created by the separate mild-reference worker and are consumed read-only. Original Jarvis baseline is the 12 existing temperature-0/legacy-repetition clips at `own-clone-analysis/wav/jarvis/`; its full re-score is `jd_old_paired_pause_off`. New mild renders use fp32, temperature 0, seed 0, repetition penalty 1.2, default legacy semantics, no punctuation normalization, pace 1.0, max 1000 tokens. Baseline’s historical max is 8192 and seed absent; greedy token selection makes sampling seed immaterial at t0, but ONNX decoder noise remains unseeded. Neither mild run truncated (maximum 255 and 252 speech tokens, respectively, versus the 1000-token cap); inference-thread counts differ (historical 4 vs current 2). No claim of exact run-level determinism.

Identity floor is matched baseline minus .01 (Jarvis 0.7227, Samantha 0.6972); improvement must hold for both voices. Voice-specific successful settings alone are not a validated shared rule.

## Shared Rust pause insertion

Implementation lives only in isolated `pipeline/cbx-v2-prosody/`; originals remain unchanged. Full `target/release/cbx` built successfully and `speak --help` confirms `--punctuation-pauses` (default off) and required `--pause-reference REF.wav`. Companion `pause-tool/target/release/cbx-pauses` uses the same `src/pauses.rs` module to provide a controlled paired WAV experiment without another stochastic synthesis pass.

Algorithm: mono RMS in 10 ms frames; quiet threshold max(0.1 × peak frame RMS, 0.0001). Reference internal quiet runs >=200 ms supply the median. Samantha: 9 runs, median 480 ms; Jarvis: 5 runs, median 380 ms. Sentence target = median; clause target = .6 × median (288/228 ms); bounds 80–800 ms. Word-fraction punctuation time estimates snap within ±250 ms to existing >=30 ms quiet regions. Only missing silence is added; voiced regions with no suitable nearby quiet boundary are skipped. No trailing pause. This is heuristic boundary selection, not forced alignment or a speech-rate/expression control.

On the original full 12-line cohorts, the tool inserted 11 boundaries for Samantha and 7 for Jarvis. Off output passed byte equality for **all 24 original WAVs**; three Rust unit tests passed (off identity, voiced-boundary avoidance, insertion). The integrated speak path bypasses the new module entirely when the flag is absent; repeated inference WAVs cannot be assumed identical because ONNX has unseeded noise. Existing audio writer/source is unchanged.

Paused Samantha: emotion 0.506203 → 0.514982 (worse), ECAPA unchanged 0.7072; pause ratio 0.07368 → 0.09463; WER improves and predicted MOS increases. Paused Jarvis: emotion 0.511399 → 0.512718 (worse), ECAPA 0.7327 → 0.7302 (within guard); pause ratio 0.05726 → 0.06783; WER unchanged, MOS slightly lower. Thus the pause feature is technically usable but **does not pass the shared emotion criterion** and remains off. Small additional pauses do not repair the full under-pausing/fast-delivery problem.

## Runtime / constraints

Only one new validation render is launched by this scheduler at a time, nice 10 with two intra-op threads; all scores are serial per job, nice 10, two threads where supported. Other workers’ HF, scoreboard, similarity, and development jobs are untouched. Slot gating permits at most one already-existing external render when >=4.8 GB is available, matching the sibling validation queue’s policy; it cannot impose a global lock on independent workers. Completed queue log: `validation12/queue-jarvis-mild.log`; no validation render remains running from this worker. The previous duplicate Samantha scheduler was stopped only while idle.

No original WAVs or voices changed, no commits/pushes/deletes, no laptop operations, no Piper training. Build used existing OpenSSL headers/libraries and the existing pipeline/lib C++ link path; no system packages installed. Historical progress report archived at `validation12/REPORT-12LINE-progress-archive.md`.

## Final 12-line decision

Completed 2026-11-05 21:54 CET (Europe/Zagreb). Both new Jarvis cohorts and all four paired pause cohorts have complete t01–t12 audio and all six Rust scorer outputs, with no articulation errors. Per-line/source coverage was checked, not inferred from directory names.

- Jarvis half-normalization: emotion **0.511399 → 0.512074** (worse); ECAPA **0.7327 → 0.7227** (at the rounded −.01 floor; no margin). Persona-prosody mean **6.511754**, versus existing baseline **6.1675**.
- Jarvis warmer first-10-second segment: emotion **0.511399 → 0.511825** (worse); ECAPA **0.7327 → 0.7304** (identity passes). Persona-prosody mean **6.242351**. Its four-line development gain did not generalize to the full 12 lines.
- Shared pause on/off: completed for both voices on identical source audio. Neither voice improves emotion, so keep the new optional flag **off**. Hygiene click/clipping counts were unchanged by pause insertion (existing waveform defects preserved, not repaired).
- Samantha t0 r2 improves full-cohort reference emotion and identity, but that is not evidence of a single shared production rule that also benefits Jarvis.

**Recommended shared production change: none.** Keep original voices and defaults; retain the experimental pause implementation default off. A future shared rule could score reference-segment candidates jointly for emotion and identity, but no such automatic rule was validated here.

Per parent steering, duplicate Samantha render/baseline cohorts and stochastic seeds were removed from this worker’s queue and the completed external results reused. Therefore **no temperature-0.8 three-seed job is running from this worker**; the reused t0.8 result is only seed 0 and lacks a matched t0.8 baseline. The earlier requested full rotated/−27-LUFS Jarvis 12-line cohort was not run in this focused continuation; only the two milder references above were newly validated. Do not present this as complete stochastic/shared-reference validation.
