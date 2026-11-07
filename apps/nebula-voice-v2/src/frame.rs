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
        Self { voice, timestamp: 0.0, vad: 1.0, energy: 0.0, valence: 0.0, arousal: 0.0, dominance: 0.0, warmth: None, weights, elements }
    }
}

#[derive(Debug)]
pub struct ParseError(pub String);

pub fn parse_frame(text: &str, fallback_voice: Voice) -> Result<NebulaFrame, ParseError> {
    let value: Value = serde_json::from_str(text).map_err(|err| ParseError(err.to_string()))?;
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
}
