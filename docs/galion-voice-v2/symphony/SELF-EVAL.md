# Symphony heartbeat self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb · 2026-11-03 · t287.

The spelling is **Galion**. The product is Galion Voice.

Harness: `docs/galion-voice-v2/symphony/HEARTBEAT-WIRE.md`.
Command: `scripts/eval/symphony-heartbeat.sh`.
Toolchain: Rust 1.85.0. Profile: `cargo test` (debug). This file records that run.

## Verdict

**PASS**

| Check | Verdict | Evidence |
|---|---|---|
| `FRAME_MS` is 80, three bands, three sectors | PASS | `frame_is_80ms_and_three_bands_and_three_sectors` |
| `Heartbeat::step` and `SymphonyWire::step` allocate nothing | PASS | counting allocator, `allocs=0`, including late frames (`steps_allocate_nothing`, `zero_alloc`) |
| Neither step blocks | PASS | source scan has no lock, channel, sleep, clock, or heap call; warm p99 under 800 µs |
| Late frame is a pause, and the next frame does not catch up | PASS | pause copies 0 and leaves the me buffer; the following step copies 1920, and the queued tail stays |
| Persona hold is 8 frames; feeling lands on the next frame; one frame never flips | PASS | `persona_needs_eight_frames_and_feeling_lands_next_frame`; floor test `empathy_switch_needs_eight_unpinned_frames` |
| One-dot mark: speak wins, pause holds the mark | PASS | `one_dot_speak_wins_and_a_pause_holds_the_mark`; `sectors_match_heartbeat_frame` |
| Yield copies nothing and asks the host to release the slot | PASS | `yield_releases_the_slot_and_copies_nothing` |
| Clone drain size is one heartbeat frame | PASS | `frame_matches_heartbeat`; `drain_takes_one_heartbeat_frame_and_leaves_the_tail` |
| Live frame loop still runs with the wire in the loop | PASS | `galion-live-demo` `tests/live.rs` (feature `dot-viz`) |
| Chatterbox `generate` (model PCM) | BLOCKED | child stays off the step; generate is still unwired; weights are not in the tree. The drain size is wired. This row does not fail the harness. |

`LOCKS.toml` was not edited.

## Measured step

Gate line from `harness_budget_and_verdict` (warmup 64 frames dropped, then 4032 frames, Rika on, one late frame in 64):

```text
verdict=PASS frame_ms=80 bands=3 sectors=3 allocs=0 late=pause p99_us=118.009 max_us=148.642 budget_us=800 n=4032
```

The ceiling is 800 µs. Three debug passes in this session stayed under it: p99 106.639 / 118.009 / 130.342 µs, max 138.387 / 148.642 / 158.090 µs. A later run may print another pair. The harness fails when either number reaches 800.

The clock read is outside `step`. The counting allocator is armed only around the two `step` calls.

## Fail and blocked

A failing assertion, or a missing `verdict=PASS` line, is **FAIL**. The script exits 1.

`cargo` missing on the machine is **BLOCKED**. The script exits 2.

Model PCM from Chatterbox is **BLOCKED** for this gate, as the table says. The 80 ms harness itself passed.

— Maciej Grajczyk
