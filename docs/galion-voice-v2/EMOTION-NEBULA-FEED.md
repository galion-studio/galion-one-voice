# Emotion and nebula feed

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Schema `galion-voice-v2-emotion-feed/1`. This is the contract between the side stream and the picture. The machine-readable field list is [`emotion-nebula-feed.schema.json`](emotion-nebula-feed.schema.json). The checker that runs before a line is sent is `tools/emotion-v2/nebula/contract.py`.

It sits on three drafts that are already on main. It does not replace them.

| Draft | What it already is | What this file adds |
|---|---|---|
| [#98](https://github.com/galion-studio/galion-one-voice/pull/98) | `galion-nebula-live-feed/1` in [`../emotion-v2/NEBULA-LIVE-FEED.md`](../emotion-v2/NEBULA-LIVE-FEED.md). EMA, gas-line ids, absence as null weights. | A named `estimate`, and a bridge that refuses a line the picture must not paint. |
| [#100](https://github.com/galion-studio/galion-one-voice/pull/100) | Offline distances in [`../emotion-v2/NEBULA-SCORING.md`](../emotion-v2/NEBULA-SCORING.md). Not a live feed. | Nothing. `emotion_match` is unchanged. |
| [#101](https://github.com/galion-studio/galion-one-voice/pull/101) | The picture at `apps/nebula-voice-v2`, schema `galion-nebula/1`, demo blend in `frame_at`. | How a live line maps onto that picture, including `element_id`. |

This bridge is not the live voice server. It does not open a microphone, score audio, write the 8-byte `EmotionWire`, or bind a public address. Nothing here is a deploy.

## Estimate

`estimate` is the field the picture reads. A zero is not enough: a real Neutral distribution also has zeros on the other emotions, and a measured valence of 0 is not an absence.

| `estimate` | `present` | `distribution` | Numbers | What the picture does |
|---|---|---|---|---|
| `none` | `false` | `false` | Every weight is null. Valence, arousal, dominance, and energy are null. | No emotion estimate. Do not paint Neutral. Do not run the demo time-blend. |
| `distribution` | `true` | `true` | Every weight for this voice is in `0..1` and the weights sum to 1 within `1e-3`. Axes may still be null. | Paint this simplex. |
| `partial` | `true` | `false` | At least one weight or axis is numeric, and the weights are not a simplex. | Paint only the numbers that are there. Do not fill the rest with Neutral. |

`none` is the absence already defined on the live feed (`present: false`). The new name is there so a parser cannot treat a missing number as Neutral = 1.

## Messages

Parsers use field names. A Jarvis message omits `seductive`. A Samantha message includes it.

No estimate yet. Jarvis, before any posterior. Neutral's weight is null, not 1.

```json
{
  "schema": "galion-voice-v2-emotion-feed/1",
  "source_schema": "galion-nebula-live-feed/1",
  "voice_id": "jarvis",
  "t_ms": 0,
  "kind": "frame",
  "estimate": "none",
  "present": false,
  "distribution": false,
  "tau_ms": 240,
  "weights": {
    "neutral": {
      "weight": null,
      "gas_line": "HeII-541.2",
      "element_id": "HeII-541.2"
    },
    "calm": {
      "weight": null,
      "gas_line": "Hbeta-486.1",
      "element_id": "H-beta"
    },
    "happy": {
      "weight": null,
      "gas_line": "[NII]-575.5",
      "element_id": "NII-575.5"
    },
    "amused": {
      "weight": null,
      "gas_line": "HeI-587.6",
      "element_id": "HeI-587.6"
    },
    "excited": {
      "weight": null,
      "gas_line": "NeI-594.5",
      "element_id": "NeI-594.5"
    },
    "tender": {
      "weight": null,
      "gas_line": "HeI-667.8",
      "element_id": "HeI-667.8"
    },
    "sad": {
      "weight": null,
      "gas_line": "HeII-468.6",
      "element_id": "HeII-468.6"
    },
    "disappointed": {
      "weight": null,
      "gas_line": "[ClIII]-551.7",
      "element_id": "ClIII-551.7"
    },
    "angry": {
      "weight": null,
      "gas_line": "[SIII]-631.2",
      "element_id": "SIII-631.2"
    },
    "fearful": {
      "weight": null,
      "gas_line": "Hdelta-410.2",
      "element_id": "H-delta"
    },
    "surprised": {
      "weight": null,
      "gas_line": "[OIII]-495.9",
      "element_id": "OIII-495.9"
    },
    "disgusted": {
      "weight": null,
      "gas_line": "[ClIII]-553.8",
      "element_id": "ClIII-553.8"
    }
  },
  "valence": null,
  "arousal": null,
  "dominance": null,
  "energy": null,
  "seq": 1,
  "origin": "server",
  "reason": null,
  "detail": null
}
```

A measured simplex. Happy is 1. The other weights are 0 because they were measured, not because they were missing.

```json
{
  "schema": "galion-voice-v2-emotion-feed/1",
  "source_schema": "galion-nebula-live-feed/1",
  "voice_id": "samantha",
  "t_ms": 0,
  "kind": "frame",
  "estimate": "distribution",
  "present": true,
  "distribution": true,
  "tau_ms": 240,
  "weights": {
    "neutral": {
      "weight": 0,
      "gas_line": "HeII-541.2",
      "element_id": "HeII-541.2"
    },
    "calm": {
      "weight": 0,
      "gas_line": "Hbeta-486.1",
      "element_id": "H-beta"
    },
    "happy": {
      "weight": 1,
      "gas_line": "[NII]-575.5",
      "element_id": "NII-575.5"
    },
    "amused": {
      "weight": 0,
      "gas_line": "HeI-587.6",
      "element_id": "HeI-587.6"
    },
    "excited": {
      "weight": 0,
      "gas_line": "NeI-594.5",
      "element_id": "NeI-594.5"
    },
    "tender": {
      "weight": 0,
      "gas_line": "HeI-667.8",
      "element_id": "HeI-667.8"
    },
    "sad": {
      "weight": 0,
      "gas_line": "HeII-468.6",
      "element_id": "HeII-468.6"
    },
    "disappointed": {
      "weight": 0,
      "gas_line": "[ClIII]-551.7",
      "element_id": "ClIII-551.7"
    },
    "angry": {
      "weight": 0,
      "gas_line": "[SIII]-631.2",
      "element_id": "SIII-631.2"
    },
    "fearful": {
      "weight": 0,
      "gas_line": "Hdelta-410.2",
      "element_id": "H-delta"
    },
    "surprised": {
      "weight": 0,
      "gas_line": "[OIII]-495.9",
      "element_id": "OIII-495.9"
    },
    "disgusted": {
      "weight": 0,
      "gas_line": "[ClIII]-553.8",
      "element_id": "ClIII-553.8"
    },
    "seductive": {
      "weight": 0,
      "gas_line": "Halpha-656.3",
      "element_id": "H-alpha"
    }
  },
  "valence": 0.8,
  "arousal": 0.4,
  "dominance": 0.2,
  "energy": 0.7,
  "seq": 2,
  "origin": "server",
  "reason": null,
  "detail": null
}
```

The client writes this itself when the socket is down. It is not a posterior. `seq` is null. `voice_id` is null until a server line has named one; after that it repeats the last voice so the particle table stays bound, with every weight null.

```json
{
  "schema": "galion-voice-v2-emotion-feed/1",
  "source_schema": null,
  "voice_id": null,
  "t_ms": 0,
  "kind": "frame",
  "estimate": "none",
  "present": false,
  "distribution": false,
  "tau_ms": 240,
  "weights": {
    "neutral": {
      "weight": null,
      "gas_line": "HeII-541.2",
      "element_id": "HeII-541.2"
    },
    "calm": {
      "weight": null,
      "gas_line": "Hbeta-486.1",
      "element_id": "H-beta"
    },
    "happy": {
      "weight": null,
      "gas_line": "[NII]-575.5",
      "element_id": "NII-575.5"
    },
    "amused": {
      "weight": null,
      "gas_line": "HeI-587.6",
      "element_id": "HeI-587.6"
    },
    "excited": {
      "weight": null,
      "gas_line": "NeI-594.5",
      "element_id": "NeI-594.5"
    },
    "tender": {
      "weight": null,
      "gas_line": "HeI-667.8",
      "element_id": "HeI-667.8"
    },
    "sad": {
      "weight": null,
      "gas_line": "HeII-468.6",
      "element_id": "HeII-468.6"
    },
    "disappointed": {
      "weight": null,
      "gas_line": "[ClIII]-551.7",
      "element_id": "ClIII-551.7"
    },
    "angry": {
      "weight": null,
      "gas_line": "[SIII]-631.2",
      "element_id": "SIII-631.2"
    },
    "fearful": {
      "weight": null,
      "gas_line": "Hdelta-410.2",
      "element_id": "H-delta"
    },
    "surprised": {
      "weight": null,
      "gas_line": "[OIII]-495.9",
      "element_id": "OIII-495.9"
    },
    "disgusted": {
      "weight": null,
      "gas_line": "[ClIII]-553.8",
      "element_id": "ClIII-553.8"
    }
  },
  "valence": null,
  "arousal": null,
  "dominance": null,
  "energy": null,
  "seq": null,
  "origin": "client",
  "reason": "disconnected",
  "detail": null
}
```

| Field | Meaning |
|---|---|
| `schema` | `galion-voice-v2-emotion-feed/1` |
| `source_schema` | `galion-nebula-live-feed/1` when this line was projected from one. Null on a rejection or a client gap. |
| `voice_id` | `samantha`, `jarvis`, or null on a gap that has not seen a voice yet. |
| `t_ms` | Milliseconds on the live feed. Client gaps do not advance this clock. |
| `kind` | `frame` or `utterance`. An utterance line still does not move the frame cloud. |
| `estimate` | `none`, `distribution`, or `partial`. |
| `present`, `distribution`, `tau_ms` | Copied from the live feed. `tau_ms` default is 240. |
| `weights.*.weight` | Probability, or null when that channel has no estimate. |
| `weights.*.gas_line` | The live-feed id. Opaque except for the wavelength after the last hyphen. |
| `weights.*.element_id` | The Demo 13 id in `crates/nebula-elements`. This is the id `label_weight` accepts. |
| `valence`, `arousal`, `dominance` | Emotion V2 axes in `-1..1`, or null. Not rescaled into `0..1`. |
| `energy` | Speech energy in `0..1`, or null. |
| `seq` | Server messages start at 1 and count every send, including a rejection. Client gaps use null. |
| `origin` | `server` or `client`. |
| `reason` | Null on a projected frame. `rejected` when the live line failed the check. `disconnected` on a client gap. |
| `detail` | Short checker text on a rejection, otherwise null. It is not a transcript and not audio. |

### Gas line and element id

Eight emotions do not use the same string in the two tables. Passing `gas_line` to `label_weight` as the element id is rejected, the mass stays 0, and the old picture path then paints Neutral.

| Emotion | `gas_line` | `element_id` |
|---|---|---|
| Neutral | `HeII-541.2` | `HeII-541.2` |
| Calm | `Hbeta-486.1` | `H-beta` |
| Happy | `[NII]-575.5` | `NII-575.5` |
| Amused | `HeI-587.6` | `HeI-587.6` |
| Excited | `NeI-594.5` | `NeI-594.5` |
| Tender | `HeI-667.8` | `HeI-667.8` |
| Sad | `HeII-468.6` | `HeII-468.6` |
| Disappointed | `[ClIII]-551.7` | `ClIII-551.7` |
| Angry | `[SIII]-631.2` | `SIII-631.2` |
| Fearful | `Hdelta-410.2` | `H-delta` |
| Surprised | `[OIII]-495.9` | `OIII-495.9` |
| Disgusted | `[ClIII]-553.8` | `ClIII-553.8` |
| Seductive | `Halpha-656.3` | `H-alpha` |

## Server-side check

`bridge` reads `galion-nebula-live-feed/1` lines. Each line is checked before it is written. The check rejects:

- a schema other than `galion-nebula-live-feed/1`, or an unknown field
- a voice other than `samantha` or `jarvis`
- `seductive` on Jarvis
- a gas line that is not the fixed id for that emotion
- a weight outside `0..1`, or a posterior that claims `distribution: true` without a simplex
- `present: false` together with any numeric weight or axis
- a coarse head (four-class or nine-class) instead of this voice's emotions

A rejected line is not forwarded. The socket and stdout get `estimate: none`, `reason: rejected`, `source_schema: null`. The bad weights are not in that object. The bridge keeps reading.

The listener is loopback only (`127.0.0.1`, `localhost`, or `[::1]`). A public address fails closed.

```bash
PYTHONPATH=tools/emotion-v2 python -m nebula feed --voice jarvis \
  | PYTHONPATH=tools/emotion-v2 python -m nebula bridge --tcp 127.0.0.1:8766
```

`feed` is unchanged. It still smooths a posterior it is given. `bridge` is the check in front of the picture.

## Reconnect

`listen` is the client. The wait is fixed, with no jitter, so a test can name it.

| Failed connects in a row | Wait |
|---|---|
| 1 | 80 ms (one heartbeat) |
| 2 | 160 ms |
| 3 | 320 ms |
| 4 | 640 ms |
| 5 and after | 1280 ms (16 heartbeats) |

A successful connect starts the count again. There is no give-up. While the socket is down, the client emits `estimate: none`, `origin: client`, `reason: disconnected`. That replaces the last emotion. It does not hold the last simplex on screen.

```bash
PYTHONPATH=tools/emotion-v2 python -m nebula listen --tcp 127.0.0.1:8766
```

`--frames N` stops after N server messages. That flag is for a test. A picture omits it.

## Picture

`parse_frame` in `apps/nebula-voice-v2` accepts both schemas.

- `estimate: none` or `present: false` sets `no_estimate`. Every weight stays 0. Neutral is not filled in.
- `drive_from` on that frame returns zero turbulence, clump, pulse, and weights.
- A weight object `{weight, gas_line, element_id}` is read by emotion name. `element_id` is what the particle table stores. `gas_line` must be the feed id.
- `--stdin` prints `"estimate":"none"` or `"estimate":"present"`.

A line with no `schema` is still `galion-nebula/1`. An empty mass on that older shape still becomes Neutral. Do not strip `schema`.

The browser page still calls `frame_at` and has no socket. Wiring that page is called out for the UI owner below. This change does not start that page and does not deploy it.

## Tests

```bash
PYTHONPATH=tools/emotion-v2 python -m unittest discover -s tools/emotion-v2/nebula/tests -v
cargo test --manifest-path apps/nebula-voice-v2/Cargo.toml --no-default-features --lib
```

No audio. No weights download. The live voice server is not contacted.
