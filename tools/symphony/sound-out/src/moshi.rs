// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Moshi stub.
//!
//! Kyutai Moshi is a full-duplex model with an 80 ms frame. The research
//! note in this repo says it needs a GPU; the CPU path stays the cascade.
//! This adapter does not look for weights and does not open a network
//! connection. It emits 80 ms frames so the mixer can be wired before a
//! real Moshi backend exists.
//!
//! TODO: a real Moshi adapter, out of process, weights already on disk,
//! behind the same `real-backends` gate. Not this file.

use crate::backend::{Capabilities, ClonerBackend, PcmChunk, SpeakMeta, SAMPLE_RATE, SAMPLES_PER_FRAME};
use crate::request::CloneRequest;

pub struct MoshiStub;

impl ClonerBackend for MoshiStub {
    fn id(&self) -> &'static str {
        "moshi-stub"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            realtime_on_cpu: false,
            streaming: true,
            weights_used: false,
            stub: true,
            note: "Moshi weights are not loaded. The real model is not a CPU realtime path. These frames are a stub.".into(),
        }
    }

    fn speak_stream(
        &mut self,
        request: &CloneRequest,
        on_chunk: &mut dyn FnMut(PcmChunk) -> Result<(), String>,
    ) -> Result<SpeakMeta, String> {
        let req = request.clone().normalized();
        let frames = (req.text.chars().count() / 12).clamp(1, 8) as u32;
        let f0 = 110.0 * (req.prosody.pitch_st / 12.0).exp2();
        let amp = 0.05 * req.prosody.energy;
        let pause = ((req.prosody.pre_pause_ms as u64 * u64::from(SAMPLE_RATE)) / 1000) as usize;
        if pause > 0 {
            on_chunk(PcmChunk {
                index: 0,
                samples: vec![0.0; pause.min(SAMPLES_PER_FRAME)],
                last: false,
            })?;
        }
        for index in 0..frames {
            let mut samples = Vec::with_capacity(SAMPLES_PER_FRAME);
            for i in 0..SAMPLES_PER_FRAME {
                let n = index as usize * SAMPLES_PER_FRAME + i;
                let t = n as f64 / SAMPLE_RATE as f64;
                samples.push((2.0 * std::f64::consts::PI * f0 * t).sin() * amp);
            }
            on_chunk(PcmChunk {
                index: index + if pause > 0 { 1 } else { 0 },
                samples,
                last: index + 1 == frames,
            })?;
        }
        Ok(SpeakMeta {
            backend: "moshi-stub".into(),
            weights_used: false,
            stub: true,
            realtime_on_cpu: false,
            controls_echo: None,
            note: "moshi stub, no weights".into(),
        })
    }
}
