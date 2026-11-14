# Nebula live feed

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Schema `galion-nebula-live-feed/1`. This is the contract for the visualiser at `apps/nebula-voice-v2`. That program is not in this repository. It reads one JSON object per line from stdout, or one JSON object per datagram from a loopback UDP port.

The feed takes per-frame or per-utterance emotion posteriors from the existing Emotion V2 scorer (a harness `Analysis`, or the same `labels` + `posterior` pair) and emits a smoothed distribution. It does not score audio itself. It does not change voice conditioning, a deployed voice, `LOCKS.toml`, `SOUL.md`, `config.yaml`, a persona TOML, `thresholds.toml`, or `emotion-v2/emotions.toml`. It does not widen the 8-byte `EmotionWire`. It commits no audio.

## Frame

Parsers use field names. Whitespace is not significant. Weight keys are emitted in Emotion V2 order, then `seductive` for Samantha. A Jarvis frame omits `seductive`.

This line is encoder output. `t_ms` 0 was a Neutral posterior. `t_ms` 80 is one heartbeat later, after a Happy posterior, with `tau_ms` 240:

```json
{"schema":"galion-nebula-live-feed/1","voice_id":"samantha","t_ms":80,"kind":"frame","present":true,"distribution":true,"tau_ms":240,"weights":{"neutral":{"weight":0.716531,"gas_line":"HeII-541.2"},"calm":{"weight":0,"gas_line":"Hbeta-486.1"},"happy":{"weight":0.283469,"gas_line":"[NII]-575.5"},"amused":{"weight":0,"gas_line":"HeI-587.6"},"excited":{"weight":0,"gas_line":"NeI-594.5"},"tender":{"weight":0,"gas_line":"HeI-667.8"},"sad":{"weight":0,"gas_line":"HeII-468.6"},"disappointed":{"weight":0,"gas_line":"[ClIII]-551.7"},"angry":{"weight":0,"gas_line":"[SIII]-631.2"},"fearful":{"weight":0,"gas_line":"Hdelta-410.2"},"surprised":{"weight":0,"gas_line":"[OIII]-495.9"},"disgusted":{"weight":0,"gas_line":"[ClIII]-553.8"},"seductive":{"weight":0,"gas_line":"Halpha-656.3"}},"valence":0.226775,"arousal":0.113387,"dominance":0.056694,"energy":0.341734}
```

An absence, before any estimate. Weights are null. This is not a Neutral weight of 1. Gas-line ids are still present so the visualiser can bind particles:

```json
{"schema":"galion-nebula-live-feed/1","voice_id":"jarvis","t_ms":0,"kind":"frame","present":false,"distribution":false,"tau_ms":240,"weights":{"neutral":{"weight":null,"gas_line":"HeII-541.2"},"calm":{"weight":null,"gas_line":"Hbeta-486.1"},"happy":{"weight":null,"gas_line":"[NII]-575.5"},"amused":{"weight":null,"gas_line":"HeI-587.6"},"excited":{"weight":null,"gas_line":"NeI-594.5"},"tender":{"weight":null,"gas_line":"HeI-667.8"},"sad":{"weight":null,"gas_line":"HeII-468.6"},"disappointed":{"weight":null,"gas_line":"[ClIII]-551.7"},"angry":{"weight":null,"gas_line":"[SIII]-631.2"},"fearful":{"weight":null,"gas_line":"Hdelta-410.2"},"surprised":{"weight":null,"gas_line":"[OIII]-495.9"},"disgusted":{"weight":null,"gas_line":"[ClIII]-553.8"}},"valence":null,"arousal":null,"dominance":null,"energy":null}
```

| Field | Meaning |
|---|---|
| `schema` | `galion-nebula-live-feed/1` |
| `voice_id` | `samantha` or `jarvis` |
| `t_ms` | Milliseconds since the stream started. Integer, `>= 0`. Strictly increasing inside one `kind`. |
| `kind` | `frame` (live) or `utterance` (closed utterance). Separate smoothers. An utterance line does not move the frame cloud. |
| `present` | `false` when this kind has no numeric estimate yet. |
| `distribution` | `true` when every weight for this voice is numeric and the weights sum to 1 within `1e-3`. |
| `tau_ms` | EMA time constant in milliseconds. See below. |
| `weights` | Object. Key is the emotion id. Value is `{ "weight", "gas_line" }`. |
| `weights.*.weight` | Probability in `0..1`, or `null` when that channel has never been measured. JSON numbers are rounded to 6 decimal places. |
| `weights.*.gas_line` | Fixed id from the table below. Opaque to the visualiser except for the wavelength after the last hyphen. |
| `valence`, `arousal`, `dominance` | Emotion V2 axes in `-1..1`, or `null`. Not rescaled into the `0..1` range used by `galion-emotion`'s arousal and dominance. |
| `energy` | Normalized speech energy in `0..1`, or `null`. `0` is silent and `1` is full scale. This is not the prosody energy control. |

`seductive` is Samantha only. A Jarvis posterior that includes it is rejected. A coarse head (SUPERB's four classes, emotion2vec's nine) is rejected rather than expanded into these twelve.

## Gas-line ids

Hard-coded. The emotion id is the key. The visualiser, this feed, and any later training label for these names use these rows. The line is a label. It does not change acoustic output.

| Emotion | Id | Wavelength nm |
|---|---|---:|
| Neutral | `HeII-541.2` | 541.2 |
| Calm | `Hbeta-486.1` | 486.1 |
| Happy | `[NII]-575.5` | 575.5 |
| Amused | `HeI-587.6` | 587.6 |
| Excited | `NeI-594.5` | 594.5 |
| Tender | `HeI-667.8` | 667.8 |
| Sad | `HeII-468.6` | 468.6 |
| Disappointed | `[ClIII]-551.7` | 551.7 |
| Angry | `[SIII]-631.2` | 631.2 |
| Fearful | `Hdelta-410.2` | 410.2 |
| Surprised | `[OIII]-495.9` | 495.9 |
| Disgusted | `[ClIII]-553.8` | 553.8 |
| Seductive | `Halpha-656.3` | 656.3 |

Seductive is Samantha only. Excited's `NeI-594.5` is the laboratory line in this thirteen. Disappointed and Disgusted share an ion and not an id.

## EMA

Time constant `tau_ms`, in milliseconds. Default **240**. Override with `--tau-ms` or with `tau_ms` in a replay file. The flag wins over the file.

```text
alpha = 1 - exp(-dt_ms / tau_ms)
y = y + alpha * (x - y)
```

`dt_ms` is the gap since that channel's previous measurement, not since the previous emitted line. The first measurement of a channel is copied. A hold, or a posterior that leaves valence, arousal, dominance, or energy out, repeats the last value and does not move it. After one time constant a step has moved `1 - 1/e` of the way (about 63%). At the 80 ms heartbeat and the default 240 ms constant, `alpha = 1 - exp(-80/240)` which is about 0.283.

The constant belongs to this side stream. It is not `GatedDetector`'s coefficient and not Rika's.

A categorical posterior must name exactly the voice's emotions and sum to 1 within `1e-3`. Partial category maps are refused. VAD axes and energy may be omitted one at a time.

## Latency

The heartbeat is **80 ms** (`LOCKS.toml` `[clock] frame_ms`, 12.5 Hz). It is not configurable, and this feed does not add a clock. The compute budget inside that frame is 60 ms p95. `affect_to_voice_max_ms` stays 160.

This package does not run a recogniser. The work on the heartbeat, once a posterior already exists, is the EMA and one JSON object. `test_smooth_and_encode_stays_inside_one_heartbeat` requires one hundred of those frames to finish inside a single 80 ms heartbeat. A twelve-way neural head is not on this path. It stays off the heartbeat until a measurement shows it fits the 60 ms budget.

The visualiser reads the side stream. A slow draw drops a line. It does not delay audio, and it does not write voice conditioning. A full Samantha frame is under 1000 bytes. The UDP payload limit in this package is 4096 bytes.

## Replay and live stdin

Numbers only. The example file has no audio.

```bash
PYTHONPATH=tools/emotion-v2 python -m nebula replay \
  tools/emotion-v2/nebula/examples/posteriors.json

PYTHONPATH=tools/emotion-v2 python -m nebula replay FILE.json \
  --tau-ms 240 --udp 127.0.0.1:8765

PYTHONPATH=tools/emotion-v2 python -m nebula feed \
  --voice jarvis --udp 127.0.0.1:8765
```

`replay` accepts three files:

- an object `{ "schema": "galion-nebula-replay/1", "voice_id", "tau_ms", "posteriors": [ ... ] }`
- a JSON array of posteriors, with `--voice`
- JSONL (suffix `.jsonl`), one posterior per line, `#` comments allowed, with `--voice`

`feed` reads the same posterior objects as JSON lines on stdin.

A posterior is either `weights` (emotion id to probability) or `labels` plus `posterior` in harness order. Not both. Optional `kind` (`frame` default, or `utterance`), `valence`, `arousal`, `dominance`, `energy`. `t_ms` is required.

Stdout is newline-delimited JSON, one frame per line, flushed per line. `--no-stdout` turns that off. `--udp` sends the same object as one datagram with no trailing newline. The destination must be loopback (`127.0.0.1`, `localhost`, or `[::1]`). The suggested port is `127.0.0.1:8765`. The feed sends. The visualiser binds. Passing neither stdout nor UDP is an error.

```bash
python -m unittest discover -s tools/emotion-v2/nebula/tests -v
```

Exit status is `0` on success and `2` when the file, the voice, or a posterior is unusable.

## UI reading

The picture does not read this wire directly. `docs/galion-voice-v2/EMOTION-NEBULA-FEED.md` is the contract in front of it: an explicit `estimate` of `none`, `distribution`, or `partial`, a check before each send, and a reconnect backoff. `estimate: none` is this file's absence (`present: false`, null weights). It is not a Neutral weight of 1. The live voice server is not part of that bridge.
