# Mouth — articulation targets on the 80 ms tick

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-17.

This is the mouth stage of the body. It does not synthesise speech and it does not reimplement the V1 readers. A feeling and an intent go in. Per-tick control targets come out for the cloner, and viseme timing comes out for any avatar.

The crate is `tools/symphony/mouth` (`galion-mouth`). The clock is 80 ms.

## What a tick carries

- **Formant shift.** F1, F2, F3 as a direction and, only when a step size is configured, a hertz offset from this speaker's own phone. The offset is not an absolute formant.
- **Lips.** The phone's spread / round / closed / labiodental class, a blended planning coordinate when coarticulation has a weight, and a smile overlay.
- **Jaw.** Open, mid, or close from the phone. Emotion jaw follows the F1 sign. Addressing someone in noise opens it further.
- **Tongue.** Front / back, high / low, and tip up for /l/. Planning coordinates, not millimetres and not SPARC units.
- **Precision.** Clear speech versus slur. Clear reduces the coarticulatory pull. Slur increases it. The amount is config.
- **Speaking rate.** Faster or slower, as a multiplier on the viseme clock when a step exists.
- **Coarticulation.** Neighbouring vowels pull on each other through a consonant. The weight is config.
- **Visemes.** Rhubarb letters A–H and X, as cues (`start_ms`, `end_ms`) and as the letter at each tick centre.

A null amount means the size is unknown. Unknown is not zero, so it blocks a numeric sum that would otherwise hide it. The components still list a step that is known.

## Directions, and where the size stays unknown

| What moves | Direction | Size |
|---|---|---|
| Smile | F1, F2 and F3 higher; lips spread | Unknown. Tartter 1980. Per-speaker Hz were not copied. |
| Anger, prosody | F1 higher, jaw more open, rate faster | F1 is Juslin & Laukka 2003, p. 796, preliminary. Rate is Juslin Table 7 and Banse & Scherer 1996 Table 6 (both hot and cold anger are faster). The Banse z is not a multiplier. |
| Fear, prosody | F1 lower, jaw more closed, rate faster | Same F1 page. Panic and anxiety are both faster in Banse Table 6. F1 was not split by pole. |
| Sadness, prosody | F1 lower, jaw more closed, rate slower | Juslin Table 7 and p. 796. Banse sadness articulation-duration z is positive (slower). |
| Adoration, prosody | Rate slower. No F1 row | Juslin love–tenderness, Table 7. |
| Elation, prosody | Rate slower. No F1 row | Banse Table 6 elation articulation duration, sign only. Not copied onto amusement or interest. |
| Amusement, interest, embarrassment, desire, and the rest with no row | No mouth shift | Juslin's happiness F1 is not assigned to amusement or interest. Hughes 2010 is an F0 drop, not a mouth target. |
| Burst | Speech signs withheld | A laugh is not a spoken-word shift. Bachorowski, Smoski & Owren 2001 measured both open-mouth and closed-mouth laughs, so amusement-as-burst does not force a jaw. |
| Address someone in noise | F1 higher, jaw and lip opening more open | Garnier, Henrich & Dubois 2010, Experiment 2. The figure was not digitised. |
| Jaw, tongue, lips of a phone | Open jaw raises F1; a front tongue raises F2; rounding lowers formants | Lindblom & Sundberg 1971. No hertz from their figures. |
| Coarticulation | Vowels pull through a consonant | Öhman 1966. The weight is not a locus from that paper. |
| Clear versus slur | Clear reaches the phone; slur undershoots toward the neighbours | Lindblom 1990 (H&H). Moon & Lindblom 1994: clear speech reduces context-induced displacement. |
| Rate versus undershoot | Not coupled | Lindblom 1963 describes undershoot as duration falls. Gay 1978 found transition duration relatively stable. The coarticulation weight does not grow just because the rate is faster. |

Valence does not spread the lips. A smile is an intent. Arousal is recorded and is not a per-unit gain. Dominance names the pole (hot / cold anger, panic / anxiety) so a config file can give them different steps. The cited signs do not change with the pole.

The viseme clock uses the rate of the feeling at t = 0. A later feeling still reports its own rate on its ticks. Onset shape was not read, so the cue list is not rebuilt mid-utterance.

## Reading, reused

SPARC, Phonet, Rhubarb and praatfan stay where V1 put them (`tools/symphony/articulation`, PR 188). This crate parses the card they already write, or the worker JSON (`ema`, Phonet `classes`, Rhubarb `mouthCues`). The 80 ms median and the tick-centre viseme rule match that crate.

`read --wav` runs a local `galion-articulation` when `--bin` or `GALION_ARTICULATION` points at it. Nothing is downloaded.

`listen` reports `(measured F1 − baseline F1) − target shift`. The residual is null when the target has no hertz or the baseline read is missing. There is no pass mark. It is not TractPrint. SPARC lip aperture is passed through in template units and is not scored. A Phonet nasal posterior stays a speech-class posterior, not nasalance.

## What is stubbed

- **Intent contract.** `IntentStub` is local. TODO(S1): replace it with `galion-mne-contract`. This crate does not run grapheme-to-phoneme. Phones are whatever the caller already has. No phones means one rest tick, viseme X.
- **Feeling.** The decoder tick (`blend`, `burst`) and the emotion-render object (`emotion` or `category`) are accepted. Agent S2 owns the feeling. Mixture weights are the caller's weights. A missing `intensity` is 0, which is no contribution, matching emotion-render.
- **Step sizes.** `configs/symphony/mouth.toml` is `unknown` on every magnitude. `phone_ms = 80` is the clock, not a measured phone length.
- **Cloner.** `clone_mouth` is the packet for the integrator (bc-ed0d7d68, draft PR 194). Apply one speaking-rate multiplier. `emotion-render` `prosody.rate` uses the same Juslin/Banse signs. This packet does not set exaggeration, cfg, or temperature, and it does not render audio.
- **Avatar.** Cues are Rhubarb letters. Any avatar maps them. There is no face rig.

## Tests

`cargo test --manifest-path tools/symphony/mouth/Cargo.toml`

Synthetic JSON only. No audio in git, no dataset, no weight download.

## Sources for a direction

1. Tartter, V. C. (1980). Happy talk: Perceptual and acoustic effects of smiling on speech. *Perception & Psychophysics, 27*, 24–27.
2. Juslin, P. N., & Laukka, P. (2003). Communication of emotions in vocal expression and music performance. *Psychological Bulletin, 129*(5), 770–814. Table 7; F1 on p. 796.
3. Banse, R., & Scherer, K. R. (1996). Acoustic profiles in vocal emotion expression. *Journal of Personality and Social Psychology, 70*(3), 614–636. Table 6, p. 625. Sign of articulation duration only.
4. Garnier, M., Henrich, N., & Dubois, D. (2010). Influence of sound immersion and communicative interaction on the Lombard effect. *JSLHR, 53*, 588–608. Experiment 2.
5. Lindblom, B. E. F., & Sundberg, J. E. F. (1971). Acoustical consequences of lip, tongue, jaw, and larynx movement. *JASA, 50*(4B), 1166–1179.
6. Öhman, S. E. G. (1966). Coarticulation in VCV utterances: Spectrographic measurements. *JASA, 39*, 151–168.
7. Lindblom, B. (1963). Spectrographic study of vowel reduction. *JASA, 35*, 1773–1781.
8. Gay, T. (1978). Effect of speaking rate on vowel formant movements. *JASA, 63*(1), 223–230.
9. Lindblom, B. (1990). Explaining phonetic variation: A sketch of the H&H theory. In W. J. Hardcastle & A. Marchal (Eds.), *Speech Production and Speech Modelling* (pp. 403–439). Kluwer.
10. Moon, S.-J., & Lindblom, B. (1994). Interaction between duration, context, and speaking style in English stressed vowels. *JASA, 96*(1), 40–55.
11. Bachorowski, J.-A., Smoski, M. J., & Owren, M. J. (2001). The acoustic features of human laughter. *JASA, 110*(3), 1581–1597.
12. Cowen, A. S., Elfenbein, H. A., Laukka, P., & Keltner, D. (2019). Mapping 24 emotions conveyed by brief human vocalization. *American Psychologist, 74*(6), 698–712.
13. Cowen, A. S., Laukka, P., Elfenbein, H. A., Liu, R., & Keltner, D. (2019). The primacy of categories in the recognition of 12 emotions in speech prosody across two cultures. *Nature Human Behaviour, 3*(4), 369–382.
14. Rhubarb Lip Sync 1.13.0, MIT, README mouth shapes A–H and X. Daniel S. Wolf.
15. SPARC, Phonet, and praatfan_rust, as already wired in `ARTICULATION-LAYER.md`. Channel order and the nasal-class warning are unchanged.
