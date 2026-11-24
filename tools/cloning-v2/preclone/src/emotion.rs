// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Emotion slot for the pre-clone profile.
//!
//! The steady voice is measured elsewhere. The 80 ms moment belongs to the
//! emotion core (`tools/symphony/feeling`, `TICK_MS`; `galion-heartbeat::FRAME_MS`;
//! `LOCKS.toml` `clock.frame_ms`). This module stores frames that core already
//! produced. It does not step the moment and it does not read affect from the waveform.

use crate::profile::EmotionProfile;

/// Feeling-core hook name. Not a second implementation of that core.
pub const MOMENT_HOOK: &str = "galion-feeling";
/// Schema string the feeling crate still accepts on its intent frames.
pub const MOMENT_SCHEMA: &str = "galion-mne-contract/0";
/// Locked clock. One moment frame is one heartbeat.
pub const MOMENT_TICK_MS: u32 = 80;

#[derive(Clone, Debug, PartialEq)]
pub struct MomentAxis {
    pub name: String,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MomentFrame {
    pub t_ms: u32,
    pub axes: Vec<MomentAxis>,
}

/// Read moment frames the emotion core already wrote.
/// Accepts `{"frames":[...]}` or a bare list. The clock check happens in `analyse`.
pub fn parse_moments(text: &str) -> Result<Vec<MomentFrame>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("moments: {e}"))?;
    let frames = if let Some(arr) = value.get("frames").and_then(|v| v.as_array()) {
        arr
    } else if let Some(arr) = value.as_array() {
        arr
    } else {
        return Err("moments: expected a frame list".into());
    };
    let mut out = Vec::with_capacity(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        let t_ms = frame
            .get("t_ms")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| format!("moments: frame {i} needs t_ms"))?;
        if t_ms > u64::from(u32::MAX) {
            return Err("moments: t_ms does not fit".into());
        }
        let axes_v = frame
            .get("axes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("moments: frame {i} needs axes"))?;
        let mut axes = Vec::with_capacity(axes_v.len());
        for (j, axis) in axes_v.iter().enumerate() {
            let name = axis
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("moments: frame {i} axis {j} needs a name"))?;
            let value = axis
                .get("value")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| format!("moments: frame {i} axis {j} needs a value"))?;
            axes.push(MomentAxis {
                name: name.to_string(),
                value,
            });
        }
        out.push(MomentFrame {
            t_ms: t_ms as u32,
            axes,
        });
    }
    Ok(out)
}

pub(crate) fn canon_moments(frames: &[MomentFrame]) -> Result<Vec<MomentFrame>, String> {
    if frames.len() > 10_000 {
        return Err("moment list is longer than 10000 frames".into());
    }
    let mut out = Vec::with_capacity(frames.len());
    for frame in frames {
        if frame.t_ms % MOMENT_TICK_MS != 0 {
            return Err(format!(
                "moment t_ms {} is not a multiple of {MOMENT_TICK_MS}",
                frame.t_ms
            ));
        }
        if frame.axes.len() > 16 {
            return Err("a moment frame has more than 16 axes".into());
        }
        let mut axes = Vec::with_capacity(frame.axes.len());
        for axis in &frame.axes {
            check_name(&axis.name)?;
            if !axis.value.is_finite() {
                return Err(format!("moment axis {} is not finite", axis.name));
            }
            axes.push(axis.clone());
        }
        axes.sort_by(|a, b| {
            a.name.cmp(&b.name).then(
                a.value
                    .partial_cmp(&b.value)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        out.push(MomentFrame {
            t_ms: frame.t_ms,
            axes,
        });
    }
    out.sort_by(|a, b| {
        a.t_ms.cmp(&b.t_ms).then_with(|| {
            let an = a.axes.first().map(|x| x.name.as_str()).unwrap_or("");
            let bn = b.axes.first().map(|x| x.name.as_str()).unwrap_or("");
            an.cmp(bn)
        })
    });
    Ok(out)
}

fn check_name(name: &str) -> Result<(), String> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 32 {
        return Err("moment axis name must be 1..=32 characters".into());
    }
    let ok = bytes.iter().enumerate().all(|(i, b)| {
        let c = *b;
        if i == 0 {
            c.is_ascii_alphabetic()
        } else {
            c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
        }
    });
    if ok {
        Ok(())
    } else {
        Err(format!("moment axis name {name} is not a plain token"))
    }
}

pub(crate) fn profile_from(frames: Vec<MomentFrame>) -> EmotionProfile {
    EmotionProfile {
        attached: !frames.is_empty(),
        frames,
    }
}
