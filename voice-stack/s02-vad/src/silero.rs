// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Silero VAD v5 ONNX, CPU execution provider only.
//!
//! The streaming contract is the one in snakers4/silero-vad v5: 512 new
//! samples at 16 kHz (32 ms), 64 samples of context from the previous window,
//! recurrent `state` of shape [2, 1, 128], and a scalar `sr` of 16000.
//! The public ear is still 20 ms (320 samples). Windows are cut from that
//! stream with no overlap, which is what the recurrent state expects.
//! A 20 ms frame that does not complete a window repeats the last probability.
//!
//! ort is the same 2.0.0-rc.15 pin as `runtimes/galion-piper-rt`. This crate
//! downloads the CPU ONNX Runtime build. It does not load a GPU provider.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;

use crate::clock;
use crate::VadError;

const WINDOW: usize = 512;
const CONTEXT: usize = 64;
/// Recurrent state is `[layers, batch, hidden]` = `[2, 1, 128]`.
const STATE_LAYERS: usize = 2;
const STATE_BATCH: usize = 1;
const STATE_HIDDEN: usize = 128;
const STATE_LEN: usize = STATE_LAYERS * STATE_BATCH * STATE_HIDDEN;

pub struct Silero {
    session: Session,
    state: Vec<f32>,
    context: Vec<f32>,
    pending: Vec<f32>,
    packed: Vec<f32>,
    last_prob: f32,
    sr_rank: usize,
    io: String,
    warm_cpu_ns: u64,
}

impl Silero {
    pub fn load(path: &Path) -> Result<Self, VadError> {
        let env = environment()?;
        let session = Session::builder(env)
            .map_err(|e| VadError::new(e.to_string()))?
            .with_execution_providers([ort::ep::CPU::default().build()])
            .map_err(|e| VadError::new(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| VadError::new(e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| VadError::new(e.to_string()))?
            .with_inter_threads(1)
            .map_err(|e| VadError::new(e.to_string()))?
            .with_parallel_execution(false)
            .map_err(|e| VadError::new(e.to_string()))?
            .with_intra_op_spinning(false)
            .map_err(|e| VadError::new(e.to_string()))?
            .commit_from_file(path)
            .map_err(|e| VadError::new(e.to_string()))?;

        let io = describe(&session);
        let names: Vec<String> = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();
        if !names.iter().any(|n| n == "input")
            || !names.iter().any(|n| n == "state")
            || !names.iter().any(|n| n == "sr")
        {
            return Err(VadError::new(format!(
                "not a Silero VAD v5 model (need inputs input, state, sr). {io}"
            )));
        }
        let sr_rank = session
            .inputs()
            .iter()
            .find(|i| i.name() == "sr")
            .map(rank_of)
            .unwrap_or(0);

        let mut s = Self {
            session,
            state: vec![0.0; STATE_LEN],
            context: vec![0.0; CONTEXT],
            pending: Vec::with_capacity(WINDOW + 320),
            packed: Vec::with_capacity(CONTEXT + WINDOW),
            last_prob: 0.0,
            sr_rank,
            io,
            warm_cpu_ns: 0,
        };
        // One silent window pays the first-run allocation. The state is then
        // cleared so the caller's first real frame is not conditioned on it.
        let t0 = clock::thread_cpu_ns();
        s.infer(&[0.0; WINDOW])
            .map_err(|e| VadError::new(format!("{e} ({})", s.io)))?;
        s.warm_cpu_ns = clock::since_ns(t0);
        s.reset_state();
        Ok(s)
    }

    pub fn io(&self) -> &str {
        &self.io
    }

    pub fn warm_cpu_ns(&self) -> u64 {
        self.warm_cpu_ns
    }

    pub fn reset_state(&mut self) {
        self.state.fill(0.0);
        self.context.fill(0.0);
        self.pending.clear();
        self.last_prob = 0.0;
    }

    pub fn push_i16(&mut self, frame: &[i16]) -> Result<FrameScore, VadError> {
        self.pending.reserve(frame.len());
        for &s in frame {
            self.pending.push(s as f32 / 32768.0);
        }
        self.drain_windows()
    }

    fn drain_windows(&mut self) -> Result<FrameScore, VadError> {
        let mut model_cpu_ns = 0u64;
        let mut ran_model = false;
        while self.pending.len() >= WINDOW {
            let window: Vec<f32> = self.pending.drain(..WINDOW).collect();
            let t0 = clock::thread_cpu_ns();
            match self.infer(&window) {
                Ok(prob) => {
                    model_cpu_ns = model_cpu_ns.saturating_add(clock::since_ns(t0));
                    ran_model = true;
                    self.last_prob = prob;
                }
                Err(e) => {
                    let mut restored = window;
                    restored.append(&mut self.pending);
                    self.pending = restored;
                    return Err(e);
                }
            }
        }
        Ok(FrameScore {
            prob: self.last_prob,
            model_cpu_ns,
            ran_model,
        })
    }

    fn infer(&mut self, window: &[f32]) -> Result<f32, VadError> {
        if window.len() != WINDOW {
            return Err(VadError::new(format!(
                "silero window is {} samples, want {WINDOW}",
                window.len()
            )));
        }
        self.packed.clear();
        self.packed.extend_from_slice(&self.context);
        self.packed.extend_from_slice(window);
        let input = Tensor::from_array(([1usize, CONTEXT + WINDOW], self.packed.clone()))
            .map_err(|e| VadError::new(e.to_string()))?;
        let state = Tensor::from_array((
            [STATE_LAYERS, STATE_BATCH, STATE_HIDDEN],
            self.state.clone(),
        ))
        .map_err(|e| VadError::new(e.to_string()))?;
        let sr = sr_tensor(self.sr_rank)?;
        let outputs = self
            .session
            .run(ort::inputs![
                "input" => input,
                "state" => state,
                "sr" => sr,
            ])
            .map_err(|e| VadError::new(format!("{e} ({})", self.io)))?;

        let prob_v = outputs.get("output").ok_or_else(|| {
            VadError::new(format!("model has no output named output ({})", self.io))
        })?;
        let (_shape, probs) = prob_v
            .try_extract_tensor::<f32>()
            .map_err(|e| VadError::new(e.to_string()))?;
        let prob = probs
            .first()
            .copied()
            .ok_or_else(|| VadError::new("empty speech probability"))?;

        let state_v = outputs.get("stateN").ok_or_else(|| {
            VadError::new(format!("model has no output named stateN ({})", self.io))
        })?;
        let (_shape, next) = state_v
            .try_extract_tensor::<f32>()
            .map_err(|e| VadError::new(e.to_string()))?;
        if next.len() != self.state.len() {
            return Err(VadError::new(format!(
                "stateN has {} values, want {} ({})",
                next.len(),
                self.state.len(),
                self.io
            )));
        }
        self.state.copy_from_slice(next);
        let from = self.packed.len() - CONTEXT;
        self.context.copy_from_slice(&self.packed[from..]);
        Ok(prob)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FrameScore {
    pub prob: f32,
    pub model_cpu_ns: u64,
    pub ran_model: bool,
}

fn sr_tensor(rank: usize) -> Result<Tensor<i64>, VadError> {
    let sr = 16_000i64;
    // v5 declares `sr` as a scalar (rank 0). A rank-1 tensor is accepted for
    // older exports that used shape [1].
    let t = if rank == 0 {
        Tensor::from_array(((), vec![sr]))
    } else {
        Tensor::from_array(([1usize], vec![sr]))
    };
    t.map_err(|e| VadError::new(e.to_string()))
}

fn rank_of(outlet: &ort::value::Outlet) -> usize {
    match outlet.dtype() {
        ort::value::ValueType::Tensor { shape, .. } => shape.len(),
        _ => 0,
    }
}

fn describe(session: &Session) -> String {
    let ins: Vec<String> = session.inputs().iter().map(fmt_outlet).collect();
    let outs: Vec<String> = session.outputs().iter().map(fmt_outlet).collect();
    format!("inputs [{}] outputs [{}]", ins.join(", "), outs.join(", "))
}

fn fmt_outlet(o: &ort::value::Outlet) -> String {
    format!("{}:{:?}", o.name(), o.dtype())
}

fn environment() -> Result<&'static ort::environment::Environment, VadError> {
    static ENV: OnceLock<ort::environment::Environment> = OnceLock::new();
    static GUARD: Mutex<()> = Mutex::new(());
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let _guard = GUARD
        .lock()
        .map_err(|_| VadError::new("ort init lock poisoned"))?;
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let env = ort::init()
        .with_name("galion-s02-vad")
        .build()
        .map_err(|e| VadError::new(e.to_string()))?;
    Ok(ENV.get_or_init(|| env))
}
