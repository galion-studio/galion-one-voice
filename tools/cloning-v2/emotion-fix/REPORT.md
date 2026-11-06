# Emotion fidelity continuation report

Completed 2026-11-05 20:55 CET (Europe/Zagreb).

## Decision

- **Samantha: best tested development candidate is `own-samantha-v2-emo-r2`, configuration `sd_r2_up`.** Reference-emotion distance and persona-prosody distance both improve, with ECAPA increasing. This is a four-line development result, not a validated 12-line or listening-test win. Its winning configuration uses temperature 0.8; it is **not** evidence that the preferred temperature 0 path improves. Keep the original production voice unchanged.
- **Jarvis: none beats baseline while preserving identity and improving reference emotion.** Keep `own-jarvis`. The small reference-distance gain of loudness normalization fails the identity guard, and its persona-prosody distance worsens.

## Before / after (matched utterances only)

Mean per-clip values; lower distance is better, higher ECAPA is better. Same t01/t04/t06/t08 utterances for all rows. Persona distance is the existing Rust prosody target distance, not a trained emotion-class probability.

| Voice / result | n | Reference emotion distance ↓ | AVD distance ↓ | Persona-prosody distance ↓ | ECAPA ↑ | F0 Hz | Words/s | SER counts |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Samantha before | 4 | 0.5334 | 0.1105 | 6.6420 | 0.7116 | 205.91 | 3.608 | ang 2, hap 1, neu 1 |
| Samantha r2/up candidate | 4 | 0.4603 | 0.1008 | 5.0434 | 0.7380 | 186.49 | 3.607 | ang 1, hap 1, neu 2 |
| Jarvis before | 4 | 0.4786 | 0.2245 | 6.7451 | 0.7458 | 135.39 | 3.495 | neu 4 |
| Jarvis after: retain baseline | 4 | 0.4786 | 0.2245 | 6.7451 | 0.7458 | 135.39 | 3.495 | neu 4 |

Samantha matched-line deltas: reference distance -0.0731; persona distance -1.5986; ECAPA +0.0264. Pace is essentially unchanged and still faster than target; pitch is closer. No claim of a complete calm/unhurried fix.

### Original 12-line baselines (not comparable as an after-test cohort)

| Baseline | n | Reference emotion distance ↓ | AVD distance ↓ | Persona-prosody distance ↓ | ECAPA ↑ | F0 Hz | Words/s | SER counts |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Samantha original | 12 | 0.5062 | 0.0969 | 6.1220 | 0.7072 | 202.97 | 3.490 | ang 7, hap 2, neu 3 |
| Jarvis original | 12 | 0.5114 | 0.2305 | 6.1675 | 0.7327 | 142.36 | 3.311 | neu 12 |

The Rust SER labels Samantha angry on 7/12, versus the earlier Python finding of 8/12. These are different measurements; neither is rewritten to match the other. Jarvis remains neutral on 12/12. ECAPA reproduces the supplied approximate 0.707 / 0.733 baselines.

## All finished variants

All 12 rendered development configurations have all four WAVs covered by both Rust emotion and Rust ECAPA scores. Existing 12-line baseline score files also cover every file in `runs/sd_old` and `runs/jd_old`; their matched subsets were extracted without rerunning models.

| Configuration | n | Reference emotion distance ↓ | AVD distance ↓ | Persona-prosody distance ↓ | ECAPA ↑ | F0 Hz | Words/s | SER counts |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| jar_old | 4 | 0.4786 | 0.2245 | 6.7451 | 0.7458 | 135.39 | 3.495 | neu 4 |
| jd_base_grU | 4 | 0.5031 | 0.2210 | 7.1088 | 0.7312 | 138.60 | 3.468 | hap 1, neu 3 |
| jd_base_leg | 4 | 0.4813 | 0.2256 | 6.5646 | 0.7516 | 136.51 | 3.495 | hap 1, neu 3 |
| jd_base_up | 4 | 0.5379 | 0.2159 | 7.0270 | 0.7438 | 135.41 | 3.458 | hap 2, neu 2 |
| jd_ln_up | 4 | 0.4669 | 0.2467 | 7.7962 | 0.7081 | 145.25 | 3.429 | hap 1, neu 3 |
| jd_r10_up | 4 | 0.5095 | 0.2604 | 6.3466 | 0.7012 | 132.16 | 3.641 | hap 2, neu 2 |
| jd_r6_up | 4 | 0.4837 | 0.2429 | 5.8870 | 0.7063 | 133.99 | 3.593 | neu 4 |
| sam_old | 4 | 0.5334 | 0.1105 | 6.6420 | 0.7116 | 205.91 | 3.608 | ang 2, hap 1, neu 1 |
| sd_base_grU | 4 | 0.5765 | 0.1039 | 7.0434 | 0.6915 | 207.77 | 3.627 | ang 3, hap 1 |
| sd_base_leg | 4 | 0.6160 | 0.1099 | 6.7136 | 0.7197 | 209.02 | 3.608 | ang 3, neu 1 |
| sd_base_up | 4 | 0.5305 | 0.1147 | 5.0812 | 0.7380 | 197.05 | 3.459 | ang 2, neu 2 |
| sd_ln_up | 4 | 0.4803 | 0.1261 | 6.5605 | 0.7016 | 208.05 | 3.662 | hap 2, neu 2 |
| sd_r2_up | 4 | 0.4603 | 0.1008 | 5.0434 | 0.7380 | 186.49 | 3.607 | ang 1, hap 1, neu 2 |
| sd_r9_up | 4 | 0.5144 | 0.1279 | 5.7700 | 0.7166 | 205.22 | 3.528 | ang 3, neu 1 |

Identity guard: approximately 0.01 maximum loss from baseline (full-cohort floors 0.6972 Samantha and 0.7227 Jarvis); matched-subset comparisons are also shown to avoid cohort bias. `sd_base_grU` fails even the full-cohort floor. Jarvis `jd_ln_up`, `jd_r6_up`, and `jd_r10_up` fail both identity comparisons. `jd_base_leg` retains identity but reference distance 0.4813 does not improve on matched baseline 0.4786. `jd_base_grU` and `jd_base_up` also worsen reference distance. Samantha r2/up dominates the tested alternatives on the joint reference/persona/identity objective; the 0.0000 rounded ECAPA tie with base/up is not a meaningful difference.

## Fixes actually tried by the interrupted worker

- Reference loudness normalization to -27 LUFS (`ln`) for both voices, into new voice profiles only.
- Quiet-boundary reference rotation: Samantha requested 2.0 s → actual cut 2.250 s (`r2`) and 9.5 s → 9.750 s (`r9`); Jarvis 6.0 s → 5.790 s (`r6`) and 10.0 s → 9.880 s (`r10`). Existing logs record a wrap crossfade and full-recording speaker conditioning.
- `up`: temperature 0.8, top-k 1000, top-p 0.95, unique-token repetition penalty and punctuation normalization. `grU`: temperature 0 greedy with unique-token repetition semantics. `leg`: temperature 0 with legacy repetition semantics. Default repetition penalty is 1.2; pace remains 1.0. These settings are not an exaggeration/CFG control.
- Prefix probes and graph inspection already existed in `probe/` and `logs/`. Probe profile files are not rendered variant WAVs and cannot receive an audio ECAPA/emotion score.
- No post-render pitch shift, no Piper training, and no new training were performed in this continuation.

### Candidate reproduction (do not overwrite production)

```sh
nice -n 10 ./cbx-emo/target/release/cbx --intra-threads 2 batch \
  --dtype fp32 --voice own-samantha-v2-emo-r2 \
  --manifest dev4.tsv --out-dir runs/NEW_UNIQUE_AUDIT_DIRECTORY \
  --target targets/samantha.json --temperature 0.8 --top-k 1000 \
  --top-p 0.95 --rep-unique --punc-norm
```

This documents the measured candidate, not a recommendation to override the preferred temperature 0 default. No temperature 0 r2 render exists in this experiment.

## What is in Rust

The existing local `cbx-emo` binary exposes: manifest batch rendering with one model load; candidate generation/selection and saved metadata; WAV prosody analysis and reference-derived target JSON; existing-candidate reranking with optional external arousal/valence; score summaries; ONNX graph scan; cached-profile tensor inspection/diff; reference crop/loudness/rotation; sampling, unique-token repetition, punctuation normalization and token-space pace controls. The separate `galion-emotion` scorer supplies SER, AVD, acoustic proxies and reference distance; `galion-ecapa` supplies identity. These are existing implementations; this continuation did not modify or rebuild Rust sources. Best-of-N/pace capabilities are present but were not exercised by the finished four-line configurations (one candidate per line).

Persona distance is computed per WAV with the existing `prosody::distance` formula, then averaged: absolute F0 error in semitones + absolute log rate ratio / 0.10 + 0.5 × absolute energy-range error / 3 dB + 0.5 × absolute tilt error / 3 dB + 0.25 × absolute F0-SD error in semitones, plus the existing runaway-rate guard. `analysis/metrics.json` retains unrounded values. Source: `cbx-emo/src/prosody.rs:234-254`. No invented A/V target was added: the existing target JSON files have null arousal and valence. The persona proxies operationalize natural pitch/pace/expressiveness, not a direct numerical warmth metric.

Targets: Samantha F0 176.7829 Hz and rate 2.8149 words/s; Jarvis F0 130.4029 Hz and rate 2.6329 words/s. The Rust scorer and cbx use different pitch/energy estimators; the table consistently uses arithmetic means of per-WAV cbx prosody for F0/rate/persona distance. Existing `summary*.jsonl` F0 fields may differ because their aggregation/estimator differs; they are not mixed into this table.

## Continuation and constraints

- Inspected live processes before acting. The in-flight `queue1.sh` / `run_cfg.sh` job finished normally, including `jd_base_leg` scores. No render was duplicated or newly launched; other unrelated render queues were already using the machine.
- Stopped only the stale old-development-score waiter PID 3670812 after its queue had finished: its `pgrep -f queue1.sh` matched its own command line indefinitely. Reused the already complete original scores instead of running duplicate baseline scoring.
- Verified rendered-file IDs against both scorer JSONs for every directory in `runs/`. Consolidated real existing scores into `analysis/metrics.json` and this report. No originals, reference WAVs, existing own-samantha/own-jarvis profiles, or .bak files were changed. No voices were created in this continuation. No deletes, commits, pushes, laptop actions, or external writes.

## Limits / what remains unvalidated

Four development lines are insufficient for a production promotion or significance claim, and selection on those lines introduces optimism. Candidate after-scores on the other eight test12 utterances are absent, not estimated. A follow-up should validate the r2 reference at preferred temperature 0 and compare 12 matched utterances, only after render capacity frees up. WER, MOS, subjective listening and repeated-seed stability were not evaluated here. Turbo graph scan found no emotion/exaggeration/CFG input; decoder RandomNormalLike nodes remain unseeded, so even temperature 0 is not fully deterministic. Reference material is one short scripted recording per persona; Samantha lacks genuine high-frequency content. ECAPA guards identity, not naturalness or correctness. SER labels are noisy on synthetic speech and are not a human emotional judgment.
