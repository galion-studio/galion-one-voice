# Emotion V2 — Samantha, twelve emotions

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Design inventory for Galion Voice V2 inside Galion One.
The numbers live in [`../../emotion-v2/emotions.toml`](../../emotion-v2/emotions.toml).
`emotion-v2/taxonomy.py` rejects a file that drifts from the rules below.

This is a taxonomy and a set of controlled-reference targets. It does not
record audio, retune a checkpoint, change the live scorer, or edit the
session policy. It does not merge and it does not deploy.

## 1. What is being named

Samantha's Emotion V2 inventory has twelve categories in four sectors.
The sector names are a Galion grouping for this inventory. They are not
Ekman's families and they are not Russell's quadrants.

| Sector | Emotions |
|---|---|
| Grounded | Neutral, Calm |
| Bright | Happy, Amused, Excited, Tender |
| Heavy | Sad, Disappointed |
| Sharp | Angry, Fearful, Surprised, Disgusted |

Published taxonomies do not contain this list as a set.

- Ekman (1992) argues for discrete basic emotions and spends the paper on anger, fear, sadness, enjoyment, disgust, and surprise. He treats the evidence for a distinctive universal facial expression as robust for anger, fear, enjoyment, sadness, and disgust, and as less firm for surprise. Neutral, calm, amusement-as-separate, tenderness, and disappointment are outside that list.
- Ekman (1993) reports that amusement, relief, pride, sensory pleasure, and exhilaration share one facial expression, a form of smiling, and may be members of one enjoyment family. Amused is separate from Happy here because vocal recognition separates them, not because the face does.
- Russell (1980) places affect on a circle whose horizontal axis is pleasure–displeasure and whose vertical axis is arousal–sleep. He does not offer twelve named sectors.
- Cowen and Keltner (2017) report 27 varieties of emotion from self-report on 2,185 short videos, with smooth gradients rather than discrete cuts. Categories such as amusement capture those reports better than valence and arousal alone. Disappointment is not one of the separable dimensions: with contempt, it coloads on anger, and that cluster loads on sadness if the anger dimension is dropped. Sympathy coloads on empathic pain and sadness. The abstract names gradients from anxiety to fear to horror to disgust, and from calmness to aesthetic appreciation to awe.
- Cowen, Elfenbein, Laukka, and Keltner (2019) report at least 24 kinds of emotion in brief nonverbal vocal bursts, including amusement, anger, disappointment, disgust, fear, sadness, sympathy, surprise (negative), and surprise (positive). That study is about vocal bursts (laughs, gasps, sighs), which the authors distinguish from speech prosody. They summarise Juslin and Laukka (2003), a review of 60 studies, as finding that hearers can judge anger, fear, happiness, sadness, and tenderness in the prosody of speech, with accuracy approaching 70%. Serenity was among the categories that did not form its own vocal-burst dimension; contentment did.

The inventory uses those results as follows.

| Inventory name | Why it is here | What it is not |
|---|---|---|
| Neutral | The rest reference. SUPERB and emotion2vec+ both have a neutral class. Russell places moderate states toward the origin of the disk. | Not one of Ekman's basic emotions, and not one of Russell's 28 perimeter adjectives. |
| Calm | Russell's word *calm*, and Cowen and Keltner's calmness. | Not a SUPERB class and not an emotion2vec+ class. |
| Happy | Ekman's enjoyment, Russell's *happy*, Cowen and Keltner's joy, the SUPERB `hap` class. | The papers do not use the word Happy as a shared measurement. Excited is a different slot. |
| Amused | Cowen and Keltner (2017) treat amusement as its own category. Cowen et al. (2019) find it in vocal bursts. | Ekman (1993) does not give it a separate face. |
| Excited | Russell's *excited* at 48.6°. Excitement was one of the positive states Cowen and Keltner named when they built the 34-category video set. | Not a claim that excitement is one of their 27 dimensions, and not Cowen et al. (2019) positive surprise. |
| Tender | The speech-prosody set summarised by Cowen et al. (2019) includes tenderness. Their vocal-burst map has sympathy. The persona already uses tenderness as a behaviour. | Not a Cowen and Keltner (2017) dimension name. Sympathy, in that paper, coloads on sadness. |
| Sad | Ekman, Russell's *sad*, both classifiers, both Cowen papers. | |
| Disappointed | Its own dimension in the 2019 vocal-burst map. | Not its own dimension in Cowen and Keltner (2017). |
| Angry | Ekman, both classifiers, both Cowen papers. Russell's figure places *angry* in the high-arousal displeasure region. | The 1980 running text does not print a degree for the word. |
| Fearful | Ekman, emotion2vec+ `fearful`, Cowen et al. (2019) fear. Posner, Russell, and Peterson (2005) describe fear as negative valence with heightened arousal. | SUPERB has no fear class. |
| Surprised | Ekman's surprise, with the weaker facial evidence he notes. This slot is Cowen et al. (2019) negative surprise, the one they found adjacent to fear. | Not Russell's *astonished* (69.8°), which is still on the pleasant side of the arousal axis. emotion2vec+ has a single `surprised` class and does not split the two. |
| Disgusted | Ekman, Cowen and Keltner's disgust, emotion2vec+ `disgusted`. | SUPERB has no disgust class. Russell's 28 terms do not include it. |

## 2. Valence and arousal

Axes follow Russell (1980): positive valence is pleasure, positive arousal is activation. Each coordinate is in [-1, 1].

Where `coordinate_basis` is `russell-1980-angle`, the stored pair is

```text
valence = round(cos θ, 2)
arousal = round(sin θ, 2)
```

for an angle θ that Russell (1980) states in the running text of the circumplex paper. Radius 1 means the perimeter of that circle. Russell's statistic P measures how precisely subjects agreed on an angle. It is not used as a radius. He also writes that a disk is a better image than a ring, because moderate intensity falls inward. Intensity in this inventory is the separate scalar in section 6, not a second claim about his figure.

| Emotion | Basis | θ | Valence | Arousal |
|---|---|---:|---:|---:|
| Neutral | circumplex origin | — | 0.00 | 0.00 |
| Calm | Russell, *calm* | 316.2° | 0.72 | −0.69 |
| Happy | Russell, *happy* | 7.8° | 0.99 | 0.14 |
| Amused | design | — | 0.74 | 0.42 |
| Excited | Russell, *excited* | 48.6° | 0.66 | 0.75 |
| Tender | design | — | 0.58 | −0.18 |
| Sad | Russell, *sad* | 207.5° | −0.89 | −0.46 |
| Disappointed | design | — | −0.52 | −0.12 |
| Angry | design | — | −0.68 | 0.55 |
| Fearful | design | — | −0.42 | 0.78 |
| Surprised | design | — | −0.08 | 0.88 |
| Disgusted | design | — | −0.74 | 0.18 |

Russell's happy sits near pleasure, not near excitement. That is why Happy and Excited are different rows: the paper places *happy* at 7.8° and *excited* at 48.6°. Calm at 316.2° is pleasant and low-arousal, past sleepy (271.9°) and short of pleased (353.2°). Sad at 207.5° is unpleasant and below the pleasure axis, between miserable (188.7°) and the low-arousal side.

The design points stay in the region the same papers describe, and they are not measurements:

- Amused sits in the pleasure–excitement quadrant, more aroused than Russell's happy and less aroused than his excited. There is no Russell angle for the word.
- Tender is positive and low-arousal, above calm, toward the sadness coload Cowen and Keltner report for sympathy.
- Disappointed is negative, less extreme and less deactivated than Russell's sad, between that sadness and anger. That placement follows the 2017 coload (disappointment with anger, and with sadness when the anger dimension is removed) and the 2019 finding that vocal bursts still distinguish disappointment.
- Angry is in the high-arousal displeasure quadrant of Russell's figure, between the published anchors alarmed (96.5°) and miserable (188.7°). The number is not either of those angles.
- Fearful is in the same quadrant, higher arousal and less extreme valence than the angry design point, which is the qualitative circumplex description in Posner, Russell, and Peterson (2005). No degree is copied from Russell.
- Surprised is near the arousal axis and slightly unpleasant. Russell's astonished at 69.8° projects to about (+0.35, +0.94) and is the wrong side of the axis for this Sharp slot.
- Disgusted is negative at moderate arousal, below the angry design point. Ekman lists disgust; Russell's scaled list of 28 does not.

## 3. Classifier maps

Two heads are named because they are the ones a scorer can actually run. A label is `in_set` only when that head has the class. Every other row is a coarse projection. A projection is not an acceptance test. Calm, amusement, excitement, tenderness, disappointment, fear, surprise, and disgust are not four-class emotions, and several of them are not nine-class emotions either.

### `superb/wav2vec2-base-superb-er`

The model card's `id2label` is `0 neu`, `1 hap`, `2 ang`, `3 sad`. The SUPERB emotion task uses IEMOCAP and, in the card's words, drops the unbalanced emotion classes to leave four classes with a similar amount of data, cross-validated on the five session folds. The card does not define a mapping for classes it dropped. This file does not claim that the training merge of IEMOCAP `exc` into `hap` is part of that card.

| Emotion | Projection | In the four-class head |
|---|---|---|
| Neutral | neu | yes |
| Calm | neu | no |
| Happy | hap | yes |
| Amused | hap | no |
| Excited | hap | no |
| Tender | hap | no |
| Sad | sad | yes |
| Disappointed | sad | no |
| Angry | ang | yes |
| Fearful | ang | no |
| Surprised | neu | no |
| Disgusted | ang | no |

Fear, disgust, and a startled surprise have nowhere to go except `ang` or `neu`. That is the collapse a twelve-way inventory exists to stop scoring as if it were the emotion.

### emotion2vec+

The model card for `emotion2vec/emotion2vec_plus_large` lists nine classes, in this order: angry, disgusted, fearful, happy, neutral, other, sad, surprised, unknown. `other` is a residual class. It is not a name for calm, amusement, tenderness, or disappointment, so those rows are not mapped to it.

| Emotion | Projection | In the nine-class head |
|---|---|---|
| Neutral | neutral | yes |
| Calm | neutral | no |
| Happy | happy | yes |
| Amused | happy | no |
| Excited | happy | no |
| Tender | happy | no |
| Sad | sad | yes |
| Disappointed | sad | no |
| Angry | angry | yes |
| Fearful | fearful | yes |
| Surprised | surprised | yes |
| Disgusted | disgusted | yes |

The representation model is the emotion2vec line (Ma et al., arXiv:2312.15185). The nine labels above are the plus finetune described on the model card, not a label list this file is attributing to that preprint.

## 4. Why each emotion needs its own reference

The current check compares a render with one mixed distribution over the four SUPERB classes.

`line_delta` in `tools/cloning-v2/emotion-blend/autocheck/src/profile.rs` subtracts a single `base.ser` from every line. The example Samantha base in `tools/cloning-v2/emotion-blend/autocheck/PROFILE-FORMAT.md` is neu 0.350, hap 0.213, ang 0.119, sad 0.318. `tools/cloning-v2/emotion-blend/analysis/FINDINGS.md` records the same whole-recording P(sad) = 0.318, and records that 2.5 s windows of that recording average P(sad) = 0.012. The findings file says that gap is context-length dependence, not proof that cloning removed sadness. It also records Samantha's merged clone lines topping out ang on 7 of 12 lines, against original windows that top out neu or ang, and it warns that neu/hap/ang/sad are classifier outputs rather than a human inventory.

The known operational description of that one reference is N 0.35, H 0.20, A 0.14, S 0.32. Those four figures sum to 1.01, so they are a rounded description, not a distribution to renormalise. Against that mix, renders lose sad and lean angry, and `emotion_match` caps near 0.80. Those last figures are the brief for this inventory. They were not remeasured in this change.

A twelve-way voice cannot be scored by its distance from one blend of four classes. Sad and Disappointed would both be asked to match a vector that is already a third sad, and Fearful, Surprised, and Disgusted would be asked to match a vector whose angry mass is small but whose head has no fear, surprise, or disgust class. V2 needs one controlled reference per emotion: the three scenario texts below, spoken at full intensity, with the 432 Hz centre, scored against that emotion's own target. The mixed vector stays in the toml under `legacy_scorer.role = "not-a-target"`.

This change does not build those recordings and does not edit the scorer.

## 5. Prosody, with the centre locked at 432 Hz

The pitch centre is the just unison, 1/1, at 432 Hz. Every bound below is a 5-limit just ratio times 432. The centre is never moved to 440 Hz. A range may contain 440 Hz as an interior frequency. Neutral's upper bound is 450 Hz. That is not a retune.

| Ratio | Interval | Hz at 432 |
|---|---|---:|
| 1/1 | unison, the centre | 432 |
| 25/24 and 24/25 | chromatic semitone | 450 and 414.72 |
| 16/15 and 15/16 | diatonic semitone | 460.8 and 405 |
| 10/9 and 9/10 | minor whole tone | 480 and 388.8 |
| 9/8 and 8/9 | major whole tone | 486 and 384 |
| 6/5 and 5/6 | minor third | 518.4 and 360 |
| 5/4 and 4/5 | major third | 540 and 345.6 |

`pace` and `energy` are multiples of Neutral. They sit inside the rate clamp 0.8..1.2 and the energy clamp 0.6..1.2 already applied by `ProsodyControl`. This file does not change that clamp. `breathiness` is 0 for clear and 1 for breathy. It is a new design field. The sizes are design choices. The directions follow the inventory, not a new acoustic experiment: Heavy and Tender are slower, quieter, and breathier, which is the same direction as the consolation profile already written in `docs/galion-voice-v2/EMPATHY.md`. Angry is not a shout. Its energy stays under Neutral, its median sits at 15/16 (405 Hz), and its breathiness is the lowest in the set, so the reference is firm and narrow rather than loud. That matches the register's rule that she does not answer by getting sharp, loud, or cold. The reference still has to be tellable from fear and disgust, which is what the separate pitch bounds are for.

The emotion papers do not measure this voice at 432 Hz. The emotion-sweep resting F0 for the Samantha Piper render is about 180.6 Hz. That is a checkpoint fact. It is not this centre, and this file does not retune a deployed voice to close the gap.

| Emotion | Pace | Energy | Breath | Low | Median | High |
|---|---:|---:|---:|---|---|---|
| Neutral | 1.00 | 1.00 | 0.15 | 24/25 · 414.72 | 1/1 · 432 | 25/24 · 450 |
| Calm | 0.92 | 0.90 | 0.32 | 15/16 · 405 | 1/1 · 432 | 25/24 · 450 |
| Happy | 1.04 | 1.04 | 0.20 | 24/25 · 414.72 | 25/24 · 450 | 9/8 · 486 |
| Amused | 1.02 | 1.00 | 0.30 | 15/16 · 405 | 1/1 · 432 | 10/9 · 480 |
| Excited | 1.12 | 1.10 | 0.16 | 1/1 · 432 | 10/9 · 480 | 6/5 · 518.4 |
| Tender | 0.90 | 0.86 | 0.46 | 9/10 · 388.8 | 24/25 · 414.72 | 25/24 · 450 |
| Sad | 0.86 | 0.80 | 0.55 | 5/6 · 360 | 9/10 · 388.8 | 1/1 · 432 |
| Disappointed | 0.94 | 0.88 | 0.34 | 15/16 · 405 | 24/25 · 414.72 | 25/24 · 450 |
| Angry | 1.04 | 0.98 | 0.08 | 8/9 · 384 | 15/16 · 405 | 16/15 · 460.8 |
| Fearful | 1.10 | 0.92 | 0.38 | 24/25 · 414.72 | 16/15 · 460.8 | 6/5 · 518.4 |
| Surprised | 1.08 | 1.06 | 0.18 | 15/16 · 405 | 10/9 · 480 | 5/4 · 540 |
| Disgusted | 0.96 | 0.94 | 0.10 | 5/6 · 360 | 15/16 · 405 | 25/24 · 450 |

## 6. Intensity and allowed blends

Intensity scalars, applied to the distance from Neutral's full prosody:

| Level | Scalar |
|---|---:|
| low | 0.35 |
| mid | 0.65 |
| full | 1.00 |

For pace, energy, and breathiness, level `s` is `neutral + s · (target − neutral)`. For each pitch bound, level `s` moves log-frequency the same fraction of the way from 1/1 to that bound's full ratio. Neutral allows only `full`, because it is the origin of the scale. Every other emotion allows `low`, `mid`, and `full`.

A blend is two emotions, each at one of its own levels. There is no third member, and this file does not define an audio crossfade. The allow-list is the pairs a later renderer may request.

| Pair | Basis |
|---|---|
| Neutral–Calm | same Grounded sector |
| Happy–Amused, Amused–Excited, Happy–Tender | same Bright sector |
| Sad–Disappointed | same Heavy sector |
| Angry–Fearful, Angry–Disgusted | same Sharp sector |
| Calm–Tender | design adjacency: both positive and low-arousal, across Grounded and Bright |
| Tender–Sad | Cowen and Keltner (2017): sympathy coloads on sadness. The inventory name is Tender, not their word sympathy. |
| Disappointed–Angry | Cowen and Keltner (2017): disappointment coloads on anger. Cowen et al. (2019) still distinguish the two in vocal bursts, so the pure categories stay separate. |
| Fearful–Surprised | Cowen et al. (2019): negative-surprise vocal bursts lie adjacent to fear. |

Pairs that are not in that list are not blends. Happy with Disgusted, in particular, is not a blend.

## 7. Scenario texts

Three fixed lines per emotion. They are original. They are not film lines. The parser rejects a line that contains a comfort phrase the persona floor already forbids (`I hear you`, `I'm sorry for your loss`, and the rest of that list). Sad's first line is the sentence the persona floor already uses for grief, so the reference and the floor can share a carrier. The lines are carriers for a future full-intensity reference. They are not a reply table, and they do not authorise a canned comfort speech.

| Emotion | Texts |
|---|---|
| Neutral | "The notes are in the folder on the desk." / "Thursday at four still works." / "I can read that back if you want." |
| Calm | "We can leave it there for now." / "Nothing else has to move today." / "I'll wait while you find the page." |
| Happy | "They kept the Thursday slot for you." / "That came out right." / "The room went quiet, and then they clapped." |
| Amused | "You did not just name the printer." / "That one landed." / "I keep hearing the way you said spreadsheet." |
| Excited | "It is Thursday, and they said yes." / "The build finished. All of it." / "Tell me the part where it actually worked." |
| Tender | "You do not have to explain the whole thing." / "I'm glad you told me that piece." / "That part matters. We can stay with the fact." |
| Sad | "Your mother died this morning." / "The house is going to feel different tonight." / "I don't have a better sentence for that." |
| Disappointed | "I thought we were going to finish it today." / "That is not the version I was hoping you would hear." / "I wanted that to go the other way." |
| Angry | "That was not yours to decide." / "Say it again, plainly, without the excuse." / "I am not raising my voice. I am not letting it pass." |
| Fearful | "Something in the hallway just changed, and I don't know what." / "If that alarm is real, we stop and check." / "I need a second. That did not sound right." |
| Surprised | "Wait. That is not the number I had." / "They moved it to Thursday. I did not see that coming." / "You finished it already?" |
| Disgusted | "That smell does not belong in a kitchen." / "Leave the rest of it. I don't want it described." / "We are not putting that back on the table." |

## 8. What this file leaves alone

The session still answers a caller's anger. It does not wear it. That rule is `docs/galion-voice-v2/SAMANTHA-V2.md` and `docs/galion-voice-v2/EMPATHY.md`: `e` rises for Sad, Tired, Anxious, and Angry, and the policy target stays positive and lower-arousal. Angry in this inventory is the controlled reference a scorer needs so an angry lean can be seen. It is not an edit to that policy.

Nothing here changes `SOUL.md`, `config.yaml`, `LOCKS.toml`, Galion OS, a demo video, or a deployed voice.

## 9. Check

From the repo root:

```bash
python3 -m unittest discover -s emotion-v2/tests -v
```

The parser requires twelve emotions, the ids above in this order, unique ids, and both coordinates inside [-1, 1]. It also requires the 432 Hz unison, refuses a centre of 440 Hz, checks each Russell angle against the stored pair, and checks that every blend is returned with the same basis.

## Sources

- Cowen, A. S., Elfenbein, H. A., Laukka, P., & Keltner, D. (2019). Mapping 24 emotions conveyed by brief human vocalization. *American Psychologist, 74*(6), 698–712. https://doi.org/10.1037/amp0000399
- Cowen, A. S., & Keltner, D. (2017). Self-report captures 27 distinct categories of emotion bridged by continuous gradients. *Proceedings of the National Academy of Sciences, 114*(38), E7900–E7909. https://doi.org/10.1073/pnas.1702247114
- Ekman, P. (1992). An argument for basic emotions. *Cognition and Emotion, 6*(3–4), 169–200. https://doi.org/10.1080/02699939208411068
- Ekman, P. (1993). Facial expression and emotion. *American Psychologist, 48*(4), 384–392. https://doi.org/10.1037/0003-066X.48.4.384
- emotion2vec/emotion2vec_plus_large. Hugging Face model card. https://huggingface.co/emotion2vec/emotion2vec_plus_large
- Ma, Z., Zheng, Z., Ye, J., Li, J., Gao, Z., Zhang, S., & Chen, X. (2023). emotion2vec: Self-supervised pre-training for speech emotion representation. arXiv:2312.15185. Cited for the representation model. The nine plus labels are taken from the model card, not from this preprint.
- Posner, J., Russell, J. A., & Peterson, B. S. (2005). The circumplex model of affect: An integrative approach to affective neuroscience, cognitive development, and psychopathology. *Development and Psychopathology*. https://pmc.ncbi.nlm.nih.gov/articles/PMC2367156/
- Russell, J. A. (1980). A circumplex model of affect. *Journal of Personality and Social Psychology, 39*(6), 1161–1178.
- superb/wav2vec2-base-superb-er. Hugging Face model card. https://huggingface.co/superb/wav2vec2-base-superb-er
- Juslin, P. N., & Laukka, P. (2003), as summarised in Cowen et al. (2019): five emotions judged in speech prosody (anger, fear, happiness, sadness, tenderness). This file does not re-cite page-level claims from the 2003 review.

In-repo, not remeasured here: `PROFILE-FORMAT.md`, `FINDINGS.md`, and `profile.rs` `line_delta`, as cited in section 4.
