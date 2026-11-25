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

`samantha_ref` on `/health` is `unset`, `set`, or `missing`. The path in `GALION_SAMANTHA_REF` is not returned.

`mouth` and `voice` name what is speaking. The turn is `horbm-voice`: ear, then `horbm::Brain::turn`, then one mouth adapter. Boot default is `brain` `horbm` and `mouth_device` `cpu`. A default build does not link Piper, so that CPU mouth is `"mouth":"fake","voice":"none"` (an in-process tone). `"mouth":"piper","voice":"ljspeech"` is selected only when the binary is built with `--features piper`. `GALION_MOUTH=native` (also `horbm` or `text`) selects `"mouth":"native","voice":"text"`. That mouth does not spawn Piper. It sends the turn as text frames on this socket: `horbm::TurnOut.first_clause`, then `rest` when the remainder is non-empty. Each frame is a `turn` object whose `reply` is that piece. The mouth returns no PCM until an MIT or Apache-2.0 speaker is linked. An empty `GALION_MOUTH`, `piper`, `cpu`, `auto`, `today`, `tone`, `s711`, or `fake` keeps today's mouth. Any other value falls back to that same mouth. The s711 clone is `"mouth":"s711","voice":"s711"` only when `GALION_SAMANTHA_REF` is a local wav, `GALION_SAMANTHA_WEIGHTS` is a local checkpoint directory, and `GALION_SAMANTHA_DEVICE=cuda` with CUDA visible to the probe. CPU does not select s711. `clone` is `off`, `ref-missing`, `weights-missing`, `cpu-not-realtime`, `cuda-unavailable`, `device-unknown`, `selected`, or `open-failed`. `cpu-not-realtime` is not a new timing: this process has not measured an s711 CPU real-time factor. The wav is read only when the s711 mouth is selected. It is not downloaded. `mouth_device` is `cpu` unless that cuda opt-in selected the clone.

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

The speaker path is 24 kHz, sliced on the 80 ms heartbeat (1920 samples). The `turn` text is sent when the reply is ready. The mouth speaks the first clause first (a comma, semicolon, colon, dash, or conjunction, once that piece has about four words; a sentence end still closes a shorter line), and each 80 ms slice is sent as soon as it exists. Later clauses are synthesised while those slices go out. The first slice is sent immediately. Each later slice waits 80 ms, unless that gap has already passed. A client `stop` sets the mouth cancel flag and drops slices that have not been sent.

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
| `turn` | After a committed line. `intent` is a JSON string from empathy (`"chat"`, `"ask"`, `"task"`, `"vent"`, or `"goodbye"`). `arm` stays `null`. `empathy_e` and `prosody` (`rate`, `energy`, `pitch_st`, `pause_ms`) are on the same object. The spoken `reply` is the card, the research line, or exactly `I don't know.` With `GALION_MOUTH=native` the socket sends the first clause, then the rest, as two `turn` frames when the remainder is non-empty, and it sends no speaker PCM. |
| `mood` | Every 80 ms. `source` is `brain` or `resting`. `pad` is arousal, valence, dominance. |
| `inner` | After the idle interval with no heard text (`horbm::WAIT_MS`, 4 s), and only while the dot is idle. One object built from the inner turn: `verb` (outbound intent token), `lean` (dialogue lean), `surprise`, `learned`, and `slots` (motive tokens). There is no sentence. The mouth does not speak it and the frame carries no audio. |
| `dot` | `idle`, `listening`, or `speaking`, plus `level` 0..1 |
| `error` | A short code. No PCM and no transcript in the string. |

`GET /health` is HTTP 200 and JSON whose `ok` field is `true`. `Brain::open` in the orchestrator still returns `horbm is not linked; Brain::open stays in crate horbm`. That function is not this server. Crate `horbm` is linked on the 7881 path. The unlinked test brain still answers a turn with `brain-unlinked`, and its mood source stays `resting` at `Pad::RESTING` (0.3, 0.25, 0.5). The live binary reads knowledge cards through `horbm::Brain::turn`. A card that matches is spoken. A miss is `I don't know.` The live mood source is `brain`. An empty 80 ms frame steps empathy toward rest. A test brain may return a speakable line. The PCM for that line is a 24 kHz tone, not Piper and not Samantha's voice. Stock comfort and a bracket tag are not spoken. A second `hello` on an open socket is ignored. Silence of about 640 ms ends one mic turn. Port 7878 is the one-dot page. Port 7881 is this server.

`partial` and `final` are the ASR sector's frames. This process does not invent them.
