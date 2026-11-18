# Articulation V2 — the core

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-17.

This note is the foundation. Everything else in the articulation layer is built on it. It is a specification of how a voice carries a feeling, not a model of a named person and not a gate.

The category inventory is `configs/emotions-v2.toml`. That file is the only list of emotion names. Acoustic directions, still unknown as z against the speaker's own neutral, stay in `configs/symphony/emotion-labels-v2.toml`. A number appears only when a source below states it. Where a source does not, the value is **unknown, to be learned from data**.

V1 (`ARTICULATION-LAYER.md`) already measures the throat, the mouth and a nasal-class posterior on the 80 ms Symphony tick. TractPrint did not separate speakers: a male control scored 0.795 overall, inside the clone band 0.785–0.850. Pitch and formants alone scored that control 0.578. Extra channels diluted the separation. V2 therefore keeps the smallest set that the literature actually uses to separate feelings, and the smallest set that separates speakers (F0 and vocal-tract length). It does not add a new score.

## 1. The core

### 1.1 Emotion is a state, not a label

On each 80 ms tick the voice carries a point, not a class. The 80 ms step is the Symphony clock. No paper cited here establishes 80 ms as the time constant of feeling.

The point has three continuous axes:

- **Arousal.** This is the axis the acoustic record actually separates. Fast or slow, loud or quiet, high or low F0, more or less high-frequency energy (Juslin & Laukka, 2003, Table 7, pp. 792–795; Banse & Scherer, 1996, Table 6, p. 625).
- **Valence.** Pleasure versus displeasure. Anger and happiness share the high-arousal pattern (fast, loud, high F0, more high-frequency energy). Valence is not one feature.
- **Dominance.** The hot/cold and panic/anxiety splits below. No source read here publishes a dominance coefficient, in Hz or in z.

Russell (1980) is the precedent for a pleasure–arousal circumplex. No coordinate is copied from his figures. Dominance is kept because the same emotion word covers opposite acoustic poles. It is not a measured third loading.

### 1.1a One set of 24, two ways of sounding

The inventory is 24 categories (Cowen, Elfenbein, Laukka & Keltner, 2019, *American Psychologist, 74*(6), 698–712, doi:10.1037/amp0000399, PMC6586540, Figure 3B): adoration, amusement, anger, awe, confusion, contempt, contentment, desire, disappointment, disgust, distress, ecstasy, elation, embarrassment, fear, interest, pain, realization, relief, sadness, surprise_negative, surprise_positive, sympathy, triumph.

Both tracks use that whole set. **Prosody** is how the words are said, including a word said with embarrassment. **Burst** is the same category with no words. The track is the channel, not a shorter list of feelings.

| Track | What it is | Categories |
|---|---|---|
| Prosody | How words are said | all 24 |
| Burst | The same category, no words | all 24 |
| Breath | Lungs and airflow (§2). Physiology. | not an emotion |

**Neutral** is the speaker's own baseline (§1.2). z = 0 by definition. It is not a twenty-fifth category. Cowen, Elfenbein, Laukka and Keltner excluded "Neutral" from the burst dimensions because a forced-choice neutral is redundant. Banse and Scherer (1996) recorded no neutral portrayals.

Confidence in *prosody* is a tier, not a gate. The 12 that the speech-prosody study preserved across US and Indian listeners are **cross-culturally reliable in prosody** (Cowen, Laukka, Elfenbein, Liu & Keltner, 2019, *Nature Human Behaviour, 3*(4), 369–382, doi:10.1038/s41562-019-0533-6, PMC6687085): adoration, amusement, anger, awe, confusion, contempt, desire, disappointment, distress, fear, interest, sadness. Each of those rotated dimensions loaded maximally on one of these words. Awe and surprise were used similarly (their dimension D). Gradients join particular pairs, including amusement–adoration, confusion–interest, fear–sadness, anger–contempt, and sadness–distress (Fig. 7).

The other 12 are **valid in prosody, lower recognition evidence**: contentment, disgust, ecstasy, elation, embarrassment, pain, realization, relief, surprise_negative, surprise_positive, sympathy, triumph. They were not among the 12 prosody dimensions preserved across those two cultures. A word may still be said in any of them. A tick may sit on a gradient, in a blend, or between regions. Onset shape and mixture weights are unknown.

Candidates that did **not** form their own burst dimension, and are not in the 24: guilt, pride, romantic love, serenity, shame.

The old 14 names are retired. The crosswalk is the Galion map; it is not a claim that the cited acoustic study used the new word.

| Old name | Goes to | What is not copied across |
|---|---|---|
| neutral | baseline, z = 0 | — |
| calm | the same baseline, low arousal on the core axis | serenity was not one of the 24 |
| tender | prosody adoration | Juslin's love–tenderness profile stays labelled as tenderness |
| seductive | prosody desire | Hughes is courtship-directed speech, not a desire portrayal |
| angry | prosody anger | hot and cold anger stay two poles |
| fearful | prosody fear | panic and anxiety stay two poles |
| sad | prosody sadness | Banse despair is not distress |
| disappointed | prosody disappointment | no acoustic row; still unknown |
| happy | prosody amusement **and** interest | Juslin/Banse happiness numbers are not assigned to either word alone |
| amused | prosody amusement | Boiten's breath change stays here. It is not the laugh numbers |
| excited | prosody interest **and** amusement | Banse elation is not copied onto either word. Elation is its own category, valid in prosody at the lower tier |
| surprised | prosody awe **and** confusion | no acoustic row. Dimension D ties surprise to awe; confusion's published gradient is with interest |
| disgusted | prosody contempt | Banse's disgust profile is not contempt. Disgust is its own category and may be spoken; its prosody tier is the lower one |
| laughter | burst channel of amusement | A word said amused is prosody amusement. Bachorowski measured laughs |
| sigh | burst relief, sadness, disappointment, or contentment | no sigh-to-category table was read; the detector may be any of the four |

A region can still contain opposite poles inside one new name. Banse and Scherer (1996, p. 625) partialed out speaker and sex and scored against the mean of the emotions they portrayed, with a strong change as 1 SD and a weak change as 0.5 SD. That is not z against a speaker's own neutral. Juslin and Laukka (2003, p. 789, note 9) warn that studies do not share a baseline.

| Prosody region | Cited split that still applies | Do not collapse to |
|---|---|---|
| anger | Juslin: fast, loud, high F0, high F0 variability, high high-frequency energy, few pauses. Banse hot anger F0 z = 1.13, energy z = 1.19. Cold anger F0 z = 0.16, energy z = 0.52. | one anger |
| fear | Juslin: mostly fast and high F0; intensity and high-frequency energy bimodal; F0-variability studies fall more often in the low bin (Table 7). Banse panic F0 z = 1.23, energy z = 0.84. Anxiety F0 z = −0.58, energy z = −0.37. | one fear |
| sadness | Juslin: slow, quiet, low F0, low F0 variability, low high-frequency energy, many pauses. Banse sadness F0 z = −0.32, energy z = −1.16, articulation-duration z = 1.04 (higher means slower). | Banse despair (F0 z = 0.99, energy z = 1.00), which is not Cowen's distress |
| adoration | Juslin love–tenderness: slow, quiet, low F0, low F0 variability, low high-frequency energy. Pauses little studied. | calm, or desire |

### 1.2 Stable baseline, fluid state

Labels are universal because they are deviations from **this speaker's own neutral**, not from a population voice.

For each working feature, on speech recorded in matched conditions:

```text
z = (x - mean_neutral) / sd_neutral
```

`mean_neutral` and `sd_neutral` are that speaker's. Sex, age, vocal-tract length and personality sit in the baseline. They are not feelings. Banse's z removed sex on purpose, which is why those z values must not be copied in as male or female targets.

No source read here reports this z for any of the 24 categories. Every `z_vs_own_neutral` in the label file is **unknown, to be learned from data**. Directions in the file are the modal pattern (Juslin) or the cross-emotion residual (Banse), and they are labelled as such.

Absolute population numbers are anchors for the baseline, never substitutes for z:

- Vocal-tract length, curved-line MRI, quiet breathing, not speech (Fitch & Giedd, 1999, Table I, p. 1514). Ages 19–25: women N = 3, mean 138.8 mm (SD 3.4, range 135.2–141.9); men N = 7, mean 155.4 mm (SD 4.4, range 146.9–160.0). The adult cells are small. Length below puberty is in the same table and is not sex-dimorphic in their account.
- Laughter F0 and formants, by sex, in §3. Those are laugh acoustics, not speech norms.
- Speech F0 norms from Hillenbrand et al. (1995) and Peterson & Barney (1952) were not read from the tables in this pass. They are unknown here, not guessed.

A tick is scored as feeling only after the external factors in §1.3 are matched or removed. A louder voice in noise is not anger.

### 1.3 What moves the voice

Only effects read from a source are listed. "How much" is the published figure. Anything else is unknown.

**Internal**

| Factor | Features it moves | How much | Source |
|---|---|---|---|
| Arousal / the portrayed feeling | F0 mean, F0 range, loudness, rate, high-frequency energy, pause proportion, and (preliminary) F1 | Directions in §1.1 and the label file. No Hz-per-arousal-unit. | Juslin & Laukka 2003, Table 7, pp. 792–795, and p. 796 for pauses and F1. Banse & Scherer 1996, Table 6, p. 625. |
| Breath, amusement / laughter | Inspiratory time, tidal volume | Both decrease. Magnitudes not in the abstract read here. | Boiten 1998, abstract, *Biological Psychology*, 49, 29–51. |
| Breath, disgust | Inspiratory pause | Prolonged (breath-holding). Duration unknown. | Boiten 1998, same abstract. |
| Breath, anticipatory anxiety | Respiratory rate | Increases, and the increase is not accounted for by oxygen consumption. The rate increase correlates with trait anxiety (their Fig. 1). Hertz or breaths/min not stated in the review text read here. | Homma & Masaoka 2008, *Experimental Physiology*, 93(9), 1011–1021, Fig. 1, citing Masaoka & Homma 2001. |
| Breath, arousal in general | Respiratory rate | "More rapid breathing during an arousal state." No breaths/min in the sentence. | Homma & Masaoka 2008, citing Nyklíček et al. 1997 and Boiten 1998. |
| Breath, phrase mechanics | Inhalation before a phrase, breath-group length, subglottal pressure, lung-volume decline across a phrase, audible inhale/exhale level | **Unknown.** Titze and Hixon were not opened in this pass, so no pressure in cm H₂O is stated. Loudness is the proxy that is actually measured (§2). | — |
| Laughter as exhalation | Call duration, calls per bout, F0, mouth | §3. Repeated voiced calls, not a speech rate. | Bachorowski, Smoski & Owren 2001. |
| Fatigue | — | Unknown. | — |
| Health / hydration | — | Unknown. | — |
| Age | Vocal-tract length | Table I of Fitch & Giedd gives means from ages 2–4 through 19–25. Effect of later adult ageing on F0 is unknown here. | Fitch & Giedd 1999, Table I. |
| Sex / hormones | Vocal-tract length; laugh F0 and F1–F3 | §1.2 and §3. Emotion directions below are pooled, or have sex partialed out. Sex-specific emotion z is unknown. | Fitch & Giedd 1999; Bachorowski et al. 2001; Banse & Scherer 1996, p. 624. |
| Intent, smile | F0 and F1–F3 | Raised for all six speakers when smiling, against straight-faced speech. Per-speaker Hz were not copied; the table image was not clean enough to cite. Frowning lowers formants. | Tartter 1980, as read in the paper text; Juslin & Laukka 2003, p. 796, also citing Tartter & Braun 1994. |
| Intent, attractive opposite-sex addressee | F0 | Both sexes lowered F0. Pooled mean 208.84 Hz (SD 69.58) toward the attractive target vs 215.44 Hz (SD 72.38) toward the unattractive target, t(44) = −2.98. Sex × condition interaction F(1, 43) = 3.17, p = .08, so the paper does not establish a different shift for women and men. Descriptive means: men 142.95 vs 145.25 Hz; women 261.55 vs 271.59 Hz. | Hughes, Farley & Rhodes 2010, p. 160. |
| Personality | The baseline, not the feeling | No published offset. Learn it per speaker. | — |

**External**

| Factor | Features it moves | How much | Source |
|---|---|---|---|
| Listener distance, eye-to-eye vs across a room | — | **Unknown.** No distance-to-decibel function was read. | — |
| Noise (Lombard), loudspeakers | Loudness | +17.2 dB SPL from quiet (40 dB SPL) to 86 dB SPL cocktail-party noise, session IM3. Ten French adults (5 men, 5 women), partner seated 2 m away. | Garnier, Henrich & Dubois 2010, *JSLHR*, 53, 588–608, Experiment 1 results. |
| Noise, same paper, headphones vs loudspeakers | Loudness, F0 | Cocktail-party noise in headphones sat 7.8 dB SPL louder, and F0 sat 1.2 tones higher, than the same noise over loudspeakers. That is an immersion offset, not the quiet-to-noise slope. Broadband noise: +4.7 dB SPL for headphones vs loudspeakers. | Garnier et al. 2010, Experiment 1. |
| Noise while talking to someone | Loudness, F0, spectral centroid, lip opening, F1 | All five rose from quiet to 85 dB SPL, and the rise tended to be larger with a partner at 2.5 m than when speaking alone. Three women. The figure was not digitised; the deltas are unknown. | Garnier et al. 2010, Experiment 2. |
| Room / playback path | Level, and the trustworthiness of voice-quality features | The headphone offset above. Eyben et al. (2016) state that voice-quality parameters lose accuracy in noise and reverberation. The size of that loss is unknown here. | Garnier et al. 2010; Eyben et al. 2016, §2. |
| Social context | F0 | The attractive-addressee shift above. Other contexts unknown. | Hughes et al. 2010, p. 160. |
| Turn-taking | Pause proportion | Sadness portrayals had a large proportion of pauses; anger portrayals a small one. Fear and happiness were inconsistent. Tenderness was little studied. Gap length in ms is unknown. | Juslin & Laukka 2003, p. 796. |

### 1.4 The working set

Ten eGeMAPS names (Eyben et al., 2016, §3.1–3.2; openSMILE 3.0.0 config `eGeMAPSv01a`). Names and the alpha-ratio sign are from that config and from `src/lldcore/spectral.cpp` in the same release: alpha ratio in dB is `10 log10(energy from 1–5 kHz / energy below 1 kHz)`. Higher means more high-frequency energy, which is Juslin's cue.

| id | eGeMAPS name | Stage | Why it stays |
|---|---|---|---|
| `f0_mean` | `F0semitoneFrom27.5Hz_sma3nz_amean` | glottis | Sex baseline and the arousal code. Semitone 0 is 27.5 Hz (Eyben 2016, §3.1 and §6). |
| `f0_range` | `F0semitoneFrom27.5Hz_sma3nz_pctlrange0-2` | glottis | 20th-to-80th percentile range. Juslin's F0 variability. |
| `loudness` | `loudness_sma3_amean` | lungs | Perceived intensity. The measured proxy of expiratory drive. Not subglottal pressure. |
| `rate` | `VoicedSegmentsPerSec` | lungs / time | Pseudo syllable rate (Eyben 2016, §3.1). Not words per minute. |
| `pause` | `MeanUnvoicedSegmentLength` | lungs / time | Mean length of F0 = 0 regions, "approximating pauses" (Eyben 2016, §3.1). Includes closures. |
| `alpha_ratio` | `alphaRatioV_sma3nz_amean` | glottis + tract | High-frequency energy on voiced frames. |
| `hnr` | `HNRdBACF_sma3nz_amean` | glottis | Harmonic energy over noise-like energy. The air-leak measure. No per-region direction was published in the sources read. |
| `f1` | `F1frequency_sma3nz_amean` | mouth / jaw | Preliminary emotion directions (Juslin, p. 796) and open-mouth laughter. |
| `f2` | `F2frequency_sma3nz_amean` | lips | Smile raises formants (Tartter 1980). No emotion has its own F2 row in Table 7. |
| `f3` | `F3frequency_sma3nz_amean` | tract length | Speaker baseline and laugh formants. Emotion direction unknown. |

Four breath quantities are in the label file and are **not** eGeMAPS features: `breath_rate`, `inspiratory_time`, `tidal_volume`, `inspiratory_pause`. They are unknown except where §1.3 cites them. Audible inhale and exhale level are unknown.

Cut, because they did not earn a target: jitter (Juslin, p. 796: no definitive conclusion; a possible tendency for more jitter in anger than sadness only), shimmer, H1–H2, open quotient, NAQ, nasality, SPARC lip aperture, Rhubarb mouth shapes, MFCCs, spectral flux, and the rest of the 88 eGeMAPS parameters. H1–H2 remains the open-quotient description in Eyben et al. (2016, §3.1). It is not a second breathiness target until it separates something HNR and alpha ratio do not.

## 2. How the core becomes sound

One chain, in order. A feeling does not skip a stage.

1. **Lungs.** Behavioural breathing is not metabolic breathing (Homma & Masaoka, 2008). Air is inhaled, then spent across a phrase. Subglottal pressure and lung-volume decline are real and, here, unmeasured. What the tick can see is loudness, the unvoiced gaps, the voiced-segment rate, and — only where a respiratory study exists — breath rate, inspiratory time, tidal volume and inspiratory pause. The planning model is `tools/symphony/lungs` (`LUNGS.md`). It cites directions and keeps every magnitude in `configs/symphony/lungs.toml`. Those magnitudes are still not measurements.
2. **Glottis.** F0 is the rate at which the folds open and close (Juslin & Laukka, 2003, Table 6, p. 790). Incomplete closure lets air through and lowers HNR. F0 and loudness rise together in the high-arousal poles and in noise (§1.3). Per-tick source controls for this stage are `tools/symphony/throat`.
3. **Vocal tract.** The radiated sound is source filtered by the tract. Fitch and Giedd (1999) cite Fant (1960) for the path from glottis to lips. Fant's pages were not re-read; no Fant formula is copied. F1, F2 and F3 are the measured resonances. Tract length is baseline (§1.2), not a feeling.
4. **Nose.** V1's nasal output is a speech-class posterior, not nasalance (`ARTICULATION-LAYER.md`). No per-region nasalance was read. Nasality stays out of the labels. The planning model is `tools/symphony/nose` (`NOSE.md`): velum opening and a nasal murmur on the 80 ms tick, and wordless sounds on their own track. Magnitudes are `configs/symphony/nose.toml`. They are not measurements.
5. **Lips and jaw.** F1 tracks jaw opening in the preliminary emotion results (Juslin, p. 796: F1 higher in anger and happiness, lower in fear and sadness). A smile shortens and spreads the tract and raises formants (Tartter, 1980). Lip opening also increases in noise when the speaker is addressing someone (Garnier et al., 2010, Experiment 2). SPARC units are not the label.

Eye-to-eye proximity, whisper, and an audible intimate exhale are human-connection cues. Their acoustic sizes were not in the sources read. They stay unknown. The one cited "speaking to someone who matters" effect is the F0 drop in Hughes et al. (2010), and it is a few hertz in that phone-message task, not a whisper recipe.

## 3. Vocal bursts

A laugh is amusement with no words. The same category, said on a word, is prosody amusement (Cowen, Elfenbein, Laukka & Keltner, 2019, Figure 3B). A sigh, also with no words, may be relief, sadness, disappointment, or contentment; Cowen names sighs as examples of bursts and does not publish that four-way assignment. Breath rate, inspiratory time, tidal volume and an inspiratory pause are the physiology track (§1.3, §2), not a category.

Bachorowski, Smoski and Owren (2001), 1,024 laugh bouts, 97 young adults. These are absolute laugh values, not z against the same person's speech, and not the prosody of a word said amused.

| | Men | Women | Where |
|---|---|---|---|
| Mean F0, voiced calls | 272 Hz (SD 148); maximum 1245 Hz | 405 Hz (SD 193); maximum 2083 Hz | p. 1586 |
| Open-mouth call F0 | 279 Hz (SD 146), 563 calls | 415 Hz (SD 193), 862 calls | Table II, p. 1588 |
| Closed-mouth call F0 | 216 Hz (SD 92), 131 calls | 355 Hz (SD 127), 276 calls | Table II |
| Within-call F0 SD | 21.41 Hz | 29.98 Hz | p. 1586 |
| F0 excursion | 59 Hz (SD 49.74) in the text; Table II open-mouth 60 Hz (SD 51) | 86 Hz (SD 76.83); Table II open-mouth 88 Hz (SD 78) | p. 1586 and Table II |
| Bout duration | 0.95 s (SD 0.82) | 0.82 s (SD 0.72) | Table I, p. 1585 |
| Call duration | 0.17 s (SD 0.14) | 0.17 s (SD 0.14) | Table I |
| Calls per bout | 3.61 (SD 2.84) | 3.20 (SD 2.58) | Table I |
| Voiced open-mouth F1, F2, F3 | 535 (112), 1592 (153), 2576 (180) Hz; 41 laughers | 653 (155), 1713 (182), 2875 (227) Hz; 34 laughers | Table IV, p. 1589 |

Open-mouth calls had higher F0 than closed-mouth calls (p. 1586). Laughter sat in central vowel space, not in a stereotyped "tee-hee" (p. 1589). z against each laugher's own speech neutral is unknown.

The wordless track is planned in `tools/symphony/nose`. A laugh uses the Bachorowski anchors as config defaults. A sigh is still not one category. Moan, gasp, hum and "mm" have no acoustic target in the sources read.

## 4. How to read the label file

`configs/symphony/emotion-labels-v2.toml`

- `z_vs_own_neutral` is the quantity to learn. It is unknown for every region.
- `juslin_modal` is the most common call in Table 7. It is not a z.
- `banse_z_cross_emotion` is Table 6. Sex is already removed. It is not a male target or a female target.
- A feature missing under a region is unknown. Do not fill it by similarity.
- `pooled` means the source did not split sex. Copying it into both sexes does not create a sex-specific finding.
- Score a tick only when the baseline was taken in matched noise, room and distance. Otherwise report the tick as confounded.

## 5. Gaps

Unknown, to be learned from data, and not filled from a neighbour: acoustics for awe, confusion, contempt, disappointment, distress, interest, and for amusement said on words (the laugh numbers are the burst channel). Desire has only the Hughes courtship F0 drop. Disgust, spoken, has Banse's speech-portrayal profile (15% recognition). Sigh has no acoustic target. Also unknown: whisper; eye-to-eye level; subglottal pressure; lung-volume slope; audible breath level; breath-group length; fatigue; hydration; health; adult-age F0; personality offsets; mixture weights; onset shape; every z against the speaker's own neutral; speech F0 population norms (Hillenbrand and Peterson & Barney were not re-read).

## Sources read for a number or a direction

1. Banse, R., & Scherer, K. R. (1996). Acoustic profiles in vocal emotion expression. *Journal of Personality and Social Psychology, 70*(3), 614–636. Table 6, p. 625; recognition text, p. 622.
2. Juslin, P. N., & Laukka, P. (2003). Communication of emotions in vocal expression and music performance: Different channels, same code? *Psychological Bulletin, 129*(5), 770–814. Table 6, p. 790; Table 7, pp. 792–795; pp. 789, 796.
3. Eyben, F., Scherer, K. R., Schuller, B. W., Sundberg, J., André, E., Busso, C., Devillers, L. Y., Epps, J., Laukka, P., Narayanan, S. S., & Truong, K. P. (2016). The Geneva Minimalistic Acoustic Parameter Set (GeMAPS) for voice research and affective computing. *IEEE Transactions on Affective Computing, 7*(2). §§2–3, 6. Feature names: openSMILE 3.0.0, `config/egemaps/v01a/`, and the alpha-ratio formula in `src/lldcore/spectral.cpp`.
4. Bachorowski, J.-A., Smoski, M. J., & Owren, M. J. (2001). The acoustic features of human laughter. *Journal of the Acoustical Society of America, 110*(3), 1581–1597. Tables I, II, IV; p. 1586.
5. Fitch, W. T., & Giedd, J. (1999). Morphology and development of the human vocal tract: A study using magnetic resonance imaging. *Journal of the Acoustical Society of America, 106*(3), 1511–1522. Table I, p. 1514.
6. Boiten, F. A. (1998). The effects of emotional behaviour on components of the respiratory cycle. *Biological Psychology, 49*(1–2), 29–51. Abstract only.
7. Homma, I., & Masaoka, Y. (2008). Breathing rhythms and emotions. *Experimental Physiology, 93*(9), 1011–1021. Fig. 1 and the anxiety paragraph, citing Masaoka & Homma (2001).
8. Hughes, S. M., Farley, S. D., & Rhodes, B. C. (2010). Vocal and physiological changes in response to the physical attractiveness of conversational partners. *Journal of Nonverbal Behavior, 34*(3), 155–167. p. 160.
9. Tartter, V. C. (1980). Happy talk: Perceptual and acoustic effects of smiling on speech. *Perception & Psychophysics, 27*, 24–27. Direction read from the paper text. Per-speaker Hz table not copied.
10. Garnier, M., Henrich, N., & Dubois, D. (2010). Influence of sound immersion and communicative interaction on the Lombard effect. *Journal of Speech, Language, and Hearing Research, 53*(3), 588–608. Experiments 1 and 2.
11. Russell, J. A. (1980). A circumplex model of affect. *Journal of Personality and Social Psychology, 39*(6), 1161–1178. Axis precedent only. No figure was digitised.
12. Cowen, A. S., Laukka, P., Elfenbein, H. A., Liu, R., & Keltner, D. (2019). The primacy of categories in the recognition of 12 emotions in speech prosody across two cultures. *Nature Human Behaviour, 3*(4), 369–382. doi:10.1038/s41562-019-0533-6. PMC6687085.
13. Cowen, A. S., Elfenbein, H. A., Laukka, P., & Keltner, D. (2019). Mapping 24 emotions conveyed by brief human vocalization. *American Psychologist, 74*(6), 698–712. doi:10.1037/amp0000399. PMC6586540.

Named by those papers and not re-read, so not used for a number: Scherer (1986), Fant (1960), Tartter and Braun (1994), Masaoka and Homma (2001), Nyklíček et al. (1997). Titze and Hixon were not opened.

— Galion Voice
