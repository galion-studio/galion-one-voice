<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# PROTOCOL-ASSUMED

Swarm 16, sector S11. This is the wire the Samantha one-dot page speaks.
The voice server for this swarm is not in the tree yet, and HORBM is not
on GitHub. Other sectors can meet this note. It is an assumption, not a
lock in `LOCKS.toml`.

Loopback only. The page refuses a WebSocket host other than `127.0.0.1`
or `localhost`. The Rust process refuses a bind that is not loopback.

## Socket

```
ws://127.0.0.1:8788/v1
```

`SAMANTHA_DOT_ADDR` or `--addr` changes the port. The path stays `/v1`.

The browser `Origin` must be `http://127.0.0.1:<port>` or
`http://localhost:<port>`. A missing `Origin` is allowed for a local
client. Any other origin is `403` and the socket is not upgraded.

One WebSocket connection is served at a time.

## Clocks

Mood, state, and level move on an **80 ms** tick. That is the heartbeat
period (`galion-heartbeat` `FRAME_MS`, 12.5 Hz). A late tick is one tick,
not a burst.

The page paints on `requestAnimationFrame`. It springs toward the latest
tick. The spring is not the 80 ms clock.

## Text frames

UTF-8 JSON. Every object has:

```json
{ "schema": "galion-samantha-dot/1" }
```

### Server → page

`kind` is `state` (the first frame, clock not advanced) or `tick`.

```json
{
  "schema": "galion-samantha-dot/1",
  "kind": "tick",
  "state": "speaking",
  "level": 0.42,
  "t_ms": 640,
  "mood": { "arousal": 0.3, "valence": 0.25, "dominance": 0.5 },
  "caption": {
    "text": "Hi. I'm here.",
    "sources": ["fake:reply-engine"],
    "final": true
  }
}
```

| Field | Meaning |
|---|---|
| `state` | `idle`, `listening`, `thinking`, or `speaking` |
| `level` | 0..1 audio level for the active side |
| `t_ms` | Milliseconds since the socket opened, steps of 80 |
| `mood` | PAD. Rest is arousal `0.3`, valence `0.25`, dominance `0.5` |
| `caption` | Omitted: keep the current line. `null`: clear it. Object: replace it |

`sources` are labels for the cards the reply used. The page shows them
when captions are on. It does not fetch them.

Valence is about -1..1, matching `galion-emotion`. Arousal and dominance
are 0..1. The page clamps. Non-finite values fall back to rest.

### Page → server

```json
{ "schema": "galion-samantha-dot/1", "kind": "control", "action": "talk" }
```

`action` is `talk`, `stop`, `end`, or `barge`.

| Action | When |
|---|---|
| `talk` | The person tapped the dot. The mic has just been requested. |
| `end` | Local quiet after speech (640 ms), or the utterance hit 15 s. |
| `barge` | The person spoke while the reply was playing. |
| `stop` | The Stop control, or Escape. |

The page does not send a transcript. ASR is not this sector.

## Binary frames

Either direction. Header is 20 bytes, then little-endian `s16` mono PCM.

| Offset | Type | Value |
|---|---|---|
| 0 | 4 bytes | `GVPC` |
| 4 | u8 | version `1` |
| 5 | u8 | format `1` (s16le) |
| 6 | u8 | channels `1` |
| 7 | u8 | `0` |
| 8 | u32 le | sample rate: 8000, 16000, 24000, or 48000 |
| 12 | u32 le | sequence, starting at 0 |
| 16 | u32 le | sample count |
| 20 | i16 le | samples |

The page sends **16 kHz**, **1280 samples** (one 80 ms frame). 24 kHz,
1920 samples, is the heartbeat rate if a later mouth uses that clock.

Page → server binary is the microphone. Server → page binary is her
voice, played as it arrives. This sector's mock sends no binary audio.
A caption with no PCM is a silent preview, and the page says so.

A frame larger than one second at 48 kHz is rejected.

## What the mock does

`--mock`, or the page opened with `?mock=1` (no socket):

1. `talk` → listening.
2. No voice for 1600 ms, or 640 ms of quiet after voice, or `end`, or 15 s → thinking.
3. 640 ms later the fake `ReplyEngine` returns `Hi. I'm here.` and source `fake:reply-engine`.
4. Speaking lasts 2400 ms. `level` is a sine envelope, not a recording.
5. Mood on the mock drifts a little around rest. Tick 0 is exactly rest. The drift is not a measured feeling.

Without `--mock` the process still serves the page and the socket. Ticks
carry rest mood and the mic level. They never invent a reply. Thinking
stays thinking.

## Brain seam

`ReplyEngine::turn(&str) -> TurnOut` matches the HORBM shape named in the
swarm note: `reply`, `intent`, `arm`, `propensity`, `evidence`, `sources`,
`violations`, `lookup_us`, `took_us`, `mood`. `FakeReplyEngine` is the
stand-in. `Brain::open` is not called. The crate `horbm` is not a dependency.
