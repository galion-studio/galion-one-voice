# S53 — timing gates

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S53. Timing gates that stay meaningful on a loaded laptop. |
| Written | 2026-11-25. No weight download, no voice audio, no paid API, no merge. |
| Base | `main` `5d14d7b6c589884691f1b9e001f00f7664a219ea` |
| Gap | `fill_frame_allocates_nothing_and_meets_the_tick_budget` failed when one wall-clock sample reached 2.006 ms against the 800 µs budget. The other ticks were inside the budget. |
| This change | The `galion-ml-voice` tick gate, and this note. `TICK_BUDGET` stays 800 µs. |
| Not edited | `horbm/crates/horbm-soak`, `LOCKS.toml`, `SOUL.md`, root workspace members, workflows. |
| Consent | `remember=deny`, `learn=deny`, `learn_in_band=false`. Not edited. No research card. Samantha stays the persona. |

## What it decides

The one-percent budget is still 800 µs. That number is `TICK_BUDGET` in `crates/galion-ml-voice/src/artifact.rs`: one percent of the 80 ms frame. The tick test judges it on the 99th percentile after warmup. It does not judge it on the slowest sample.

The zero-allocation check is unchanged and exact. The counting allocator is armed around `fill_frame`, a deliberate allocation must be visible, and the armed calls must add zero. The allocator is disarmed before the timed series, so the counter is not inside the tick.

The clock is thread CPU time on Unix and wall time elsewhere.

- Unix (`cfg(unix)`): `libc::clock_gettime` with `CLOCK_THREAD_CPUTIME_ID`. `libc` 0.2 is already in the workspace lock. The voice library stays `deny(unsafe_code)`. The two `unsafe` calls live in the integration test, next to the counting allocator.
- Any other target: `std::time::Instant`.

`clock_gettime` has to succeed on Unix. A failure fails the test. A clock that does not move fails the test too (`p50` of the gated series must be greater than zero), so a stuck clock cannot pass by reporting zeros.

The ranked series is 4,096 ticks after 64 warmup ticks. The warmup drop matches the heartbeat wire harness. One sample is 1/4096 of the series, so it cannot move p99. The index is the same rounding the old gate used, in integer arithmetic: `round((n - 1) * 0.99)`, half up. For 4,096 that index is 4,054. For the old length of 2,000 it is 1,979.

The single-sample ceiling is ten times the budget: 8 ms. That is `TICK_BUDGET * 10`. It is not a second budget. A sample that lands on 8 ms fails, because the comparison is strict `<`. The ceiling is still under one frame.

Wall time is still printed (`wall_p99_ns`, `wall_max_ns`). It is not a gate on Unix. The wall interval wraps the two CPU-clock reads, so on a quiet run it sits a few hundred nanoseconds above the CPU sample. A multi-millisecond gap between them is time the thread was not running.

## Measured

Host: Linux 6.12.113+, 4 cores, the hypervisor's `Intel(R) Xeon(R) Processor` at 2400 MHz. `rustc` 1.85.0. Debug, the `cargo test` profile, which is the profile that runs this gate. `CLOCK_THREAD_CPUTIME_ID` resolution was 1 ns. Each run is one process of `fill_frame_allocates_nothing_and_meets_the_tick_budget`: 64 warmup ticks, then 4,096 ranked ticks. `fill_frame` copies or clears 1,920 `f32` samples (7,680 bytes).

The 800 µs number was not raised. Thread-CPU p99 stayed between 7.0 µs and 13.1 µs, about sixty to a hundred times under the budget, including the loaded runs. The worst thread-CPU sample in the whole set was 110 µs, still under 800 µs.

The comparison that moved is the single-sample check, from 800 µs to 8 ms.

Quiet, 50 runs, load average about 0.1:

| Stat | Thread CPU | Wall |
|---|---:|---:|
| p50 of the per-run medians | 266 ns | (not gated) |
| p99, min … max across runs | 7.0 µs … 13.1 µs | 7.2 µs … 13.4 µs |
| max, median across runs | 19 µs | 24 µs |
| max, worst run | 49 µs | 2.511 ms |

One quiet run is the reported failure in miniature. Wall max was 2,511,061 ns (2.511 ms). On that same run the thread-CPU p99 was 7,593 ns and the thread-CPU max was 14,180 ns. The old gate failed the run on the wall max. The new gate passes it. The budget was not the number that was wrong.

Loaded, 50 runs. Sixteen `yes` processes, the test niced to 19, load average 15 on 4 cores:

| Stat | Thread CPU | Wall |
|---|---:|---:|
| p99, min … max across runs | 7.0 µs … 11.5 µs | 7.2 µs … 11.8 µs |
| max, worst run | 110 µs | 859 ms |
| runs with that clock's max ≥ 800 µs | 0 | 50 |

The worst CPU sample and a long wall stall were the same run: thread-CPU max 109,774 ns, wall max 578,006,858 ns, both p99 figures still about 12 µs. Another loaded run reached a wall max of 858,568,150 ns with a thread-CPU max of 16,408 ns.

Pinned to one core with eight `yes` processes, 20 runs: thread-CPU p99 stayed at or under 10.4 µs and thread-CPU max at or under 21 µs. Wall p99 stayed at or under 10.8 µs. Wall max was 1.14 s to 1.15 s on every run. One parked sample does not move p99. It does fail a max check written against 800 µs, and it would also fail an 8 ms wall check. That is why the Unix gate reads the thread clock. The 8 ms ceiling is not an attempt to absorb a parked thread.

Why the ceiling is 8 ms, and not a higher or lower number:

- It has to sit above the reported 2.006 ms sample. Twice the budget is 1.6 ms, which is still under that sample. Three times is 2.4 ms, with almost no margin over it. Ten times is 8 ms, four times the incident.
- It has to stay under a frame. 8 ms is a tenth of 80 ms. A tick that spends 8 ms of this thread's CPU on a 7,680-byte copy fails. That copy's worst measured CPU sample here was 110 µs, so 8 ms is about seventy times that sample. The room is for one outlier, not for a slower function.
- Leaving the max check at 800 µs would have passed these CPU series (the worst sample was 110 µs). It would still fail the reported incident on the wall-clock fallback, and it would fail a single CPU-accounted fault in that same 2 ms class. The percentile gate is what keeps 800 µs meaningful. Measured p99 did not ask for a higher budget, so the budget stays.

## Soak, later

`samantha_duplex_long_call` is not edited here. The same pattern applies when that test is next changed. The voice numbers above are not a measurement of `Duplex::tick` and must not be copied onto it.

Today the soak times `Instant` around `Duplex::tick` and stores every tick, including the 250 warmup ticks, in the window vector. Every 3,750 ticks become a window. The rank is `ceil(pct / 100 * n) - 1` in `horbm_soak::stats::percentile`. `Report::enforce` then requires:

| Gate | Constant | Value |
|---|---|---|
| First window p99 | `TICK_P99_MAX_NS` | 50 ms |
| First window max | `TICK_MAX_NS` | 90 ms |
| Later window p99 above the first | `P99_DRIFT_NS` | 2 ms |
| Later window max above the first | `MAX_DRIFT_NS` | 20 ms |

`F-soak.md` measured the wall p99 of that tick near 20 ms and the wall max near 29 ms. The allocation gates are exact and separate: idle 0, hold 1, playback 4, probes 0, RSS growth ≤ 128 KiB, buffer ≤ 29,440 samples, emotion span ≤ 0.05, barge 160 ms.

What flakes is a single wall sample. It sets `max_ns`, which can miss `TICK_MAX_NS` or `MAX_DRIFT_NS` while p99 is fine. The voice runs show the shape: wall p99 stayed near 8 µs while one wall sample ran past a second.

When that test is changed, in a later patch that owns `horbm-soak`:

1. Leave the allocation, RSS, buffer, emotion, and barge gates alone. Do not retune them in the same patch as the clock.
2. Read the clock the same way. On Unix, `libc::clock_gettime(CLOCK_THREAD_CPUTIME_ID)` immediately before and after `duplex.tick`. Elsewhere, `Instant`. Print the clock name, the gated p99, the gated max, and the wall max. Wall max stays a diagnostic on Unix. Fail the run if the Unix clock call fails or the gated p50 is zero.
3. Drop the 250 warmup ticks from the ranked series. They already exist for the allocation buckets and the RSS snapshot. Today they still sit inside window 0's wall vector. Rank only ticks after warmup. Keep 3,750 as the window length unless a new measurement says why it should change.
4. Keep the soak's own percentile rank. Do not switch it to the voice test's `round((n - 1) * 0.99)` in the same patch. The clock and the max policy are the change. A new index would make the before and after incomparable.
5. Keep `TICK_P99_MAX_NS` at 50 ms and `P99_DRIFT_NS` at 2 ms unless a thread-CPU measurement of this tick says otherwise. The voice p99 of about 8 µs is a 7,680-byte copy. It is not evidence that the duplex tick has that much room, and it is not a reason to lower 50 ms either.
6. Do not copy the 8 ms voice ceiling onto the soak. A ceiling under the measured ~20 ms body of `Duplex::tick` would fail the real call. Re-run the five-minute test (and the hour if it is run) on the thread clock and record per-window CPU p99, CPU max, and wall max.
7. Keep `TICK_MAX_NS` at 90 ms if the new CPU max fits under it. If CPU max is under 90 ms while wall max is over it, the clock change is what removes the flake, and 90 ms should stay. It still catches a tick that burned 90 ms of CPU. Raise it only with those CPU numbers written into `F-soak.md`.
8. If, after the clock switch, one window's CPU max still exceeds the first window's CPU max by more than 20 ms while p99 does not move, drop the max-drift check and keep the absolute ceiling only. Two noisy maxima compared with each other are a flake of their own. Do not raise `MAX_DRIFT_NS` to hide wall time.

`libc` is a dev-dependency of `galion-ml-voice` only. The soak crate should take the same direct dev-dependency in that later patch, at the version already locked for its workspace. This note does not add it there.

## Files

- `crates/galion-ml-voice/tests/voice_artifact.rs` — the gate.
- `crates/galion-ml-voice/src/artifact.rs` — comment on `TICK_BUDGET` only. The constant is still `Duration::from_micros(800)`.
- `crates/galion-ml-voice/Cargo.toml` — `libc` dev-dependency.
- `Cargo.lock` — `galion-ml-voice` now depends on the `libc` that was already locked.
- `docs/horbm/foundations/S53-timing-gates.md` — this note.

Linux note: the thread clock is POSIX, behind `cfg(unix)`. This measurement was Linux. Non-Unix builds use `Instant` for the same p99 budget and the same 8 ms ceiling. A non-Unix run can still fail that ceiling when the scheduler parks the thread, as the loaded wall column shows. The supported host is Linux, where the gate is thread CPU time.
