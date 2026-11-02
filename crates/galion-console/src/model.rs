// Galion Voice — Perpetual Open MIT License
//! Renderer-agnostic state. Terminal, window and web front-ends all draw this.

use std::collections::VecDeque;

use crate::heartbeat::{HeartbeatEvent, YesNo};
use crate::live::LiveView;

/// One benchmark measurement (e.g. one CPU test-gate run on the VPS).
#[derive(Clone, Debug, PartialEq)]
pub struct BenchSample {
    /// Free-form label, e.g. "matmul 512x512" or "frame step 80ms".
    pub label: String,
    /// Real-time factor: compute time / audio time. < 1.0 means faster than real time.
    pub rtf: f64,
    /// Per-frame latency in milliseconds.
    pub latency_ms: f64,
}

/// Snapshot of tensor-runtime activity (placeholder for galion-tensor metrics).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TensorFrame {
    /// Ops executed since the previous frame.
    pub ops: u64,
    /// Achieved GFLOP/s over the last frame.
    pub gflops: f64,
    /// Normalised (0..=1) activation magnitudes, sampled — drives the "matrix rain".
    pub activations: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Running,
    Passed,
    Failed,
}

impl JobState {
    pub fn label(self) -> &'static str {
        match self {
            JobState::Queued => "QUEUED",
            JobState::Running => "RUNNING",
            JobState::Passed => "PASS",
            JobState::Failed => "FAIL",
        }
    }
}

/// A pipeline job: build, CPU test gate, deploy, demo replay…
#[derive(Clone, Debug, PartialEq)]
pub struct JobStatus {
    pub name: String,
    pub state: JobState,
    /// 0.0..=1.0
    pub progress: f64,
}

/// Everything the console shows. Bounded history so it is safe to run for days over SSH.
#[derive(Clone, Debug)]
pub struct ConsoleState {
    pub bench_history: VecDeque<BenchSample>,
    pub bench_capacity: usize,
    pub tensor: TensorFrame,
    pub jobs: Vec<JobStatus>,
    pub tick: u64,
    /// Matrix-rain column heads (row index per column); sized lazily by the renderer.
    pub rain_heads: Vec<u16>,
    /// YES / NO widget; answers commit on the heartbeat in [`ConsoleState::advance`].
    pub yes_no: YesNo,
    /// Live stage-latency / affect strip (A11). `None` = synthetic-only mode.
    pub live: Option<LiveView>,
    /// `false` when `NO_COLOR` is set: the renderer strips all colours.
    pub color: bool,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self::with_capacity(120)
    }
}

impl ConsoleState {
    pub fn with_capacity(bench_capacity: usize) -> Self {
        Self {
            bench_history: VecDeque::with_capacity(bench_capacity),
            bench_capacity: bench_capacity.max(1),
            tensor: TensorFrame::default(),
            jobs: Vec::new(),
            tick: 0,
            rain_heads: Vec::new(),
            yes_no: YesNo::default(),
            live: None,
            color: true,
        }
    }

    pub fn push_bench(&mut self, s: BenchSample) {
        if self.bench_history.len() == self.bench_capacity {
            self.bench_history.pop_front();
        }
        self.bench_history.push_back(s);
    }

    /// Advance one 80 ms heartbeat tick. Commits any armed YES/NO answer as a
    /// [`HeartbeatEvent`] stamped with the new tick, and returns it.
    pub fn advance(&mut self) -> Option<HeartbeatEvent> {
        self.tick = self.tick.wrapping_add(1);
        let ev = self.yes_no.on_tick(self.tick);
        for (i, h) in self.rain_heads.iter_mut().enumerate() {
            // Columns move at different speeds, cheaply.
            if (self.tick + i as u64) % (1 + (i as u64 % 3)) == 0 {
                *h = h.wrapping_add(1);
            }
        }
        ev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_history_is_bounded() {
        let mut s = ConsoleState::with_capacity(3);
        for i in 0..10 {
            s.push_bench(BenchSample { label: format!("{i}"), rtf: 0.5, latency_ms: 10.0 });
        }
        assert_eq!(s.bench_history.len(), 3);
        assert_eq!(s.bench_history.front().unwrap().label, "7");
    }
}
