//! Live nebula frames. Element ids are labels from the fixed table.

use nebula_elements::{label_weight, DEMO};
use serde::Deserialize;
use serde_json::Value;

use crate::shade::Voice;
use crate::sim::Drive;

#[derive(Clone, Debug, PartialEq)]
pub struct NebulaFrame {
    pub voice: Voice,
    pub timestamp: f64,
    pub vad: f32,
    pub energy: f32,
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
    pub warmth: Option<f32>,
    pub weights: [f32; 13],
    pub elements: [&'static str; 13],
    /// True when the feed said there is no emotion estimate. Weights stay 0.
    pub no_estimate: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Mapped {
    pub turbulence: f32,
    pub clump: f32,
    pub warmth: f32,
    pub pulse: f32,
    pub weights: [f32; 13],
}

impl NebulaFrame {
    pub fn neutral(voice: Voice) -> Self {
        let mut weights = [0.0; 13];
        let mut elements = [""; 13];
        weights[0] = 1.0;
        elements[0] = DEMO[0].element_id;
        Self {
            voice,
            timestamp: 0.0,
            vad: 1.0,
            energy: 0.0,
            valence: 0.0,
            arousal: 0.0,
            dominance: 0.0,
            warmth: None,
            weights,
            elements,
            no_estimate: false,
        }
    }
}

#[derive(Debug)]
pub struct ParseError(pub String);

/// Feed id, then the Demo 13 element id. Eight of the thirteen strings differ.
const FEED_GAS: &[(&str, &str, &str)] = &[
    ("neutral", "HeII-541.2", "HeII-541.2"),
    ("calm", "Hbeta-486.1", "H-beta"),
    ("happy", "[NII]-575.5", "NII-575.5"),
    ("amused", "HeI-587.6", "HeI-587.6"),
    ("excited", "NeI-594.5", "NeI-594.5"),
    ("tender", "HeI-667.8", "HeI-667.8"),
    ("sad", "HeII-468.6", "HeII-468.6"),
    ("disappointed", "[ClIII]-551.7", "ClIII-551.7"),
    ("angry", "[SIII]-631.2", "SIII-631.2"),
    ("fearful", "Hdelta-410.2", "H-delta"),
    ("surprised", "[OIII]-495.9", "OIII-495.9"),
    ("disgusted", "[ClIII]-553.8", "ClIII-553.8"),
    ("seductive", "Halpha-656.3", "H-alpha"),
];

fn feed_row(name: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    FEED_GAS.iter().find(|row| row.0.eq_ignore_ascii_case(name))
}

fn is_feed_schema(value: &Value) -> bool {
    matches!(
        value.get("schema").and_then(|v| v.as_str()),
        Some("galion-nebula-live-feed/1") | Some("galion-voice-v2-emotion-feed/1")
    )
}

fn feed_voice(value: &Value, fallback: Voice) -> Voice {
    value
        .get("voice_id")
        .or_else(|| value.get("voice"))
        .and_then(|v| v.as_str())
        .and_then(Voice::parse)
        .unwrap_or(fallback)
}

fn feed_timestamp(value: &Value) -> f64 {
    if let Some(ms) = value.get("t_ms").and_then(|v| v.as_f64()) {
        return ms / 1000.0;
    }
    value.get("timestamp").or_else(|| value.get("t")).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn absent_frame(voice: Voice, timestamp: f64) -> NebulaFrame {
    NebulaFrame {
        voice,
        timestamp,
        vad: 0.0,
        energy: 0.0,
        valence: 0.0,
        arousal: 0.0,
        dominance: 0.0,
        warmth: None,
        weights: [0.0; 13],
        elements: [""; 13],
        no_estimate: true,
    }
}

fn assert_absence(value: &Value) -> Result<(), ParseError> {
    if let Some(map) = value.get("weights").and_then(|v| v.as_object()) {
        for (name, slot) in map {
            let numeric = match slot {
                Value::Object(fields) => fields.get("weight").and_then(|v| v.as_f64()).is_some(),
                Value::Number(_) => true,
                _ => false,
            };
            if numeric {
                return Err(ParseError(format!("estimate none cannot carry a weight for {name}")));
            }
        }
    }
    for key in ["valence", "arousal", "dominance", "energy"] {
        if value.get(key).and_then(|v| v.as_f64()).is_some() {
            return Err(ParseError(format!("estimate none cannot carry {key}")));
        }
    }
    Ok(())
}

fn parse_feed_frame(value: &Value, fallback_voice: Voice) -> Result<NebulaFrame, ParseError> {
    let voice = feed_voice(value, fallback_voice);
    let timestamp = feed_timestamp(value);
    let estimate = value.get("estimate").and_then(|v| v.as_str());
    let present = value.get("present").and_then(|v| v.as_bool());
    if let Some(name) = estimate {
        if name != "none" && name != "distribution" && name != "partial" {
            return Err(ParseError(format!("unknown estimate {name}")));
        }
    }
    let none = match (estimate, present) {
        (Some("none"), Some(true)) => return Err(ParseError("estimate none cannot be present".into())),
        (Some("distribution") | Some("partial"), Some(false)) => {
            return Err(ParseError("a present estimate cannot set present false".into()));
        }
        (Some("none"), _) | (_, Some(false)) => true,
        (None, None) => return Err(ParseError("feed frame needs estimate or present".into())),
        _ => false,
    };
    if none {
        assert_absence(value)?;
        return Ok(absent_frame(voice, timestamp));
    }
    if voice == Voice::Jarvis {
        if let Some(map) = value.get("weights").and_then(|v| v.as_object()) {
            if map.keys().any(|key| key.eq_ignore_ascii_case("seductive")) {
                return Err(ParseError("seductive is Samantha only".into()));
            }
        }
    }
    let mut weights = [0.0f32; 13];
    let mut elements = [""; 13];
    let map = value.get("weights").and_then(|v| v.as_object()).ok_or_else(|| ParseError("feed weights must be an object".into()))?;
    for (name, slot) in map {
        let fields = slot.as_object().ok_or_else(|| ParseError(format!("weight for {name} must be an object")))?;
        let weight = match fields.get("weight") {
            None | Some(Value::Null) => continue,
            Some(Value::Number(number)) => number.as_f64().ok_or_else(|| ParseError(format!("weight for {name} is not a number")))? as f32,
            _ => return Err(ParseError(format!("weight for {name} must be a number or null"))),
        };
        let row = feed_row(name).ok_or_else(|| ParseError(format!("unknown emotion {name}")))?;
        match fields.get("gas_line").and_then(|v| v.as_str()) {
            Some(gas) if gas == row.1 => {}
            Some(gas) => return Err(ParseError(format!("gas_line {gas} is not {} for {name}", row.1))),
            None => return Err(ParseError(format!("weight for {name} needs gas_line"))),
        }
        if let Some(element) = fields.get("element_id").or_else(|| fields.get("element")).and_then(|v| v.as_str()) {
            if !element.eq_ignore_ascii_case(row.2) {
                return Err(ParseError(format!("element_id {element} is not {} for {name}", row.2)));
            }
        }
        apply_weight(&mut weights, &mut elements, voice, name, weight, Some(row.2))?;
    }
    let sum: f32 = weights.iter().sum();
    if sum > 1e-4 {
        for w in &mut weights {
            *w /= sum;
        }
    }
    let energy = num(value, "energy").unwrap_or(0.0).clamp(0.0, 1.0);
    Ok(NebulaFrame {
        voice,
        timestamp,
        vad: 1.0,
        energy,
        valence: num(value, "valence").unwrap_or(0.0).clamp(-1.0, 1.0),
        arousal: num(value, "arousal").unwrap_or(0.0).clamp(-1.0, 1.0),
        dominance: num(value, "dominance").unwrap_or(0.0).clamp(-1.0, 1.0),
        warmth: num(value, "warmth"),
        weights,
        elements,
        no_estimate: false,
    })
}

pub fn parse_frame(text: &str, fallback_voice: Voice) -> Result<NebulaFrame, ParseError> {
    let value: Value = serde_json::from_str(text).map_err(|err| ParseError(err.to_string()))?;
    if is_feed_schema(&value) {
        return parse_feed_frame(&value, fallback_voice);
    }
    let voice = value.get("voice").and_then(|v| v.as_str()).and_then(Voice::parse).unwrap_or(fallback_voice);
    let timestamp = value.get("timestamp").or_else(|| value.get("t")).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let energy = num(&value, "energy").unwrap_or(0.0).clamp(0.0, 1.0);
    let mut valence = num(&value, "valence");
    let mut arousal = num(&value, "arousal");
    let mut dominance = num(&value, "dominance");
    let warmth = num(&value, "warmth");
    let mut vad = None;
    if let Some(raw) = value.get("vad") {
        match raw {
            Value::Bool(flag) => vad = Some(if *flag { 1.0 } else { 0.0 }),
            Value::Number(n) => vad = n.as_f64().map(|v| v as f32),
            Value::Object(map) => {
                if let Some(active) = map.get("active").and_then(|v| v.as_f64()) {
                    vad = Some(active as f32);
                }
                valence = valence.or_else(|| map.get("valence").or_else(|| map.get("v")).and_then(|v| v.as_f64()).map(|v| v as f32));
                arousal = arousal.or_else(|| map.get("arousal").or_else(|| map.get("a")).and_then(|v| v.as_f64()).map(|v| v as f32));
                dominance = dominance.or_else(|| map.get("dominance").or_else(|| map.get("d")).and_then(|v| v.as_f64()).map(|v| v as f32));
            }
            _ => {}
        }
    }
    if let Some(affect) = value.get("affect").and_then(|v| v.as_object()) {
        valence = valence.or_else(|| affect.get("valence").and_then(|v| v.as_f64()).map(|v| v as f32));
        arousal = arousal.or_else(|| affect.get("arousal").and_then(|v| v.as_f64()).map(|v| v as f32));
        dominance = dominance.or_else(|| affect.get("dominance").and_then(|v| v.as_f64()).map(|v| v as f32));
    }
    let mut weights = [0.0f32; 13];
    let mut elements = [""; 13];
    if let Some(list) = value.get("weights").and_then(|v| v.as_array()) {
        for item in list {
            let (name, weight, element) = match item {
                Value::Object(map) => {
                    let name = map.get("emotion").and_then(|v| v.as_str()).unwrap_or("");
                    let weight = map.get("weight").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let element = map.get("element").and_then(|v| v.as_str());
                    (name, weight, element)
                }
                _ => continue,
            };
            apply_weight(&mut weights, &mut elements, voice, name, weight, element)?;
        }
    } else if let Some(map) = value.get("weights").and_then(|v| v.as_object()) {
        for (name, weight) in map {
            let weight = weight.as_f64().unwrap_or(0.0) as f32;
            apply_weight(&mut weights, &mut elements, voice, name, weight, None)?;
        }
    }
    if voice == Voice::Jarvis {
        if let Some(index) = DEMO.iter().position(|line| line.samantha_only) {
            weights[index] = 0.0;
            elements[index] = "";
        }
    }
    let sum: f32 = weights.iter().sum();
    if sum > 1e-4 {
        for w in &mut weights {
            *w /= sum;
        }
    } else {
        weights[0] = 1.0;
        elements[0] = DEMO[0].element_id;
    }
    Ok(NebulaFrame {
        voice,
        timestamp,
        vad: vad.unwrap_or(1.0).clamp(0.0, 1.0),
        energy,
        valence: valence.unwrap_or(0.0).clamp(-1.0, 1.0),
        arousal: arousal.unwrap_or(0.0).clamp(-1.0, 1.0),
        dominance: dominance.unwrap_or(0.0).clamp(-1.0, 1.0),
        warmth,
        weights,
        elements,
        no_estimate: false,
    })
}

fn apply_weight(weights: &mut [f32; 13], elements: &mut [&'static str; 13], voice: Voice, name: &str, weight: f32, element: Option<&str>) -> Result<(), ParseError> {
    if voice == Voice::Jarvis && name.eq_ignore_ascii_case("seductive") {
        return Ok(());
    }
    let labeled = label_weight(name, weight, element).map_err(|err| ParseError(format!("{err:?} for {name}")))?;
    let index = DEMO.iter().position(|line| line.emotion == labeled.emotion).ok_or_else(|| ParseError(name.into()))?;
    weights[index] += labeled.weight.max(0.0);
    elements[index] = labeled.element_id;
    Ok(())
}

fn num(value: &Value, key: &str) -> Option<f32> {
    value.get(key).and_then(|v| v.as_f64()).map(|v| v as f32)
}

pub fn map_frame(frame: &NebulaFrame) -> Mapped {
    let arousal = frame.arousal.clamp(-1.0, 1.0);
    let valence = frame.valence.clamp(-1.0, 1.0);
    let dominance = frame.dominance.clamp(-1.0, 1.0);
    let turbulence = 0.12 + 1.15 * (0.5 + 0.5 * arousal);
    let clump = 0.5 + 0.5 * dominance;
    let from_valence = 0.5 + 0.5 * valence;
    let warmth = match frame.warmth {
        Some(w) => (0.75 * from_valence + 0.25 * w.clamp(0.0, 1.0)).clamp(0.0, 1.0),
        None => from_valence,
    };
    let pulse = frame.energy.clamp(0.0, 1.0) * (0.2 + 0.8 * frame.vad.clamp(0.0, 1.0));
    Mapped { turbulence, clump, warmth, pulse, weights: frame.weights }
}

pub fn drive_from(frame: &NebulaFrame, active: usize, octaves: u32) -> Drive {
    if frame.no_estimate {
        return Drive { weights: [0.0; 13], turbulence: 0.0, clump: 0.0, pulse: 0.0, active, octaves };
    }
    let mapped = map_frame(frame);
    Drive { weights: mapped.weights, turbulence: mapped.turbulence, clump: mapped.clump, pulse: mapped.pulse, active, octaves }
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct RawHint {
    voice: Option<String>,
}

pub fn dominant<'a>(frame: &'a NebulaFrame, limit: usize) -> Vec<&'a nebula_elements::DemoLine> {
    let mut order: Vec<usize> = (0..13).collect();
    order.sort_by(|a, b| frame.weights[*b].partial_cmp(&frame.weights[*a]).unwrap_or(std::cmp::Ordering::Equal));
    order.into_iter().filter(|i| frame.weights[*i] > 0.08).take(limit).filter_map(|i| DEMO.get(i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arousal_valence_dominance_and_energy_map_as_documented() {
        let mut calm = NebulaFrame::neutral(Voice::Samantha);
        calm.arousal = -1.0;
        calm.valence = -1.0;
        calm.dominance = -1.0;
        calm.energy = 0.0;
        calm.vad = 1.0;
        let mut hot = calm.clone();
        hot.arousal = 1.0;
        hot.valence = 1.0;
        hot.dominance = 1.0;
        hot.energy = 1.0;
        hot.vad = 1.0;
        let a = map_frame(&calm);
        let b = map_frame(&hot);
        assert!(b.turbulence > a.turbulence);
        assert!(b.warmth > a.warmth);
        assert!(b.clump > a.clump);
        assert!(b.pulse > a.pulse);
        assert_eq!(a.pulse, 0.0);
    }

    #[test]
    fn jarvis_drops_seductive_and_rejects_a_remapped_element() {
        let text = r#"{"voice":"jarvis","weights":[{"emotion":"seductive","weight":1,"element":"H-alpha"},{"emotion":"calm","weight":1,"element":"H-beta"}]}"#;
        let frame = parse_frame(text, Voice::Jarvis).unwrap();
        assert!(frame.weights[DEMO.iter().position(|l| l.emotion == "seductive").unwrap()] < 1e-6);
        let bad = r#"{"voice":"samantha","weights":[{"emotion":"angry","weight":1,"element":"H-alpha"}]}"#;
        assert!(parse_frame(bad, Voice::Samantha).is_err());
    }

    #[test]
    fn live_absence_is_not_neutral_and_does_not_drive() {
        let text = r#"{"schema":"galion-nebula-live-feed/1","voice_id":"jarvis","t_ms":0,"kind":"frame","present":false,"distribution":false,"tau_ms":240,"weights":{"neutral":{"weight":null,"gas_line":"HeII-541.2"},"calm":{"weight":null,"gas_line":"Hbeta-486.1"}},"valence":null,"arousal":null,"dominance":null,"energy":null}"#;
        let frame = parse_frame(text, Voice::Samantha).unwrap();
        assert!(frame.no_estimate);
        assert_eq!(frame.voice, Voice::Jarvis);
        assert_eq!(frame.weights, [0.0; 13]);
        assert_ne!(frame.weights[0], 1.0);
        let drive = drive_from(&frame, 8, 2);
        assert_eq!(drive.weights, [0.0; 13]);
        assert_eq!(drive.turbulence, 0.0);
        assert_eq!(drive.pulse, 0.0);
        let named = r#"{"schema":"galion-voice-v2-emotion-feed/1","voice_id":"jarvis","estimate":"none","present":false,"weights":{"neutral":{"weight":null,"gas_line":"HeII-541.2","element_id":"HeII-541.2"}}}"#;
        assert!(parse_frame(named, Voice::Jarvis).unwrap().no_estimate);
    }

    #[test]
    fn live_weight_object_uses_the_picture_element_id() {
        let text = r#"{"schema":"galion-voice-v2-emotion-feed/1","voice_id":"samantha","t_ms":80,"estimate":"distribution","present":true,"weights":{"happy":{"weight":1,"gas_line":"[NII]-575.5","element_id":"NII-575.5"},"calm":{"weight":0,"gas_line":"Hbeta-486.1","element_id":"H-beta"}}}"#;
        let frame = parse_frame(text, Voice::Jarvis).unwrap();
        assert!(!frame.no_estimate);
        assert_eq!(frame.voice, Voice::Samantha);
        let happy = DEMO.iter().position(|line| line.emotion == "happy").unwrap();
        let calm = DEMO.iter().position(|line| line.emotion == "calm").unwrap();
        assert!((frame.weights[happy] - 1.0).abs() < 1e-5);
        assert!(frame.weights[calm].abs() < 1e-5);
        assert_eq!(frame.elements[happy], "NII-575.5");
        assert_eq!(frame.elements[calm], "H-beta");
        assert!((frame.timestamp - 0.08).abs() < 1e-9);
        let swapped = r#"{"schema":"galion-nebula-live-feed/1","voice_id":"samantha","present":true,"weights":{"calm":{"weight":1,"gas_line":"H-beta"}}}"#;
        assert!(parse_frame(swapped, Voice::Samantha).is_err());
    }
}
