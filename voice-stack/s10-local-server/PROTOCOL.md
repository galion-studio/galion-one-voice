<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# samantha-local protocol `galion-samantha-local/1`

The browser talks to one process on `127.0.0.1`. There is no session token.
The bind is the boundary. A WebSocket `Origin` other than `http://127.0.0.1`
or `http://localhost` (optional port) is HTTP 403. A missing Origin is
accepted so a same-machine test client can connect.

## HTTP

| Method | Path | Body |
| --- | --- | --- |
| GET | `/health` | JSON, `Cache-Control: no-store` |
| GET | `/v1/stream` | WebSocket upgrade |
| GET | `/`, `/app.js`, `/dot.css` | Files from the `--ui` directory |

`/health`:

```json
{
  "ok": true,
  "service": "samantha-local",
  "protocol": "galion-samantha-local/1",
  "bind": "127.0.0.1:8790",
  "loopback": true,
  "frame_ms": 80,
  "mic_rate": 16000,
  "tts_rate": 22050,
  "samantha_ref": "unset"
}
```

`samantha_ref` is `unset`, `set`, or `missing`. The path in
`GALION_SAMANTHA_REF` is not returned and the file is not read.

## WebSocket `/v1/stream`

Text frames are one JSON object. Binary frames are PCM.

Rates match the crates this sector does not reimplement:

| Constant | Value | Source |
| --- | --- | --- |
| `frame_ms` | 80 | `galion-heartbeat` `FRAME_MS` |
| mic rate | 16000 Hz mono s16le | `galion-asr-vosk` `SAMPLE_RATE` |
| mic frame | 320 samples (20 ms) | `galion-asr-vosk` `FRAME_SAMPLES` |
| tts rate | 22050 Hz mono s16le | Piper `PIPER_HZ` in `galion-v2-live` |

The server slices TTS into 80 ms pieces (1764 samples at 22050 Hz) and
sends the first piece immediately. Each later piece waits 80 ms. That is
the streaming bar for this sector. Mood is a separate 80 ms tick from
`Pad::RESTING` (`arousal` 0.3, `valence` 0.25, `dominance` 0.5) toward the
last reply's pad, closing 25% of the gap per tick.

### Binary frame

Little-endian. 20 byte header, then mono s16le PCM. At most 4096 samples.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | magic `G1PC` (`47 31 50 43`) |
| 4 | 1 | version `1` |
| 5 | 1 | kind: `1` mic (client to server), `2` tts (server to client) |
| 6 | 2 | flags |
| 8 | 4 | sample rate |
| 12 | 4 | sequence, wrapping unsigned |
| 16 | 2 | channels, must be `1` |
| 18 | 2 | reserved, must be `0` |
| 20 | n*2 | PCM |

Flags: bit 0 `start` (1), bit 1 `end` (2), bit 2 `barge` (4). A one-slice
utterance sets both `start` and `end`. The client sends kind `1` at
16000 Hz. Any other rate or kind is an error event and the audio is dropped.

### Client text

```json
{"type":"hello"}
{"type":"ping","t_ms":0}
{"type":"barge"}
{"type":"text","text":"the kettle is loud"}
```

On a ping, `t_ms` is the client's own clock. The server copies that number to `echo_ms` and sets `t_ms` to milliseconds since the socket opened, same as every other server object.

`text` is at most 2000 Unicode scalar values. Empty text is an error.
`barge` drops PCM that has not been sent yet and answers
`{"type":"state","state":"idle","reason":"barge"}`.

### Server text

Every object also has `t_ms`, milliseconds since the socket opened.

`ready` is the first message:

```json
{
  "type": "ready",
  "protocol": "galion-samantha-local/1",
  "bind": "127.0.0.1",
  "mic_rate": 16000,
  "tts_rate": 22050,
  "frame_ms": 80,
  "mic_frame_samples": 320,
  "mood_rest": {"arousal": 0.3, "valence": 0.25, "dominance": 0.5},
  "ear": "unwired",
  "mouth": "hold",
  "mind": "fake"
}
```

`ear` is `unwired` or `scripted`. `mouth` is `hold` until Piper is attached.
`mind` is `fake` or `horbm`. `recognized` on a heard event is always false
in this sector: the unwired ear does not invent words, and the scripted ear
repeats a test line.

```json
{"type":"hello","protocol":"galion-samantha-local/1"}
{"type":"pong","echo_ms":0,"t_ms":1}
{"type":"state","state":"listening"}
{"type":"state","state":"thinking"}
{"type":"state","state":"speaking"}
{"type":"state","state":"idle","reason":"end"}
{"type":"heard","text":"","ear":"unwired","recognized":false}
{"type":"mood","arousal":0.3,"valence":0.25,"dominance":0.5,"tick":1,"frame_ms":80,"state":"idle"}
{"type":"error","error":"mic rate must be 16000"}
```

`state` is `idle`, `listening`, `thinking`, or `speaking`.

A reply carries the `TurnOut` fields from the brain trait. `mouth_us` is
how long this process spent building the hold tone. It is not a Piper time.
`took_us` and `lookup_us` belong to the reply engine. The hold mouth is a
tone, not Samantha's voice.

```json
{
  "type": "reply",
  "text": "Okay. the kettle is loud",
  "intent": "reply",
  "arm": "fake",
  "propensity": 0.0,
  "evidence": [],
  "sources": [],
  "violations": 0,
  "lookup_us": 0,
  "took_us": 12,
  "mouth_us": 40,
  "mouth": "hold",
  "mood": {"arousal": 0.3, "valence": 0.25, "dominance": 0.5}
}
```

The reply `mood` is the engine target. `mood` events after that are the
eased clock.

## Mic energy gate

The fake pipeline, not a product VAD. RMS at or above 0.02 of full scale is
speech. Two 20 ms speech frames open `listening`. Eight 20 ms quiet frames
(160 ms) close it. The unwired ear then emits `heard` with an empty string
and does not call the reply engine. The scripted ear emits its fixed line
with `recognized: false` and does call the engine. A typed `text` message
skips the gate.

## What is not on this socket

Piper's stdin line `{"text":"..."}` and raw stdout PCM stay in
`galion-ml-voice`. Vosk's NDJSON (`partial` / `final`) stays in
`galion-asr-vosk`. Chatterbox stays a separate loopback unit and is not
called. `horbm::Brain::open` is not called here.
