// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Optional reference: the LF flow derivative, one 80 ms hop per tick.
//!
//! This is a test signal. It is not filtered by a vocal tract, not radiated,
//! and not a recording. Jitter and shimmer, when configured, are a
//! deterministic four-step sequence with mean zero. They are the control,
//! not a model of physiological noise.

use crate::config::Config;
use crate::lf;
use crate::render::TickOut;

pub fn render_source(ticks: &[TickOut], cfg: &Config) -> Result<Vec<f32>, String> {
    let hop = (cfg.source_hz as u64 * cfg.tick_ms as u64 / 1000) as usize;
    if hop == 0 {
        return Err("source hop is empty".into());
    }
    let mut out = vec![0.0f32; hop * ticks.len()];
    let mut cursor = 0usize;
    let mut pulse_i: u32 = 0;
    while cursor < out.len() {
        let tick_i = cursor / hop;
        let tick = &ticks[tick_i];
        let region_end = (tick_i + 1) * hop;
        if !tick.voiced {
            cursor = region_end;
            continue;
        }
        let f0 = tick.f0_hz.ok_or("voiced tick has no F0")?;
        let rd = tick.rd.ok_or("voiced tick has no Rd")?;
        let ee = tick.ee_rel.unwrap_or(1.0);
        let jitter = tick.jitter.unwrap_or(0.0);
        let shimmer = tick.shimmer.unwrap_or(0.0);
        let period_scale = (1.0 + jitter * unit(pulse_i)).max(0.25);
        let ee_scale = (1.0 + shimmer * unit(pulse_i.wrapping_add(1))).max(0.05);
        let shape = lf::wave(rd, f0 / period_scale, ee * ee_scale, cfg.rd_min, cfg.rd_max)?;
        let n = (cfg.source_hz as f64 * shape.t0).round().max(2.0) as usize;
        for i in 0..n {
            if cursor >= region_end || cursor >= out.len() {
                break;
            }
            let t = i as f64 / cfg.source_hz as f64;
            out[cursor] = lf::derivative_at(&shape, t) as f32;
            cursor += 1;
        }
        pulse_i = pulse_i.wrapping_add(1);
        if n == 0 {
            return Err("LF period produced no samples".into());
        }
    }
    Ok(out)
}

/// Mean of one cycle of four is zero. Peak absolute value is 1.
fn unit(n: u32) -> f64 {
    match n % 4 {
        0 => 1.0,
        1 => 0.5,
        2 => -1.0,
        _ => -0.5,
    }
}
