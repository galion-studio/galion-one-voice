# S22 — speech recognition

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S22, speech recognition |
| Written | 2026-11-24. No model download, no paid API, no voice audio committed. |
| Plan | R4 §11 and §4.3 on draft PR #207 (`docs/horbm/research/R4-cpu-realtime-integration.md`, unmerged). The ear frame matches open PR #272. The conductor on `main` is the clock this port lines up with. |
| Crate | `horbm/crates/horbm-asr`. Member of the `horbm` workspace, which the root workspace excludes. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this checkout and was not created. |

## What this crate is

A small port, [`SpeechRecognizer`](../../horbm/crates/horbm-asr/src/lib.rs), in front of one engine.

The host pushes one ear frame, reads partials, and either finishes the utterance or cancels it. Cancel is the barge-in hook. It drops the open hypothesis and does not emit a final. The conductor still owns the floor. This crate does not call `Conductor::tick` and it does not run inside the heartbeat frame step.

## Frame

| | Ear, this crate | Mouth, heartbeat |
|---|---|---|
| Period | 80 ms (`LOCKS.toml` `clock.frame_ms`) | 80 ms |
| Rate | 16 kHz | 24 kHz |
| Samples | 1280 | 1920 |

1280 is `galion-sam-asr` `HEARTBEAT_SAMPLES` and the native ear's frame (PR #272). A 1920-sample mouth frame is an error. A 320-sample Vosk slice is an error. The engine, when the feature is on, still splits a legal frame into 20 ms pieces inside `galion-sam-asr`. This crate does not redo that.

## Stand-in

[`StandIn`](../../horbm/crates/horbm-asr/src/standin.rs) is the default engine. It reveals one scripted word per accepted frame. It does not read sample values. The same frame count always yields the same partials, the same word spans, and the same final. The final contains only the words those frames revealed. No weights, no audio file.

## Real engine

`--features vosk` is off by default, including CI's default `cargo test --workspace`.

The feature calls `voice-stack/asr` (`galion-sam-asr`) as-is. That crate already streams 16 kHz PCM, emits partials and finals, and drops `[unk]`. This crate maps those events onto the port and converts word times from seconds to milliseconds. It does not reimplement the decoder and it does not bind `vosk_gpu_init`.

`open_vosk` on a directory without `am/final.mdl` and `conf/model.conf` returns an error. The error is not a transcript. Model bytes are not in git. Paths stay in `galion-sam-asr` (`GALION_VOSK_MODEL`, `GALION_VOSK_LIB`). This crate hard-codes neither a library name with `.exe` nor a drive path.

whisper-rs and sherpa-onnx are not wired. whisper.cpp is a GGML-family runtime. sherpa-onnx links ONNX Runtime. `LOCKS.toml` sets `ggml_container_allowed = false` and lists `ggml` and `ort` in `forbidden_runtimes`. R4 §14 says domain 10's whisper.cpp advice conflicts with that lock and is not followed. `scripts/check-locks.sh` requires those bans to stay. Editing the lock was not required, because the in-tree CPU engine already exists.

## Barge-in

`audio_ms` is how much audio the open utterance has accepted. `within_barge_window` is true for 1 through 200 ms, the lock. Two frames are 160 ms and inside. Three frames are 240 ms and outside. `cancel` still runs when asked after the window; the host decides. After cancel, `finish` is empty and the next frame starts a new utterance at 0.

## CPU

`place()` is `ComputePlace::Cpu`. The enum is non-exhaustive so a later backend can add a variant. This crate does not stat a device node, does not read a GPU environment variable, and does not follow the boot profile's `api` line. R4 §7: a GPU does not move the voice clock.

## Linux

CI for this crate is the Ubuntu job in `.github/workflows/voice-stack-horbm-router.yml`. No Windows workflow was added. The new crate has no `cfg(target_os)` and no Linux-only path. The optional engine's `dlopen` / `LoadLibrary` split already lives in `voice-stack/asr/src/vosk_ffi.rs` and was not changed.

## Left where it was

`voice-stack/asr`, `crates/galion-asr-vosk`, and `crates/galion-conductor` were not edited. Nothing was deleted.
