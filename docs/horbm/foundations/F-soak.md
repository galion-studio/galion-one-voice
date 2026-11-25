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
| `PlaybackEcho::cancel_into` | 60 | `fdaf_aec::FdafAec::process` returns a new `Vec<f32>` and builds intermediate FFT buffers. One frame is five 256-sample blocks. |
| Idle tick | 60 | The same 60. The reused ear frame adds none. |
| Hold tick | 61 | Idle, plus `Duplex::tick` cloning the held partial into `FrameOut`. |
| Playback tick | 64 | Idle, plus `Conductor::tick` cloning the four strings of the committed clause. |

Word reveals, mouth start, eager end of turn, and barge frames are outside those buckets. Flite synthesis runs on the commit frame, not on the steady pull.

## Measured

Debug `cargo test -p horbm-soak`, mouth `flite`, prosody on, echo on. RSS is `VmRSS` from `/proc/self/status`, in KiB. Wall times are nanoseconds around `Duplex::tick`.

Five minutes (3,750 ticks, 84 turns, 18 barges, 11.1 s wall):

```text
alloc idle n=1443 min=60 max=60 play n=891 min=64 max=64 hold n=156 min=61 max=61
barge start_ms=160 end_ms=160
rss_kib start=8452 warm=11928 end=11948 peak=11948
buffer max_samples=28160 end_samples=11520 overflows=0
emotion min=0.1500,0.3500,0.4800,0.0500 max=0.1500,0.3500,0.4800,0.0500
window 0 p50_ns=1893026 p99_ns=21384136 max_ns=44536622
```

Sixty minutes (45,000 ticks, 976 turns, 196 barges, 137.8 s wall). RSS growth after warmup was 20 KiB. Emotion did not move: `IntentMsg::steady` holds the stand-in baseline. The buffer high-water mark was 28,160 samples. Every barge was 160 ms.

| Window | p50 ns | p99 ns | max ns |
|---|---:|---:|---:|
| 0 | 1882765 | 21242302 | 23318000 |
| 1 | 1775871 | 21263854 | 23618469 |
| 2 | 1928125 | 21225192 | 24518309 |
| 3 | 1808972 | 21261335 | 25936245 |
| 4 | 1815517 | 21334323 | 23356591 |
| 5 | 1808886 | 21293981 | 21584740 |
| 6 | 1788248 | 21284283 | 23465665 |
| 7 | 1819335 | 21241581 | 22841765 |
| 8 | 1807643 | 21259570 | 30803872 |
| 9 | 1798237 | 21269557 | 30493771 |
| 10 | 1805739 | 21305542 | 29588499 |
| 11 | 1787177 | 21350166 | 30376402 |

p99 rose 0.11 ms from the first window to the last. The largest max sat 7.5 ms above the first window's max.

## Gates

Fail closed. A missing RSS read, an empty steady bucket, or fewer than two barges fails. Allocation gates are the measured counts (the buckets did not vary). The other gates are the measurement plus the margin named here.

| Gate | Value |
|---|---|
| Idle / hold / playback allocs | 60 / 61 / 64, and hold = idle + 1, playback = idle + 4 |
| Prosody, emotion, output ring probes | 0 |
| RSS growth after warmup | ≤ 128 KiB (measured 20) |
| Buffer depth | ≤ 29,440 samples (measured 28,160, plus one frame) |
| Emotion span per axis, and no non-finite sample | ≤ 0.05 (measured 0), inside the axis limits |
| Barge, start and end and every sample | 160 ms |
| Later window p99 / max above the first | ≤ 2 ms / ≤ 20 ms |
| First window p99 / max | ≤ 50 ms / ≤ 90 ms |
