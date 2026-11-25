# S73 — A hold does not open a new topic

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S73, brain plan round 2, R2-C |
| Written | 2026-11-25. No weight download, no voice audio, no paid API, no merge. |
| Base | `main` `fea54fb9af9eb098170c443cd7d67a556a113638`, then merged `26239c9` (PR #327, `docs/horbm/PLAN-brain.md`) |
| Spec | `docs/horbm/PLAN-brain.md` section R2-C |
| Gap | `DialogueLean.hold_topic` is already computed. `Dialogue::decide` does not read it, so a selected `Move::OfferTopic` can still say `Back to {topic}?`. |
| This change | `Dialogue::decide_held` and this note. No new dependency. |
| Not edited | `horbm-inner`, `brain.rs`, `lib.rs`, `tests/dialogue.rs`, `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, workflows, root and horbm manifests, `Cargo.lock`. `SOUL.md` is not in this tree. |

## What it decides

`Dialogue::decide` stays. It calls `decide_held(input, false)`.

`Dialogue::decide_held(&mut self, input, hold_topic: bool)` takes a plain `bool`. It does not read `horbm-inner`. `false` is the same decision, the same ring, and the same bandit counts as `decide`. `true` leaves every other move alone. When the heuristic has already selected `Move::OfferTopic`, that selection becomes `Move::Ask` before the offer line is built, so the turn does not format `Back to {topic}?` and does not speak a stored callback. Comfort can score just under an offer. A hold still asks. It does not take that next score.

The offline bandit still only counts the move that is returned, and only when it is enabled and learn is `grant`. The count does not pick the move. A research hit still cannot be an answer or a citation. An independent card clause still answers. Remember, learn, and check-ins are unchanged. Nothing here allocates on a path `decide` did not already take, and nothing here does I/O or takes a lock.

`Brain::turn` still calls `decide`. Round 3 is the caller that passes `note.lean().hold_topic`.

The new test is a unit test on `Dialogue`. The type is crate-private, and this swarm does not edit `lib.rs`, so `tests/dialogue.rs` cannot call it. That file is unchanged. `Brain::turn` still offers a topic when `decide` does.

Nothing added here is Linux-only. No `cfg` gate, no hard-coded path, no `.exe`. The run below is Linux, rustc 1.85.0 (`4d91de4e4`, 2025-02-17).

## Tests

`hold_false_matches_decide_and_hold_true_blocks_offer_topic` plays the same lines through `decide` and `decide_held(false)`, including an enabled bandit with learn granted and with learn denied. The decisions and the ring match. `decide_held(true)` turns the `something else` offer into ask, keeps the other moves, and the ring clause is `I don't know.` rather than `Back to kettle?`. Thirty counted comforts do not flip the offer, and the held vent-plus-switch is ask rather than comfort. A granted check-in callback is not spoken while the hold is on. A research hit is not an answer and not a citation, and the held offer does not speak the example. A card clause with that hit still in hand stays an answer.

`cargo test` rewrote `voice-stack/Cargo.lock` while resolving. That file is not part of this change and was restored. No `Cargo.toml` and no lockfile is committed.

| Command | Result |
|---|---|
| `cargo test --manifest-path horbm/Cargo.toml -p horbm --lib hold_false_matches_decide_and_hold_true_blocks_offer_topic` | 1 passed |
| `cargo test --manifest-path horbm/Cargo.toml -p horbm --test dialogue` | 17 passed |
| `cargo test --manifest-path horbm/Cargo.toml --workspace` | 279 passed, 0 failed, 2 ignored |
| `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 1069 passed, 0 failed, 5 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
