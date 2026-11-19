# galion-mne-contract

Perpetual Open MIT License. (c) 2026 Galion Labs.

Versioned frames between Inner Life and Mouth-and-Expressions (MnE). This is the foundation the other MnE layers share. It does not decode a waveform, plan a breath, or call a cloner.

One document is either **intent** (Inner Life to MnE) or **read-back** (MnE to Inner Life). Frames are contiguous steps of 80 ms, the Symphony clock in `LOCKS.toml`. That step is not a published feeling time constant.

## What a frame carries

Intent, each tick:

- an open intent verb and a strength in 0..1
- the target feeling
- a UTF-8 byte span into the utterance text
- energy, fatigue, and health, which the lungs layer already takes as inputs

The document, shared by every tick:

- persona id and a pluggable voice-profile id
- moment, memory id, optional memory snippet, place, distance token, noise token, listener token
- the speaker baseline this utterance treats as neutral

Read-back, each tick:

- the feeling that was heard
- body state for lungs, throat, mouth, and nose
- confidence in the feeling, confidence in the body, and a confounded flag

## Feeling

The feeling is a continuous point: arousal 0..1, valence −1..1, dominance 0..1. Those ranges are the `galion-emotion` wire. A point with no pin is unnamed. That includes the speaker baseline. Neutral is not a twenty-fifth category.

A pin is one of the 24 names in `configs/emotions-v2.toml` (Cowen, Elfenbein, Laukka and Keltner, 2019, *American Psychologist*, Figure 3B), on either the prosody track or the burst track, with a caller-supplied weight in (0, 1]. The twelve Nature Human Behaviour names are a prosody tier (`CowenName::tier`), not a gate. Embarrassment may be pinned on a word. Breath is a lungs segment, not a pin.

Mixture weights and onset shapes are unpublished. The weight field is the caller's, not a fitted mixture. No coordinate is copied from Russell, Cowen, Banse, or Juslin.

PR #191 stores arousal, valence, and dominance on −1..1. This crate does not convert that scale. A negative arousal fails validation.

## Body

Slots line up with the layers that already exist. This crate does not fill them.

| Slot | Lines up with | Left empty when |
|---|---|---|
| `lungs.segment`, `lung_volume_rel` | PR #195 tick (`inhale`, `hold`, `speech`, `exhale`) | the layer has not run |
| `subglottal_pressure_rel`, `loudness_db` | the same tick's relative pressure and loudness offset | no cm H₂O and no dB SPL are claimed |
| `throat.f0_hz`, `hnr_db` | articulation `f0_hz` and HNR | no hertz was measured |
| `mouth.f1_hz`, `f2_hz`, `f3_hz` | articulation formants | same |
| `nose.nasalance` | not a label in the V2 spec | nasalance was not given a target |

`origin = "unmeasured"` forbids every level. `stand_in` and `measured` require at least a segment or a level, and they are not interchangeable: a fixture that says `stand_in` is a knob.

## Binary and mirrors

`GMN1` is schema version 1, little-endian, finite binary64 floats. The layout is the comment at the top of `src/binary.rs`. The same document encodes to the same bytes.

JSON and TOML are a lossless mirror of the document. Whitespace is not part of the mirror. Encode refuses a document that fails validation.

```text
galion-mne-contract encode <in.toml> <out.mne>
galion-mne-contract decode <in.mne> <out.json>
galion-mne-contract validate <file>
galion-mne-contract fixtures [dir]
```

`project_emotion_wire` quantizes valence, arousal, dominance, and confidence the way `galion-emotion` `EmotionWire` bytes 3..6 do. It does not pack a Cowen pin into the three-bit affect label. The projection is lossy and is not the mirror.

## Directions, and what is not a number

- Categories and the prosody tier: Cowen et al. 2019, *American Psychologist*, 74(6), 698–712, Figure 3B; Cowen et al. 2019, *Nature Human Behaviour*, 3(4), 369–382.
- A feeling can be a point rather than a class: Russell, J. A. (1980). A circumplex model of affect. *Journal of Personality and Social Psychology*, 39(6), 1161–1178. No coordinate is copied.
- Mood lingers: Kuppens, P., Allen, N. B., & Sheeber, L. B. (2010). Emotional inertia and psychological maladjustment. *Psychological Science*, 21(7), 984–991. No lag is copied. The trajectory itself is the linger.
- Own-neutral baseline, z = 0, not a category: `docs/galion-voice-v2/symphony/ARTICULATION-V2-SPEC.md` §1.2, and the inventory file.
- Anger is the high-arousal pole in the argue fixture, as a direction only: Juslin, P. N., & Laukka, P. (2003). *Psychological Bulletin*, 129(5), 770–814, Table 7. No hertz is stored.
- Whisper and eye-to-eye level have no acoustic size in the sources that spec read. The whisper fixture stores a distance token and does not store an F0 or a gain. The cited social F0 drop (Hughes, Farley and Rhodes, 2010) is a few hertz in a phone task and is not copied in.
- Fatigue has no published magnitude. The lungs layer records `literature = "none"`. The tired fixture's fatigue is a knob in the fixture, not a breaths-per-minute norm.

Shared knobs for the fixtures live in `configs/symphony/mne-contract.toml`. Every number there is `literature = "none"`.

## What works

`cargo test --manifest-path tools/symphony/mne-contract/Cargo.toml` parses the 24 names against `configs/emotions-v2.toml`, rejects the split aliases, round-trips binary, JSON, and TOML, checks the eight fixtures, and runs the CLI on a temporary file. No audio, no download, no measuring run.

## What is stubbed

- No feeling dynamics. Linger is whatever trajectory the caller wrote. This crate does not step a state.
- No lungs, throat, mouth, or nose model. Unmeasured body is the honest default.
- No bipolar conversion for PR #191 `TickState`.
- No packing of a pin into `EmotionWire`'s three-bit label.
- No cloner, no eval harness, no audio. The integrator (PR #194) stitches those.

## Reused, not rewritten

The inventory file, the 80 ms lock, the `galion-emotion` scalar layout, and the field names of the articulation card, the emotion-render tick, the voice-v2 feeling, and the lungs tick. Their crates are not copied into this one.
