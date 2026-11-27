# S75 — Soak RSS flake

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S75. Intermittent resident-set growth in `samantha_duplex_long_call`. |
| Crate | `horbm/crates/horbm-soak` only. No new dependency. |
| Gate | 128 KiB of `VmRSS`, growth and peak growth, warm to end. Not raised. S186 moved the gate to live heap bytes, end minus the warmup sample. The limit stayed 128 KiB. `VmRSS` is printed and is not gated. |
| Clock | Virtual 80 ms. Five minutes is 3,750 ticks. Warm sample stays at tick 250. |

## What was growing

`VmRSS` is sampled at the end of tick 250 and after the last tick. Growth is end minus warm. On this host, before the fix, ten runs with different `RUST_HASH_SEED` values grew by exactly 24 KiB with Flite (for example warm 38900, end 38924). The native mouth grew by 32 KiB. Steady allocation counts did not move: idle 0, hold 1, playback 4. Live bytes at the end of the call were lower than at the warm sample, so the extra pages were freed objects the allocator had not returned.

The same run's first large renders are not spread across the call. A counting allocator recorded every allocation of at least 130 KiB. With Flite they were 162,240 bytes, at ticks 27, 50, 84, 115, 185, 247, 287, and 345. Tick 247 is three ticks before the warm sample. The next one is tick 287, after it. The duplex events around that window were: committed at tick 27, first audio at 28, barge at 31, and another first audio at tick 248.

162,240 bytes is 158.4 KiB, above glibc's default `mmap` threshold of 128 KiB (`DEFAULT_MMAP_THRESHOLD` in glibc 2.39). Freeing that mapping raises the dynamic threshold to the freed chunk, up to 32 MiB on 64-bit. A later allocation below the new threshold stays in the heap. `mallinfo2` at tick 251 showed one live mapping of 323,584 bytes (the fixed 10-second output ring, `16000 * 10 * 2`) and a heap arena of 1,552,384 bytes. The 162,240-byte renders were already inside that arena or had been returned. The arena size did not change between tick 251 and the end of `run` on this host, which is why growth stayed at 24 KiB here.

That 24 KiB is not the flake. The reported spikes are 156, 184, and 204 KiB on unchanged main (PR #327, a #331 bare-head run, and once on #321). 158 KiB plus the steady 24 to 40 KiB is that band. On this host the second 162,240-byte render (tick 50) is the one that can stick, and every later one is the same size, so tick 247 does not raise the high water. The warm sample is only three ticks after that copy. A schedule that lands the first retained copy after tick 250, which this host did not do, is a 158 KiB step. File pages are the other intermittent source: the old prefault ran once, before `libflite` was opened, and a reclaim before tick 250 followed by a fault after it counts as growth. `RssFile` was flat here (35,512 KiB at both samples). It is not flat when the page cache is under pressure.

## What changed

`SoakMouth::open` renders one long line twice before it returns. `Duplex` is built after that, and `rss_start` is read after `Duplex::new`, so both the threshold update and the retained chunk exist before either measured sample. Later utterances of the same shape reuse that chunk. The line is longer than a soak reply (`The kettle is on the hob.` twice). The samples are dropped. The tick path is the same: commit frames still render, and the steady buckets stay 0 / 1 / 4.

The file prefault runs again immediately before the warm read and immediately before the end read. Both samples include every file mapping that exists then, including `libflite` opened during `SoakMouth::open`. A page reclaimed between the two reads is faulted for both, so it is not growth. A new anonymous page is. The maps buffer is reused, so the second and third reads do not allocate a fresh string.

The warm tick is still 250. That is after the primed renders, after the first commit (tick 27), the first audio (tick 28), and the first barge (tick 31). It is not an arbitrary later tick.

The report line prints `persona={id}` from `Persona::id` on the persona the duplex loaded. The format string does not name a persona.

## Measured after the fix

Debug `cargo test -p horbm-soak --test soak`, rustc 1.85.0, Linux, glibc 2.39. Flite `slt`. Growth is end minus warm. Peak equalled end on every run. Ten runs:

| Run | start | warm | end | peak | growth KiB | p99 ns | max ns |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 39892 | 41424 | 41424 | 41424 | 0 | 20703275 | 24607189 |
| 2 | 39908 | 41424 | 41424 | 41424 | 0 | 20886346 | 28768737 |
| 3 | 39924 | 41424 | 41424 | 41424 | 0 | 20581640 | 23746085 |
| 4 | 39924 | 41424 | 41424 | 41424 | 0 | 20878175 | 25209862 |
| 5 | 39916 | 41428 | 41428 | 41428 | 0 | 21559467 | 24946294 |
| 6 | 39928 | 41428 | 41428 | 41428 | 0 | 20894961 | 26112049 |
| 7 | 39924 | 41424 | 41424 | 41424 | 0 | 20941402 | 29847490 |
| 8 | 39924 | 41424 | 41424 | 41424 | 0 | 20805737 | 23026691 |
| 9 | 39924 | 41424 | 41424 | 41424 | 0 | 20564916 | 23866779 |
| 10 | 39924 | 41424 | 41424 | 41424 | 0 | 20696765 | 23082645 |

Before the fix the same binary's p99 sat around 20.6 to 22.6 ms and the max around 22 to 36 ms. These are in that band.

The native mouth (`GALION_FLITE_VOICE` set to an unknown id, so Flite is not opened) grew 36 KiB: start 33076, warm 33144, end 33180, peak 33180, p99 3.14 ms, max 4.40 ms. The Ubuntu job installs `libflite1` and takes the Flite path.

Nothing here is Linux-only beyond the existing `/proc/self/maps` prefault, which already was. The prime runs on every platform; it only renders samples.
