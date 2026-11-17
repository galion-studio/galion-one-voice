// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.

use galion_mne_contract::mirror::{self, to_json, to_toml};
use galion_mne_contract::model::{BodyOrigin, DocKind};
use galion_mne_contract::names::CowenName;
use galion_mne_contract::{binary, FRAME_MS};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures() -> BTreeMap<String, galion_mne_contract::Document> {
    let dir = root().join("fixtures");
    mirror::load_dir(&dir)
        .unwrap_or_else(|e| panic!("{e}"))
        .into_iter()
        .map(|(p, d)| (p.file_name().unwrap().to_string_lossy().into_owned(), d))
        .collect()
}

fn intent<'a>(
    all: &'a BTreeMap<String, galion_mne_contract::Document>,
    name: &str,
) -> &'a galion_mne_contract::Document {
    all.get(&format!("{name}.intent.toml"))
        .unwrap_or_else(|| panic!("missing {name}"))
}

#[test]
fn eight_scenarios_round_trip_and_use_the_config_knobs() {
    let cfg_path = root().join("../../../configs/symphony/mne-contract.toml");
    let cfg: toml::Value = toml::from_str(&std::fs::read_to_string(&cfg_path).unwrap()).unwrap();
    let origin = &cfg["baseline_origin"];
    let feeling_c = cfg["confidence_stand_in"]["feeling"].as_float().unwrap();
    let body_bare = cfg["confidence_stand_in"]["body_unmeasured"]
        .as_float()
        .unwrap();
    let body_set = cfg["confidence_stand_in"]["body_stand_in"]
        .as_float()
        .unwrap();
    let whisper_vol = cfg["body_stand_in"]["whisper_lung_volume_rel"]
        .as_float()
        .unwrap();
    let tired_vol = cfg["body_stand_in"]["tired_lung_volume_rel"]
        .as_float()
        .unwrap();

    let all = fixtures();
    assert_eq!(all.len(), 16, "8 scenarios, intent and read-back");
    for (name, doc) in &all {
        assert!(!doc.note.is_empty(), "{name}");
        assert_eq!(doc.value_origin, galion_mne_contract::ValueOrigin::StandIn);
        assert_eq!(doc.frame_ms, FRAME_MS);
        assert_eq!(doc.baseline.arousal, origin["arousal"].as_float().unwrap());
        assert_eq!(doc.baseline.valence, origin["valence"].as_float().unwrap());
        assert_eq!(
            doc.baseline.dominance,
            origin["dominance"].as_float().unwrap()
        );
        assert!(doc.frame_count() >= 8, "{name}");
        let times = doc.times_ms();
        assert_eq!(times[0], 0);
        for w in times.windows(2) {
            assert_eq!(w[1] - w[0], u32::from(FRAME_MS));
        }
        let bin = binary::encode(doc).unwrap();
        assert_eq!(binary::decode(&bin).unwrap(), *doc);
        assert_eq!(binary::encode(doc).unwrap(), bin);
        let json = to_json(doc).unwrap();
        assert!(
            bin.len() < json.len(),
            "{name} binary should be smaller than json"
        );
        let toml_bytes = to_toml(doc).unwrap();
        let re = mirror::from_toml(&toml_bytes).unwrap();
        assert_eq!(binary::encode(&re).unwrap(), bin);
        assert!(!doc.moment.memory_snippet.is_empty());
        assert_eq!(doc.persona.persona_id, "galion-one");
        assert!(doc.persona.voice_id.starts_with("profile:"));
    }

    for name in [
        "comfort",
        "tease",
        "confess",
        "argue",
        "whisper-close",
        "tired-late-night",
        "reassure",
        "quiet-awe",
    ] {
        let i = intent(&all, name);
        let r = all.get(&format!("{name}.readback.toml")).unwrap();
        assert_eq!(i.kind, DocKind::Intent);
        assert_eq!(r.kind, DocKind::Readback);
        assert_eq!(i.times_ms(), r.times_ms());
        assert_eq!(i.text, r.text);
        for frame in &r.readback_frames {
            assert!(!frame.confidence.confounded);
            assert_eq!(frame.confidence.feeling, feeling_c);
            assert!(
                frame.body.throat.f0_hz.is_none(),
                "{name} must not invent an F0"
            );
            assert!(frame.body.nose.nasalance.is_none());
            assert!(frame.body.lungs.subglottal_pressure_rel.is_none());
            match frame.body.origin {
                BodyOrigin::Unmeasured => assert_eq!(frame.confidence.body, body_bare),
                BodyOrigin::StandIn => assert_eq!(frame.confidence.body, body_set),
                BodyOrigin::Measured => panic!("{name} fixture must not claim a measurement"),
            }
        }
    }
    let wr = all.get("whisper-close.readback.toml").unwrap();
    let tr = all.get("tired-late-night.readback.toml").unwrap();
    assert_eq!(
        wr.readback_frames[0].body.lungs.lung_volume_rel,
        Some(whisper_vol)
    );
    assert_eq!(
        tr.readback_frames[0].body.lungs.lung_volume_rel,
        Some(tired_vol)
    );
}

#[test]
fn shapes_match_the_scenario_names() {
    let all = fixtures();
    let base_a = intent(&all, "comfort").baseline.arousal;
    let base_v = intent(&all, "comfort").baseline.valence;
    let base_d = intent(&all, "comfort").baseline.dominance;

    let comfort = intent(&all, "comfort");
    let last = comfort.intent_frames.last().unwrap();
    let first_pin = comfort
        .intent_frames
        .iter()
        .find(|f| f.feeling.pin.is_some())
        .unwrap();
    assert_eq!(last.feeling.pin.as_ref().unwrap().name, CowenName::Sympathy);
    assert!(
        last.feeling.pin.as_ref().unwrap().weight >= first_pin.feeling.pin.as_ref().unwrap().weight
    );
    assert!(last.feeling.pin.as_ref().unwrap().weight > 0.4);
    assert!(last.feeling.valence > base_v);
    assert!((last.feeling.arousal - base_a).abs() < 0.1);
    assert!(comfort.intent_frames[0].feeling.pin.is_none());

    let tease = intent(&all, "tease");
    let t0 = &tease.intent_frames[0];
    let t1 = tease.intent_frames.last().unwrap();
    assert_eq!(t1.feeling.pin.as_ref().unwrap().name, CowenName::Amusement);
    assert!(t1.feeling.valence > t0.feeling.valence);
    assert!(t1.feeling.pin.as_ref().unwrap().weight > 0.3);

    let confess = intent(&all, "confess");
    let c0 = &confess.intent_frames[0];
    let c1 = confess.intent_frames.last().unwrap();
    assert_eq!(
        c1.feeling.pin.as_ref().unwrap().name,
        CowenName::Embarrassment
    );
    assert_eq!(
        c1.feeling.pin.as_ref().unwrap().track,
        galion_mne_contract::Track::Prosody
    );
    assert!(c1.feeling.valence < c0.feeling.valence);
    assert!(c1.feeling.dominance < base_d);

    let argue = intent(&all, "argue");
    let a1 = argue.intent_frames.last().unwrap();
    assert_eq!(a1.feeling.pin.as_ref().unwrap().name, CowenName::Anger);
    assert!(a1.feeling.arousal > base_a + 0.2);
    assert!(a1.feeling.dominance > base_d);
    assert!(a1.feeling.pin.as_ref().unwrap().weight > 0.5);

    let whisper = intent(&all, "whisper-close");
    assert_eq!(whisper.moment.distance, "whisper_close");
    let w1 = whisper.intent_frames.last().unwrap();
    assert_eq!(w1.feeling.pin.as_ref().unwrap().name, CowenName::Adoration);
    assert!(w1.feeling.arousal <= base_a);
    let wr = all.get("whisper-close.readback.toml").unwrap();
    assert_eq!(wr.readback_frames[0].body.origin, BodyOrigin::StandIn);
    assert!(wr.readback_frames[0].body.throat.f0_hz.is_none());
    assert!(wr.readback_frames[0].body.lungs.lung_volume_rel.is_some());

    let tired = intent(&all, "tired-late-night");
    assert_eq!(tired.moment.place, "late_night_room");
    assert!(tired.intent_frames.iter().all(|f| f.feeling.pin.is_none()));
    assert!(tired.intent_frames.iter().all(|f| f.inner.fatigue >= 0.7));
    assert!(tired.intent_frames.last().unwrap().feeling.arousal < base_a - 0.1);
    let tr = all.get("tired-late-night.readback.toml").unwrap();
    let tired_vol = tr.readback_frames[0].body.lungs.lung_volume_rel.unwrap();
    let whisper_vol = wr.readback_frames[0].body.lungs.lung_volume_rel.unwrap();
    assert!(tired_vol < whisper_vol);

    let reassure = intent(&all, "reassure");
    let weights: Vec<f64> = reassure
        .intent_frames
        .iter()
        .map(|f| f.feeling.pin.as_ref().unwrap().weight)
        .collect();
    assert!(weights.iter().all(|w| *w > 0.0));
    let max = weights.iter().cloned().fold(0.0, f64::max);
    assert!(*weights.last().unwrap() < max);
    assert_eq!(
        reassure.intent_frames[0].feeling.pin.as_ref().unwrap().name,
        CowenName::Sympathy
    );

    let awe = intent(&all, "quiet-awe");
    assert!(awe.intent_frames[0].feeling.pin.is_none());
    assert_eq!(
        awe.intent_frames
            .last()
            .unwrap()
            .feeling
            .pin
            .as_ref()
            .unwrap()
            .name,
        CowenName::Awe
    );
    assert!(
        awe.intent_frames
            .last()
            .unwrap()
            .feeling
            .pin
            .as_ref()
            .unwrap()
            .weight
            > 0.4
    );
    let first_awe = awe
        .intent_frames
        .iter()
        .position(|f| f.feeling.pin.is_some())
        .unwrap();
    assert!(first_awe > 0);
    assert!(awe.intent_frames[first_awe..]
        .iter()
        .all(|f| f.feeling.pin.as_ref().unwrap().name == CowenName::Awe));
}
