// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Long-call soak for the configured persona's duplex loop.
//!
//! [`run`] drives [`horbm_duplex::Duplex::tick`] on the virtual 80 ms clock.
//! Five virtual minutes is the CI length. `GALION_SOAK_FULL=1` runs sixty.
//! The microphone is generated in memory. Prosody stays on. The mouth is
//! Flite when the library opens, and the native tone otherwise.
//!
//! # Safety
//!
//! No `unsafe`.

#![forbid(unsafe_code)]

mod buffer;
mod drive;
mod mouth;
mod recog;
mod stats;

pub use drive::{run, ticks_for, SEED};
pub use stats::{parse_smaps_rss, parse_vm_rss, smaps_rss_kib, vm_rss_kib};

/// Steady-path allocation counts. `min == max` means the count did not move.
#[derive(Clone, Copy, Debug)]
pub struct AllocSpan {
    pub n: u64,
    pub min: u64,
    pub max: u64,
}

impl AllocSpan {
    pub(crate) fn new() -> Self {
        Self {
            n: 0,
            min: u64::MAX,
            max: 0,
        }
    }

    pub(crate) fn push(&mut self, value: u64) {
        self.n = self.n.saturating_add(1);
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }
}

/// Wall time for one 5-minute block of ticks.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub index: u32,
    pub ticks: u64,
    pub p50_ns: u64,
    pub p99_ns: u64,
    pub max_ns: u64,
}

/// What one soak measured. The gate line is [`Report::table`].
pub struct Report {
    pub minutes: u64,
    pub ticks: u64,
    pub seed: u64,
    /// Id reported by the persona the duplex actually loaded.
    pub persona: String,
    pub mouth: &'static str,
    pub prosody: bool,
    pub aec: bool,
    pub barges: Vec<u64>,
    pub idle: AllocSpan,
    pub play: AllocSpan,
    pub hold: AllocSpan,
    pub echo_probe: u64,
    pub prosody_probe: u64,
    pub emotion_probe: u64,
    pub buffer_probe: u64,
    pub windows: Vec<Window>,
    /// Requested bytes still live when the clock starts.
    pub heap_start: u64,
    /// Requested bytes still live at the end of the warmup tick.
    pub heap_warm: Option<u64>,
    /// Requested bytes still live after the last tick.
    pub heap_end: u64,
    pub rss_start: Result<u64, String>,
    pub rss_warm: Option<Result<u64, String>>,
    pub rss_end: Result<u64, String>,
    pub rss_peak: u64,
    pub smaps_start: Result<u64, String>,
    pub smaps_warm: Option<Result<u64, String>>,
    pub smaps_end: Result<u64, String>,
    pub buffer_max: usize,
    pub buffer_end: usize,
    pub buffer_overflows: u64,
    pub desync: u64,
    pub emotion_non_finite: u64,
    pub emotion_out_of_range: u64,
    pub emotion_min: [f32; 4],
    pub emotion_max: [f32; 4],
    pub user_sent: u64,
    pub user_heard: u64,
    pub prosody_bad: u64,
    pub turns: u32,
    pub failures: Vec<String>,
}

impl Report {
    /// Stages that allocate, and the ones that do not.
    ///
    /// Counts are the probes and the steady buckets from this run.
    #[must_use]
    pub fn stages(&self) -> String {
        format!(
            "ear prosody ProsodyEar::hear allocates {prosody} per frame (stack autocorrelation)\n\
             emotion core CoreAffect::step / EmotionCore::step allocates {emotion} per frame\n\
             output buffer enqueue and dequeue allocate {buffer} (fixed ring)\n\
             echo PlaybackEcho::cancel_into allocates {echo} per frame; the fdaf-aec overlap-save steps run on buffers stored for the call, including the FFT scratch\n\
             steady idle tick allocates {idle_min}..={idle_max} (n={idle_n})\n\
             steady hold tick allocates {hold_min}..={hold_max} (n={hold_n}); a held partial is cloned into FrameOut\n\
             steady playback tick allocates {play_min}..={play_max} (n={play_n}); Conductor::tick clones the committed clause onto TickOut",
            prosody = self.prosody_probe,
            emotion = self.emotion_probe,
            buffer = self.buffer_probe,
            echo = self.echo_probe,
            idle_min = show_span(self.idle),
            idle_max = self.idle.max,
            idle_n = self.idle.n,
            hold_min = show_span(self.hold),
            hold_max = self.hold.max,
            hold_n = self.hold.n,
            play_min = show_span(self.play),
            play_max = self.play.max,
            play_n = self.play.n,
        )
    }

    /// One block of counts. No transcript and no samples.
    ///
    /// `heap_bytes` is the growth gate. `rss_kib` is `VmRSS`. `smaps_rss_kib`
    /// is exact `Rss` from `/proc/self/smaps_rollup` when that file can be
    /// read. Those two resident-set lines are information.
    #[must_use]
    pub fn table(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "soak minutes={} ticks={} seed={:#x} persona={} mouth={} prosody={} aec={} turns={}",
            self.minutes,
            self.ticks,
            self.seed,
            self.persona,
            self.mouth,
            self.prosody,
            self.aec,
            self.turns
        ));
        lines.push(format!(
            "alloc idle n={} min={} max={} play n={} min={} max={} hold n={} min={} max={}",
            self.idle.n,
            show_span(self.idle),
            self.idle.max,
            self.play.n,
            show_span(self.play),
            self.play.max,
            self.hold.n,
            show_span(self.hold),
            self.hold.max
        ));
        lines.push(format!(
            "probe prosody={} emotion={} buffer={} echo={}",
            self.prosody_probe, self.emotion_probe, self.buffer_probe, self.echo_probe
        ));
        lines.push(format!(
            "barge n={} start_ms={} end_ms={} values={}",
            self.barges.len(),
            self.barges.first().copied().unwrap_or(0),
            self.barges.last().copied().unwrap_or(0),
            barge_list(&self.barges)
        ));
        lines.push(format!(
            "heap_bytes start={} warm={} end={} growth={}",
            self.heap_start,
            show_heap(self.heap_warm),
            self.heap_end,
            self.heap_warm
                .map(|warm| self.heap_end.saturating_sub(warm).to_string())
                .unwrap_or_else(|| "unread".to_string())
        ));
        lines.push(format!(
            "rss_kib start={} warm={} end={} peak={}",
            show_rss(&self.rss_start),
            self.rss_warm
                .as_ref()
                .map(show_rss)
                .unwrap_or_else(|| "unread".to_string()),
            show_rss(&self.rss_end),
            self.rss_peak
        ));
        lines.push(format!(
            "smaps_rss_kib start={} warm={} end={}",
            show_rss(&self.smaps_start),
            self.smaps_warm
                .as_ref()
                .map(show_rss)
                .unwrap_or_else(|| "unread".to_string()),
            show_rss(&self.smaps_end)
        ));
        lines.push(format!(
            "buffer max_samples={} end_samples={} overflows={} desync={}",
            self.buffer_max, self.buffer_end, self.buffer_overflows, self.desync
        ));
        lines.push(format!(
            "emotion non_finite={} out_of_range={} min={} max={}",
            self.emotion_non_finite,
            self.emotion_out_of_range,
            fmt_axes(&self.emotion_min),
            fmt_axes(&self.emotion_max)
        ));
        lines.push(format!(
            "user_frames sent={} heard={} prosody_bad={}",
            self.user_sent, self.user_heard, self.prosody_bad
        ));
        for window in &self.windows {
            lines.push(format!(
                "window {} ticks={} p50_ns={} p99_ns={} max_ns={}",
                window.index, window.ticks, window.p50_ns, window.p99_ns, window.max_ns
            ));
        }
        if !self.failures.is_empty() {
            lines.push(format!("failures {}", self.failures.join(" | ")));
        }
        lines.join("\n")
    }

    /// Fail closed. A missing live-heap sample is a failure.
    ///
    /// `VmRSS` and smaps `Rss` are printed by [`Report::table`] and are not
    /// gated.
    pub fn enforce(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        if !self.failures.is_empty() {
            problems.push(self.failures.join(" | "));
        }
        if !self.prosody {
            problems.push("ear prosody is off".to_string());
        }
        if !self.aec {
            problems.push("echo cancellation is off".to_string());
        }
        if self.mouth != "flite" && self.mouth != "native" {
            problems.push(format!("mouth {}", self.mouth));
        }
        if self.prosody_probe != 0 {
            problems.push(format!("prosody probe allocated {}", self.prosody_probe));
        }
        if self.emotion_probe != 0 {
            problems.push(format!("emotion probe allocated {}", self.emotion_probe));
        }
        if self.buffer_probe != 0 {
            problems.push(format!(
                "output buffer probe allocated {}",
                self.buffer_probe
            ));
        }
        self.span("idle", self.idle, &mut problems);
        self.span("play", self.play, &mut problems);
        self.span("hold", self.hold, &mut problems);
        if self.barges.len() < 2 {
            problems.push(format!("barge samples {}", self.barges.len()));
        } else {
            let start = self.barges[0];
            let end = *self.barges.last().expect("barge");
            if start != end {
                problems.push(format!("barge start {start} ms end {end} ms"));
            }
            if self.barges.iter().any(|ms| *ms != start) {
                problems.push(format!("barge drifted {}", barge_list(&self.barges)));
            }
            if start > 200 {
                problems.push(format!("barge {start} ms above the 200 ms lock"));
            }
            if start != BARGE_MS {
                problems.push(format!("barge {start} ms, gate {BARGE_MS} ms"));
            }
        }
        if self.emotion_non_finite != 0 || self.emotion_out_of_range != 0 {
            problems.push(format!(
                "emotion non_finite {} out_of_range {}",
                self.emotion_non_finite, self.emotion_out_of_range
            ));
        }
        if self.prosody_bad != 0 {
            problems.push(format!("prosody frames bad {}", self.prosody_bad));
        }
        if self.buffer_overflows != 0 || self.desync != 0 {
            problems.push(format!(
                "buffer overflows {} desync {}",
                self.buffer_overflows, self.desync
            ));
        }
        if self.buffer_max > BUFFER_DEPTH_MAX {
            problems.push(format!(
                "buffer depth {} above {}",
                self.buffer_max, BUFFER_DEPTH_MAX
            ));
        }
        for axis in 0..4 {
            let span = self.emotion_max[axis] - self.emotion_min[axis];
            if !span.is_finite() || span > EMOTION_SPAN_MAX {
                problems.push(format!(
                    "emotion axis {axis} span {span}, gate {EMOTION_SPAN_MAX}"
                ));
            }
        }
        if self.idle.n > 0 {
            if self.idle.max != self.echo_probe {
                problems.push(format!(
                    "idle allocs {} echo probe {}",
                    self.idle.max, self.echo_probe
                ));
            }
            if self.hold.n > 0 && self.hold.max != self.idle.max.saturating_add(1) {
                problems.push(format!(
                    "hold allocs {} idle {}",
                    self.hold.max, self.idle.max
                ));
            }
            if self.play.n > 0 && self.play.max != self.idle.max.saturating_add(4) {
                problems.push(format!(
                    "playback allocs {} idle {}",
                    self.play.max, self.idle.max
                ));
            }
        }
        if self.user_sent == 0 || self.user_heard.saturating_mul(2) < self.user_sent {
            problems.push(format!(
                "user frames sent {} heard {}",
                self.user_sent, self.user_heard
            ));
        }
        match self.heap_warm {
            Some(warm) => {
                let growth = self.heap_end.saturating_sub(warm);
                let limit = HEAP_GROWTH_KIB.saturating_mul(1024);
                if growth > limit {
                    problems.push(format!(
                        "heap growth {growth} bytes, gate {HEAP_GROWTH_KIB} KiB"
                    ));
                }
            }
            None => problems.push("heap warmup was not read".to_string()),
        }
        let expect_windows = self.minutes / 5;
        if self.windows.len() as u64 != expect_windows {
            problems.push(format!(
                "windows {} want {expect_windows}",
                self.windows.len()
            ));
        }
        if let Some(first) = self.windows.first() {
            for window in self.windows.iter().skip(1) {
                if window.p99_ns > first.p99_ns.saturating_add(P99_DRIFT_NS) {
                    problems.push(format!(
                        "window {} p99 {} ns, first {} ns, drift gate {} ns",
                        window.index, window.p99_ns, first.p99_ns, P99_DRIFT_NS
                    ));
                }
                if window.max_ns > first.max_ns.saturating_add(MAX_DRIFT_NS) {
                    problems.push(format!(
                        "window {} max {} ns, first {} ns, drift gate {} ns",
                        window.index, window.max_ns, first.max_ns, MAX_DRIFT_NS
                    ));
                }
            }
            if first.p99_ns > TICK_P99_MAX_NS || first.max_ns > TICK_MAX_NS {
                problems.push(format!(
                    "window 0 p99 {} ns max {} ns, gates {} / {} ns",
                    first.p99_ns, first.max_ns, TICK_P99_MAX_NS, TICK_MAX_NS
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(format!("{}\n{}", self.table(), problems.join("\n")))
        }
    }

    fn span(&self, name: &str, span: AllocSpan, problems: &mut Vec<String>) {
        if span.n == 0 {
            problems.push(format!("{name} steady ticks were not observed"));
            return;
        }
        if span.min != span.max {
            problems.push(format!(
                "{name} allocs moved min {} max {}",
                span.min, span.max
            ));
        }
        let cap = match name {
            "idle" => IDLE_ALLOC_MAX,
            "play" => PLAY_ALLOC_MAX,
            "hold" => HOLD_ALLOC_MAX,
            _ => u64::MAX,
        };
        if span.max > cap {
            problems.push(format!("{name} allocs {} above {cap}", span.max));
        }
    }
}

fn show_span(span: AllocSpan) -> u64 {
    if span.n == 0 {
        0
    } else {
        span.min
    }
}

fn show_rss(value: &Result<u64, String>) -> String {
    match value {
        Ok(kib) => kib.to_string(),
        Err(err) => err.clone(),
    }
}

fn show_heap(value: Option<u64>) -> String {
    value
        .map(|bytes| bytes.to_string())
        .unwrap_or_else(|| "unread".to_string())
}

fn fmt_axes(values: &[f32; 4]) -> String {
    format!(
        "{:.4},{:.4},{:.4},{:.4}",
        values[0], values[1], values[2], values[3]
    )
}

fn barge_list(values: &[u64]) -> String {
    let mut out = String::new();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&value.to_string());
        if index == 11 && values.len() > 12 {
            out.push_str(",...");
            break;
        }
    }
    out
}

/// Conductor barge, in milliseconds. Measured 160 on every sample.
const BARGE_MS: u64 = 160;
/// Steady idle allocations per tick. Measured 0, same as the echo probe.
const IDLE_ALLOC_MAX: u64 = 0;
/// Steady playback allocations per tick. Measured 4 (the committed clause's four strings).
const PLAY_ALLOC_MAX: u64 = 4;
/// Steady user-hold allocations per tick. Measured 1 (the partial cloned into FrameOut).
const HOLD_ALLOC_MAX: u64 = 1;
/// Live-heap growth from the warmup sample to the end, in KiB.
/// 128 KiB is 131_072 bytes. The limit is not raised.
/// `VmRSS` and smaps `Rss` are printed and are not gated.
const HEAP_GROWTH_KIB: u64 = 128;
/// Output buffer high-water mark, in samples.
/// Measured 28160. One extra frame (1280) is the margin.
const BUFFER_DEPTH_MAX: usize = 29_440;
/// How far an axis may travel. Measured span was 0 on `IntentMsg::steady`.
const EMOTION_SPAN_MAX: f32 = 0.05;
/// Later window p99 above the first window. Measured drift was under 0.2 ms.
const P99_DRIFT_NS: u64 = 2_000_000;
/// Later window max above the first window. Measured drift was 7.3 ms.
const MAX_DRIFT_NS: u64 = 20_000_000;
/// Absolute p99 ceiling for the first window. Measured about 20.5 ms with Flite.
const TICK_P99_MAX_NS: u64 = 50_000_000;
/// Absolute max ceiling for the first window. Measured 28.9 ms on the hour.
const TICK_MAX_NS: u64 = 90_000_000;

/// CI length. `GALION_SOAK_FULL=1` selects the hour.
#[must_use]
pub fn minutes_from_env(flag: Option<&str>) -> u64 {
    if flag == Some("1") {
        60
    } else {
        5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_minutes_is_3750_ticks_and_an_hour_is_45000() {
        assert_eq!(ticks_for(5), 3_750);
        assert_eq!(ticks_for(60), 45_000);
        assert_eq!(minutes_from_env(None), 5);
        assert_eq!(minutes_from_env(Some("0")), 5);
        assert_eq!(minutes_from_env(Some("1")), 60);
    }
}
