# Shared expression-diverse conditioning (experimental / OFF by default)

## Why
The deployed Turbo speech encoder's `audio_features` branch crops 16 kHz audio to **96000 samples (6 s)**. Its prompt tokens and speaker features crop 24 kHz audio to 240000 samples (10 s). Speaker embeddings use global ReduceMean. Thus later expressive moments can be absent from conditioning even with a complete recording. This stage does not claim to remove graph pooling or to reconstruct unsupported human emotion labels.

## Build on this Cursor computer
```
nice -n 10 cargo test --offline -j 2 --manifest-path pipeline/emotion-blend-ref/Cargo.toml
nice -n 10 cargo build --release --offline -j 2 --manifest-path pipeline/emotion-blend-ref/Cargo.toml
PATH="$PWD/pipeline/bin:$PATH" LIBRARY_PATH="$PWD/pipeline/lib:${LIBRARY_PATH:-}" LD_LIBRARY_PATH="$PWD/pipeline/lib:${LD_LIBRARY_PATH:-}" nice -n 10 cargo build --release --offline -j 2 --manifest-path pipeline/cbx-v2-timbre/Cargo.toml --bin cbx-blend
```
The new bin is **cbx-blend**, not cbx. The existing production cbx executable remains byte-identical. No production voice/config is changed.

## Shared entry point for every newly cloned voice
`pipeline/bin/cbx-add-voice --name NEW --voice-wav ORIGINAL.wav --dtype fp32`

Default path directly executes the existing cbx binary's original voice-add path. No segmentation/scoring/tensor replacement is called.

Explicit opt-in:
```
pipeline/bin/cbx-add-voice --emotion-blend --blend-work-dir NEW_RUN_DIR \
  --name own-PERSONA-v2-blend-TAG --voice-wav ORIGINAL.wav --dtype fp32
```
The wrapper owns `/tmp/cbx-render.lock` for encoding; do not wrap this entry point in another acquisition of that same non-reentrant lock. It uses nice 10 / intra+inter 2. Speak candidates using the existing wrapper, locking each render as usual:
```
flock /tmp/cbx-render.lock pipeline/bin/cbx-speak-voice --voice own-PERSONA-v2-blend-TAG --dtype fp32 --temperature 0 --text "Test" --out-wav NEW.wav
```

Per-voice config: `pipeline/voice-defaults/NEW.json` with `{"emotion_blend":{"enabled":true}}`; or `--blend-config CONFIG.json`. `--no-emotion-blend` overrides config. `emotion-blend.example.json` is an example only and matches no deployed voice. Opt-in requires fresh `own-<persona>-v2-blend-<tag>` names. Existing candidate/deployed profiles cannot be overwritten.

## Algorithm (Rust-first; same code/settings for every voice)
1. Read-only original -> equal-duration ~2.5s windows, preserving native samples.
2. Rust galion-emotion **fp32**, two threads, sequential inference gives SER probabilities + A/V/D + acoustic proxies.
3. Exclude windows with <10% voiced frames or below -55 dBFS. No eligible speech -> fail closed.
4. Find the most distant pair in SER/AVD/prosody space, then the window farthest from the selected set. No highest-arousal, voice-name, or desired-label heuristic. A medoid-first version was rejected because its middle-tone anchor crowded out minority styles.
5. Chronological 2s excerpts from up to three selected windows -> <=6s conditioning montage. Edge-only 5ms fades; **no per-segment loudness normalization**. All intermediate WAVs, score JSON and timestamps retained.
6. Sequentially encode original and montage in cbx-blend. Replace only `audio_features` + `audio_tokens`; retain original `speaker_embeddings` + `speaker_features` byte-for-byte. Tensor replacement has a unit test. Voice storage uses exclusive create, never deployment overwrite.

This is a crop-aware conditioning mitigation, not a guaranteed quality win. The remaining ONNX mean pooling and greedy sampling are unchanged. Promote only after per-voice 12-line WER <=1.8% and identity >0.7057, plus measured blend/prosody improvement; no promotion is performed here.

## Tests
- 6 Rust builder unit tests: diversity, minority-style regression, deterministic unique selection, short/empty inputs, amplitude-dynamics preservation, missing-score rejection.
- 1 cbx Rust test: exact identity-tensor byte preservation under split conditioning.
- 6 wrapper dispatch/safety tests: untouched off-path args/binary, flag/config opt-in, explicit off override, protected names and existing profile refusal.
`python3 pipeline/emotion-blend-ref/tests/test_wrapper.py -v`

Measured analysis and run logs: `emotion-blend/A-global/PROFILE.md`, `profile.json`, `encoder-graph-evidence.json`, `*-test.log`, protected hashes. No TTS render or deployment change by A-global.
