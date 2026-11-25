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

`MouthControls::from_breath_frame` reads a `horbm_breath::Frame`. The four articulation fields are that frame's. They are compared with `horbm_breath::controls` at the rest drive so a rest speech frame leaves the PCM alone. This crate does not recompute Rd.

| Control | What the knob does | Where the number comes from |
|---|---|---|
| Tension | Raises spectral tilt, up to `TENSION_DB` (9 dB). | How far the frame's open quotient sits below the rest-speech quotient. |
| Breathiness | Lowers that tilt, up to `BREATHINESS_DB` (8 dB). | The frame's breathiness above that rest frame. |
| Openness | Moves F1 and F2 (`F1_SHIFT` 0.70, `F2_SHIFT` 0.30) and lowers Q (`OPEN_Q_SCALE` 1.5). | The frame's mouth openness relative to the rest-speech mouth. `Lungs` starts that target at 0.5. |
| Nasal | Dips 250 Hz and 1.5 kHz, depth up to `NASAL_DEPTH_DB` (18 dB). | The frame's nasal coupling above the rest-speech leak. |
| Breath noise | Noise centred at `BREATH_CENTER_HZ` (700 Hz). | Exhale phase and outward flow on the same frame. |

## Overlap with horbm-breath

PR 278 is on main. `horbm-breath` is the only law for Fant open quotient, breathiness, nasal coupling, and mouth openness. This filter renders those fields and the lungs sample. It does not own a second glottal model, and `Lungs::write_inhale` remains the inhale.

| Concern | Owner |
|---|---|
| Lung phase, volume, flow, pressure, phrase pauses, inhale noise | `horbm-breath` |
| Fant open quotient, breathiness, nasal coupling, mouth openness | `horbm-breath` |
| Spectral tilt, F1/F2 shift, nasal dips, exhale noise on Piper and flite | `galion-source-filter`, from that frame |

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

`cargo test -p galion-source-filter` — 20 tests (9 in the crate, 10 probes, 1 alloc counter).

`cargo test -p galion-ml-voice --features speech` — the new stack test is `samantha_source_filter_sits_between_tts_and_the_playback_buffer`. Neutral Samantha matches the engine buffer. Tension moves the Piper-class buffer. Openness moves the flite buffer. Jarvis with every control set matches the engine buffer. The existing clone landing test still requires the playback buffer to equal the packet.

Probes are generated in the test. Nothing is written as audio.

```shell
cargo test -p galion-source-filter
cargo test -p galion-ml-voice --features speech
cargo clippy -p galion-source-filter --all-targets -- -D warnings
cargo clippy -p galion-ml-voice --features speech --all-targets -- -D warnings
bash scripts/check-locks.sh
```
