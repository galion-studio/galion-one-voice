<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# PROTOCOL-ASSUMED

Swarm 16, sector S11. The page speaks `galion-samantha-ws/1` from
`voice-stack/ARCHITECTURE.md` (pull request 220). This note is the part
the page implements. HORBM is still not linked.

Bind `127.0.0.1:7881`. `GALION_SAMANTHA_WS` may set another loopback
address. `0.0.0.0` is refused.

WebSocket path: `GET /v1/samantha`.

The page also serves itself on `GET /` of that same process so the dot
can be opened. The architecture's `GET /health` and `POST /session`
belong to sector S10. This page sends `hello` with `"session": null`.

`Origin`, when the browser sends one, must be `http://127.0.0.1:7881`,
`http://127.0.0.1:7878`, or `http://127.0.0.1:<the bound port>`.

## Text

Schema on every object: `galion-samantha-ws/1`.

The page sends `hello` first, then microphone PCM. Stop and a second tap
send `{"type":"stop"}`.

The server sends `welcome`, then a `mood` frame each 80 ms. In this
preview the mood source is `resting` and the pad is arousal `0.3`,
valence `0.25`, dominance `0.5`. It does not drift. A `dot` frame is
`idle`, `listening`, or `speaking`, plus `level`. A `turn` frame carries
`reply` and `sources`. The page shows that reply near the dot until the
state returns to idle.

Thinking is a local motion between the end of speech and the reply. It
is not a `dot` state on the wire.

## Binary

Eight bytes, then little-endian s16 mono PCM.

| Offset | Value |
|---|---|
| 0–1 | `G` `S` |
| 2 | version `1` |
| 3 | `0` microphone (page to server), `1` speaker (server to page) |
| 4 | `0` = 24 kHz, `1` = 16 kHz |
| 5 | `0` |
| 6–7 | `seq` u16 little-endian |
| 8.. | samples |

The page sends 16 kHz microphone frames, 1280 samples (80 ms). Speaker
frames, when a mouth sends them, are played. This preview sends none.
