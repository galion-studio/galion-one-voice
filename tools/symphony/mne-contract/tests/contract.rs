// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.

use galion_mne_contract::mirror::{self, from_json, from_toml, to_json, to_toml};
use galion_mne_contract::model::*;
use galion_mne_contract::names::{
    resolve_name, CowenName, ProsodyTier, Track, COWEN, EXTENDED, RELIABLE,
};
use galion_mne_contract::wire::project_emotion_wire;
use galion_mne_contract::{binary, validate, FRAME_MS, SCHEMA};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn sample() -> Document {
    Document {
        schema: SCHEMA.into(),
        schema_version: 1,
        kind: DocKind::Intent,
        frame_ms: FRAME_MS,
        value_origin: ValueOrigin::StandIn,
        note: "unit".into(),
        persona: PersonaRef {
            persona_id: "galion-one".into(),
            voice_id: "profile:galion-one-steady".into(),
        },
        moment: Moment {
            moment_id: "unit-moment".into(),
            memory_id: "mem-unit".into(),
            memory_snippet: "A made-up note, not a person.".into(),
            place: "quiet_room".into(),
            distance: "conversational".into(),
            noise: "quiet".into(),
            listener: "one_person".into(),
        },
        text: "Hello there.".into(),
        baseline: BaselinePoint {
            arousal: 0.40,
            valence: 0.0,
            dominance: 0.50,
        },
        intent_frames: vec![
            frame(0, None, 0.40, 0.0, 0.50, 0, 12),
            frame(
                80,
                Some(Pin {
                    name: CowenName::Embarrassment,
                    track: Track::Prosody,
                    weight: 0.4,
                }),
                0.62,
                -0.2,
                0.30,
                0,
                12,
            ),
        ],
        readback_frames: vec![],
    }
}

fn frame(t_ms: u32, pin: Option<Pin>, a: f64, v: f64, d: f64, start: u32, end: u32) -> IntentFrame {
    IntentFrame {
        t_ms,
        intent: Intent {
            verb: "confess".into(),
            strength: 0.5,
        },
        feeling: Feeling {
            arousal: a,
            valence: v,
            dominance: d,
            pin,
        },
        inner: InnerFactors {
            energy: 0.5,
            fatigue: 0.1,
            health: 1.0,
        },
        text_span: TextSpan {
            start_byte: start,
            end_byte: end,
        },
    }
}

fn readback_with(body: BodyState) -> Document {
    Document {
        schema: SCHEMA.into(),
        schema_version: 1,
        kind: DocKind::Readback,
        frame_ms: FRAME_MS,
        value_origin: ValueOrigin::Config,
        note: String::new(),
        persona: sample().persona,
        moment: sample().moment,
        text: sample().text,
        baseline: sample().baseline,
        intent_frames: vec![],
        readback_frames: vec![ReadbackFrame {
            t_ms: 0,
            feeling: Feeling {
                arousal: 0.4,
                valence: 0.1,
                dominance: 0.5,
                pin: Some(Pin {
                    name: CowenName::Awe,
                    track: Track::Prosody,
                    weight: 0.3,
                }),
            },
            body,
            confidence: Confidence {
                feeling: 0.5,
                body: 0.0,
                confounded: false,
            },
        }],
    }
}

#[test]
fn inventory_matches_emotions_v2() {
    let path = repo_root().join("configs/emotions-v2.toml");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let v: toml::Value = toml::from_str(&text).unwrap();
    let labels: Vec<String> = v["inventory"]["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert_eq!(labels, COWEN);
    let reliable: Vec<String> = v["prosody_tier"]["cross_culturally_reliable"]["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert_eq!(reliable, RELIABLE);
    let extended: Vec<String> = v["prosody_tier"]["valid_lower_recognition_evidence"]["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert_eq!(extended, EXTENDED);
    assert_eq!(CowenName::ALL.len(), 24);
    for (i, name) in CowenName::ALL.into_iter().enumerate() {
        assert_eq!(name.index(), i as u8);
        assert_eq!(name.as_str(), COWEN[i]);
        assert_eq!(CowenName::parse(name.as_str()), Some(name));
        let tier = if RELIABLE.contains(&name.as_str()) {
            ProsodyTier::Reliable
        } else {
            ProsodyTier::Extended
        };
        assert_eq!(name.tier(), tier);
    }
    assert_eq!(CowenName::Embarrassment.tier(), ProsodyTier::Extended);
    assert_eq!(CowenName::Anger.tier(), ProsodyTier::Reliable);
    assert!(EXTENDED.contains(&"embarrassment"));
}

#[test]
fn old_names_follow_the_inventory_map() {
    let calm = resolve_name("calm", Track::Burst).unwrap();
    assert!(calm.pin.is_none());
    assert_eq!(calm.alias_from, Some("calm"));

    let tender = resolve_name("tender", Track::Prosody).unwrap();
    assert_eq!(tender.pin, Some(CowenName::Adoration));

    let seductive = resolve_name("seductive", Track::Prosody).unwrap();
    assert_eq!(seductive.pin, Some(CowenName::Desire));
    assert_eq!(seductive.track, Track::Prosody);

    let laugh = resolve_name("laughter", Track::Prosody).unwrap();
    assert_eq!(laugh.pin, Some(CowenName::Amusement));
    assert_eq!(laugh.track, Track::Burst);

    let disgusted = resolve_name("disgusted", Track::Prosody).unwrap();
    assert_eq!(disgusted.pin, Some(CowenName::Contempt));

    assert!(resolve_name("disgust", Track::Prosody).unwrap().pin == Some(CowenName::Disgust));
    assert!(resolve_name("breath", Track::Prosody).is_err());
    assert!(resolve_name("sigh", Track::Prosody).is_err());
    assert!(resolve_name("happy", Track::Prosody).is_err());
    assert!(resolve_name("excited", Track::Prosody).is_err());
    assert!(resolve_name("surprised", Track::Prosody).is_err());
    assert!(resolve_name("serenity", Track::Prosody).is_err());
    assert!(resolve_name("shame", Track::Prosody).is_err());
}

#[test]
fn binary_json_and_toml_are_lossless() {
    let doc = sample();
    let bin = binary::encode(&doc).unwrap();
    assert_eq!(binary::encode(&doc).unwrap(), bin);
    let back = binary::decode(&bin).unwrap();
    assert_eq!(back, doc);

    let json = to_json(&doc).unwrap();
    assert!(
        json.len() > bin.len(),
        "binary {} vs json {}",
        bin.len(),
        json.len()
    );
    assert_eq!(from_json(&json).unwrap(), doc);
    assert_eq!(
        from_json(&to_json(&from_json(&json).unwrap()).unwrap()).unwrap(),
        doc
    );

    let toml_bytes = to_toml(&doc).unwrap();
    let from_t = from_toml(&toml_bytes).unwrap();
    assert_eq!(from_t, doc);
    assert_eq!(binary::encode(&from_t).unwrap(), bin);
    let via_json = from_json(&to_json(&from_t).unwrap()).unwrap();
    assert_eq!(binary::encode(&via_json).unwrap(), bin);

    let burst = {
        let mut d = doc.clone();
        d.intent_frames[1].feeling.pin = Some(Pin {
            name: CowenName::Amusement,
            track: Track::Burst,
            weight: 1.0,
        });
        d
    };
    assert_eq!(
        binary::decode(&binary::encode(&burst).unwrap()).unwrap(),
        burst
    );
}

#[test]
fn readback_roundtrip_keeps_absent_levels_absent() {
    let bare = readback_with(BodyState::default());
    assert_eq!(
        binary::decode(&binary::encode(&bare).unwrap()).unwrap(),
        bare
    );
    assert_eq!(from_toml(&to_toml(&bare).unwrap()).unwrap(), bare);

    let lungs = LungsState {
        segment: BreathSegment::Speech,
        lung_volume_rel: Some(0.62),
        ..LungsState::default()
    };
    let set = readback_with(BodyState {
        origin: BodyOrigin::StandIn,
        lungs,
        ..BodyState::default()
    });
    assert!(validate(&set).is_ok(), "{}", validate(&set).unwrap_err());
    let back = binary::decode(&binary::encode(&set).unwrap()).unwrap();
    assert_eq!(back, set);
    assert!(back.readback_frames[0].body.throat.f0_hz.is_none());
    assert!(back.readback_frames[0]
        .body
        .lungs
        .subglottal_pressure_rel
        .is_none());
}

#[test]
fn validation_rejects_structurally_bad_frames() {
    let mut gap = sample();
    gap.intent_frames[1].t_ms = 160;
    assert!(validate(&gap).unwrap_err().contains("step"));

    let mut span = sample();
    span.intent_frames[0].text_span.end_byte = 4;
    span.intent_frames[0].text_span.start_byte = 6;
    assert!(validate(&span).is_err());

    let mut mid = sample();
    mid.text = "é".into();
    mid.intent_frames[0].text_span = TextSpan {
        start_byte: 1,
        end_byte: 1,
    };
    mid.intent_frames[1].text_span = TextSpan {
        start_byte: 0,
        end_byte: 2,
    };
    assert!(validate(&mid).unwrap_err().contains("char boundary"));

    let mut weight = sample();
    weight.intent_frames[1].feeling.pin.as_mut().unwrap().weight = 0.0;
    assert!(validate(&weight).is_err());

    let mut arousal = sample();
    arousal.intent_frames[0].feeling.arousal = -0.2;
    assert!(validate(&arousal).unwrap_err().contains("arousal"));

    let mut clock = sample();
    clock.frame_ms = 40;
    assert!(validate(&clock).is_err());

    let mut verb = sample();
    verb.intent_frames[0].intent.verb = "Comfort".into();
    assert!(validate(&verb).is_err());

    let mut body = readback_with(BodyState::default());
    body.readback_frames[0].body.lungs.lung_volume_rel = Some(0.5);
    assert!(validate(&body).unwrap_err().contains("unmeasured"));

    let empty_claim = readback_with(BodyState {
        origin: BodyOrigin::Measured,
        ..BodyState::default()
    });
    assert!(validate(&empty_claim).is_err());

    assert!(binary::decode(b"GMN1").is_err());
    let bin = binary::encode(&sample()).unwrap();
    assert!(binary::decode(&bin[..bin.len() - 1]).is_err());
}

#[test]
fn emotion_wire_projection_matches_the_existing_quantisation() {
    let feeling = Feeling {
        arousal: 1.0,
        valence: -1.0,
        dominance: 0.5,
        pin: None,
    };
    let s = project_emotion_wire(&feeling, 1.0);
    assert_eq!(s.valence_i8, -127);
    assert_eq!(s.arousal_u8, 255);
    assert_eq!(s.dominance_u8, 128);
    assert_eq!(s.confidence_u8, 255);
    let zero = project_emotion_wire(
        &Feeling {
            arousal: 0.0,
            valence: 0.0,
            dominance: 0.0,
            pin: None,
        },
        0.0,
    );
    assert_eq!(
        (
            zero.valence_i8,
            zero.arousal_u8,
            zero.dominance_u8,
            zero.confidence_u8
        ),
        (0, 0, 0, 0)
    );
}

#[test]
fn cli_encode_decode_validate() {
    let dir = std::env::temp_dir().join(format!("mne-contract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("in.toml");
    let bin = dir.join("frame.mne");
    let json = dir.join("out.json");
    mirror::store_path(&src, &sample()).unwrap();
    let exe = env!("CARGO_BIN_EXE_galion-mne-contract");
    let ok = std::process::Command::new(exe)
        .args(["validate", src.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(ok.success());
    let enc = std::process::Command::new(exe)
        .args(["encode", src.to_str().unwrap(), bin.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(enc.success());
    let dec = std::process::Command::new(exe)
        .args(["decode", bin.to_str().unwrap(), json.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(dec.success());
    let back = mirror::load_path(&json).unwrap();
    assert_eq!(back, sample());
    let bad = std::process::Command::new(exe)
        .args(["validate", bin.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(bad.success());
    let _ = std::fs::remove_dir_all(&dir);
}
