# galion-voice-v2

Perpetual Open MIT License. (c) 2026 Galion Labs.

The Voice V2 glue: one pipeline, then the Symphony call.

```text
reference wav → clean → decode → profile.json
text + target feeling → render → environment mix → read back → one correction
```

`speak` is what a full-duplex turn calls: text and a target state in, audio and the read-back state out. The 80 ms heartbeat does not call it.

```bash
cargo test --manifest-path tools/symphony/voice-v2/Cargo.toml
cargo run --manifest-path tools/symphony/voice-v2/Cargo.toml -- \
  loop --wav tone.wav --text "Hello." --emotion embarrassment \
  --voice tools/symphony/voice-v2/voices/synthetic.toml --out /tmp/v2
```

`--emotion` accepts all 24 Cowen categories on `--track prosody` (the default). The 12 Nature Human Behaviour names are a confidence tier, `reliable`. The other 12, including embarrassment, are `extended` and still legal on a spoken word. `--track burst` uses the same 24. Neutral is the speaker baseline. Breath is physiology.

Older names, and only these: `seductive` → desire, `tender` → adoration, `calm` → neutral, `laughter` → burst amusement.

The voice file is TOML. The baseline in that file is not overwritten by the feeling. `voices/synthetic.toml` is a test tone, not a person.

## What this crate reuses

- `galion-articulation` / praatfan, in process, for throat and mouth on the 80 ms tick.
- `refprep`, only when `GALION_REFPREP` points at that binary.
- `cbx speak` from `tools/cloning-v2/pipeline/cbx-v2`, only when `chatterbox.bin`, a voice, and a local fp32 snapshot are already on disk. `HF_HUB_OFFLINE=1` is set. Nothing is downloaded. `--ep` is `cpu` unless a GPU is visible (`--device cpu` forces CPU). Pitch sent to that CLI is clamped to ±2 semitones, which is the limit that CLI documents.

## What is stubbed

Waiting on the features agent:

- eGeMAPS names, nasality, breathiness, breath stage
- TractPrint v2 speaker/emotion split
- `chatterbox.params_json` with schema `galion-voice-v2-chatterbox-params` (decode JSON → Chatterbox params). Until that file exists, pitch comes from a pipeline default, not a cited coordinate.
- Real-life isolate in front of `refprep` (DeepFilterNet, Demucs, pyannote, Silero VAD). Unset `GALION_REFPREP` is a passthrough.

Waiting on the spec agent (PR #189): cited z against the speaker's own neutral. This crate does not invent those targets.

The synthetic vowel under-applies pitch so the correction pass can be tested without a clone. That undershoot is not a model of Chatterbox. Region prototypes are pipeline defaults, not Cowen coordinates.

No voice audio is committed. Tests use the vowel above.
