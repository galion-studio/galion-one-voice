# W21-02 · Sector 01 Stack / A6 Mimi: synthetic streaming codec lane for the live demo

Date: 2026-11-02 (Europe/Zagreb) · Signed: Maciej Grajczyk
Scope: LOCAL ONLY. No git push, gh, VPS, weight downloads, he-IL or training.

## Result
The Mimi streaming encode/decode is now callable from the live demo. Each call takes one 80 ms PCM frame,
encodes it to codes, decodes the codes and returns one 80 ms PCM frame. The entry points are
`galion_student_backend::MimiPassthrough::step_frame` and `StudentBackend::step_frame`.
**`IS_STUB` stays `true`.** The lane is built only on `ZeroWeights` (real kernels, all-zero tensors) or on the
no-kernel shape path. Codes are all 0 and the codec output is silence. That is not Mimi audio, and
`StepReport.is_stub` makes sure the UI labels it "synthetic".

## API (crates/galion-student-backend)
```rust
pub const FRAME_SAMPLES: usize = 1920;               // 80 ms @ 24 kHz (== galion_heartbeat::FRAME_SAMPLES)
pub enum KernelMode { NoKernels, ZeroWeights }       // no real-weights variant on purpose
pub enum Monitor { Codec /*default*/, DryBypass }    // DryBypass: codec runs, out = sanitised copy of in, labelled
pub struct MimiLaneConfig { pub mimi: MimiConfig, pub kernels: KernelMode, pub monitor: Monitor }
impl MimiLaneConfig { fn demo() -> Self; fn full() -> Self; fn no_kernels() -> Self; fn demo_mimi() -> MimiConfig; fn with_monitor(self, Monitor) -> Self }
pub struct StepReport { frame: u64, paused: bool, codec_ran: bool, bypassed: bool, codes_nonzero: bool,
                        codec_peak: f32, sanitized: u32, is_stub: bool }      // Copy
impl MimiPassthrough {
    fn new(cfg: MimiLaneConfig) -> Result<Self, MimiLaneError>;   // all allocation here
    fn demo() -> Result<Self, MimiLaneError>;
    fn step_frame(&mut self, pcm_in: &[f32], pcm_out: &mut [f32]) -> Result<StepReport, MimiLaneError>; // 0 alloc
    fn reset(&mut self); fn set_monitor(&mut self, Monitor); fn last_codes(&self) -> &[u16]; fn frames(&self) -> u64;
}
impl StudentBackend {
    fn demo() -> Result<Self, MimiLaneError>;            // stub + demo lane
    fn with_mimi(lane: MimiPassthrough) -> Self;
    fn step_frame(&mut self, pcm_in: &[f32], pcm_out: &mut [f32]) -> Result<StepReport, MimiLaneError>;
    fn mimi(&self) / mimi_mut(&mut self) -> Option<&(mut) MimiPassthrough>;
}   // ReplayBackend::reset() now also resets the lane; ReplayBackend::step unchanged.
```
Semantics:
- `pcm_out.len() != 1920` returns `Err(Shape)`. This is the only error a caller can trigger.
- `pcm_in.len() != 1920` is a **paused** frame. The output is silence, the codec is not stepped and the call returns `Ok`.
  A late frame is a pause, never a glitch.
- **Tone-safe:** `pcm_out` is always finite and clamped to [-1, 1]. Non-finite samples become 0, and `sanitized` counts the samples that were changed.
- Hot path: no heap allocation, locks, I/O or clock reads.

## How W21-01 should call it
```toml
# crates/galion-live-demo/Cargo.toml
galion-student-backend = { path = "../galion-student-backend" }
```
```rust
use galion_student_backend::{StudentBackend, Monitor, FRAME_SAMPLES};
let mut student = StudentBackend::demo()?;                 // once, at startup
// optional, audible demo: student.mimi_mut().unwrap().set_monitor(Monitor::DryBypass);
let mut me = [0.0f32; FRAME_SAMPLES];
// per 80 ms heartbeat, alongside Heartbeat::step(&FrameIn { you_pcm, .. }):
let rep = student.step_frame(you_pcm, &mut me)?;          // me = codec output (silence today)
// console: show "MIMI synthetic (IS_STUB)" when rep.is_stub; "BYPASS" when rep.bypassed
```
**Build the demo with `--release`.** The demo config is not real time in a debug build (see below).
For the cheapest real-time-safe option use `MimiPassthrough::new(MimiLaneConfig::no_kernels())`.

## Latency (this box, scalar kernel path, 440 Hz tone, enc+dec per 80 ms frame)
`cargo run --release -p galion-student-backend --example mimi_passthrough [demo|no-kernels|full] [frames]`

| Config | Build | Mean | Worst | Real time (<80 ms) |
|---|---|---|---|---|
| `demo` (release): 8 filters, 64-d, 2 layers, 4 heads, ffn 256, ctx 32, 8×256 codebooks | 2.3 ms | 18.8 ms | 37.0 ms | yes |
| `no-kernels` (release), full shapes | 0.0 ms | 0.006 ms | 0.009 ms | yes |
| `full` (release), full Mimi VERIFY shapes on ZeroWeights | 403 ms | 1290 ms | 1311 ms | **no** (matches W20-15) |
| `demo` (debug) | 2.8 ms | 151 ms | 159 ms | **no**. Use `--release` |

The demo widths are for demo use only. They are not Mimi hyper-parameters. The frame is still 1920 samples, 80 ms, 12.5 Hz.

## Tests
- `cargo test -p galion-student-backend --offline`: 6 unit tests, 8 in `tests/mimi_lane.rs` (1 ignored) and
  1 in `tests/mimi_zero_alloc.rs`. All pass.
  - The demo ZeroWeights lane runs 12 frames, and the no-kernels full-shape lane runs 25 frames, without panic. Codes are 0 and output is silence.
  - DryBypass is labelled, and its output is clipped to ±1 with NaN replaced by 0. A short input frame produces a pause with silence. A bad output buffer returns `Err`.
  - `reset` restarts the stream. `StudentBackend::step_frame` works with and without a lane, and `ReplayBackend::reset` resets the lane.
  - The counting allocator records 0 allocations in `step_frame` across demo, DryBypass, NoKernels, a paused frame and `reset`.
  - `IS_STUB` (both the re-export and `galion_mimi::IS_STUB`) is still `true`.
- `cargo test --release -p galion-student-backend --test mimi_lane -- --ignored`: the full-size ZeroWeights lane passes 2 frames.
- `cargo test --workspace --exclude galion-live-demo --offline`: **311 passed, 0 failed, 4 ignored**. Building the student-backend
  tests and examples produces 0 warnings. `galion-live-demo` (W21-01, still being built) was excluded because its `src/main.rs` did not exist yet when the tests ran.

## Files
- `crates/galion-student-backend/Cargo.toml`: adds the `galion-mimi` path dependency and the example.
- `crates/galion-student-backend/src/mimi_lane.rs` (new): the lane, config, report, sanitiser and `tone_frame` helper.
- `crates/galion-student-backend/src/lib.rs`: module and re-exports, `StudentBackend::{demo, with_mimi, step_frame, mimi, mimi_mut}`,
  and `reset` now resets the lane.
- `crates/galion-student-backend/tests/{mimi_lane.rs, mimi_zero_alloc.rs}`, `examples/mimi_passthrough.rs` (new).
- Not touched: `crates/galion-mimi`, galion-tensor (A5 kernels), galion-heartbeat. Pre-change backup: `/workspace/w21-02-backup/galion-student-backend/`.

## Still blocking `IS_STUB = false` (unchanged)
A21 licence clearance. A7 `.gq` → `WeightSource` adapter, including the `permute_qk_interleaved_to_neox` step. Golden frames.
The `VERIFY` hyper-parameters. Real-time performance at full size (SIMD/parallel GEMV, int8).

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
