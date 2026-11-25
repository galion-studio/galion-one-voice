# S48 — native mouth, no Piper

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S48. First-principles audit step 5. |
| Written | 2026-11-25. No voice audio, no weights, no merge. |
| Base | `main` `67ade7dab35894f0ff5f0665c3647e70533dedd5`. |
| Process | `samantha-local` in `voice-stack/local-server`. The socket is the one this process already serves. |
| Mouth | `NativeMouth` when `GALION_MOUTH` is `native`, `horbm`, or `text`. Today's mouth otherwise. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. |

## What it decides

The HORBM turn already returns `TurnOut.first_clause` and `TurnOut.rest` from `horbm::Brain::turn`. This server does not edit that crate. The native mouth reads those two fields and speaks them, in that order, as text frames on the socket the dot page already renders (`type: turn`, `reply` set to the piece). It returns an empty sample buffer. It does not spawn Piper, and it does not link Flite, Chatterbox, or a rewritten synthesizer.

`PiperMouth` stays behind `--features piper`. With `GALION_MOUTH` unset, the process speaks the way it does today: Piper when that feature is on, otherwise the in-process tone, and the s711 clone only under the existing cuda rule. An unrecognised `GALION_MOUTH` falls back to that same choice.

A card such as the shipped kettle line is two words, then the remainder (`The kettle`, then `is on the stove.`). The miss `I don't know.` stays one piece, because that is how `horbm::TurnOut` stores it. When a turn has no clause fields (a test double, not the HORBM mind), the native mouth speaks the whole line once and still returns no samples.

There is no MIT or Apache-2.0 speaker in this process. PCM stays empty until one is actually linked. The 80 ms clock, the ear, and the brain are unchanged.

## Selector

| `GALION_MOUTH` | Mouth |
|---|---|
| unset, `piper`, `cpu`, `auto`, `today`, `tone`, `s711`, `fake` | Today's mouth (`decide`) |
| `native`, `horbm`, `text` | Native text mouth |
| anything else | Today's mouth, with one log line |

`/health` reports `"mouth":"native"` and `"voice":"text"` when the native mouth is selected. `mouth_device` stays `cpu`.

## Files

- `voice-stack/local-server/src/mouth.rs` — `NativeMouth`, `GALION_MOUTH`, clause pair checked against `horbm::TurnOut`
- `voice-stack/local-server/src/live.rs` — socket feed, no PCM on that path
- `voice-stack/local-server/src/server.rs` — text frames on the existing socket
- `voice-stack/local-server/src/brain.rs` — copies `first_clause` and `rest` off the reply the bridge already returned
- `voice-stack/local-server/PROTOCOL.md` — the wire
- this note

`horbm/crates/**`, `crates/galion-sideband`, and `crates/galion-conductor` were not edited. The live server on port 7881 was not restarted.
