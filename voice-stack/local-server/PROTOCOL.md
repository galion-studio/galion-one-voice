<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# `galion-samantha-ws/1`

This is the companion socket from the Swarm 16 architecture (PR #220). It is not the phone HTTP path and not the nebula feed on port 8766.

Default bind is `127.0.0.1:7881`. `GALION_SAMANTHA_WS` may set another `127.0.0.1` or `[::1]` port. `0.0.0.0` and every non-loopback address refuse to start. Port 7878 stays the existing one-dot page.

## HTTP

| Method | Path | Body |
| --- | --- | --- |
| GET | `/health` | JSON `ok: true`, `Cache-Control: no-store` |
| POST | `/session` | body `{"op":"open"}` → `{"token","ttl_s","transport":"body"}` |
| GET | `/v1/samantha` | WebSocket upgrade |
| GET | `/` | one-dot page from `--ui` |

A `token` or `session` query parameter is HTTP 400. The token travels in the `hello` frame body. TTL defaults to 120 seconds and clamps to 15…900 (`GALION_SESSION_TTL_SECS`). `GALION_REQUIRE_SESSION=1` closes a socket whose `hello.session` is missing or unknown. When that variable is unset, `session: null` is accepted.

`samantha_ref` on `/health` is `unset`, `set`, or `missing`. The path in `GALION_SAMANTHA_REF` is not returned and the file is not read.

## WebSocket `/v1/samantha`

The first client text frame, within 5 seconds:

```json
{"schema":"galion-samantha-ws/1","type":"hello","session":null}
```

The server answers:

```json
{"schema":"galion-samantha-ws/1","type":"welcome","frame_ms":80,"sample_rate":24000,"dot":"idle"}
```

`Origin`, when present, must be `http://127.0.0.1:7881`, `http://127.0.0.1:7878`, or `http://` plus the address this process actually bound (including `[::1]` when that is the bind). Anything else is HTTP 403.

### Binary frames

Eight-byte header, then little-endian PCM16 mono.

| Offset | Value |
| --- | --- |
| 0–1 | `G` `S` |
| 2 | version `1` |
| 3 | `0` microphone (client → server), `1` speaker (server → client) |
| 4 | `0` = 24 kHz, `1` = 16 kHz |
| 5 | `0` |
| 6–7 | `seq` u16 LE |
| 8.. | samples, at most 4800 |

The speaker path is 24 kHz, sliced on the 80 ms heartbeat (1920 samples). The first slice is sent immediately. Each later slice waits 80 ms. A client `stop` drops slices that have not been sent.

### Client text

```json
{"schema":"galion-samantha-ws/1","type":"ping"}
{"schema":"galion-samantha-ws/1","type":"stop"}
{"schema":"galion-samantha-ws/1","type":"text","text":"the fence"}
{"schema":"galion-samantha-ws/1","type":"consent","sense":false,"adapt":false,"remember":false,"share":false}
```

`ping` is answered with `{"schema":"galion-samantha-ws/1","type":"pong"}`. `stop` is barge-in. Omitted consent bits stay as they are. Defaults are all false.

### Server text

| `type` | When |
| --- | --- |
| `vad` | `event` `start` or `end`, plus `t_ms` |
| `turn` | After a committed line. `intent` and `arm` are JSON values (`null` until `horbm` names them). |
| `mood` | Every 80 ms. `source` is `brain` or `resting`. `pad` is arousal, valence, dominance. |
| `dot` | `idle`, `listening`, or `speaking`, plus `level` 0..1 |
| `error` | A short code. No PCM and no transcript in the string. |

With no `horbm` crate linked, `turn` is the error code `brain-unlinked` and mood stays `source: resting` at `Pad::RESTING` (0.3, 0.25, 0.5). The tick does not ease toward another pad. A test brain may return a speakable line. The PCM for that line is a 24 kHz tone, not Piper and not Samantha's voice. Stock comfort and a bracket tag are not spoken.

`partial` and `final` are the ASR sector's frames. This process does not invent them.
