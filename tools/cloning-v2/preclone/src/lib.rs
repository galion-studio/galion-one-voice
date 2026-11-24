// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Pre-clone analysis. One stage, before any cloning.
//!
//! It reads a reference buffer, segments it, and writes one versioned profile:
//! the steady voice, a hook for the 80 ms emotion moment, and the room.
//! Segmentation of other speakers, music, and noise is this same stage.
//!
//! The measurement is CPU DSP. No model, no embedding of a person, no clone.
//! Real reference audio stays on the owner's machine. Tests synthesize the signal.

#![forbid(unsafe_code)]

mod dsp;
mod emotion;
mod environment;
mod profile;
mod segment;
mod vad;
mod voice;
mod wav;

pub mod device;
pub mod synth;

pub use emotion::{
    parse_moments, MomentAxis, MomentFrame, MOMENT_HOOK, MOMENT_SCHEMA, MOMENT_TICK_MS,
};
pub use environment::{estimate_rt60_s, estimate_snr_db};
pub use profile::{profile_bytes, Profile, Request, Segment, SegmentKind, SCHEMA, STAGE};
pub use vad::{EnergyVad, VAD_CLAMP_DBFS, VAD_FLOOR_DBFS, VAD_FRAME_MS, VAD_ON_DB};
pub use wav::read_wav;

use emotion::canon_moments;
use environment::measure_room;
use segment::select;
use vad::summarize as summarize_vad;
use voice::measure_voice;

/// Fifteen minutes at the file's own rate. A reference longer than that is refused.
const MAX_MINUTES: u64 = 15;

/// Analyse one reference. `samples` are mono linear PCM, not a file path.
pub fn analyse(req: &Request<'_>) -> Result<Profile, String> {
    validate(req)?;
    let moments = canon_moments(req.moments)?;
    let mut room = measure_room(req.samples, req.sample_rate_hz);
    let segments = select(
        req.samples,
        req.sample_rate_hz,
        room.noise_floor_dbfs,
        req.target_f0_hz,
    );
    room.music_bed = segments.iter().any(|s| {
        let n = s.end_sample.saturating_sub(s.start_sample);
        s.kind == SegmentKind::Music && n as f64 >= f64::from(req.sample_rate_hz) * 0.25
    });
    let voice = measure_voice(req.samples, req.sample_rate_hz, &segments);
    let emotion = emotion::profile_from(moments);
    let energy_vad = summarize_vad(req.samples, req.sample_rate_hz);
    Ok(Profile {
        sample_rate_hz: req.sample_rate_hz,
        duration_samples: req.samples.len(),
        voice,
        emotion,
        environment: room,
        energy_vad,
        segments,
    })
}

fn validate(req: &Request<'_>) -> Result<(), String> {
    let sr = req.sample_rate_hz;
    if !(8_000..=96_000).contains(&sr) {
        return Err(format!("sample rate {sr} is outside 8000..=96000"));
    }
    if req.samples.is_empty() {
        return Err("reference is empty".into());
    }
    let max = sr as usize * 60 * MAX_MINUTES as usize;
    if req.samples.len() > max {
        return Err(format!("reference is longer than {MAX_MINUTES} minutes"));
    }
    if req.samples.iter().any(|s| !s.is_finite()) {
        return Err("reference sample is not finite".into());
    }
    if let Some(f0) = req.target_f0_hz {
        if !(70.0..=400.0).contains(&f0) {
            return Err("target f0 must sit between 70 Hz and 400 Hz".into());
        }
    }
    let win = dsp::ms_samples(sr, dsp::WIN_MS);
    if req.samples.len() < win {
        return Err("reference is shorter than one 40 ms analysis window".into());
    }
    Ok(())
}
