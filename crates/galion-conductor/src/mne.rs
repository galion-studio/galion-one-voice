// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! MnE intent in, read-back out.
//!
//! The conductor accepts a `galion-mne-contract` intent document and returns
//! a read-back on the same 80 ms grid. The body is a stand-in: rest breath
//! plus a nose slot for the wordless reaction. No formant, no recording.

use galion_mne_contract::{
    validate, BaselinePoint, BodyOrigin, BodyState, BreathSegment, Confidence, DocKind, Document,
    Feeling, InnerFactors, Intent, IntentFrame, LungsState, Moment, NoseState, PersonaRef,
    ReadbackFrame, TextSpan, ValueOrigin, FRAME_MS, SCHEMA, SCHEMA_VERSION,
};

use crate::reaction::{Reaction, ReactionKind};

/// A one-frame listen intent. Numbers are stand-ins, not a measurement.
pub fn stand_in_listen_intent() -> Document {
    Document {
        schema: SCHEMA.to_string(),
        schema_version: SCHEMA_VERSION,
        kind: DocKind::Intent,
        frame_ms: FRAME_MS,
        value_origin: ValueOrigin::StandIn,
        note: "Listen. Wordless reaction is the nose slot.".to_string(),
        persona: PersonaRef {
            persona_id: "galion-one".to_string(),
            voice_id: "profile:steady".to_string(),
        },
        moment: Moment {
            moment_id: "listen".to_string(),
            memory_id: "none".to_string(),
            memory_snippet: String::new(),
            place: "quiet_room".to_string(),
            distance: "conversational".to_string(),
            noise: "quiet".to_string(),
            listener: "one".to_string(),
        },
        text: String::new(),
        baseline: BaselinePoint {
            arousal: 0.4,
            valence: 0.0,
            dominance: 0.5,
        },
        intent_frames: vec![IntentFrame {
            t_ms: 0,
            intent: Intent {
                verb: "listen".to_string(),
                strength: 0.4,
            },
            feeling: Feeling {
                arousal: 0.4,
                valence: 0.1,
                dominance: 0.5,
                pin: None,
            },
            inner: InnerFactors {
                energy: 0.6,
                fatigue: 0.1,
                health: 1.0,
            },
            text_span: TextSpan {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        readback_frames: Vec::new(),
    }
}

/// Build a read-back from a validated intent. The nose slot is the wordless reaction.
pub fn read_back(intent: &Document, reaction: Option<&Reaction>) -> Result<Document, String> {
    if intent.kind != DocKind::Intent {
        return Err("conductor read-back expects an intent document".to_string());
    }
    validate(intent)?;
    let loudness_db = match reaction.map(|item| item.kind) {
        Some(ReactionKind::Synthetic) => -12.0,
        Some(ReactionKind::Silence) | None => 0.0,
    };
    let frames = intent
        .intent_frames
        .iter()
        .map(|frame| ReadbackFrame {
            t_ms: frame.t_ms,
            feeling: frame.feeling.clone(),
            body: BodyState {
                origin: BodyOrigin::StandIn,
                lungs: LungsState {
                    segment: BreathSegment::Rest,
                    lung_volume_rel: Some(0.45),
                    subglottal_pressure_rel: None,
                    loudness_db: Some(loudness_db),
                },
                throat: Default::default(),
                mouth: Default::default(),
                nose: NoseState {
                    nasalance: Some(0.35),
                },
            },
            confidence: Confidence {
                feeling: 0.5,
                body: 0.5,
                confounded: false,
            },
        })
        .collect();
    let doc = Document {
        schema: SCHEMA.to_string(),
        schema_version: SCHEMA_VERSION,
        kind: DocKind::Readback,
        frame_ms: FRAME_MS,
        value_origin: ValueOrigin::StandIn,
        note: "Wordless reaction. Nose slot is a stand-in.".to_string(),
        persona: intent.persona.clone(),
        moment: intent.moment.clone(),
        text: String::new(),
        baseline: intent.baseline,
        intent_frames: Vec::new(),
        readback_frames: frames,
    };
    validate(&doc)?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reaction::ReactionKind;

    #[test]
    fn intent_comes_back_as_a_valid_readback_with_a_nose_slot() {
        let intent = stand_in_listen_intent();
        validate(&intent).unwrap();
        let reaction = Reaction::from_kind(ReactionKind::Silence);
        let doc = read_back(&intent, Some(&reaction)).unwrap();
        assert_eq!(doc.kind, DocKind::Readback);
        assert!(doc.intent_frames.is_empty());
        assert_eq!(doc.readback_frames.len(), 1);
        assert_eq!(doc.readback_frames[0].t_ms, 0);
        assert_eq!(doc.readback_frames[0].body.nose.nasalance, Some(0.35));
        assert_eq!(
            doc.readback_frames[0].body.lungs.segment,
            BreathSegment::Rest
        );
        assert!(doc.text.is_empty());
    }
}
