# Heartbeat wire

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb · 2026-11-03 · t287.

The spelling is **Galion**. The product is Galion Voice.

A conversation is one heartbeat. Clone, persona, and the one dot sit on that step.

## Clock

The period is `galion_heartbeat::FRAME_MS`. It is 80. The sample rate is `SAMPLE_RATE`, 24 kHz. One frame is `FRAME_SAMPLES`, 1920 mono samples.

`LOCKS.toml` already records that period. This wire does not edit it.

Bands, in order: **you**, **me**, **feeling**. Three names. `galion_heartbeat::symphony::BANDS`.

Sectors, in order: **clone**, **persona**, **one-dot**. `Sector::ALL`.

## The step

The host calls `Heartbeat::step`, then `SymphonyWire::step`, once per frame. Both return before the host sleeps, paints, or talks to a worker.

`SymphonyWire::step` keeps its state in the struct. The clone sector drains at most one frame from `NanoPcmRing` when that ring already holds 1920 samples. Otherwise it copies at most one me-band frame from a slot the caller already filled. It applies the previous frame's feeling sample. It counts persona holds. It writes the one-dot mark.

`me_emit` is the question the host asks before it pops a reply queue. It does not change wire state. A true answer means one frame may move. A false answer means the queue waits.

## Hot path

Inside either `step`:

- no lock, channel, sleep, clock read, file, or socket
- no heap allocation
- a late frame is a pause

A pause leaves the me buffer untouched, does not pop the Nano ring, leaves the persona hold where it was, and keeps the one-dot mark. The next frame takes one frame. It does not skip ahead through a queued tail, and it does not play two frames to make up the late one.

A yield sets `release_slot`. The host drops the rest of the queue after `step` returns, including `NanoPcmRing::release`. The drop is outside the step.

A me buffer whose length is not 1920 is a pause. A short write would be a glitch.

Persona id may change only after `PERSONA_HOLD_FRAMES` (8) sensed samples at or above `PERSONA_HOLD_E` (0.65). One frame never flips it. The feeling sample decided on frame *t* is applied on frame *t+1*. Without SENSE the sample is dropped and the hold clears on the next frame that is not a pause.

The one-dot mark is `Speaking` when this frame copied me samples, `Listening` when you are voiced and me is quiet, `Idle` otherwise. Speak wins. A pause holds the previous mark.

The budget checked by the harness is `STEP_BUDGET_US` (800). A step whose warm p99 or max reaches that budget fails. The number is the harness ceiling. It is not a second clock.

## Off the step

These stay outside both calls. They may allocate. They may block. They must not be moved inside `step`.

| Work | Where it runs |
|---|---|
| Chatterbox model call | out of process, off the frame. Still unwired. |
| `A21RingFeed::poll` when `GALION_SPEECH=a21` | host, before `Heartbeat::step`. Reads Piper stdout, resamples the native rate to 24 kHz, pushes the new samples. This is the ring feed. |
| `CloneWorker::poll_pcm` | not the feed. Protocol stub bytes are not a voice. |
| Persona reply text (`Floor::say`) | turn path, off the frame |
| Paint, JSON, HTTP, glyph history | after the step, best effort |

`clone_worker::hot_frame_samples` is the drain size. It equals `FRAME_SAMPLES`. It does not start the child. `CLONE_FRAME_MS` is `FRAME_MS`.

## Nano ring

`NanoPcmRing` is a fixed buffer of `NANO_RING_FRAMES` (4) heartbeat frames. No heap. No lock. No syscall inside `step`.

When `GALION_SPEECH=a21`, `A21RingFeed` is the feed. Piper writes native-rate s16le (the ljspeech pack is 22050 Hz). `poll` resamples that to 24 kHz and pushes it. `step` then takes 1920 samples or nothing. A remainder shorter than one frame stays queued. A full ring drops new samples. It does not grow a catch-up queue.

`GALION_CLONE_STUB_FRAMES` is not this feed. Those bytes are a protocol tone. They are not Piper and they are not a voice.

The me slot is used for that tick only when the ring did not supply a full frame. `nano_frame` is true only for a ring frame.

Host order, once per frame:

1. `A21RingFeed::poll(&mut ring)` when `GALION_SPEECH=a21`. Skip this otherwise.
2. `Heartbeat::step`.
3. `SymphonyWire::step(..., Some(&mut ring))`.
4. If `release_slot`, `ring.release()` after `step` returns.

## Hard drain, stub model

| Piece | Kind | Meaning |
|---|---|---|
| Ring drain on `SymphonyWire::step` | hard | One 1920-sample frame at 24 kHz. `allocs=0`. Non-blocking. Late frame is a pause. No catch-up. |
| Piper feed when `GALION_SPEECH=a21` | hard, measured | `A21RingFeed` resamples real Piper PCM into the ring. `piper_measured=true` only for that run. |
| `GALION_CLONE_STUB_FRAMES` | not a feed | Protocol tone. Not Piper. Not a voice. Not `piper_measured`. |
| Chatterbox `generate` | BLOCKED | The worker does not import torch and does not call the model. |

Piper remains the fallback when the clone slot is absent or the ring has no full frame.

The persona floor uses the same hold count (`HOLD_FRAMES`, `HOLD_E`, `RELEASE_E`). The render crate stays free of a heartbeat dependency. `sectors_match_heartbeat_frame` checks the two copies.

## Where the live loop calls it

| Surface | Call |
|---|---|
| `galion-live-demo` `run_frames` | `SymphonyWire::step` after `step_live` and `MeVoice::render`. Without a ring, the audible me buffer stays the voice render. `run_frames_draining` attaches `NanoPcmRing`; a full frame replaces that tick's audible me buffer. The wire's mark is `FrameRecord.dot`. |
| `DotSink` | `DotConsole::frame_marked` with that mark, after the step. |
| `dot_console_demo` | `me_emit`, then `poll_me` / `release_me`, then `SymphonyWire::step` with the ring, then `ring.release` on yield, then `frame_marked`. An empty ring leaves the me slot in place. |
| `PhoneHost::poll_me` | worker poll always; PCM pop only when `emit` is true; at most `FRAME_SAMPLES` samples. |
| `PhoneHost::release_me` | queue drop on yield, before the poll, so a reply published on that tick survives. |

`Heartbeat::step` does not call the wire. The host does, on the same frame.

## Re-run

```bash
scripts/eval/symphony-heartbeat.sh
```

The script prints `verdict=PASS`, `verdict=FAIL`, or `verdict=BLOCKED`. The recorded run is `docs/galion-voice-v2/symphony/SELF-EVAL.md`.

— Maciej Grajczyk
