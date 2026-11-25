<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S01 audio I/O

Package `galion-sam-audio-io`, directory `voice-stack/audio-io`. CPU only.
This directory is its own Cargo workspace. It is not a member of the repo
root workspace. The binary is `galion-sam-audio-io`.

The crate does not run VAD, ASR, TTS, or HORBM. It feeds the rates those
parts already use:

| Path | Rate | Frame | Same as |
| --- | --- | --- | --- |
| Capture for VAD/ASR | 16 kHz mono PCM16 | 20 ms, 320 samples | `galion-asr-vosk` `FRAME_SAMPLES`, one-dot page `VAD_FRAME` |
| Heartbeat mic (`AudioIn::fill`) | 24 kHz mono f32 | 80 ms, 1920 samples | `MicSource` and the contract `AudioIn` |
| Playback ring (`AudioOut`) | 24 kHz mono f32 | 80 ms, 1920 samples | `galion-heartbeat` `SAMPLE_RATE` / `FRAME_SAMPLES` |
| Piper blocks | 22050 Hz PCM16 | resampled on push | `archive/2026-11-24/crates/galion-v2-live` `PIPER_HZ` |

Downmix is an average across channels, as in `archive/2026-11-24/crates/galion-v2-live`'s `to_mono`.
Capture resampling is a streaming box average, the same idea as `onCapChunk`
in `crates/galion-console/assets/v1-screen.html`, with the remainder kept
across short device periods. Playback resampling is linear, the same shape
as `archive/2026-11-24/crates/galion-v2-live`'s `resample`.

`Playback::barge_in` drops every sample queued at that call. The render
period that sees it writes silence. Samples pushed after `barge_in` returns
stay in the ring. `Playback::set_gain(0.35)` is the heartbeat duck
(`DUCK_GAIN` in `galion-heartbeat`'s mixer) and does not flush. Acoustic
silence still waits until the sound card runs the next period. That wait
was not measured here.

Windows uses WASAPI shared mode (`cpal` host `Wasapi`). Linux uses ALSA
(`cpal` host `Alsa`). PulseAudio and PipeWire are not a separate cpal host.
They are reached when the ALSA `default` device is the Pulse or PipeWire
plugin. `list` reports whether a plugin file is on disk. It does not open it.

No reference clip is read. `GALION_SAMANTHA_REF` is not consulted.
`GALION_AUDIO_IN` and `GALION_AUDIO_OUT` are optional device-name substrings.

## Linux

Install the ALSA headers once (Debian/Ubuntu names):

```bash
sudo apt install build-essential pkg-config libasound2-dev
cd voice-stack/audio-io
cargo test
cargo run --release -- list
cargo run --release -- measure
```

`measure` prints JSON on stdout. With no sound card, ALSA writes its own
errors to stderr and the `live` object has `open_error` set. The `software`
object is the in-process loopback and does not need a device.

`play` needs a real output device:

```bash
cargo run --release -- play /path/to/pcm16.wav
```

## Windows

From a Visual Studio or GNU Rust toolchain, in `voice-stack\audio-io`:

```bat
cargo test
cargo run --release -- list
cargo run --release -- measure
```

WASAPI shared mode is the host. No ASIO SDK. The Windows target was not
compiled on the Linux machine that produced the numbers below.

## Measured on this build

One Linux machine, `rustc 1.85.0`, release build, no `/dev/snd`, cpal host
`ALSA`, `list` reported 0 devices, no Pulse/PipeWire plugin file. Three
runs of `cargo +1.85.0 run --release -- measure`. Live capture-to-callback
was not measured: open failed with `The requested device is no longer
available. For example, it has been unplugged.`

Software loopback, fake 48 kHz card, 480-sample (10 ms) period. The impulse
was capture sample 0 of the first 16 kHz frame. That frame's virtual ready
time was 20.0000 ms on every run (the 20 ms frame, not a processing stall).
One second of 48 kHz audio became 50 frames. The flush dropped 4000 queued
samples and the render that observed it was silence.

Wall time, one shot per run, not a percentile:

| Run | 1 s capture | 1 s playback | one 80 ms frame | flush honor |
| --- | --- | --- | --- | --- |
| 1 | 91 µs | 166 µs | 14 µs | 365 ns |
| 2 | 87 µs | 163 µs | 13 µs | 538 ns |
| 3 | 88 µs | 170 µs | 13 µs | 322 ns |

`cargo +1.85.0 test` and `cargo +1.85.0 test --release`: 28 passed, 0 failed.

## What this crate does not do

- It does not open a mic on a machine with no card, and it does not invent a hardware latency for that case.
- It does not implement VAD, endpointing, Vosk, Piper, Chatterbox, the one-dot page, or HORBM.
- The systemd stage unit is unchanged. This crate is not installed there.
