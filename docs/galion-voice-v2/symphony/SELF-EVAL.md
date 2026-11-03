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
| Nano ring drain is one frame, hard | PASS | `nano_ring_drains_one_frame_and_a_late_tick_does_not_catch_up`. Late tick pops nothing. The next tick takes 1920. The tail stays. A full ring drops overflow instead of growing a catch-up queue. `steps_allocate_nothing` drains the ring inside the counting allocator (`allocs=0`). |
| Piper feed, `GALION_SPEECH=a21` | HARD, measured | `a21_piper_pcm_feeds_one_heartbeat_frame`. Real Piper PCM, native 22050 Hz resampled to 24 kHz. 41 full frames, `scheduler_late=0`, `allocs=0`, late=pause. `piper_measured=true`. `GALION_CLONE_STUB_FRAMES` is not this feed. The harness fails the ship if that line is missing. |
| Live frame loop still runs with the wire in the loop | PASS | `galion-live-demo` `tests/live.rs` (feature `dot-viz`), including `live_loop_drains_one_nano_frame_and_leaves_the_tail` |
| Chatterbox `generate` (model PCM) | BLOCKED | The worker does not import torch and does not call generate. Weights are not in the tree. That row does not fail the harness. |

`LOCKS.toml` was not edited.

## Measured step

Gate line from `harness_budget_and_verdict` (warmup 64 frames dropped, then 4032 frames, Rika on, one late frame in 64):

```text
verdict=PASS frame_ms=80 bands=3 sectors=3 allocs=0 late=pause p99_us=107.175 max_us=118.434 budget_us=800 n=4032
```

The ceiling is 800 µs. The timed region includes the Nano ring drain. Other debug passes in this session stayed under it: p99 103.553 / 107.175 / 109.088 µs, max 118.434 / 135.156 / 359.494 µs. A later run may print another pair. The harness fails when either number reaches 800.

## Piper feed

`GALION_SPEECH=a21`, Piper `en_US-ljspeech-medium` on CPU. Native 22050 Hz resampled to 24 kHz. One frame is 1920 samples. Late frame paused and did not pop the ring. The next step took one frame. `allocs=0`. Stub frames were not the source.

```text
piper_measured=true engine=piper-cpu voice=en_US-ljspeech-medium native_hz=22050 rate_hz=24000 frame=1920 frames=41 scheduler_late=0 first_frame_ms=343 drain_us=0.512 late_us=1.102 allocs=0 late=pause
```

Phrase: `Streaming check. Galion Voice on Piper CPU.` `first_frame_ms` is submit until the ring held one frame, including the cold Piper load. `drain_us` is one `Heartbeat::step` plus `SymphonyWire::step` on that PCM. `frames=41` is every full 1920-sample frame the step took after that. Playback steps were not marked late (`scheduler_late=0`). The short tail under one frame stays queued. A separate arrival-time count of the same phrase, same pack, resampled 22050 to 24000, was 42 frames including that tail, `late_ticks=0`. `GALION_CLONE_STUB_FRAMES` is not this run. The harness exits 1 if `piper_measured=true` is missing.

The clock read is outside `step`. The counting allocator is armed only around the two `step` calls.

## Fail and blocked

A failing assertion, or a missing `verdict=PASS` line, is **FAIL**. The script exits 1.

`cargo` missing on the machine is **BLOCKED**. The script exits 2.

Model PCM from Chatterbox is **BLOCKED**. The ring feed that was measured is Piper under `GALION_SPEECH=a21`, with `piper_measured=true`. `GALION_CLONE_STUB_FRAMES` is not that measurement. The 80 ms harness itself passed.

— Maciej Grajczyk
