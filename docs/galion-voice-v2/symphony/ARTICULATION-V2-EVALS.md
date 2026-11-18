# Articulation evals

Perpetual Open MIT License. (c) 2026 Galion Labs.

The harness lives in `tools/symphony/articulation/evals/`. It is not a gate. The rollout gate stays WER ≤ 1.8%, ECAPA > 0.7057, emotion ≥ 0.83.

## Taxonomy

The list of names is `configs/emotions-v2.toml` (PR #189). This harness carries the same lists.

One inventory of 24 Cowen, Elfenbein, Laukka and Keltner (2019) categories. The prosody track and the burst track both use it. A word can be said with any of the 24, including embarrassment, disgust and elation.

The 12 from Cowen, Laukka, Elfenbein, Liu and Keltner (2019, Nature Human Behaviour) are the reliable prosody tier: adoration, amusement, anger, awe, confusion, contempt, desire, disappointment, distress, fear, interest, sadness. The other 12 are valid in prosody with lower recognition evidence. The tier is confidence, not membership.

Neutral is the speaker baseline, not a category. Breath is physiology, not a category. Laughter is amusement on the burst track. A sigh may be relief, sadness, disappointment or contentment, and is not collapsed to one of them.

`emotion-labels-v2.toml` is read when it is on the branch. Direction checks against it are not measured until then.

## What CI runs

`python3 tools/symphony/articulation/evals/loop_eval.py smoke`

Synthetic vowels only, through `galion-articulation analyze`:

- the config matches the 24, the two tiers of 12, and the neutral baseline
- a word name in the 24 stays that category on the prosody track
- laughter is burst amusement, and breath is not a category
- a second decode of the same signal does not move a quantile
- a 0.97 gain does not move median F0 by more than 1 Hz
- two different synthetic pitches separate (speaker AUC above 0.9)

No dataset is downloaded. No dataset number is reported here.

## Decoder

The loop is one binary. `galion-articulation analyze` writes the profile and reads the next file. CI uses that acoustic path (praatfan). SPARC, DisVoice, Phonet and Rhubarb are not started. If a profile carries `tractprint_v2.speaker` and `tractprint_v2.emotion`, a later dataset run uses those vectors. This decoder does not write them.

Dataset licences, if a run is asked for later, are in `config.toml`. CREMA-D is ODbL 1.0 / DbCL 1.0. RAVDESS speech is CC BY-NC-SA 4.0. Both fetchers are disabled. Audio is never committed.
