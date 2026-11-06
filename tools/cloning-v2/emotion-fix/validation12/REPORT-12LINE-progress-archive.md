# Cloning V2: 12-line validation status

Updated 2026-11-05 21:29 CET (Europe/Zagreb).

## Decision

**No shared configuration is recommended or promoted.** Full matched 12-line evidence is not available yet. The four-line development win must not be represented as a 12-line result.

## Requested outcomes

| Outcome | Status |
|---|---|
| Samantha r2, temperature 0, 12 matched lines | Queued, not measured |
| Samantha r2, temperature 0.8, seeds 0/1/2, 12 lines each | Queued, not measured |
| Samantha baseline at every matching temperature/seed | Queued, not measured |
| Jarvis normalized + rotated reference, temperature 0 | Existing r6 profile queued, not measured on 12 lines |
| Jarvis milder ablation | Half-normalization profile `own-jarvis-v2-emo-m1` queued (-24.85 LUFS, no rotation), created by the separate mild-reference worker |
| Jarvis matched temperature-0 baseline | Queued |
| Rust punctuation pause insertion | Implemented in isolated `pipeline/cbx-v2-prosody/`; paired 12-line scoring queued, not yet measured |
| Shared identity/emotion gate | Cannot evaluate until all matched results complete |

### Actual existing evidence (not new validation)

The previous report records Samantha r2/up on only t01/t04/t06/t08: reference-emotion distance 0.4603 versus baseline 0.5334; persona-prosody distance 5.0434 versus 6.6420; ECAPA 0.7380 versus 0.7116. These are four-line development numbers, temperature 0.8, not new 12-line results. Earlier Jarvis normalization/rotation trials did not jointly improve emotion and retain identity. Original 12-line baseline ECAPA: Samantha 0.7072; Jarvis 0.7327. None is substituted for the newly requested setting-matched comparisons.

## Running validation

`validation12/run_validation.py` is running as a low-priority scheduler, log `validation12/queue.log`. It is currently in its healthy **WAIT external render/queue** state: the existing similarity priority and HF Jarvis queues are active. They have not been interrupted. No new render has started in this continuation.

Earlier queue plan (superseded by the priority plan below): Samantha baseline/r2 for temperature 0 seed 0 and temperature 0.8 seeds 0/1/2; Jarvis baseline/r6/normalized-unrotated at temperature 0 seed 0. Existing voice profiles only are consumed, with no profile edits. The existing r6 recipe uses the previously prepared quiet-boundary rotation (requested 6 s, actual 5.790 s) and -27 LUFS normalization. This tests the same normalization/quiet-boundary-rotation *family*, not an identical Samantha cut position or a newly optimized shared segment-selection rule.

All cohorts use the same decoder rules: fp32, two intra-op threads, nice 10, top-k 1000, top-p .95, repetition penalty 1.2, unique-token repetition, punctuation normalization, pace 1.0. Temperature and seed are explicit. Temperature 0 remains subject to unseeded ONNX decoder noise. Matched baseline voices are `own-samantha` and `own-jarvis`; candidate voices are existing `own-*-v2-emo-*` only.

Before each cohort the scheduler waits for existing render processes and named external render queues to end. It launches one batch at a time and scores serially. It does not control other workers; an external worker launched later could overlap, so this is not a system-wide lock honored by all producers. There is no new parallel rendering from this scheduler. Partial output directories cause a stop rather than an overwrite.

Each completed cohort will receive Rust galion-emotion (fp32 binary, --ref), ECAPA (--no-ceiling), articulation profile, WER (test12 transcripts), MOS (other persona non-matching reference), and hygiene JSONs alongside WAVs and audit.log. `COMPLETE.json` is written only after all commands succeed. Actual score tables still require aggregation after completion; the scheduler deliberately never promotes voices automatically.

## Remaining acceptance checks

1. Verify exactly t01–t12 in every cohort and all scorer outputs; report failures rather than substitute development subsets.
2. Aggregate matched emotion and ECAPA, including all three stochastic seeds without best-seed cherry-picking. Identity loss must not exceed .01 for **either** voice versus its matched baseline.
3. Review WER/MOS/hygiene and pause/rate measurements before recommending a configuration.
4. Implement optional Rust clause/sentence pauses only when practical, default off, and verify flag-off byte identity. No pause result exists here.
5. A shared recommended rule requires both voices to improve emotion under the identity guard. Until then retain originals.

No commits, pushes, deletes, laptop actions, Piper training, original voice changes, or original pipeline source changes were performed.

## Priority continuation (21:29 CET, Europe/Zagreb)

The idle scheduler was stopped and replaced safely; no render was interrupted. Current queue log is `validation12/queue-priority.log`.

Priority order: Samantha baseline/r2 t0; Jarvis baseline/half-normalized m1 t0; paired baseline pause-on for Samantha/Jarvis; Jarvis full r6 recipe t0; then Samantha baseline/r2 t0.8 seeds 0/1/2. Four priority rendered cohorts plus two postprocessed cohorts = 72 scored WAVs; pause pairs reuse identical baseline source samples rather than confound the comparison with decoder randomness. Six stochastic cohorts follow. Each completion appends real summary metrics below and writes all scorer JSONs in its cohort directory.

### Pause implementation

Isolated source copy: `pipeline/cbx-v2-prosody/`. `cbx speak --punctuation-pauses --pause-reference REF.wav` is default off and only enters the new code path when enabled. A tiny companion Rust executable at `pause-tool/target/release/cbx-pauses` shares the exact pause module and can process existing WAVs without re-rendering. It defaults off (byte-copy); enabling requires `--punctuation-pauses`.

Shared rule: mono RMS 10 ms frames, quiet threshold max(0.1 × peak frame RMS, 0.0001); reference internal quiet runs >=200 ms supply median phrase-pause duration. Sentence target is that median; clause target is 0.6 × median; target bounded 80–800 ms. Punctuation locations are word-fraction estimates, snapped within ±250 ms to an existing >=30 ms low-energy boundary. It only tops up existing quiet intervals, skips missing boundaries, and never intentionally cuts voiced audio. Final punctuation does not add trailing silence. This is a conservative heuristic, **not forced alignment**, and cannot slow uninterrupted speech or add expression to ONNX conditioning.

Three Rust tests passed: default-off sample identity, no voiced-region cuts, and quiet-boundary insertion. Actual WAV default-off copy passed `cmp` and identical SHA-256 (smoke test). Integrated flag-off has no new audio transform; repeated TTS runs cannot prove identity because the decoder contains unseeded noise. Full pipeline compile-check initially lacked pkg-config, so it was restarted using already installed explicit OpenSSL include/library paths; its final status will be recorded. Standalone Rust tool compiled successfully.

### Additional existing-baseline paired pause test sd_old_paired_pause_off

```json
{
  "tag": "sd_old_paired_pause_off",
  "n": 12,
  "emotion": 0.5062032217780749,
  "ecapa": 0.7072,
  "pause_ratio": 0.07368380529806018,
  "valence": 0.5369159802794456
}
```
All six Rust scorers completed; original 12 baseline WAVs were reused, not re-rendered.

### Additional existing-baseline paired pause test sd_old_paired_pause_on

```json
{
  "tag": "sd_old_paired_pause_on",
  "n": 12,
  "emotion": 0.514981726805369,
  "ecapa": 0.7072,
  "pause_ratio": 0.09463008306920528,
  "valence": 0.5393904397885004
}
```
All six Rust scorers completed; original 12 baseline WAVs were reused, not re-rendered.

### Additional existing-baseline paired pause test jd_old_paired_pause_off

```json
{
  "tag": "jd_old_paired_pause_off",
  "n": 12,
  "emotion": 0.5113990157842636,
  "ecapa": 0.7327,
  "pause_ratio": 0.05725702798614899,
  "valence": 0.4988602151473363
}
```
All six Rust scorers completed; original 12 baseline WAVs were reused, not re-rendered.

### Additional existing-baseline paired pause test jd_old_paired_pause_on

```json
{
  "tag": "jd_old_paired_pause_on",
  "n": 12,
  "emotion": 0.5127177884181341,
  "ecapa": 0.7302,
  "pause_ratio": 0.06783112852523725,
  "valence": 0.498756468296051
}
```
All six Rust scorers completed; original 12 baseline WAVs were reused, not re-rendered.
