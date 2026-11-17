# Lungs and air

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-17.

Breath is a physiology track. It is not one of the 24 categories in Cowen, Elfenbein, Laukka and Keltner (2019, *American Psychologist, 74*(6), 698–712) and it is not a vocal burst. Sigh and laughter stay bursts. The code is `tools/symphony/lungs`. The numbers it multiplies by are `configs/symphony/lungs.toml`. A number in that file is a knob. It is not a result copied out of a table.

The 80 ms step is the Symphony clock. No paper below says that feeling, or breathing, ticks at 80 ms.

## What is planned

Text is split into words. A sentence end, or a line break, closes a breath group. A comma, semicolon or colon closes one only once the group is already long enough. A group that would run past the length limit is split before the next word. That is the direction in Winkworth, Davis, Adams and Ellis (1994, *Journal of Speech and Hearing Research, 37*, 535–556; 1995, *38*, 124–144): inhalations sit at linguistic boundaries. The limit in seconds is config. Their papers were not re-measured here.

Inspiration duration rises with the duration of the phrase it is about to support (Whalen and Kinsella-Shaw, 1997, *Phonetica, 54*, 138–152). Inspiration depth rises with that same duration (Winkworth and colleagues, same papers). Both slopes are config. Depth is a fraction of a relative volume span, not litres and not percent vital capacity.

## Volume and pressure

Speech is exhaled. Lung volume falls across the breath group (Hixon, Goldman and Mead, 1973, *Journal of Speech and Hearing Research, 16*, 78–115). The model spends a config fraction of the volume just inhaled. The volume track is relative. It is not a spirometer.

Relative subglottal pressure falls as that volume falls. Lieberman (1967, *Intonation, perception, and language*, MIT Press) treated the falling pressure of a breath group as one cause of F0 declination. Strik and Boves (1995, *Journal of Phonetics, 23*, 375–394) measured the F0 and subglottal-pressure downtrend together. Loudness rises with subglottal pressure (Ladefoged and McKinney, 1963, *Journal of the Acoustical Society of America, 35*, 454–460). F0 rises with subglottal pressure (Titze, 1989, *Journal of the Acoustical Society of America, 85*(2), 901–906). The dB and the semitones per relative-pressure unit are config. No cm H₂O value is stated. Titze's and Hixon's pressure tables were not copied.

Energy adds to that pressure. The spec calls loudness the proxy of expiratory drive. The slope is config. Arousal adds to it as well, because high arousal is louder (Juslin and Laukka, 2003, *Psychological Bulletin, 129*(5), 770–814). Positive arousal also shortens the breath group.

## What emotion is allowed to change

Only rows that a source actually gives:

| Input | What changes | Source | Size |
|---|---|---|---|
| amusement | shorter inspiration, smaller depth | Boiten 1998, abstract, *Biological Psychology, 49*, 29–51 | config scales below 1 |
| disgust | a silent hold after the inhale | Boiten 1998, same abstract (prolonged inspiratory pause) | config seconds |
| fear | shorter breath groups, so more inhalations | Homma and Masaoka 2008, *Experimental Physiology, 93*(9), 1011–1021, the anxiety rate increase, which the label file already maps onto fear | config scale below 1 |
| arousal > 0, any category | shorter breath groups, higher pressure | Homma and Masaoka 2008 for rate; Juslin and Laukka 2003 for loudness | config |
| the other 21 categories | nothing of their own | no breath row was read | they use the phrase plan |

Panic and anxiety are not two breath recipes. Awe, desire, sadness and the rest do not inherit amusement's breath or disgust's hold. Desire's audible exhale is unknown in the spec, so desire does not get a special exhale.

Fatigue is wired and unpublished. The spec lists it as unknown. `fatigue.literature` is `none`. The coefficients may be set to zero. Their sign is not a finding.

## Audible air

Two sources, and no third:

1. The speaker's own inhale, when the caller passes samples plus a cue list from the features agent's breath stage (bc-56b9f22c, `breath.rs`: voicing-gap `measure`, or `workers/breath_worker.py` when a local model is already present). `splice_inhales` is that agent's fallback and is kept as-is. The planned track fits the same cue to the planned duration and the planned level.
2. Otherwise the heartbeat's breath noise (`crates/galion-heartbeat/src/presence.rs`: white noise, seed 13, RMS 0.003), with a Hann envelope so the block starts and ends at zero, then rescaled. The envelope is not a measured breath spectrum.

The −55 dBFS line is the presence gate's breath-versus-silence floor, used as a check that the neutral synth is in the range that gate already calls a breath. It is not a sound-pressure level of a person.

An unvoiced release follows each phrase. It is an exhale, not a sigh. Sigh acoustics are unknown.

## What this does not do

It does not measure a speaker. It does not emit cm H₂O, litres, or breaths per minute as a published norm. It does not run YAMNet, Praat, or CPPS. It does not write a reference recording into git. Detection of breaths in a reference stays in the features agent's crate.
