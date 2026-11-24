// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! The twelve projection poles stay lined up with the emotion-v2 harness.

use std::path::PathBuf;

use galion_emotion_core::UNIVERSAL;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn harness_ids(text: &str) -> Vec<String> {
    let start = text
        .find("EMOTIONS: tuple[str, ...] = (")
        .expect("EMOTIONS tuple");
    let rest = &text[start..];
    let end = rest.find(')').expect("end of tuple");
    let body = &rest[..end];
    let mut ids = Vec::new();
    for token in body.split('"') {
        if token.chars().all(|c| c.is_ascii_lowercase()) && !token.is_empty() {
            ids.push(token.to_string());
        }
    }
    ids
}

fn inventory_poles(text: &str) -> Vec<(String, f32, f32)> {
    let mut out = Vec::new();
    let mut id: Option<String> = None;
    let mut valence: Option<f32> = None;
    let mut arousal: Option<f32> = None;
    let flush = |id: &mut Option<String>,
                 valence: &mut Option<f32>,
                 arousal: &mut Option<f32>,
                 out: &mut Vec<_>| {
        if let (Some(name), Some(v), Some(a)) = (id.take(), valence.take(), arousal.take()) {
            out.push((name, v, a));
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("id = \"") {
            if let Some(name) = rest.strip_suffix('"') {
                if name.chars().all(|c| c.is_ascii_lowercase()) {
                    flush(&mut id, &mut valence, &mut arousal, &mut out);
                    id = Some(name.to_string());
                    valence = None;
                    arousal = None;
                }
            }
            continue;
        }
        if id.is_some() && valence.is_none() {
            if let Some(rest) = line.strip_prefix("valence = ") {
                valence = rest.parse().ok();
            }
        }
        if id.is_some() && arousal.is_none() {
            if let Some(rest) = line.strip_prefix("arousal = ") {
                arousal = rest.parse().ok();
            }
        }
    }
    flush(&mut id, &mut valence, &mut arousal, &mut out);
    out
}

#[test]
fn projection_poles_match_the_harness_and_the_inventory() {
    let root = repo();
    let py = std::fs::read_to_string(root.join("tools/emotion-v2/harness/emotions.py")).unwrap();
    let toml = std::fs::read_to_string(root.join("emotion-v2/emotions.toml")).unwrap();
    let ids = harness_ids(&py);
    let poles = inventory_poles(&toml);
    assert_eq!(ids.len(), UNIVERSAL.len());
    assert_eq!(poles.len(), UNIVERSAL.len());
    for (i, pole) in UNIVERSAL.iter().enumerate() {
        assert_eq!(ids[i], pole.id);
        assert_eq!(poles[i].0, pole.id);
        assert!(
            (poles[i].1 - pole.valence).abs() < 1e-5,
            "{} valence {} vs {}",
            pole.id,
            poles[i].1,
            pole.valence
        );
        assert!(
            (poles[i].2 - pole.arousal_bipolar).abs() < 1e-5,
            "{} arousal {} vs {}",
            pole.id,
            poles[i].2,
            pole.arousal_bipolar
        );
    }
}
