# F — Samantha mouth, source-filter

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S41. Mouth and expressions, MnE V2. One stage, after TTS, before the playback buffer. |
| Written | 2026-11-24. Generated probes only. No voice audio, no recording, no weight. |
| Base | `main` `02f8ac909054da06f3f7f78405e0f8394efee896`. |
| Crate | `crates/galion-source-filter`. The speech stack calls it from `galion-ml-voice` (`feature = "speech"`). |
| DSP | `biquad` 0.6.0, MIT OR Apache-2.0. Direct Form 1. This crate is glue. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this checkout and was not created. |

Piper or flite has already spoken. This stage does not synthesise a word and it does not replace the glottal pulse with an LF waveform. It shapes the PCM that engine returned. `SpeechStack` does that on each 80 ms frame, then `SpeechStream` appends the frame to the playback buffer. Clone PCM is not passed through: the landed `.gqvoice` packet still has to equal that buffer. `applies_to` is exact `samantha`. Jarvis, and any other id, skips the stage. Piper and flite start as Samantha until `set_persona` says otherwise. With neutral controls the stage returns without writing, so an unset moment does not change a sample.

## Baseline and moment

`TractBaseline` is the steady voice. `TractBaseline::SAMANTHA` anchors F1 at 431.512 Hz and F2 at 1857.581 Hz, the Burg medians already written in `ARTICULATION-LAYER.md`. They are the frequencies an openness delta moves. They are not a new measurement. Bandwidths are not taken from that note.

`MouthControls` is what the filters render. `MouthControls::from_moment` reads a `VoiceBaseline` and a `MomentState` and does not write either, and it does not turn those axes into glottal or mouth targets. The emotion-core stand-in's persona bytes are not the product persona, so the filter does not look at them. The stack's persona gate is the one that decides whether Samantha is speaking.

The filter knobs are not a z against this speaker's own neutral. The control law for open quotient, breathiness, nasal coupling, and mouth openness is `horbm-breath` (PR 278, not this crate).

| Control | What the knob does | Where the number comes from |
|---|---|---|
| Tension | Raises spectral tilt, up to `TENSION_DB` (9 dB) at full scale. Low shelf down, high shelf up, corner 1 kHz. | The caller. `from_moment` leaves it at 0. |
| Breathiness | Lowers that same tilt, up to `BREATHINESS_DB` (8 dB). It does not add noise. | The caller, from `horbm-breath`'s breathiness once that frame is wired. `from_moment` leaves it at 0. |
| Openness | Moves F1 by up to `F1_SHIFT` (0.70) and F2 by up to `F2_SHIFT` (0.30), and lowers Q as the tract opens (`OPEN_Q_SCALE` 1.5). Depth is `FORMANT_DB` (12 dB) once openness leaves rest. | The caller, from that crate's mouth openness. `from_moment` leaves it at 0. |
| Nasal | Antiresonances at 250 Hz and 1.5 kHz, depth up to `NASAL_DEPTH_DB` (18 dB). | The caller's velum, copied through. The health mix stays in `horbm-breath`. |
| Breath noise | Band-limited noise, centre `BREATH_CENTER_HZ` (700 Hz), mixed on the way out. | Breath phase `exhale` only. Speech, inhale, hold, and rest add none. |

## What overlaps PR 278

PR 278 (`horbm-breath`, branch `cursor/horbm-breath-s20-5330`) adds glottal open quotient (Fant Rd), breathiness, nasal coupling, and mouth openness on the lungs frame. It does not filter speech PCM. This crate does not recompute those four. One source of truth for the numbers: `horbm-breath`. One source of truth for shaping Piper and flite: this filter.

| Concern | Owner |
|---|---|
| Lung phase, volume, flow, pressure, phrase pauses, inhale noise | `horbm-breath` (already on main) |
| Fant open quotient, breathiness from Rd, nasal coupling, mouth openness | `horbm-breath` when PR 278 lands |
| Spectral tilt, F1/F2 shift, nasal dips, exhale noise on TTS PCM | `galion-source-filter` |

`from_moment` still maps an exhale onto breath noise. That is the contract `horbm-breath` already names. Inhale noise stays in `horbm-breath` (`Lungs::write_inhale`). This filter does not write a second inhale.

A rest moment, a closed velum, and any phase but exhale produce `MouthControls::neutral()`.

## Measured cost

One machine. Linux 6.12.113+, 4 cores, `/proc/cpuinfo` model name `Intel(R) Xeon(R) Processor` (that string has no SKU). rustc 1.85.0 (4d91de4e4 2025-02-17). Wall time is `std::time::Instant` on one thread. A frame is 80 ms at 24 kHz, 1920 samples. The timed region is `set_controls` plus `process`, 400 frames, after a 4-frame warmup. The control value is passed through `black_box` so the retune stays inside the frame. Real-time factor here is processing time divided by audio duration. Below 1, the frame finishes inside its own 80 ms.

Release (`cargo test -p galion-source-filter --release`), five runs. Full controls are tension 1, openness 1, nasal 1, breath noise 1.

| Path | ns/frame | RTF | Times inside the 80 ms frame |
|---|---:|---:|---:|
| Neutral, early return. Median (range 7–11 ns) | 10 | 1.25×10⁻⁷ | 8.0×10⁶ |
| Full controls. Median of 32 766, 32 792, 32 833, 33 312, 33 734 | 32 833 | 0.000410 | 2 437 |

Unoptimized, one run, same probe: neutral 75 ns/frame (RTF 9.38×10⁻⁷); full 316 116 ns/frame (RTF 0.00395, 253 times inside the frame). The alloc budget on `set_moment` plus `process` is 0. `tests/zero_alloc.rs` counts allocations and the count stayed 0 across 200 frames.

## Tests

`cargo test -p galion-source-filter` — 19 tests (8 in the crate, 10 probes, 1 alloc counter).

`cargo test -p galion-ml-voice --features speech` — the new stack test is `samantha_source_filter_sits_between_tts_and_the_playback_buffer`. Neutral Samantha matches the engine buffer. Tension moves the Piper-class buffer. Openness moves the flite buffer. Jarvis with every control set matches the engine buffer. The existing clone landing test still requires the playback buffer to equal the packet.

Probes are generated in the test. Nothing is written as audio.

```shell
cargo test -p galion-source-filter
cargo test -p galion-ml-voice --features speech
cargo clippy -p galion-source-filter --all-targets -- -D warnings
cargo clippy -p galion-ml-voice --features speech --all-targets -- -D warnings
bash scripts/check-locks.sh
```
