// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Dynamics input. Not the wire contract.
//!
//! The wire document is `galion-mne-contract` (S1). [`frames_from_document`]
//! maps that document onto this frame, which is what the 80 ms chase reads.
//! [`SCHEMA`] stays `galion-mne-contract/0` so the existing synthetic tests
//! still name the old JSON shape. New callers use the real document.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Old JSON shape used by this crate's own tests. Not S1's wire schema.
pub const SCHEMA: &str = "galion-mne-contract/0";
#[allow(dead_code)]
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

/// Map S1's document onto the frames this chase reads.
///
/// The contract's arousal and dominance are 0..1. This chase's axes are
/// −1..1, so those two are expanded across the whole axis: 0 → −1, 0.5 → 0,
/// 1 → +1. Valence is already −1..1. Energy and fatigue stay 0..1. Approach
/// is not on the contract, so it is omitted and is not pulled to zero.
pub fn frames_from_document(
    doc: &galion_mne_contract::Document,
) -> Result<Vec<IntentFrame>, String> {
    if doc.kind != galion_mne_contract::DocKind::Intent {
        return Err("feeling input must be an intent document".into());
    }
    if doc.intent_frames.is_empty() {
        return Err("intent document has no frames".into());
    }
    let mut out = Vec::with_capacity(doc.intent_frames.len());
    for frame in &doc.intent_frames {
        let mut axes = vec![
            AxisTarget {
                name: "arousal".into(),
                value: unit_to_axis(frame.feeling.arousal),
            },
            AxisTarget {
                name: "valence".into(),
                value: frame.feeling.valence.clamp(-1.0, 1.0),
            },
            AxisTarget {
                name: "dominance".into(),
                value: unit_to_axis(frame.feeling.dominance),
            },
        ];
        axes.retain(|a| a.value.is_finite());
        let pins = match &frame.feeling.pin {
            Some(pin) => vec![PinIntent {
                id: pin.name.as_str().into(),
                w: pin.weight,
            }],
            None => Vec::new(),
        };
        let track = frame.feeling.pin.as_ref().map(|pin| match pin.track {
            galion_mne_contract::Track::Prosody => "prosody".into(),
            galion_mne_contract::Track::Burst => "burst".into(),
        });
        out.push(IntentFrame {
            schema: None,
            t_ms: frame.t_ms,
            axes,
            pins,
            blend: Some(frame.intent.strength),
            track,
            energy: Some(frame.inner.energy),
            fatigue: Some(frame.inner.fatigue),
            environment: Some(doc.moment.place.clone()),
            listener_distance_m: None,
            release: false,
        });
    }
    Ok(out)
}

/// Contract 0..1 onto the chase's −1..1 axis.
fn unit_to_axis(unit: f64) -> f64 {
    (unit.clamp(0.0, 1.0) * 2.0 - 1.0).clamp(-1.0, 1.0)
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
    fn s1_unit_axes_fill_the_chase_scale() {
        let doc = sample_doc(0.0, -0.4, 1.0);
        let frames = frames_from_document(&doc).unwrap();
        let axis = |name: &str| {
            frames[0]
                .axes
                .iter()
                .find(|a| a.name == name)
                .unwrap()
                .value
        };
        assert!((axis("arousal") - -1.0).abs() < 1e-9);
        assert!((axis("valence") - -0.4).abs() < 1e-9);
        assert!((axis("dominance") - 1.0).abs() < 1e-9);
        assert!(frames[0].axes.iter().all(|a| a.name != "approach"));
        let mid = frames_from_document(&sample_doc(0.5, 0.0, 0.5)).unwrap();
        let arousal = mid[0]
            .axes
            .iter()
            .find(|a| a.name == "arousal")
            .unwrap()
            .value;
        assert!(arousal.abs() < 1e-9);
    }

    fn sample_doc(arousal: f64, valence: f64, dominance: f64) -> galion_mne_contract::Document {
        use galion_mne_contract::*;
        Document {
            schema: SCHEMA.into(),
            schema_version: SCHEMA_VERSION,
            kind: DocKind::Intent,
            frame_ms: FRAME_MS,
            value_origin: ValueOrigin::StandIn,
            note: String::new(),
            persona: PersonaRef {
                persona_id: "galion-one".into(),
                voice_id: "synthetic".into(),
            },
            moment: Moment {
                moment_id: "scale".into(),
                memory_id: "mem".into(),
                memory_snippet: String::new(),
                place: "quiet_room".into(),
                distance: "conversational".into(),
                noise: "quiet".into(),
                listener: "one_person".into(),
            },
            text: "Hello.".into(),
            baseline: BaselinePoint {
                arousal: 0.5,
                valence: 0.0,
                dominance: 0.5,
            },
            intent_frames: vec![IntentFrame {
                t_ms: 0,
                intent: Intent {
                    verb: "greet".into(),
                    strength: 1.0,
                },
                feeling: Feeling {
                    arousal,
                    valence,
                    dominance,
                    pin: None,
                },
                inner: InnerFactors {
                    energy: 0.5,
                    fatigue: 0.1,
                    health: 1.0,
                },
                text_span: TextSpan {
                    start_byte: 0,
                    end_byte: 6,
                },
            }],
            readback_frames: Vec::new(),
        }
    }

    #[test]
    fn a_negative_pin_weight_is_refused() {
        let raw = r#"{"t_ms":0,"pins":[{"id":"anger","w":-0.2}]}"#;
        assert!(parse_frames(raw).is_err());
    }
}
