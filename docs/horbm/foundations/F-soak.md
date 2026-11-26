# F — duplex soak (S47)

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S47. One long call on Samantha's duplex loop. |
| Crate | `horbm/crates/horbm-soak`. Member of the `horbm` workspace. |
| Clock | 80 ms, virtual. `Duplex::tick` is the only step. 5 minutes is 3,750 ticks. 60 minutes is 45,000. |
| Persona | Samantha. The mouth is Flite (`slt`) when `libflite` opens, otherwise the native tone. |
| Seed | `0x53414d414e544831`. The schedule and the sine come from this seed. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not added. |

The microphone is a generated sine while the script is speaking, and zeros in the pauses. There is no recording and no model. Prosody stays on (the default). Echo cancellation stays on. The emotion port is `CoreAffect` (`EmotionCore::step`). The brain is the duplex brain. The mouth writes each utterance into a fixed output ring and the pull takes one frame back.

A turn can follow another turn. When the mouth queue is empty and the caller has been quiet for 1.28 s, the floor listens again. That wait is longer than the end-to-end smoke's 800 ms tail, so a finished clip can still be barged. After a barge, three quiet frames (240 ms) listen again. The partial is cleared when the utterance closes, so a residual does not look like a new sentence. The barge measurement is the virtual time from the first interrupting frame to silence. It was 160 ms on every sample, at the start and at the end.

`GALION_SOAK_FULL=1` runs the hour. Any other value, including unset, runs five minutes. The Ubuntu job `voice-stack horbm router` runs the five-minute test and does not set the flag.

## What allocates

Steady ticks were counted after 250 ticks of warmup, with a counting allocator. Each bucket's minimum and maximum were the same number.

| Stage | Allocations per call | Why |
|---|---|---|
| `ProsodyEar::hear` | 0 | Autocorrelation is on the stack. |
| `CoreAffect::step` | 0 | `EmotionCore::step` does not allocate. |
| Output ring | 0 | The storage is allocated once, at 10 seconds. |
| `PlaybackEcho::cancel_into` | 0 | The `fdaf-aec` 0.1.0 overlap-save steps run on buffers stored for the call, including the FFT scratch. `FdafAec::process` builds a new scratch `Vec` on every block, and that churn was what kept growing the resident set. |
| Idle tick | 0 | Echo, prosody, emotion, and the output ring add none. |
| Hold tick | 1 | `Duplex::tick` clones the held partial into `FrameOut`. |
| Playback tick | 4 | `Conductor::tick` clones the four strings of the committed clause. |

Word reveals, mouth start, eager end of turn, and barge frames are outside those buckets. Flite synthesis runs on the commit frame, not on the steady pull.

`Brain` records a card miss while a partial is still arriving (`where`, then `where is`, before `where is the kettle`). The queue keeps the newest 32 misses and drops the older ones. It used to keep every miss for the life of the call.

## Measured

Debug `cargo test -p horbm-soak`, prosody on, echo on. The growth gate is live heap: requested bytes still outstanding, from the test binary's counting allocator, at the end minus the warmup sample at tick 250. The limit is 128 KiB (131072 bytes). `VmRSS` from `/proc/self/status` and exact `Rss` from `/proc/self/smaps_rollup` are printed in KiB and are not gated. Wall times are nanoseconds around `Duplex::tick`.

Sixty minutes, mouth `flite` (45,000 ticks, 776 turns, 161 barges, 85.8 s wall). The resident set, which is not the gate, grew 24 KiB after warmup. Emotion did not move: `IntentMsg::steady` holds the stand-in baseline. The buffer high-water mark was 28,160 samples. Every barge was 160 ms.

```text
alloc idle n=15354 min=0 max=0 play n=8431 min=4 max=4 hold n=1540 min=1 max=1
rss_kib start=10200 warm=13944 end=13968 peak=13968
buffer max_samples=28160 end_samples=14080 overflows=0
```

| Window | p50 ns | p99 ns | max ns |
|---|---:|---:|---:|
| 0 | 932465 | 20491075 | 21615891 |
| 1 | 913508 | 20383711 | 21547104 |
| 2 | 912975 | 20475682 | 28931263 |
| 3 | 913413 | 20467442 | 21449657 |
| 4 | 911735 | 20483755 | 21178828 |
| 5 | 914963 | 20483480 | 21001016 |
| 6 | 914914 | 20489664 | 21061264 |
| 7 | 915836 | 20425227 | 21175904 |
| 8 | 912329 | 20418449 | 20770993 |
| 9 | 912107 | 20432350 | 20847579 |
| 10 | 910493 | 20407150 | 20968059 |
| 11 | 909742 | 20402539 | 20700987 |

No later window's p99 rose above the first. The largest max sat 7.3 ms above the first window's max.

The native mouth, which is what the Ubuntu job runs when `libflite` is absent, grew 32 KiB of resident set in five minutes (warm 8304, end 8336) and 48 KiB over sixty (warm 8024, end 8072, peak 8072). The same hour before the queue cap grew 180 KiB and was still climbing at the last window. Those readings are `VmRSS`. The current report also prints `heap_bytes` and `smaps_rss_kib`.

## Gates

Fail closed. A missing live-heap warmup sample, an empty steady bucket, or fewer than two barges fails. A missing `VmRSS` or smaps read is printed and does not fail the run. Allocation gates are the measured counts (the buckets did not vary). The other gates are the measurement plus the margin named here.

| Gate | Value |
|---|---|
| Idle / hold / playback allocs | 0 / 1 / 4, and hold = idle + 1, playback = idle + 4 |
| Prosody, emotion, output ring, echo probes | 0 |
| Live heap growth after warmup | ≤ 128 KiB (131072 bytes). `VmRSS` and smaps `Rss` are printed and are not gated. |
| Buffer depth | ≤ 29,440 samples (measured 28,160, plus one frame) |
| Emotion span per axis, and no non-finite sample | ≤ 0.05 (measured 0), inside the axis limits |
| Barge, start and end and every sample | 160 ms |
| Later window p99 / max above the first | ≤ 2 ms / ≤ 20 ms |
| First window p99 / max | ≤ 50 ms / ≤ 90 ms |
