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

`SymphonyWire::step` keeps its state in the struct. It copies at most one me-band frame from a slot the caller already filled. It applies the previous frame's feeling sample. It counts persona holds. It writes the one-dot mark.

`me_emit` is the question the host asks before it pops a reply queue. It does not change wire state. A true answer means one frame may move. A false answer means the queue waits.

## Hot path

Inside either `step`:

- no lock, channel, sleep, clock read, file, or socket
- no heap allocation
- a late frame is a pause

A pause leaves the me buffer untouched, leaves the persona hold where it was, and keeps the one-dot mark. The next frame copies one frame. It does not skip ahead through a queued tail.

A yield sets `release_slot`. The host drops the rest of the queue after `step` returns. The drop is outside the step.

A me buffer whose length is not 1920 is a pause. A short write would be a glitch.

Persona id may change only after `PERSONA_HOLD_FRAMES` (8) sensed samples at or above `PERSONA_HOLD_E` (0.65). One frame never flips it. The feeling sample decided on frame *t* is applied on frame *t+1*. Without SENSE the sample is dropped and the hold clears on the next frame that is not a pause.

The one-dot mark is `Speaking` when this frame copied me samples, `Listening` when you are voiced and me is quiet, `Idle` otherwise. Speak wins. A pause holds the previous mark.

The budget checked by the harness is `STEP_BUDGET_US` (800). A step whose warm p99 or max reaches that budget fails. The number is the harness ceiling. It is not a second clock.

## Off the step

These stay outside both calls. They may allocate. They may block. They must not be moved inside `step`.

| Work | Where it runs |
|---|---|
| Clone synthesis (the child process) | out of process, off the frame |
| Persona reply text (`Floor::say`) | turn path, off the frame |
| Paint, JSON, HTTP, glyph history | after the step, best effort |

`clone_worker::hot_frame_samples` is the drain size. It equals `FRAME_SAMPLES`. It does not start the child. `CLONE_FRAME_MS` is `FRAME_MS`.

The persona floor uses the same hold count (`HOLD_FRAMES`, `HOLD_E`, `RELEASE_E`). The render crate stays free of a heartbeat dependency. `sectors_match_heartbeat_frame` checks the two copies.

## Where the live loop calls it

| Surface | Call |
|---|---|
| `galion-live-demo` `run_frames` | `SymphonyWire::step` after `step_live` and `MeVoice::render`. The audible me buffer stays the voice render. The wire's mark is `FrameRecord.dot`. |
| `DotSink` | `DotConsole::frame_marked` with that mark, after the step. |
| `dot_console_demo` | `me_emit`, then `poll_me` / `release_me`, then `SymphonyWire::step`, then `frame_marked`. |
| `PhoneHost::poll_me` | worker poll always; PCM pop only when `emit` is true; at most `FRAME_SAMPLES` samples. |
| `PhoneHost::release_me` | queue drop on yield, before the poll, so a reply published on that tick survives. |

`Heartbeat::step` does not call the wire. The host does, on the same frame.

## Re-run

```bash
scripts/eval/symphony-heartbeat.sh
```

The script prints `verdict=PASS`, `verdict=FAIL`, or `verdict=BLOCKED`. The recorded run is `docs/galion-voice-v2/symphony/SELF-EVAL.md`.

— Maciej Grajczyk
