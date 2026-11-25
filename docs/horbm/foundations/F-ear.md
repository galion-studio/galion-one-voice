# F-ear — HORBM's native ear

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S18, four seats: VAD and endpointer, streaming ASR, full-duplex, tests and this page |
| Written | 2026-11-24. Synthetic tones and noise only. No recording, no model download. |
| Repo | `galion-studio/galion-one-voice`, on the voice-loop branch (PR #264). |
| This change | `horbm/crates/horbm-voice` ear. The 80 ms tick owns listening. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this checkout and was not created. |

The brain stays the center. Each conductor frame hands the ear the mouth frame already played, then polls the ear, then on a real transcript calls `horbm::Brain::turn`, then pulls one mouth frame. The clock is the virtual frame counter. This tick is not `Heartbeat::step`.

The default ear is energy, with no model. The gate is the orchestrator's: floor −60 dBFS, +12 dB, −48 dB clamp, 60 ms onset, floor adapting on unvoiced audio. Inside one 80 ms frame those are four 20 ms slices. Onset is reported on the frame that crosses 60 ms. Turn-end is the first silent frame (80 ms hang), so the offset is that frame. A boundary that falls mid-frame is reported on that frame or the next, which is the one-frame window. Empty text is not a line. The loop does not invent one, and it does not call the brain.

Full duplex stays on while she speaks. Her playback, or a scaled copy of it, is echo: the residual is what the gate scores, and echo is not barge-in. Other energy is the user. One or two frames (a short "yeah", 80–160 ms) emit a backchannel and do not cancel. The third consecutive frame yields on that same frame, 160 ms from the first overlap, inside the 200 ms lock. A listed backchannel word still does not cancel, including a long "yeah" from a transcript.

Streaming recognition is `--features vosk`. `StreamingEar` wraps `galion_sam_asr::Asr` and closes the utterance when the energy gate ends the turn. `FakeAsr` is the test double. `VoskAsr` still dlopens libvosk and is not in the default tree. Piper stays `--features piper`. `ort` is not a dependency. The socket's 640 ms hang is the same `EnergyGate` with a longer hang, so a sentence is not split; Silero stays the out-of-process `vad-ear`.

## Archive

Nothing was moved. `galion_sam_orch::EnergyVad` is still the orchestrator's own listen loop:

| Still referenced by | Why it stays |
|---|---|
| `voice-stack/pipeline-orchestrator/src/samantha_traits.rs` | The type, the `Vad` impl, and the gate tests |
| `voice-stack/pipeline-orchestrator/src/lib.rs` | Re-export |
| `voice-stack/pipeline-orchestrator/src/demo.rs` | `measure_barge` and the scripted turn |
| `voice-stack/pipeline-orchestrator/src/pipeline.rs` | The listen task takes `impl Vad` |
| `voice-stack/pipeline-orchestrator/tests/e2e.rs` | End-to-end fake loop |
| `voice-stack/pipeline-orchestrator/tests/router_pipeline.rs` | Router loop |

`samantha-local` no longer uses that type for the mic. It uses `horbm_voice::EnergyGate`. It still uses the orchestrator for the gated mouth test (`FakeTts`, `WaitGate`), which is not the endpointer. Archive means move aside, never delete. A later pass can move the orchestrator loop once those callers are gone.

## Tests

Synthetic only, in `horbm/crates/horbm-voice/tests/ear.rs`: onset and offset on the boundary frame; a mid-frame boundary within one frame; barge-in at 160 ms (lock 200); two frames of tone, the short "yeah", never cancel; six frames of her own playback, and a half-scale copy, are not barge-in; the same samples produce the same events. `tests/tree.rs` runs `cargo tree` for default `horbm-voice` and default `samantha-local-server` and rejects piper, vosk, and ort.
