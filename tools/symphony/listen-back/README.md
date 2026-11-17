# galion-listen-back

Perpetual Open MIT License. (c) 2026 Galion Labs.

Listening back, layer S7 of MnE V2. For each fixture scenario it runs the stack, decodes the audio, scores the read-back against the fixture, applies one correction, and scores again.

```bash
cargo test --manifest-path tools/symphony/listen-back/Cargo.toml
cargo run --manifest-path tools/symphony/listen-back/Cargo.toml -- run --out /tmp/listen-back.json --summary /tmp/listen-back.txt
```

The human summary is the stdout. `--out` is the JSON report (`galion-listen-back/1`). Both are deterministic: no clock, no samples, rounded to 4 decimals.

## What a scenario compares

The fixture states the intended shape: feeling trajectory (category, track, pitch offset), breath spans, and the baseline voice. The read-back is `galion-articulation analyse` (praatfan). When `GALION_ARTICULATION` points at the PR #191 binary, a `galion-voice-decode/2` profile is merged in and the feeling label is taken from that profile. The acoustic tracks stay the analyser's.

Scores, each from 0 to 1:

| Score | Compared |
|---|---|
| feeling match | Pitch contour against the fixture offset. Category only when a decode/2 profile is present. |
| breath plan match | Heard inhalations against the fixture spans. |
| pitch / voice quality | Median pitch error, plus HNR. Not MOS and not WER. |
| articulation | TractPrint v1, `galion-articulation` `score::compare`, against the baseline vowel. That score includes `f0_st`, so an expression pitch move can lower it. |
| speaker kept | F1 and F2. The baseline F0 field is not steered. F3 and VTL are reported and not scored; TractPrint v2 on PR #191 is the speaker split that owns them. |

The correction changes the asked pitch offset and, when the fixture's inhalation was missing, asks for it on the second render. It does not write the speaker baseline. There is no third pass.

## Local cloner

CI does not set these. A machine that already has the binaries can:

```bash
GALION_RENDER_CMD=/path/to/cloner \
GALION_ARTICULATION=/path/to/galion-articulation \
GALION_LUNGS=/path/to/galion-lungs \
  cargo run --manifest-path tools/symphony/listen-back/Cargo.toml -- run --out /tmp/listen-back.json
```

`GALION_RENDER_CMD` is invoked as `BIN --request request.json --out out.wav` with `HF_HUB_OFFLINE=1`. The request schema is `galion-listen-back-render/1`. The same command is the second pass, with the corrected pitch and breaths. Nothing is downloaded. No voice audio is committed.

`GALION_LUNGS` is `galion-lungs plan` (PR #195). Its inhale segments are recorded and compared with the fixture. They do not replace the read-back score.

`GALION_MNE_CONTRACT` and `GALION_FEELING` replace the local stubs when those agents publish a binary that prints the JSON this crate already deserializes.

## Rollout gate

The report records WER ≤ 1.8% (`0.018`), ECAPA > 0.7057, emotion ≥ 0.83, with `measured: false`. This harness does not run that gate and does not add a bar to it.

## Not in this crate

The decoder profile builder, the lungs planner, emotion-to-controls, and the voice-v2 pipeline stay on their drafts (PR #191, #195, #192, #194). Throat, mouth, and nose planners are stubs. Nose is `stub` in the body block. The synthetic vowel's first-pass undershoot (0.55 of the asked pitch, breaths omitted) is only so the correction can be seen. It is not a model of Chatterbox.
