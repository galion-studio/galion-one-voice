# F — Aria blend

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S17. Roles: blend math, identity-distance guard, persona traits, tests and this note. |
| Stage | `galion-aria-blend`. Reads two `galion-preclone/1` profiles. Writes one `galion-voice-design/1` spec. |
| Voice | Aria. A new synthetic voice, the planned 23-year-old daughter of the Samantha and Jarvis roles. Not a real person. |
| Compute | CPU only. The same two profiles write the same spec bytes. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. |

Aria takes Samantha's warmth and curiosity and Jarvis's calm precision. She is playful, honest, emotionally attentive, and steady at work. Honest is how she answers. It does not move a formant. The two roles are fictional test registers. They are not the film performers, and they are not the LibriVox donors. This stage never opens a wav.

The real pair of profiles is blended on the owner's laptop. CI and this tree blend generated profiles only. No voice audio is committed.

## Command

`galion-aria-blend --samantha PROFILE.json --jarvis PROFILE.json --out SPEC.json`

The first file is the warmth slot. The second is the calm slot. Paths are not copied into the spec. `--help` exits 2. A refusal exits 1.

## Consent

Each input is a `galion-preclone/1` profile with `source.consent`. The allowed marks are exactly `own-voice` and `fictional-reference`. Anything else refuses the run before the blend.

These marks are refused as real third parties, even when the numbers would otherwise blend: `third-party`, `real-person`, `real-third-party`, `celebrity`, `donor`, `cc0-donor`, `performer`, `film-audio`. A profile from `galion-preclone` with no `source` object is refused. The stage does not listen, so it cannot recognise a person. The mark is the operator's declaration on the laptop.

## Blend

Acoustic weight is 1/2 on each parent: F0, F1, F2, F3, centroid, HNR, voiced fraction. Then the age-target shift, then three trait offsets.

| Knob | Value |
|---|---|
| Age target | 23 years |
| Anchor | 32 years. A design anchor for the adult roles, not a birthday. |
| Shift | 23 − 32 = −9 years |
| F0 scale | +0.35% per year younger → 1.0315 |
| Formant and centroid scale | +0.30% per year younger → 1.027 |
| Warmth | HNR −0.6 dB, which raises breath a little |
| Play | F0 span +0.5 semitone |
| Steady at work | rate × 0.985, then clamped to 0.80..=1.20 |

Breath is `(24 − HNR) / 24`, clamped to 0..=1. 24 dB is clear. 0 dB is fully breathy. Rate is `1 + (F0 − 150) / 2000 + (voiced − 0.70) × 0.10`, then the steady scale. It is a design rate, not a syllable count. F0 span in semitones is `3 + 3 × (1 − clamp(HNR / 24, 0, 1))`, plus the play bonus, clamped to 2..=12. The published range is symmetric in semitones around the centre, and the child's endpoints stay inside 70..=400 Hz.

Timbre is the formant triple and the centroid. The child is labelled `blended-formant`. Each parent baseline, before the age shift and the trait offsets, is labelled `parent-steady`.

## Emotion default

`emotion_default` is a value for `galion-emotion-core`'s `VoiceBaseline` (PR #263). This crate does not depend on that crate. `EmotionCore::step` is not called. The block is what a later copy fills in:

| `VoiceBaseline` | Spec field |
|---|---|
| `persona: [u8; 16]` | `persona_id` `aria` (NUL room left in the 16 bytes) |
| `voice: [u8; 16]` | `voice_id` `aria-steady` |
| `pos[0]` valence, −1..1 | 0.62 × 0.34 + 0.38 × 0.10 = 0.2488 |
| `pos[1]` arousal, 0..1 | 0.45 × 0.40 + 0.55 × 0.24 + 0.04 = 0.352 |
| `pos[2]` dominance, 0..1 | 0.40 × 0.38 + 0.60 × 0.60 − 0.04 = 0.472 |
| `pos[3]` approach, −1..1 | 0.70 × 0.30 + 0.30 × 0.08 = 0.234 |
| `pitch_centre_hz` | the child's F0 centre |

The four parent anchors are warmth/curiosity against calm/precision. Play adds the +0.04 arousal. Steady work subtracts 0.04 dominance. Approach stays with Samantha, which is the attentive part. `axis_order` is `valence`, `arousal`, `dominance`, `approach`.

## Identity distance

Nine baseline features, each divided by one step of "a different voice", then the Euclidean length:

| Feature | Step |
|---|---|
| F0 centre | 25 Hz |
| F0 span | 12 Hz |
| F1 | 60 Hz |
| F2 | 120 Hz |
| F3 | 150 Hz |
| Centroid | 150 Hz |
| Rate | 0.05 |
| Breathiness | 0.12 |
| HNR | 3 dB |

The minimum is 1.25. The guard uses the same 6-decimal rounding as the file. A distance under that, or a non-finite distance, refuses the run. Copying one parent into both slots stays under the bar: the age and trait move is not enough to count as a new voice. The two synthetic roles clear it. Equal weight is what keeps the child off both parents. The guard runs on the published child, after the age shift and the trait offsets.

## Tests

`cargo test --manifest-path tools/cloning-v2/preclone/Cargo.toml`

`bash scripts/check-locks.sh`

The Aria gates live in `tools/cloning-v2/preclone/tests/aria.rs`. Profiles are built in the test. One gate also runs `galion-preclone` on a synthesized formant and refuses that profile until it is marked. CI runs this manifest from `gates.yml`. No wav, mp3, flac, ogg, or m4a is part of the stage.
