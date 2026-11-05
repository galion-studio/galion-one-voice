# Emotion sweep (persona × user emotion × mood history)

Reply line for every persona case: "I have moved the meeting to Thursday at four, and the notes are in your inbox."

Metrics are means over 8 renders (Piper sampling is stochastic); each WAV is the first render. ΔF0 (semitones), rate and ΔRMS in the persona table are relative to that persona's own resting reference (resting mood, no user cue, same Piper noise regime). The V1 path (no plan: deterministic noise 0) is listed separately.

## V1 baselines (no persona plan)

| persona | pack | speaker | F0 Hz | speech s | RMS dBFS | file |
|---|---|---|---|---|---|---|
| samantha | en_US-libritts_r-medium | 126 | 172.3 | 2.846 | -14.6 | `samantha-v1-baseline.wav` |
| jarvis | en_GB-vctk-medium | 95 | 109.2 | 3.698 | -15.1 | `jarvis-v1-baseline.wav` |

## Persona resting references (the conditioning with no user cue)

| persona | length_scale | noise_scale | noise_w | silence s | pitch st | energy | pre-pause ms | F0 Hz | speech s | RMS dBFS | file |
|---|---|---|---|---|---|---|---|---|---|---|---|
| samantha | 1.000 | 0.679 | 0.800 | 0.20 | +0.40 | 1.00 | 0 | 180.6 | 3.411 | -15.6 | `samantha-reference-rest.wav` |
| jarvis | 1.053 | 0.498 | 0.593 | 0.27 | -0.30 | 0.91 | 80 | 111.0 | 4.051 | -16.3 | `jarvis-reference-rest.wav` |

## User side (synthetic user audio → consent-gated loudness detector + text cue → fused)

| user | voice arousal (detector) | fused valence | fused arousal | conf |
|---|---|---|---|---|
| neutral | 0.40 | +0.00 | 0.40 | 0.30 |
| happy | 0.49 | +0.80 | 0.62 | 0.51 |
| sad | 0.15 | -0.75 | 0.19 | 0.59 |
| angry | 0.56 | -0.70 | 0.69 | 0.51 |
| anxious | 0.53 | -0.50 | 0.59 | 0.51 |
| calm | 0.10 | +0.50 | 0.15 | 0.68 |

## Persona cases

| persona | user | mood history | mood | e | length_scale | pitch st (plan) | energy | pause ms | F0 Hz | ΔF0 vs rest st | rate vs rest | ΔRMS dB | file |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| samantha | neutral | rest | warm (v 0.25, a 0.36) | 0.00 | 0.997 | +0.42 | 1.00 | 0 | 181.2 | +0.05 | +3.5% | +0.4 | `samantha-neutral-rest.wav` |
| samantha | happy | rest | happy (v 0.41, a 0.41) | 0.00 | 0.976 | +0.78 | 1.03 | 0 | 185.1 | +0.42 | +8.2% | +0.7 | `samantha-happy-rest.wav` |
| samantha | sad | rest | warm (v 0.13, a 0.30) | 0.57 | 1.078 | -0.19 | 0.86 | 142 | 172.1 | -0.83 | +0.3% | -0.8 | `samantha-sad-rest.wav` |
| samantha | angry | rest | warm (v 0.15, a 0.36) | 0.49 | 1.045 | +0.02 | 0.91 | 122 | 174.0 | -0.65 | +3.7% | -0.2 | `samantha-angry-rest.wav` |
| samantha | anxious | rest | warm (v 0.18, a 0.37) | 0.41 | 1.033 | +0.13 | 0.93 | 102 | 175.1 | -0.54 | +1.3% | -0.3 | `samantha-anxious-rest.wav` |
| samantha | calm | rest | happy (v 0.33, a 0.30) | 0.00 | 1.021 | +0.42 | 0.98 | 0 | 180.8 | +0.02 | +3.3% | -0.3 | `samantha-calm-rest.wav` |
| samantha | neutral | after-joy | happy (v 0.46, a 0.44) | 0.00 | 0.964 | +0.93 | 1.04 | 0 | 185.8 | +0.49 | +10.6% | +1.2 | `samantha-neutral-after-joy.wav` |
| samantha | happy | after-joy | happy (v 0.60, a 0.48) | 0.00 | 0.947 | +1.25 | 1.06 | 0 | 191.2 | +0.99 | +8.3% | +0.7 | `samantha-happy-after-joy.wav` |
| samantha | sad | after-joy | happy (v 0.32, a 0.37) | 0.57 | 1.050 | +0.26 | 0.89 | 142 | 179.5 | -0.10 | +3.5% | -1.1 | `samantha-sad-after-joy.wav` |
| samantha | angry | after-joy | happy (v 0.34, a 0.43) | 0.49 | 1.016 | +0.48 | 0.94 | 122 | 180.8 | +0.02 | +3.0% | -0.3 | `samantha-angry-after-joy.wav` |
| samantha | anxious | after-joy | happy (v 0.37, a 0.44) | 0.41 | 1.004 | +0.59 | 0.96 | 102 | 181.0 | +0.04 | +5.0% | -0.2 | `samantha-anxious-after-joy.wav` |
| samantha | calm | after-joy | happy (v 0.51, a 0.37) | 0.00 | 0.994 | +0.85 | 1.01 | 0 | 185.2 | +0.43 | +8.0% | +0.4 | `samantha-calm-after-joy.wav` |
| samantha | neutral | after-distress | steady (v 0.10, a 0.29) | 0.00 | 1.025 | +0.02 | 0.97 | 0 | 173.0 | -0.75 | +4.2% | -0.1 | `samantha-neutral-after-distress.wav` |
| samantha | happy | after-distress | warm (v 0.27, a 0.35) | 0.00 | 1.002 | +0.42 | 1.00 | 0 | 178.2 | -0.23 | +5.9% | +0.2 | `samantha-happy-after-distress.wav` |
| samantha | sad | after-distress | steady (v 0.00, a 0.23) | 0.57 | 1.103 | -0.53 | 0.84 | 142 | 167.9 | -1.27 | -5.4% | -1.5 | `samantha-sad-after-distress.wav` |
| samantha | angry | after-distress | steady (v 0.01, a 0.30) | 0.49 | 1.070 | -0.34 | 0.88 | 122 | 168.4 | -1.22 | +3.2% | -0.4 | `samantha-angry-after-distress.wav` |
| samantha | anxious | after-distress | steady (v 0.04, a 0.31) | 0.41 | 1.058 | -0.23 | 0.90 | 102 | 171.8 | -0.86 | +0.8% | -0.9 | `samantha-anxious-after-distress.wav` |
| samantha | calm | after-distress | warm (v 0.21, a 0.24) | 0.00 | 1.044 | +0.09 | 0.95 | 0 | 177.0 | -0.35 | -1.4% | +0.3 | `samantha-calm-after-distress.wav` |
| jarvis | neutral | rest | steady (v 0.10, a 0.28) | 0.00 | 1.052 | -0.29 | 0.91 | 80 | 109.7 | -0.20 | -0.2% | -0.2 | `jarvis-neutral-rest.wav` |
| jarvis | happy | rest | warm (v 0.15, a 0.29) | 0.00 | 1.051 | -0.25 | 0.91 | 80 | 110.8 | -0.02 | -3.2% | -0.3 | `jarvis-happy-rest.wav` |
| jarvis | sad | rest | steady (v 0.07, a 0.27) | 0.57 | 1.077 | -0.38 | 0.88 | 103 | 109.9 | -0.17 | -1.6% | +0.0 | `jarvis-sad-rest.wav` |
| jarvis | angry | rest | steady (v 0.08, a 0.28) | 0.49 | 1.072 | -0.36 | 0.89 | 100 | 110.4 | -0.09 | -1.6% | -0.4 | `jarvis-angry-rest.wav` |
| jarvis | anxious | rest | steady (v 0.08, a 0.28) | 0.41 | 1.069 | -0.35 | 0.89 | 96 | 109.1 | -0.29 | -1.5% | -0.5 | `jarvis-anxious-rest.wav` |
| jarvis | calm | rest | warm (v 0.14, a 0.27) | 0.00 | 1.053 | -0.28 | 0.91 | 80 | 109.4 | -0.25 | +1.5% | -0.2 | `jarvis-calm-rest.wav` |
| jarvis | neutral | after-joy | warm (v 0.19, a 0.30) | 0.00 | 1.049 | -0.23 | 0.91 | 80 | 110.8 | -0.03 | -0.3% | +0.3 | `jarvis-neutral-after-joy.wav` |
| jarvis | happy | after-joy | warm (v 0.24, a 0.31) | 0.00 | 1.048 | -0.19 | 0.91 | 80 | 110.6 | -0.05 | +0.5% | -0.3 | `jarvis-happy-after-joy.wav` |
| jarvis | sad | after-joy | warm (v 0.15, a 0.28) | 0.57 | 1.075 | -0.32 | 0.89 | 103 | 109.2 | -0.27 | -2.7% | -1.1 | `jarvis-sad-after-joy.wav` |
| jarvis | angry | after-joy | warm (v 0.16, a 0.30) | 0.49 | 1.070 | -0.30 | 0.89 | 100 | 110.1 | -0.13 | -1.7% | -0.4 | `jarvis-angry-after-joy.wav` |
| jarvis | anxious | after-joy | warm (v 0.16, a 0.30) | 0.41 | 1.066 | -0.28 | 0.89 | 96 | 109.2 | -0.28 | -1.6% | -0.6 | `jarvis-anxious-after-joy.wav` |
| jarvis | calm | after-joy | warm (v 0.22, a 0.29) | 0.00 | 1.051 | -0.22 | 0.91 | 80 | 110.3 | -0.11 | -2.7% | +0.2 | `jarvis-calm-after-joy.wav` |
| jarvis | neutral | after-distress | steady (v 0.06, a 0.26) | 0.00 | 1.055 | -0.34 | 0.90 | 80 | 110.1 | -0.14 | -0.6% | +0.1 | `jarvis-neutral-after-distress.wav` |
| jarvis | happy | after-distress | steady (v 0.11, a 0.27) | 0.00 | 1.054 | -0.29 | 0.91 | 80 | 110.2 | -0.12 | -2.2% | -0.5 | `jarvis-happy-after-distress.wav` |
| jarvis | sad | after-distress | steady (v 0.03, a 0.25) | 0.57 | 1.081 | -0.42 | 0.88 | 103 | 109.4 | -0.25 | -4.1% | -0.6 | `jarvis-sad-after-distress.wav` |
| jarvis | angry | after-distress | steady (v 0.03, a 0.26) | 0.49 | 1.076 | -0.40 | 0.88 | 100 | 109.7 | -0.19 | -2.3% | -0.3 | `jarvis-angry-after-distress.wav` |
| jarvis | anxious | after-distress | steady (v 0.04, a 0.26) | 0.41 | 1.072 | -0.39 | 0.89 | 96 | 108.0 | -0.47 | -0.4% | -0.3 | `jarvis-anxious-after-distress.wav` |
| jarvis | calm | after-distress | steady (v 0.10, a 0.26) | 0.00 | 1.056 | -0.32 | 0.90 | 80 | 109.8 | -0.18 | -0.8% | -0.2 | `jarvis-calm-after-distress.wav` |
