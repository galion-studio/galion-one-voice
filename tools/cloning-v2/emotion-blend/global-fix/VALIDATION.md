# Coherent-prefix validation (authoritative completed-score table)

| Voice | n | WER (baseline) | ECAPA (baseline) | MOS | Emotion distance | WER/identity guard |
|---|---:|---:|---:|---:|---:|---|
| samantha | 12 | 0.01455 (0.018) | 0.7081 (0.7057) | 3.8757 | 0.437251 | PASS |
| jarvis | 12 | 0.02963 (0.019) | 0.7303 (0.7331) | 4.1622 | 0.538103 | FAIL |

All12-line T0/fp32; no profile promoted. Samantha MOS regression remains material even when its requested WER/identity guard passes. Jarvis MOS uses OTHER-persona NMR whereas merged Jarvis baseline used Jarvis NMR, so these MOS figures are not directly comparable. Full fields, baseline thresholds, source directories and guard booleans: results12.json.
