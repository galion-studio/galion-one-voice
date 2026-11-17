# MnE contract (Inner Life to mouth and expressions)

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-17.

The crate is `tools/symphony/mne-contract` (`galion-mne-contract`). It is the versioned frame the MnE layers share. The wire layout, the CLI, and the fixture knobs are in that crate's README. This page is the place of the contract in the stack.

Inner Life writes an intent document. MnE writes a read-back document. Both are one frame per 80 ms. The binary is `GMN1`. JSON and TOML are a lossless mirror of the same document.

The feeling is a continuous point. A Cowen 2019 name is an optional pin. Neutral is the speaker baseline on the document, not a pin and not one of the 24. Breath is a lungs segment. The twelve Nature Human Behaviour names are a prosody tier, not a gate, which is the rule already stated in `ARTICULATION-V2-SPEC.md` and `configs/emotions-v2.toml`.

Nothing in the contract is a z against the speaker's own neutral, a subglottal pressure in cm H₂O, a nasalance target, or a whisper gain. Those magnitudes are unknown in the spec. Fixture numbers are knobs in `configs/symphony/mne-contract.toml` (`literature = "none"`).

Directions, with no magnitude copied:

- Cowen, Elfenbein, Laukka and Keltner (2019), *American Psychologist*, 74(6), 698–712, Figure 3B, for the 24 names.
- Cowen, Laukka, Elfenbein, Liu and Keltner (2019), *Nature Human Behaviour*, 3(4), 369–382, for the prosody tier.
- Russell (1980), *Journal of Personality and Social Psychology*, 39(6), 1161–1178, for a continuous point rather than a class. No coordinate is copied.
- Kuppens, Allen and Sheeber (2010), *Psychological Science*, 21(7), 984–991, for mood that lingers. No lag is copied. The frames are the linger.

The eight stand-ins under `tools/symphony/mne-contract/fixtures/` are invented moments: comfort, tease, confess, argue, whisper close, tired late night, reassure, quiet awe. They are not recordings.

This crate does not replace `galion-emotion`'s 8-byte `EmotionWire`, the articulation decoder (PR #191), emotion-to-controls (PR #192), the voice pipeline (PR #194), or the lungs planner (PR #195). Field names line up so those layers can be stitched without a second inventory.
