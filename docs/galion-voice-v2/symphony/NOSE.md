# Nose and wordless sounds

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-17.

The nose is a control, and a wordless sound is its own track. The code is `tools/symphony/nose`. The numbers it multiplies by are `configs/symphony/nose.toml`. A number in that file is a knob. It is not a result copied out of a table.

The 80 ms step is the Symphony clock. No paper below says that the velum, or a laugh, ticks at 80 ms.

## Velum

The velopharyngeal port is what couples the nasal cavity to the rest of the tract (Stevens, 1998, *Acoustic Phonetics*, MIT Press). This layer's control is `velum_open`, from 0 (closed) to 1 (coupled). It is not a port area in square centimetres. Stevens's pole frequencies are not copied.

On each tick the opening is:

1. The speaker's own resting value. That is the baseline. z = 0 there, by the same rule as the rest of articulation V2. The caller passes it. A missing card rests at 0, which is "no nasal coupling was given," not a measured nasalance.
2. An explicit nasal span from the intent. That is the planner saying the port is open for this stretch (a nasal consonant, or a held hum). The amount is config.
3. A feeling term: intensity times `feeling.velum_per_intensity`. No per-region nasalance was read (`ARTICULATION-V2-SPEC` §2), so the gain ships at 0 and the literature field is `none`. Setting the gain moves the velum. The sign is not a finding.

`phon_nasal` from the articulation card is stored and ignored. It is a speech-class posterior, not nasal resonance (`ARTICULATION-LAYER.md`).

A hum and an "mm" force the port open for their own span. Those sounds leave through the nose: the lips are closed and the velum is open (Stevens, 1998, nasal consonants and the nasal murmur). A laugh does not. Bachorowski, Smoski and Owren (2001) split laughs into open-mouth and closed-mouth by F0, not by nasalance.

## Murmur and A1–P0

Opening the velum adds low-frequency energy and an antiresonance in the oral transfer (House and Stevens, 1956, *Journal of the Acoustical Society of America, 28*(3), 303–310). The murmur track is a sinusoid whose RMS follows `velum_open`. The frequency is config. It is not Chen's P0 and not a nasal formant.

Chen (1997, *Journal of the Acoustical Society of America, 102*(4), 2360–2370) showed that A1–P0 rises as a vowel becomes nasalized. Each tick records that direction against this speaker's rest: `rises`, `falls`, or `flat`. The dB value is `a1_p0.db_per_velum` times the velum change, and that slope ships at 0. The antiresonance is a control for a later oral filter. This layer does not filter a spoken vowel. The mouth layer owns that path.

## Wordless track

Laughs, sighs, moans, gasps, hums and "mm" are gestures. They are not extra categories. The 24 names stay the feeling.

| Gesture | Category | Shape |
|---|---|---|
| laugh | amusement, always | Bachorowski call train. See below. |
| sigh | none, unless the caller names one of relief, sadness, disappointment, contentment | one recipe. The four names do not get four acoustics. Spec §3 and §5: a sigh has no acoustic target. |
| moan, gasp, hum, mm | none, unless the caller names one of the 24 | config duration. No Cowen assignment was read. |

A word said with amusement is prosody. It does not place a laugh. A laugh is placed when the intent asks for one, or when the caller asked the planner to place from feeling and amusement is strictly the top of the **burst** weights. A tie places nothing. Any other burst-track top is reported and does not grow a gesture, because no gesture was published for it.

The open feeling is copied onto the event (arousal, valence, dominance, intensity, prosody category). Valence and dominance do not move a knob. Intensity and arousal change level only through slopes that ship at 0. Juslin and Laukka's speech loudness signs are not applied here: a wordless sound is not a shift against this speaker's speech neutral.

### Laugh anchors

Bachorowski, Smoski and Owren (2001), *Journal of the Acoustical Society of America, 110*(3), 1581–1597.

- Table I, p. 1585: call duration 0.17 s for both sexes. Calls per bout 3.61 (men) and 3.20 (women). The config integers 4 and 3 are this planner's rounding, not a new count. Bout duration 0.95 s (men) and 0.82 s (women) is recorded and is not the rendered length, because the inter-call gap was not in that table.
- Table II, p. 1588: open-mouth F0 is higher than closed-mouth, and the women's open-mouth mean is higher than the men's. The config validator keeps those inequalities. The hertz are the published means, used as laugh F0, not as z against speech.

Which column: F0 at or above 165 Hz uses the women's means, below it the men's. That cut is the decoder's label-column prior (`speaker_baseline` in draft PR 191), not Bachorowski's sexing. A missing F0 uses neither mean.

## Reading

Detection stays in draft PR 191. This layer parses the JSON that crate already writes:

- `workers/laughter_worker.py`: `source`, `gap`, `events`. An empty event list with a gap is the result that worker writes when no local checkout is present. It is not turned into a laugh.
- `decode.rs` `BurstTrack`: `weights`, `top`, `source`. Tick bursts are averaged. Weights are not a time span.

A laugh event is amusement, the same assignment as `state::resolve("laughter")`. A sigh event stays unassigned, with the four candidates. The planner adopts a detected span only when the intent says so, and then it keeps that span instead of the config duration.

## What this does not do

It does not measure nasalance. It does not run Phonet, Praat, or the laughter model. It does not download that model. It does not write a reference recording into git. A reference, when the caller passes samples, is stretched to the event and rescaled; that fit is not a model of the nose. The oral antiresonance is not applied to speech. A cloner is not called. The intent type is a local stub (`galion-mne-intent-stub/0`) until agent S1's `galion-mne-contract` crate replaces `contract.rs`.
