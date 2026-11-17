// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! TODO(S1): `galion-mne-contract` is owned by agent S1.
//!
//! This is a local stand-in, version 0, so the Feeling layer can compile
//! before that contract lands. It carries one intent frame. It has no body,
//! no voice, and no audio. When S1's crate lands, point Feeling at it and
//! delete this copy. Do not grow the stand-in into a second contract.
//!
//! A frame whose `schema` is present and is not [`SCHEMA`] is refused.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wire schema of this stand-in. Not S1's finished contract.
pub const SCHEMA: &str = "galion-mne-contract/0";
pub const VERSION: u32 = 0;
/// Symphony clock. `LOCKS.toml` `clock.frame_ms`. Not a feeling time constant.
pub const TICK_MS: u32 = 80;

/// One named continuous target. The name is not a closed list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AxisTarget {
    pub name: String,
    pub value: f64,
}

/// An optional readable pin. A name and a mixture weight, not a coordinate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PinIntent {
    pub id: String,
    pub w: f64,
}

/// One intent sample on the 80 ms clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntentFrame {
    /// Absent is this stand-in. Any other schema is refused.
    #[serde(default)]
    pub schema: Option<String>,
    pub t_ms: u32,
    /// Axes this frame sets. An omitted name is not a request to go to zero.
    #[serde(default)]
    pub axes: Vec<AxisTarget>,
    /// Optional category pins. Weights are mixture weights, not the state.
    #[serde(default)]
    pub pins: Vec<PinIntent>,
    /// How far this frame replaces the held target on the axes it sets. 0..=1.
    /// Absent means the feeling config's default.
    #[serde(default)]
    pub blend: Option<f64>,
    /// `prosody` or `burst`. Absent leaves the choice to the pins.
    #[serde(default)]
    pub track: Option<String>,
    #[serde(default)]
    pub energy: Option<f64>,
    #[serde(default)]
    pub fatigue: Option<f64>,
    /// Outer scene label. The feeling layer records it and does not interpret it.
    #[serde(default)]
    pub environment: Option<String>,
    /// Metres, when the sender knows them. Absent means unknown, not zero.
    #[serde(default)]
    pub listener_distance_m: Option<f64>,
    /// Pull the held target back to the baseline. Ignores `axes` and `pins`.
    #[serde(default)]
    pub release: bool,
}

/// A frame, a list of frames, or `{ "frames": [ ... ] }`.
pub fn parse_frames(text: &str) -> Result<Vec<IntentFrame>, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("json: {e}"))?;
    frames_from_value(&value)
}

pub fn frames_from_value(value: &Value) -> Result<Vec<IntentFrame>, String> {
    match value {
        Value::Array(rows) => rows
            .iter()
            .enumerate()
            .map(|(i, row)| one_frame(row).map_err(|e| format!("frame {i}: {e}")))
            .collect(),
        Value::Object(map) => {
            if let Some(rows) = map.get("frames").and_then(|v| v.as_array()) {
                rows.iter()
                    .enumerate()
                    .map(|(i, row)| one_frame(row).map_err(|e| format!("frame {i}: {e}")))
                    .collect()
            } else {
                one_frame(value).map(|f| vec![f])
            }
        }
        _ => Err("intent must be a frame, a list of frames, or {frames: [...]}".into()),
    }
}

fn one_frame(value: &Value) -> Result<IntentFrame, String> {
    let frame: IntentFrame =
        serde_json::from_value(value.clone()).map_err(|e| format!("frame: {e}"))?;
    validate(&frame)?;
    Ok(frame)
}

fn validate(frame: &IntentFrame) -> Result<(), String> {
    if let Some(schema) = &frame.schema {
        if schema != SCHEMA {
            return Err(format!(
                "intent schema {schema} is not the local stub {SCHEMA}. Agent S1 owns galion-mne-contract"
            ));
        }
    }
    let mut names = Vec::new();
    for ax in &frame.axes {
        if ax.name.is_empty() {
            return Err("an axis name is empty".into());
        }
        if !ax.value.is_finite() {
            return Err(format!("axis {} is not finite", ax.name));
        }
        if names.iter().any(|n: &String| n == &ax.name) {
            return Err(format!("axis {} is repeated", ax.name));
        }
        names.push(ax.name.clone());
    }
    let mut ids = Vec::new();
    for pin in &frame.pins {
        if pin.id.is_empty() {
            return Err("a pin id is empty".into());
        }
        if !pin.w.is_finite() {
            return Err(format!("pin {} weight is not finite", pin.id));
        }
        if pin.w < 0.0 {
            return Err(format!("pin {} weight is negative", pin.id));
        }
        if ids.iter().any(|n: &String| n == &pin.id) {
            return Err(format!("pin {} is repeated", pin.id));
        }
        ids.push(pin.id.clone());
    }
    if let Some(b) = frame.blend {
        if !b.is_finite() {
            return Err("blend is not finite".into());
        }
    }
    for (name, v) in [
        ("energy", frame.energy),
        ("fatigue", frame.fatigue),
        ("listener_distance_m", frame.listener_distance_m),
    ] {
        if let Some(x) = v {
            if !x.is_finite() {
                return Err(format!("{name} is not finite"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_schema_is_version_zero_on_the_symphony_clock() {
        assert_eq!(SCHEMA, "galion-mne-contract/0");
        assert_eq!(VERSION, 0);
        assert_eq!(TICK_MS, 80);
    }

    #[test]
    fn a_frame_round_trips() {
        let raw = r#"{"schema":"galion-mne-contract/0","t_ms":80,"axes":[{"name":"valence","value":0.2}],"pins":[{"id":"anger","w":1.0}]}"#;
        let frames = parse_frames(raw).unwrap();
        assert_eq!(frames.len(), 1);
        let again = serde_json::to_string(&frames[0]).unwrap();
        assert_eq!(frames, parse_frames(&again).unwrap());
    }

    #[test]
    fn a_list_and_a_wrapper_both_parse() {
        let list = r#"[{"t_ms":0},{"t_ms":80}]"#;
        assert_eq!(parse_frames(list).unwrap().len(), 2);
        let wrap = r#"{"note":"synthetic","frames":[{"t_ms":0}]}"#;
        assert_eq!(parse_frames(wrap).unwrap()[0].t_ms, 0);
    }

    #[test]
    fn a_foreign_schema_is_refused() {
        let raw = r#"{"schema":"galion-mne-contract/9","t_ms":0}"#;
        let err = parse_frames(raw).unwrap_err();
        assert!(err.contains("S1"), "{err}");
    }

    #[test]
    fn a_negative_pin_weight_is_refused() {
        let raw = r#"{"t_ms":0,"pins":[{"id":"anger","w":-0.2}]}"#;
        assert!(parse_frames(raw).is_err());
    }
}
