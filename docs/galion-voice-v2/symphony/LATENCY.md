# Latency bar

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

The live tick is 80 ms. `galion-heartbeat::FRAME_MS` is 80. The same constant is the Mimi frame, the render dot, and the console heartbeat. `LOCKS.toml` `[clock] frame_ms = 80` and `frame_budget_p95_ms` stays at or under 80. The clock is not configurable. This page does not edit the lock.

`Heartbeat::step` never blocks and never allocates. A late frame is a pause, never a catch-up. Those two rules are already tested: `crates/galion-heartbeat/tests/zero_alloc.rs` and `late_frame_is_a_pause_no_extra_trigger_no_burst` in `tests/heartbeat.rs`.

## 1% of the tick

Non-inference work on the tick is at most 1% of 80 ms, which is 800 µs. The design aim from the CPU foundation note is a p99 much tighter than that. The pass/fail ceiling this harness enforces is the 1% line: measured p99 ≤ 800 µs. No second cutoff is invented.

The measurement is `Heartbeat::step` with Rika v0 and the RMS/ZCR stub, after a short warmup, 256 frames, microsecond ranks. The printed line starts with `GALION_FRAME_BUDGET` and `kind=measured-stub`. Debug is the pessimistic profile. If debug p99 is over 800 µs, the check remeasures `--release` and records that profile instead. Either number is a measurement of this machine's stub step. It is not host CPU%, not ASR, not TTS, not the clone child, and not an EPYC 9655P, CX23, or CX43 result.

`step_duty_ppm` is p99 divided by the 80 ms frame, in parts per million. 10000 ppm is the 1% ceiling. It is the step's share of the frame, not the host's idle percentage.

## Sectors

| Sector | Bar | What is measured |
|---|---|---|
| `galion-heartbeat` step | 80 ms frame, p99 ≤ 800 µs, no alloc, late = pause | `measured-stub` when section G runs |
| Mimi frame | `FRAME_MS` 80 | Constant only. Kernel time is not measured. |
| Render dot | `FRAME_MS` 80 | Constant only. `Luti::STEP_COST` 48 is a flop hint, not a time. |
| Console heartbeat | `FRAME_MS` 80 | Constant only. |
| Live demo | Same tick. `--realtime` marks a missed wall deadline `late` | The demo's RTF is not a model number. See [LOCAL-DEMO.md](LOCAL-DEMO.md). |
| Phone turn, ASR, TTS | Off the tick. The turn worker is not inside `step` | No TTFA is recorded here. |
| Clone worker | Out of process | No latency number. The Rust test says the clock is not a product claim. |

CPU only, no GPU. Capacity for Plus (CX23-class) and Ultra (CX43-class) is planned in [CPU.md](CPU.md) and is not measured.

```bash
bash scripts/symphony/check-g-latency.sh
bash galion-gates/sy-frame.sh
```

Research notes that talk about mouth-to-ear in the hundreds of milliseconds (`docs/research/galion-voice-v2/domains/04-latency-streaming.md`) are a different clock. They are not the tick bar, and this harness does not copy them in as measurements.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
