# F — duplex echo (S33)

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S33. Take Samantha's playback out of the microphone before the ear. |
| Crate | `horbm/crates/horbm-duplex`. Same member of the `horbm` workspace. |
| Clock | 80 ms, 16 kHz, 1280 samples. `Duplex::tick` is still the only step. |
| Persona | Samantha. Aria stays parked. |
| Canceller | `fdaf-aec` 0.1.0, MIT. Frequency-domain adaptive filter, used as published. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not added. |

The microphone frame is cleaned, then prosody, the recognizer, and the rest of the tick see that frame. The far end is the mouth frame noted on the previous tick: the live loop plays at the end of a tick, and the next capture is what the mic heard. Five 256-sample blocks fill one 80 ms frame. The filter is 512 samples, 32 ms.

`aec` is on by default. `Duplex::set_aec(false)` and `--no-aec` are the bypass. `--no-default-features` does not link the crate. The virtual-clock gate and `--self-test` bypass it. Those scripts are not an acoustic path, and the mouth square matches the scripted mic.

Prosody stays on by default. `Duplex::set_prosody(false)` and `--no-prosody` still turn it off.

## Why this crate

`sonora-aec3` needs rustc 1.91. `aec3` 0.1 through 0.4 needs `nalgebra` 0.34 (rustc 1.87) and, from 0.1.6, `let` chains that rustc 1.85 rejects. This workspace is rustc 1.85. `fdaf-aec` 0.1.0 is pure Rust, MIT, and builds on that compiler. WebRTC's C++ wrapper was not linked.

## Linux

CI stays the Ubuntu job `voice-stack horbm router`. It tests `horbm-duplex` with default features, so the echo tests run there. No Windows workflow was added. This crate has no hard-coded `.exe`, drive letter, or device path. The ALSA host id is still the existing `cfg(target_os = "linux")` in `voice-stack/audio-io`, which was not edited.

The synthetic room is a 160-sample delay, gain 0.08, and taps at 48 and 96 samples. Tone, noise, and a mix. No voice audio is committed.

Measured on rustc 1.85.0, release build, the same signals the tests lock:

| Signal | ERLE | Near-end delta | False barge-ins | Barge-in | CPU / 80 ms frame |
|---|---|---|---|---|---|
| tone | 47.81 dB | −0.37 dB | 0 | 160 ms | 30.6 µs |
| noise | 24.63 dB | +0.12 dB | 0 | 160 ms | 30.8 µs |
| mix | 26.87 dB | −0.28 dB | 0 | 160 ms | 27.2 µs |

With the canceller bypassed, the same echo barges. The debug test build spends about 1.7 ms on a frame and still asserts under 20 ms.
