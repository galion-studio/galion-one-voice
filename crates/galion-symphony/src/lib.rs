// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! # galion-symphony
//!
//! V6 one-percent engine account for the 80 ms tick.
//!
//! The non-inference engine (heartbeat, feeling band, this account) has two
//! bars and one pause rule:
//!
//! * **P1** — non-inference CPU ≤ 1% of the tick. 1% of 80 ms is 800 µs.
//! * **P3** — p99 of one tick's non-inference work is much less than 800 µs.
//!   The hard check is `p99 < 800 µs`. `p3_much_less` is true when p99 is
//!   strictly under 80 µs (one tenth of that budget).
//! * **Late frames are a pause.** A scheduler-late frame, a short you-band,
//!   or a step over the P1 budget is delivered once and not caught up.
//!
//! Stub durations and measured clocks are separate series. A stub sample
//! never flips a measured claim. `product_claim` is false: a local Instant
//! and a local process-CPU span are not a hosted SKU result.
//!
//! The clock sits outside [`galion_heartbeat::Heartbeat::step`]. That call
//! still does not read a clock.
//!
//! Design: `docs/galion-voice-v2/symphony/ONE-PERCENT.md`.

#![forbid(unsafe_code)]

mod cpu;

use std::time::Instant;

use galion_emotion::{ConsentGate, DefaultConsentGate, Purpose};
use galion_heartbeat::{FrameIn, FrameOut, Heartbeat, HeartbeatConfig, FRAME_SAMPLES, RIKA_BITS};

/// 80 ms tick, in nanoseconds. Same period as [`galion_heartbeat::FRAME_MS`].
pub const TICK_NS: u64 = 80_000_000;
/// P1 budget: 1% of [`TICK_NS`] = 800 µs.
pub const P1_BUDGET_NS: u64 = TICK_NS / 100;
/// P3 "much less" divisor. One tenth of the 1% budget is 80 µs.
pub const P3_LL_FACTOR: u64 = 10;
/// p99 strictly under this is `p3_much_less`.
pub const P3_LL_NS: u64 = P1_BUDGET_NS / P3_LL_FACTOR;
/// This tree does not emit a product latency or CPU claim.
pub const PRODUCT_CLAIM: bool = false;

/// Where a duration came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleKind {
    /// Caller-supplied nanoseconds. No clock was read for this sample.
    Stub,
    /// [`Instant`] around the non-inference step on this process.
    Measured,
}

impl SampleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SampleKind::Stub => "stub",
            SampleKind::Measured => "measured",
        }
    }
}

/// One non-inference tick, already timed or injected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickSample {
    pub engine_ns: u64,
    pub kind: SampleKind,
    /// Scheduler late, or the heartbeat already paused this frame.
    pub late: bool,
}

/// What the account did with one sample. `catch_up` is always 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickVerdict {
    pub paused: bool,
    pub over_budget: bool,
    pub kind: SampleKind,
    pub catch_up: u32,
}

#[derive(Clone, Debug)]
struct Series {
    ns: Vec<u64>,
    pauses: u32,
    over_budget: u32,
    late_flags: u32,
}

impl Series {
    fn new() -> Self {
        Self {
            ns: Vec::new(),
            pauses: 0,
            over_budget: 0,
            late_flags: 0,
        }
    }

    fn push(&mut self, engine_ns: u64, paused: bool, over: bool, late: bool) {
        self.ns.push(engine_ns);
        if paused {
            self.pauses = self.pauses.saturating_add(1);
        }
        if over {
            self.over_budget = self.over_budget.saturating_add(1);
        }
        if late {
            self.late_flags = self.late_flags.saturating_add(1);
        }
    }
}

/// Process-CPU span covering a measured window. Not used for stub samples.
#[derive(Clone, Copy, Debug)]
struct CpuWindow {
    cpu_ns: u64,
    resolution_ns: u64,
    source: &'static str,
    frames: u32,
}

/// The one-percent account. Stub and measured samples stay in separate series.
#[derive(Debug)]
pub struct OnePercent {
    stub: Series,
    measured: Series,
    cpu: Option<CpuWindow>,
    /// Span refused because measured samples already existed, or the counter did not move.
    cpu_note: Option<&'static str>,
}

impl Default for OnePercent {
    fn default() -> Self {
        Self::new()
    }
}

impl OnePercent {
    pub fn new() -> Self {
        Self {
            stub: Series::new(),
            measured: Series::new(),
            cpu: None,
            cpu_note: None,
        }
    }

    pub fn reserve_measured(&mut self, n: usize) {
        self.measured.ns.reserve(n);
    }

    pub fn observe(&mut self, sample: TickSample) -> TickVerdict {
        let over = sample.engine_ns > P1_BUDGET_NS;
        let paused = sample.late || over;
        let series = match sample.kind {
            SampleKind::Stub => &mut self.stub,
            SampleKind::Measured => &mut self.measured,
        };
        series.push(sample.engine_ns, paused, over, sample.late);
        TickVerdict {
            paused,
            over_budget: over,
            kind: sample.kind,
            catch_up: 0,
        }
    }

    fn measured_len(&self) -> usize {
        self.measured.ns.len()
    }

    /// Attach a process-CPU delta that covers exactly the measured samples already stored.
    fn attach_cpu(&mut self, sample: cpu::CpuSample, frames: u32) {
        if frames == 0 || self.measured.ns.len() != frames as usize || self.cpu.is_some() {
            self.cpu_note = Some("span-mismatch");
            return;
        }
        self.cpu = Some(CpuWindow {
            cpu_ns: sample.ns,
            resolution_ns: sample.resolution_ns,
            source: sample.source,
            frames,
        });
    }

    pub fn report(&self) -> Report {
        let late_flags = self
            .stub
            .late_flags
            .saturating_add(self.measured.late_flags);
        let pauses = self.stub.pauses.saturating_add(self.measured.pauses);
        // Every late sample is pushed with `paused = late || over`, so a late
        // flag always has a pause. Pauses can also come from the budget.
        let late_is_pause = pauses >= late_flags;
        Report {
            product_claim: PRODUCT_CLAIM,
            late_is_pause,
            catch_up_steps: 0,
            stub: series_report(&self.stub, SampleKind::Stub, None, None),
            measured: series_report(
                &self.measured,
                SampleKind::Measured,
                self.cpu,
                self.cpu_note,
            ),
        }
    }
}

fn series_report(
    series: &Series,
    kind: SampleKind,
    cpu: Option<CpuWindow>,
    cpu_note: Option<&'static str>,
) -> SeriesReport {
    let n = series.ns.len() as u32;
    let p99 = p99_ns(&series.ns);
    let max_ns = series.ns.iter().copied().max();
    let (under, much_less, ratio) = p3_flags(p99);
    let (share, share_hi, p1_pass, p1_note, p1_instrument, cpu_ns, cpu_resolution_ns, cpu_source) =
        match kind {
            SampleKind::Stub => {
                let sum: u128 = series.ns.iter().map(|&v| v as u128).sum();
                let (share, pass, note) = p1_from_sum(sum, n);
                (share, share, pass, note, "injected", None, None, None)
            }
            SampleKind::Measured => match cpu {
                Some(c) if c.frames == n => {
                    let (share, share_hi, pass, note) = p1_from_cpu(c.cpu_ns, c.resolution_ns, n);
                    (
                        share,
                        share_hi,
                        pass,
                        note,
                        c.source,
                        Some(c.cpu_ns),
                        Some(c.resolution_ns),
                        Some(c.source),
                    )
                }
                _ => (
                    None,
                    None,
                    None,
                    cpu_note.unwrap_or("unavailable"),
                    "none",
                    None,
                    None,
                    None,
                ),
            },
        };
    SeriesReport {
        kind,
        n,
        pauses: series.pauses,
        over_budget: series.over_budget,
        p99_ns: p99,
        max_ns,
        p3_under_budget: under,
        p3_much_less: much_less,
        p3_ratio: ratio,
        p3_instrument: match kind {
            SampleKind::Stub => "injected",
            SampleKind::Measured => "instant",
        },
        share,
        share_hi,
        p1_pass,
        p1_note,
        p1_instrument,
        cpu_ns,
        cpu_resolution_ns,
        cpu_source,
    }
}

fn p99_ns(samples: &[u64]) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut v = samples.to_vec();
    v.sort_unstable();
    let n = v.len() as u64;
    let rank = (99 * n + 99) / 100; // ceil(0.99 * n)
    Some(v[(rank - 1) as usize])
}

fn p3_flags(p99: Option<u64>) -> (Option<bool>, Option<bool>, Option<u64>) {
    match p99 {
        None => (None, None, None),
        Some(0) => (Some(true), Some(true), None),
        Some(p) => (
            Some(p < P1_BUDGET_NS),
            Some(p < P3_LL_NS),
            Some(P1_BUDGET_NS / p),
        ),
    }
}

fn p1_from_sum(sum_ns: u128, frames: u32) -> (Option<f64>, Option<bool>, &'static str) {
    if frames == 0 {
        return (None, None, "empty");
    }
    let window = frames as u128 * TICK_NS as u128;
    let pass = sum_ns.saturating_mul(100) <= window;
    let share = (sum_ns as f64) / (window as f64);
    (Some(share), Some(pass), if pass { "ok" } else { "over" })
}

fn p1_from_cpu(
    cpu_ns: u64,
    resolution_ns: u64,
    frames: u32,
) -> (Option<f64>, Option<f64>, Option<bool>, &'static str) {
    if frames == 0 {
        return (None, None, None, "empty");
    }
    let window = frames as u128 * TICK_NS as u128;
    if cpu_ns == 0 {
        if resolution_ns <= 1 {
            return (None, None, None, "unresolved");
        }
        let hi = resolution_ns as u128;
        let pass = hi.saturating_mul(100) <= window;
        let share_hi = (hi as f64) / (window as f64);
        return (
            None,
            Some(share_hi),
            Some(pass),
            if pass {
                "below-resolution"
            } else {
                "unresolved"
            },
        );
    }
    let pass = (cpu_ns as u128).saturating_mul(100) <= window;
    let share = (cpu_ns as f64) / (window as f64);
    let hi_ns = (cpu_ns as u128).saturating_add(resolution_ns.saturating_sub(1) as u128);
    let share_hi = (hi_ns as f64) / (window as f64);
    (
        Some(share),
        Some(share_hi),
        Some(pass),
        if pass { "ok" } else { "over" },
    )
}

/// One series in the gate JSON. Numbers are this process, or injected.
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesReport {
    pub kind: SampleKind,
    pub n: u32,
    pub pauses: u32,
    pub over_budget: u32,
    pub p99_ns: Option<u64>,
    pub max_ns: Option<u64>,
    pub p3_under_budget: Option<bool>,
    pub p3_much_less: Option<bool>,
    pub p3_ratio: Option<u64>,
    pub p3_instrument: &'static str,
    pub share: Option<f64>,
    pub share_hi: Option<f64>,
    pub p1_pass: Option<bool>,
    pub p1_note: &'static str,
    pub p1_instrument: &'static str,
    pub cpu_ns: Option<u64>,
    pub cpu_resolution_ns: Option<u64>,
    pub cpu_source: Option<&'static str>,
}

/// Gate report. `product_claim` is always false.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub product_claim: bool,
    pub late_is_pause: bool,
    pub catch_up_steps: u32,
    pub stub: SeriesReport,
    pub measured: SeriesReport,
}

impl Report {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"engine\":\"v6-one-percent\",\"product_claim\":{product},\
\"tick_ns\":{tick},\"p1_budget_ns\":{p1},\"p3_ll_ns\":{ll},\"p3_ll_factor\":{factor},\
\"late_is_pause\":{late},\"catch_up_steps\":{catch},\
\"stub\":{stub},\"measured\":{measured}}}",
            product = bool_json(self.product_claim),
            tick = TICK_NS,
            p1 = P1_BUDGET_NS,
            ll = P3_LL_NS,
            factor = P3_LL_FACTOR,
            late = bool_json(self.late_is_pause),
            catch = self.catch_up_steps,
            stub = self.stub.to_json(),
            measured = self.measured.to_json(),
        )
    }
}

impl SeriesReport {
    fn to_json(&self) -> String {
        format!(
            "{{\"kind\":\"{kind}\",\"n\":{n},\"pauses\":{pauses},\"over_budget\":{over},\
\"p99_ns\":{p99},\"max_ns\":{max},\
\"p3_under_budget\":{under},\"p3_much_less\":{ll},\"p3_ratio\":{ratio},\
\"p3_instrument\":\"{p3i}\",\"share\":{share},\"share_hi\":{share_hi},\
\"p1_pass\":{p1},\"p1_note\":\"{note}\",\"p1_instrument\":\"{p1i}\",\
\"cpu_ns\":{cpu},\"cpu_resolution_ns\":{res},\"cpu_source\":{src}}}",
            kind = self.kind.as_str(),
            n = self.n,
            pauses = self.pauses,
            over = self.over_budget,
            p99 = opt_u64(self.p99_ns),
            max = opt_u64(self.max_ns),
            under = opt_bool(self.p3_under_budget),
            ll = opt_bool(self.p3_much_less),
            ratio = opt_u64(self.p3_ratio),
            p3i = self.p3_instrument,
            share = opt_f64(self.share),
            share_hi = opt_f64(self.share_hi),
            p1 = opt_bool(self.p1_pass),
            note = self.p1_note,
            p1i = self.p1_instrument,
            cpu = opt_u64(self.cpu_ns),
            res = opt_u64(self.cpu_resolution_ns),
            src = opt_str(self.cpu_source),
        )
    }
}

fn bool_json(v: bool) -> &'static str {
    if v {
        "true"
    } else {
        "false"
    }
}
fn opt_bool(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "true",
        Some(false) => "false",
        None => "null",
    }
}
fn opt_u64(v: Option<u64>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    }
}
fn opt_f64(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{x:.8}"),
        _ => "null".to_string(),
    }
}
fn opt_str(v: Option<&str>) -> String {
    match v {
        Some(s) => format!("\"{s}\""),
        None => "null".to_string(),
    }
}

/// Symphony: one heartbeat plus the one-percent account.
///
/// Inference (ASR, reply, speech, clone) is not called from here.
pub struct Symphony<G: ConsentGate> {
    hb: Heartbeat<G>,
    account: OnePercent,
}

impl Symphony<DefaultConsentGate> {
    /// Rika v0 on, sense+adapt granted, personal-assistant purpose.
    /// The heavier non-inference path. Still no inference.
    pub fn granted_rika() -> Self {
        let mut gate = DefaultConsentGate::default();
        gate.grant(0, "en", Purpose::PersonalAssistant);
        Self::new(HeartbeatConfig::rika_v0(), gate)
    }
}

impl<G: ConsentGate> Symphony<G> {
    pub fn new(cfg: HeartbeatConfig, gate: G) -> Self {
        Self {
            hb: Heartbeat::new(cfg, gate),
            account: OnePercent::new(),
        }
    }

    pub fn heartbeat(&self) -> &Heartbeat<G> {
        &self.hb
    }

    pub fn catch_up_steps(&self) -> u32 {
        0
    }

    /// Record one injected or pre-timed sample. Does not step the heartbeat.
    pub fn observe(&mut self, sample: TickSample) -> TickVerdict {
        self.account.observe(sample)
    }

    /// Fixed stub script: seven 50 µs frames and one late 40 µs frame.
    /// Under the 1% budget. The late frame pauses. Nothing is caught up.
    pub fn load_stub_script(&mut self) {
        for _ in 0..7 {
            self.account.observe(TickSample {
                engine_ns: 50_000,
                kind: SampleKind::Stub,
                late: false,
            });
        }
        self.account.observe(TickSample {
            engine_ns: 40_000,
            kind: SampleKind::Stub,
            late: true,
        });
    }

    /// One tick. The clock is outside `Heartbeat::step`. One call, no catch-up.
    pub fn tick(&mut self, input: &FrameIn<'_>) -> (FrameOut, TickVerdict) {
        let started = Instant::now();
        let out = self.hb.step(input);
        let engine_ns = elapsed_ns(started);
        let verdict = self.account.observe(TickSample {
            engine_ns,
            kind: SampleKind::Measured,
            late: input.late || out.paused,
        });
        (out, verdict)
    }

    /// Warm the heartbeat, then time `n` silence frames.
    ///
    /// P3 uses [`Instant`] on each step. P1 uses process CPU across the same
    /// window divided by `n` ticks of 80 ms. If measured samples already
    /// exist, the CPU span is not attached.
    pub fn run_budget(&mut self, n: usize) -> Report {
        let pcm = [0f32; FRAME_SAMPLES];
        let frame = |late: bool| FrameIn {
            you_pcm: &pcm,
            affect: None,
            tension: None,
            consent_bits: RIKA_BITS,
            purpose: Purpose::PersonalAssistant,
            session_open: true,
            me_speaking: false,
            late,
        };
        for _ in 0..4 {
            let _ = self.hb.step(&frame(false));
        }
        if n == 0 || self.account.measured_len() != 0 {
            if self.account.cpu.is_none() {
                self.account.cpu_note = Some("span-mismatch");
            }
            return self.account.report();
        }
        self.account.reserve_measured(n);
        // schedstat moves on a schedule point. Yield so the opening sample is
        // current, then yield again so the window is published. Neither yield
        // is inside Heartbeat::step. On-CPU time only; time spent waiting is
        // not counted.
        let _ = cpu::read();
        std::thread::yield_now();
        let start = cpu::read();
        for _ in 0..n {
            let _ = self.tick(&frame(false));
        }
        std::thread::yield_now();
        let end = cpu::read();
        if let (Some(a), Some(b)) = (start, end) {
            if let Some(d) = cpu::delta(a, b) {
                self.account.attach_cpu(d, n as u32);
            } else {
                self.account.cpu_note = Some("unavailable");
            }
        } else {
            self.account.cpu_note = Some("unavailable");
        }
        self.account.report()
    }

    pub fn report(&self) -> Report {
        self.account.report()
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    let d = started.elapsed();
    d.as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(d.subsec_nanos()))
}

/// Gate hook. Stub script plus one measured silence window on this process.
/// `measured_frames` is the measured series length. `product_claim` is false.
pub fn gate_report(measured_frames: usize) -> Report {
    let mut symphony = Symphony::granted_rika();
    symphony.load_stub_script();
    symphony.run_budget(measured_frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_matches_the_heartbeat_period() {
        assert_eq!(TICK_NS, u64::from(galion_heartbeat::FRAME_MS) * 1_000_000);
        assert_eq!(P1_BUDGET_NS, 800_000);
        assert_eq!(P3_LL_NS, 80_000);
        assert!(!PRODUCT_CLAIM);
        let locks = include_str!("../../../LOCKS.toml");
        assert!(locks.contains("frame_ms = 80"));
    }

    #[test]
    fn p99_rank_is_ceil_of_99_percent() {
        let one = [7u64];
        assert_eq!(p99_ns(&one), Some(7));
        let eight = [
            50_000u64, 50_000, 50_000, 50_000, 50_000, 50_000, 50_000, 40_000,
        ];
        assert_eq!(p99_ns(&eight), Some(50_000));
    }
}
