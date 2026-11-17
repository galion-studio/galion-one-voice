// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Structural checks. No acoustic target is filled in here.

use crate::model::{
    BodyOrigin, BodyState, BreathSegment, DocKind, Document, Feeling, InnerFactors, IntentFrame,
    LungsState, MouthState, NoseState, ReadbackFrame, ThroatState,
};
use crate::{FRAME_MS, SCHEMA, SCHEMA_VERSION};

pub fn validate(doc: &Document) -> Result<(), String> {
    let mut errs = Vec::new();
    check(doc, &mut errs);
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs.join("\n"))
    }
}

fn check(doc: &Document, errs: &mut Vec<String>) {
    if doc.schema != SCHEMA {
        errs.push(format!("schema must be {SCHEMA}"));
    }
    if doc.schema_version != SCHEMA_VERSION {
        errs.push(format!("schema_version must be {SCHEMA_VERSION}"));
    }
    if doc.frame_ms != FRAME_MS {
        errs.push(format!("frame_ms must be {FRAME_MS}"));
    }
    if doc.note.len() > 500 {
        errs.push("note longer than 500 bytes".into());
    }
    token("persona.persona_id", &doc.persona.persona_id, true, errs);
    token("persona.voice_id", &doc.persona.voice_id, true, errs);
    token("moment.moment_id", &doc.moment.moment_id, false, errs);
    token("moment.memory_id", &doc.moment.memory_id, false, errs);
    token("moment.place", &doc.moment.place, false, errs);
    token("moment.distance", &doc.moment.distance, false, errs);
    token("moment.noise", &doc.moment.noise, false, errs);
    token("moment.listener", &doc.moment.listener, false, errs);
    if doc.moment.memory_snippet.len() > 400 {
        errs.push("memory_snippet longer than 400 bytes".into());
    }
    if doc.text.len() > 4000 {
        errs.push("text longer than 4000 bytes".into());
    }
    point(
        "baseline",
        doc.baseline.arousal,
        doc.baseline.valence,
        doc.baseline.dominance,
        errs,
    );

    match doc.kind {
        DocKind::Intent => {
            if !doc.readback_frames.is_empty() {
                errs.push("intent document carries read-back frames".into());
            }
            if doc.intent_frames.is_empty() {
                errs.push("intent document has no frames".into());
            }
            times(
                &doc.intent_frames.iter().map(|f| f.t_ms).collect::<Vec<_>>(),
                errs,
            );
            for (i, frame) in doc.intent_frames.iter().enumerate() {
                intent_frame(i, frame, &doc.text, errs);
            }
        }
        DocKind::Readback => {
            if !doc.intent_frames.is_empty() {
                errs.push("read-back document carries intent frames".into());
            }
            if doc.readback_frames.is_empty() {
                errs.push("read-back document has no frames".into());
            }
            times(
                &doc.readback_frames
                    .iter()
                    .map(|f| f.t_ms)
                    .collect::<Vec<_>>(),
                errs,
            );
            for (i, frame) in doc.readback_frames.iter().enumerate() {
                readback_frame(i, frame, errs);
            }
        }
    }
}

fn intent_frame(i: usize, frame: &IntentFrame, text: &str, errs: &mut Vec<String>) {
    let at = format!("intent_frames[{i}]");
    verb(&at, &frame.intent.verb, errs);
    unit(
        &format!("{at}.intent.strength"),
        frame.intent.strength,
        errs,
    );
    feeling(&format!("{at}.feeling"), &frame.feeling, errs);
    inner(&format!("{at}.inner"), frame.inner, errs);
    span(
        &at,
        text,
        frame.text_span.start_byte,
        frame.text_span.end_byte,
        errs,
    );
}

fn readback_frame(i: usize, frame: &ReadbackFrame, errs: &mut Vec<String>) {
    let at = format!("readback_frames[{i}]");
    feeling(&format!("{at}.feeling"), &frame.feeling, errs);
    body(&format!("{at}.body"), &frame.body, errs);
    unit(
        &format!("{at}.confidence.feeling"),
        frame.confidence.feeling,
        errs,
    );
    unit(
        &format!("{at}.confidence.body"),
        frame.confidence.body,
        errs,
    );
}

fn feeling(at: &str, feeling: &Feeling, errs: &mut Vec<String>) {
    point(
        at,
        feeling.arousal,
        feeling.valence,
        feeling.dominance,
        errs,
    );
    if let Some(pin) = &feeling.pin {
        if !pin.weight.is_finite() || pin.weight <= 0.0 || pin.weight > 1.0 {
            errs.push(format!("{at}.pin.weight must be in (0, 1]"));
        }
    }
}

fn inner(at: &str, inner: InnerFactors, errs: &mut Vec<String>) {
    unit(&format!("{at}.energy"), inner.energy, errs);
    unit(&format!("{at}.fatigue"), inner.fatigue, errs);
    unit(&format!("{at}.health"), inner.health, errs);
}

fn body(at: &str, body: &BodyState, errs: &mut Vec<String>) {
    let empty = lungs_empty(&body.lungs)
        && throat_empty(&body.throat)
        && mouth_empty(&body.mouth)
        && nose_empty(&body.nose);
    match body.origin {
        BodyOrigin::Unmeasured if !empty => {
            errs.push(format!("{at} is unmeasured but a body field is set"));
        }
        BodyOrigin::StandIn | BodyOrigin::Measured if empty => {
            errs.push(format!(
                "{at} origin claims a body read but every field is empty"
            ));
        }
        _ => {}
    }
    if body.lungs.segment == BreathSegment::Unknown && !lungs_values_empty(&body.lungs) {
        errs.push(format!("{at}.lungs segment is unknown but a level is set"));
    }
    opt_unit(
        &format!("{at}.lungs.lung_volume_rel"),
        body.lungs.lung_volume_rel,
        errs,
    );
    opt_finite(
        &format!("{at}.lungs.subglottal_pressure_rel"),
        body.lungs.subglottal_pressure_rel,
        errs,
    );
    opt_finite(
        &format!("{at}.lungs.loudness_db"),
        body.lungs.loudness_db,
        errs,
    );
    opt_positive(&format!("{at}.throat.f0_hz"), body.throat.f0_hz, errs);
    opt_finite(&format!("{at}.throat.hnr_db"), body.throat.hnr_db, errs);
    opt_positive(&format!("{at}.mouth.f1_hz"), body.mouth.f1_hz, errs);
    opt_positive(&format!("{at}.mouth.f2_hz"), body.mouth.f2_hz, errs);
    opt_positive(&format!("{at}.mouth.f3_hz"), body.mouth.f3_hz, errs);
    opt_unit(&format!("{at}.nose.nasalance"), body.nose.nasalance, errs);
}

fn lungs_values_empty(l: &LungsState) -> bool {
    l.lung_volume_rel.is_none() && l.subglottal_pressure_rel.is_none() && l.loudness_db.is_none()
}

fn lungs_empty(l: &LungsState) -> bool {
    l.segment == BreathSegment::Unknown && lungs_values_empty(l)
}

fn throat_empty(t: &ThroatState) -> bool {
    t.f0_hz.is_none() && t.hnr_db.is_none()
}

fn mouth_empty(m: &MouthState) -> bool {
    m.f1_hz.is_none() && m.f2_hz.is_none() && m.f3_hz.is_none()
}

fn nose_empty(n: &NoseState) -> bool {
    n.nasalance.is_none()
}

fn times(times: &[u32], errs: &mut Vec<String>) {
    if times.is_empty() {
        return;
    }
    if times[0] % u32::from(FRAME_MS) != 0 {
        errs.push(format!("t_ms {} is not a multiple of {FRAME_MS}", times[0]));
    }
    for pair in times.windows(2) {
        if pair[1] != pair[0] + u32::from(FRAME_MS) {
            errs.push(format!(
                "frames must step by {FRAME_MS} ms, found {} then {}",
                pair[0], pair[1]
            ));
        }
    }
}

fn span(at: &str, text: &str, start: u32, end: u32, errs: &mut Vec<String>) {
    if end < start {
        errs.push(format!("{at}.text_span ends before it starts"));
        return;
    }
    if end as usize > text.len() {
        errs.push(format!("{at}.text_span runs past the text"));
        return;
    }
    if !text.is_char_boundary(start as usize) || !text.is_char_boundary(end as usize) {
        errs.push(format!("{at}.text_span is not on a char boundary"));
    }
}

fn point(at: &str, arousal: f64, valence: f64, dominance: f64, errs: &mut Vec<String>) {
    unit(&format!("{at}.arousal"), arousal, errs);
    if !valence.is_finite() || !(-1.0..=1.0).contains(&valence) {
        errs.push(format!("{at}.valence must be in [-1, 1]"));
    }
    unit(&format!("{at}.dominance"), dominance, errs);
}

fn verb(at: &str, verb: &str, errs: &mut Vec<String>) {
    let ok = verb.len() <= 64
        && verb.starts_with(|c: char| c.is_ascii_lowercase())
        && verb
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !ok {
        errs.push(format!("{at}.intent.verb must be a short snake_case token"));
    }
}

fn token(at: &str, value: &str, allow_colon: bool, errs: &mut Vec<String>) {
    let ok = !value.is_empty()
        && value.len() <= 64
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value.chars().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || c == '_'
                || c == '-'
                || c == '.'
                || (allow_colon && c == ':')
        });
    if !ok {
        errs.push(format!("{at} must be a short token"));
    }
}

fn unit(at: &str, value: f64, errs: &mut Vec<String>) {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        errs.push(format!("{at} must be in [0, 1]"));
    }
}

fn opt_unit(at: &str, value: Option<f64>, errs: &mut Vec<String>) {
    if let Some(v) = value {
        unit(at, v, errs);
    }
}

fn opt_finite(at: &str, value: Option<f64>, errs: &mut Vec<String>) {
    if let Some(v) = value {
        if !v.is_finite() {
            errs.push(format!("{at} must be finite"));
        }
    }
}

fn opt_positive(at: &str, value: Option<f64>, errs: &mut Vec<String>) {
    if let Some(v) = value {
        if !v.is_finite() || v <= 0.0 {
            errs.push(format!("{at} must be finite and > 0"));
        }
    }
}
