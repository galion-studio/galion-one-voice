# F — duplex loop, live (S28)

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S28. The S26 loop, on real ports, still one clock. |
| Crate | `horbm/crates/horbm-duplex`. Same member of the `horbm` workspace. |
| Clock | 80 ms, 16 kHz, 1280 samples. `Duplex::tick` is still the only step. |
| Persona | Samantha. Aria is parked (`ARIA_ACTIVE = false`) and `open_persona("aria")` refuses her. |
| Devices | `horbm-duplex-live`, feature `live`, off by default. cpal. On Linux the host is ALSA, chosen inside `galion-sam-audio-io`. |
| Network | The binary binds nothing. The brain is built with no research ask. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not added. |

The branch also merges `main` at the emotion-core commit so this crate can call `EmotionCore::step`. That crate was already merged (PR 263). This page does not change it.

## Adapters

| Port | What runs | Feature |
|---|---|---|
| Recognizer | `AsrRecognizer<A: galion_sam_asr::Asr>`. `finish` when the ear frame is quiet, `cancel` on barge-in. Same split as StreamingEar. | `vosk`, off. CI does not build it. Contract tests use `FakeAsr` and a missing model directory. |
| Recognizer, feature off | `SilentRecognizer`. Frames are accepted. No text is invented. | default |
| Emotion | `CoreAffect` calls `galion_emotion_core::EmotionCore::step`. Samantha's id, the published rest point. The baseline bits do not move. | always |
| Prosody / ear | `ProsodyEar`. RMS against the −48 dBFS clamp, pitch by autocorrelation, 50–500 Hz. | `prosody`, **on** by default. `Duplex::set_prosody(false)` and `--no-prosody` turn it off. `--no-default-features` compiles it out. |
| Persona | `SamanthaPersona`. Brevity 0.55, warmth 0.8, miss `I don't know.` | always |
| Mouth | `NativeMouth` over `horbm_voice::FakeMouth`. Eight generated frames. Piper is not linked. | always |
| Breath | `SpeakingBreath`. Exhale while the me band speaks. The breath crate is not on this branch. | stand-in |

The virtual-clock test in `tests/duplex.rs` still drives its own scripted recognizer, constant affect, exhale cue, and tone mouth. That run is the CI gate. `tests/adapters.rs` checks the real adapters on synthetic sines and tones. No voice audio is committed.

## Live binary

`cargo run --manifest-path horbm/Cargo.toml -p horbm-duplex --features live --bin horbm-duplex-live`

Four 20 ms capture slices become one tick. Both bands step on that tick. The mouth frame is resampled to the speaker rate by `galion-sam-audio-io`. A barge-in calls `Playback::barge_in`.

Stdout, one line per voice onset and per barge-in. Times are the frame clock (one frame is 80 ms of audio), not a wall-clock probe:

```text
duplex s28 event=voice clock=frame eager_to_voice_ms=160 barge_to_silence_ms=none backchannel_cancels=0
duplex s28 event=barge clock=frame eager_to_voice_ms=160 barge_to_silence_ms=160 backchannel_cancels=0
```

`--json` prints the same three fields. `--self-test` runs the closed script on a synthetic square wave and does not open a device.

## Linux

CI stays the Ubuntu job `voice-stack horbm router`. It tests `horbm-duplex` with default features. It does not pass `--features vosk` or `--features live`. No Windows workflow was added.

This crate has no hard-coded `.exe`, drive letter, or device path. The ALSA host id lives in `voice-stack/audio-io`, behind `cfg(target_os = "linux")`. That file already existed and was not edited. On Windows the same crate selects WASAPI. This PR does not run that path.

## Still a stand-in

- Breath. `SpeakingBreath` is not `plan_speech`.
- Recognizer, unless `--features vosk`. Even then, `FakeAsr` is the test double. A Vosk model is not in the repo. A missing model is an error.
- The virtual-clock gate's scripted recognizer, fixed affect, and tone mouth. They keep the S26 measured line stable.
- The native mouth is the in-process tone, not Piper and not a clone.
- Aria. The name is reserved. She is not loaded.
- The persona TOML loader (open PR) is not linked. Samantha's style numbers are the ones the S26 gate already required.
