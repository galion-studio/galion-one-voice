# S50 — local clause first

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S50, speak the local clause before Galion Research |
| Written | 2026-11-25. No weight download, no voice file, no paid API, no merge. |
| Base | `main` `67ade7dab35894f0ff5f0665c3647e70533dedd5` |
| Step | First-principles audit step 4. The integration gap is that `first_clause` is not what 7881 speaks. |
| Persona | Samantha only. Aria is not loaded. |
| Consent | `remember=deny`, `learn=deny`, `learn_in_band=false`. Not edited. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. |

## What it decides

`Brain::turn` builds the local answer, then returns it. Galion Research is not on that call.

The local answer is the person line, the retrieval cell, the card, the idle fixture, or `UNKNOWN_REPLY` (`I don't know.`), then the dialogue move. `TurnOut.reply` is that whole line. `TurnOut.first_clause` is the first spoken piece: at most two words when the clause can be cut, and the whole miss when it cannot. Both fields are set before any research ask.

A research port is asked only when all of these hold:

- the port is attached
- the act is not comfort
- the local route is a deep miss (`ResearchOff`, because the turn does not pass the port into `route_heard`)

The ask is armed on a helper thread and blocked on the returned `TurnOut`. `ResearchAsk::ask` runs when the last clone of that value is dropped. Until then the call count stays zero, including when `ask` would sleep for `RESEARCH_CAP` (3 seconds). Dropping the value does not wait for the answer.

`Brain::research_follow_up` is non-blocking. `None` means the worker has not finished. `Some` is the gateway result: outcome, speakable line when there is one, sources, and the gap. It does not rewrite `reply` or `first_clause`. The gap queue replaces the turn's `ResearchOff` row for that question when the follow-up is reaped. The next `turn` reaps as well, and it does not wait if the worker is still in `ask`.

No port is deny. `AskOutcome::Unreachable` is offline. Those two turns match on `reply`, `first_clause`, `rest`, `sources`, `evidence`, `arm`, and whether a side-band mark was posted. The offline case then reports `ResearchUnreachable` on the follow-up. The spoken turn stays the miss.

`route_heard` is unchanged for a caller that passes a port. The bridge router still asks inside that function. `Brain::turn` does not. This crate still dials nothing. `posted` on the side-band mark stays false.

## What 7881 still does

The mouth on loopback 7881 copies `reply`. It does not copy `first_clause`. That bridge is outside this change. After this turn, `reply` is the local clause, and the call returns without holding the research cap. A later bridge can speak `first_clause` first. The follow-up is already the place a later turn would read the research line.

## Files

- `horbm/crates/horbm/src/brain.rs` — local turn, armed follow-up, `research_follow_up`
- `horbm/crates/horbm/src/route.rs` — `classify_ask` shared with the synchronous router
- `horbm/crates/horbm/src/lib.rs` — `TurnOut` keeps `reply` and `first_clause`
- `horbm/crates/horbm/src/dialogue/mod.rs` — the move runs before the ask
- `horbm/crates/horbm/tests/clause_first.rs` — the clause does not wait; deny and offline match
- `horbm/crates/horbm/tests/turn.rs`, `tests/samantha_inner.rs` — the old in-turn wait is now a follow-up
- this note

Not edited: `horbm/crates/horbm-brain`, `voice-stack/**`, `crates/galion-sideband`, `crates/galion-conductor`, `LOCKS.toml`, `SOUL.md`, root `Cargo.toml`, workflows. No new dependency. No socket, no address, no audio file.

The helper thread is `std::thread`. That is portable. Nothing here is Linux-only.
