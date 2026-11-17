# Throat / glottal source

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-17.

The throat layer is the glottal source. It sits after the lungs and before the mouth. The code is `tools/symphony/throat`. The numbers it multiplies by are `configs/symphony/throat.toml`. A number in the scales section is a knob. It is not a z against this speaker's neutral, and it is not an Rd copied out of a table of emotions.

The 80 ms step is the Symphony clock. No paper below says that a feeling, or a glottal cycle, ticks at 80 ms.

## What a tick carries

On each voiced tick the cloner adapter gets:

- pitch, in semitones versus this speaker's own baseline F0
- a phonation label: breathy, modal, pressed, or creaky
- open quotient, from the LF timing
- spectral tilt, in dB per octave, from the harmonics of that one model pulse
- jitter and shimmer, as controls

Unvoiced ticks (inhale, hold, exhale, or speech with no subglottal pressure) carry none of those. The folds are not being driven.

The optional reference is the Liljencrants–Fant flow derivative. It is not a vowel. The vocal tract is a different layer. The wav the CLI can write is peak-scaled and is not a calibration of Ee.

## The model

The pulse is the model in Fant, Liljencrants and Lin (1985), STL-QPSR 26(4), 1–13. Rd collapses the shape the way Fant (1995), STL-QPSR 36(2–3), 119–156, defined it, with the regressions restated in Fant and Kruckenberg (1996), TMH-QPSR 37(4), 45–56:

- Ra = (−1 + 4.8 Rd) / 100
- Rk = (22.4 + 11.8 Rd) / 100
- Rg from their approximation Rd = (1/0.11) (0.5 + 1.2 Rk) (Rk/(4 Rg) + Ra)
- open quotient = (1 + Rk) / (2 Rg), the Ra term dropped, which is the column in their Table 2
- Fa = F0 / (2 π Ra), where the flow-derivative spectrum steepens by another 6 dB/octave

Those coefficients are the model. They are not in the config file. The regressions are used inside Rd 0.3 to 2.7 (Gobl, 2017, Interspeech, citing Fant 1995). Outside that, Rd is clamped and the tick says so.

Table 2's prose is the label rule. Rd = 0.5 is a highly adducted, somewhat pressed voice. Rd = 2.7 is breathy. Modal is the open interval between those two anchors. The config reference Rd is 1.0, their table row, not a measurement of a speaker. A speaker may pass their own Rd. Creak is not a row in that table. It is a label the caller can request, and it does not by itself change Rd or jitter.

No open LF library is linked. DisVoice measures glottal features and, on the glottal path, imports a GPL Praat binding. This layer does not run it and does not copy it. The equations above are the implementation.

## What is allowed to move the pulse

| Input | What changes | Source | Size |
|---|---|---|---|
| Lungs `f0_st` | the pitch contour, added as semitones | Titze 1989, JASA 85(2), 901–906, via the lungs layer. This layer does not keep a second pressure-to-F0 slope | the lungs tick |
| Lungs `loudness_db` | Ee, the excitation scale | Ladefoged and McKinney 1963, JASA 35, 454–460. Loudness is their pressure proxy | the lungs tick, as an amplitude ratio |
| Feeling, prosody, pitch sign | pitch, when a step exists | Juslin and Laukka 2003, Table 7; Banse and Scherer 1996, Table 6, sign only; Hughes, Farley and Rhodes 2010, p. 160, for desire | config, shipped as unknown |
| Feeling, prosody, high-frequency energy | Rd, opposite the energy sign (more high-frequency energy, lower Rd, higher Fa) | Juslin and Laukka 2003, Table 7, for the sign. Fant and Kruckenberg 1996 for the Rd direction | config, shipped as unknown |
| Lungs loudness, when `ks` is set | Rd falls as Ee rises | Fant and Kruckenberg 1996: Ks is typically about 0.5 and ranges from 0 to 1. That typical value is not copied in | config, shipped as unknown |
| Jitter, shimmer | the reference period and the peak, if set | Juslin and Laukka 2003, p. 796: no definitive jitter conclusion. No per-category sign is applied | config, shipped as unknown |

Fear's high-frequency energy is bimodal in Juslin and Laukka, so it does not move Rd. Categories with no acoustic row (awe, embarrassment, and the rest of the unknown list) do not inherit a neighbour's sign. Burst keeps the category and does not take the speech signs. A wordless sound is not a shift against this speaker's speech neutral.

Valence is recorded on the feeling and does not move a knob. Arousal and energy already entered the lungs pressure. They are not added a second time here.

## What this does not do

It does not plan breaths, and it does not recompute subglottal pressure. It does not synthesize a vocal tract or a nose. It does not call Chatterbox. The integrator (bc-ed0d7d68, PR 194) is the stitch. `galion-mne-contract` is not in this tree; `contract.rs` is a local stand-in, marked TODO(S1). H1–H2 is not emitted. The articulation spec cut it as a target, and a numeric formula was not needed for the controls above.

MIT. No secrets. No voice audio in git.
