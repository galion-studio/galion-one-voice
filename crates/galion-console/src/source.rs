// Galion Voice — Perpetual Open MIT License
//! Metrics ingestion. `galion-tensor` / bench runner / job runner implement [`MetricsSource`].

use crate::model::{BenchSample, ConsoleState, JobState, JobStatus, TensorFrame};

/// Anything that can update the console state once per tick.
///
/// Planned implementors:
/// - `galion-tensor` op counters + activation sampler -> `TensorFrame`
/// - VPS CPU test-gate runner -> `BenchSample`s
/// - build/deploy pipeline -> `JobStatus`
/// - a JSONL tail reader so the console can attach to a remote run over SSH
pub trait MetricsSource {
    fn poll(&mut self, state: &mut ConsoleState);
}

/// Deterministic placeholder data (tiny LCG, no RNG dependency) so the skeleton is
/// reproducible in tests and headless snapshots.
pub struct SyntheticSource {
    seed: u64,
}

impl Default for SyntheticSource {
    fn default() -> Self {
        Self { seed: 0x6A1_10E }
    }
}

impl SyntheticSource {
    fn next(&mut self) -> f64 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.seed >> 33) as f64) / ((1u64 << 31) as f64)
    }
}

impl MetricsSource for SyntheticSource {
    fn poll(&mut self, state: &mut ConsoleState) {
        if state.jobs.is_empty() {
            state.jobs = vec![
                job("cargo build (workspace)", JobState::Passed, 1.0),
                job("galion-tensor unit tests", JobState::Running, 0.0),
                job("CPU test gate: RTF < 1.0", JobState::Queued, 0.0),
                job("canned-recording demo", JobState::Queued, 0.0),
                job("VPS deploy (placeholder)", JobState::Queued, 0.0),
            ];
        }

        // Bench: one sample every 4 ticks, RTF drifting around 0.6.
        if state.tick % 4 == 0 {
            let rtf = 0.45 + 0.3 * self.next();
            let latency_ms = 40.0 + 60.0 * self.next();
            state.push_bench(BenchSample { label: "frame step (synthetic)".into(), rtf, latency_ms });
        }

        // Tensor placeholder.
        let n = 64;
        let mut acts = Vec::with_capacity(n);
        for _ in 0..n {
            acts.push(self.next() as f32);
        }
        state.tensor = TensorFrame {
            ops: 1_000 + (self.next() * 9_000.0) as u64,
            gflops: 2.0 + 6.0 * self.next(),
            activations: acts,
        };

        // Jobs: advance the running one, roll to the next when done.
        let mut promote_next = false;
        for j in state.jobs.iter_mut() {
            if promote_next && j.state == JobState::Queued {
                j.state = JobState::Running;
                promote_next = false;
            }
            if j.state == JobState::Running {
                j.progress = (j.progress + 0.02 + 0.03 * self.next()).min(1.0);
                if j.progress >= 1.0 {
                    j.state = JobState::Passed;
                    promote_next = true;
                }
            }
        }
    }
}

fn job(name: &str, state: JobState, progress: f64) -> JobStatus {
    JobStatus { name: name.into(), state, progress }
}
