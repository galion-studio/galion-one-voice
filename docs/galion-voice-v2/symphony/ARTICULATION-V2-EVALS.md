# Articulation evals

Perpetual Open MIT License. (c) 2026 Galion Labs.

The harness lives in `tools/symphony/articulation/evals/`. It is not a gate. The rollout gate stays WER ≤ 1.8%, ECAPA > 0.7057, emotion ≥ 0.83.

## Emotion set

Neutral is the baseline. The prosody emotions are the 12 from Cowen et al. 2019:

adoration, amusement, anger, awe, confusion, contempt, desire, disappointment, distress, fear, interest, sadness.

Laughter, sigh and breath are vocal-burst detectors. They are not emotions, and the harness will not score them as emotions.

A public-dataset name is kept as itself unless it is the same category under another spelling (`angry` → anger, `fearful` → fear, `sad` → sadness, `disappointed` → disappointment, `amused` → amusement). Happy, disgust, surprise and calm are not renamed into amusement, contempt, awe or neutral.

`emotion-labels-v2.toml` is read when a sibling adds it. Until that file is on the branch, direction checks against it are not measured.

## What CI runs

`python3 tools/symphony/articulation/evals/loop_eval.py smoke`

Synthetic vowels only, through `galion-articulation analyze`:

- the config lists exactly those 12 emotions, neutral, and the three bursts
- a second decode of the same signal does not move a quantile
- a 0.97 gain does not move median F0 by more than 1 Hz
- two different synthetic pitches separate (speaker AUC above 0.9)

No dataset is downloaded. No dataset number is reported here.

## Decoder

The loop is one binary. `galion-articulation analyze` writes the profile and reads the next file. CI uses that acoustic path (praatfan). SPARC, DisVoice, Phonet and Rhubarb are not started. If a profile carries `tractprint_v2.speaker` and `tractprint_v2.emotion`, a later dataset run uses those vectors. This decoder does not write them.

Dataset licences, if a run is asked for later, are in `config.toml`. CREMA-D is ODbL 1.0 / DbCL 1.0. RAVDESS speech is CC BY-NC-SA 4.0. Both fetchers are disabled. Audio is never committed.
