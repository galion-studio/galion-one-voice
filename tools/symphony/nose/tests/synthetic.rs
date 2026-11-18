// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic checks. No person, no recording, no download.

use galion_nose::baseline::Speaker;
use galion_nose::bursts::BurstAsk;
use galion_nose::config::Config;
use galion_nose::contract::{self, Intent};
use galion_nose::feeling::{self, OpenFeeling, Weight};
use galion_nose::gestures::SIGH_CANDIDATES;
use galion_nose::inventory::Inventory;
use galion_nose::plan::{self, NasalSpan, Request};
use galion_nose::read;
use galion_nose::synth::{self, GestureRef};
use galion_nose::{handoff, TICK_MS};
use serde_json::json;

fn cfg() -> Config {
    Config::bundled().expect("bundled nose config")
}

fn inv() -> Inventory {
    Inventory::bundled().expect("emotions inventory")
}

fn speaker(f0: f64, rest: f64) -> Speaker {
    Speaker {
        f0_hz: f0,
        velum_rest: rest,
        phon_nasal: None,
    }
}

fn feel(category: &str, intensity: f64, burst: &[(&str, f64)]) -> OpenFeeling {
    OpenFeeling {
        arousal: 0.0,
        valence: 0.0,
        dominance: 0.0,
        intensity,
        category: category.into(),
        burst: burst
            .iter()
            .map(|(id, w)| Weight {
                id: (*id).into(),
                w: *w,
            })
            .collect(),
    }
    .sanitized()
    .unwrap()
}

fn burst(gesture: &str, place: &str) -> BurstAsk {
    BurstAsk {
        gesture: gesture.into(),
        at_s: None,
        place: Some(place.into()),
        category: None,
        mouth: None,
        end_s: None,
    }
}

fn planned(
    text: &str,
    feeling: OpenFeeling,
    speaker: Speaker,
    asks: Vec<BurstAsk>,
    from_feeling: bool,
) -> plan::Plan {
    let mut req = Request::new(text, feeling, speaker);
    req.bursts = asks;
    req.place_from_feeling = from_feeling;
    plan::plan(&req, &cfg(), &inv()).unwrap()
}

fn on_grid(plan: &plan::Plan) {
    assert_eq!(plan.tick_ms, TICK_MS);
    assert_eq!(plan.n_ticks, plan.ticks.len());
    for (i, t) in plan.ticks.iter().enumerate() {
        assert_eq!(t.t_ms, (i as u32) * TICK_MS);
    }
    for ev in &plan.bursts {
        assert!(near_grid(ev.start_s), "start {}", ev.start_s);
        let dur = ev.end_s - ev.start_s;
        assert!(dur >= 0.08 - 1e-9);
        assert!(near_grid(dur), "dur {dur}");
    }
}

fn near_grid(t: f64) -> bool {
    let k = (t / 0.08).round();
    (t - k * 0.08).abs() < 1e-6
}

#[test]
fn inventory_is_the_twenty_four() {
    let inv = inv();
    assert_eq!(inv.labels.len(), 24);
    assert!(inv.is_category("embarrassment"));
    assert!(inv.is_category("amusement"));
    assert!(inv.check_prosody("breath").is_err());
    assert!(inv.check_prosody("laughter").is_err());
    assert!(inv.check_prosody("sigh").is_err());
    assert!(inv.check_prosody("neutral").is_ok());
    assert!(inv.check_burst_category("neutral").is_err());
}

#[test]
fn bundled_config_keeps_bachorowski_directions() {
    let c = cfg();
    assert!(c.laugh.f0_low_open_hz > c.laugh.f0_low_closed_hz);
    assert!(c.laugh.f0_high_open_hz > c.laugh.f0_high_closed_hz);
    assert!(c.laugh.f0_high_open_hz > c.laugh.f0_low_open_hz);
    assert_eq!(c.laugh.f0_when_baseline_missing_hz, 0.0);
    assert_eq!(c.feeling.velum_per_intensity, 0.0);
    assert_eq!(c.feeling.literature, "none");
    assert_eq!(c.level.db_per_intensity, 0.0);
}

#[test]
fn contract_stub_names_its_owner() {
    assert!(contract::TODO.contains("galion-mne-contract"));
    assert!(contract::TODO.contains("TODO(S1)"));
    let bad = Intent::parse(r#"{"schema":"galion-mne-contract/1"}"#);
    assert!(bad.is_err());
}

#[test]
fn feeling_does_not_invent_nasality() {
    let plan = planned(
        "hello",
        feel("amusement", 1.0, &[("amusement", 0.0)]),
        speaker(120.0, 0.15),
        vec![],
        true,
    );
    on_grid(&plan);
    assert!(plan.bursts.is_empty(), "prosody amusement is not a laugh");
    assert!(plan
        .ticks
        .iter()
        .all(|t| (t.velum_open - 0.15).abs() < 1e-9));
    assert!(plan.ticks.iter().all(|t| t.a1_p0_direction == "flat"));
    assert!(plan.ticks.iter().all(|t| t.a1_p0_db_config == 0.0));
    assert!(plan.gaps.iter().any(|g| g.contains("TODO(S1)")));
    assert!(plan.gaps.iter().any(|g| g.contains("phon_nasal")));
}

#[test]
fn configured_feeling_gain_moves_velum() {
    let mut c = cfg();
    c.feeling.velum_per_intensity = 0.4;
    let mut req = Request::new("hello", feel("sadness", 1.0, &[]), speaker(120.0, 0.1));
    req.feeling.intensity = 1.0;
    let plan = plan::plan(&req, &c, &inv()).unwrap();
    assert!(plan.ticks.iter().all(|t| (t.velum_open - 0.5).abs() < 1e-9));
    assert!(plan.ticks.iter().all(|t| t.a1_p0_direction == "rises"));
}

#[test]
fn negative_gain_lowers_velum_and_does_not_add_an_antiresonance() {
    let mut c = cfg();
    c.feeling.velum_per_intensity = -0.3;
    let req = Request::new("", feel("neutral", 1.0, &[]), speaker(120.0, 0.5));
    let plan = plan::plan(&req, &c, &inv()).unwrap();
    assert!((plan.ticks[0].velum_open - 0.2).abs() < 1e-9);
    assert_eq!(plan.ticks[0].a1_p0_direction, "falls");
    assert_eq!(plan.ticks[0].antiresonance, 0.0);
}

#[test]
fn phon_nasal_posterior_does_not_set_velum() {
    let mut with = speaker(140.0, 0.0);
    with.phon_nasal = Some(0.9);
    let mut without = with.clone();
    without.phon_nasal = None;
    let a = planned("hi", feel("neutral", 0.0, &[]), with, vec![], false);
    let b = planned("hi", feel("neutral", 0.0, &[]), without, vec![], false);
    assert_eq!(a.phon_nasal, Some(0.9));
    assert_eq!(
        a.ticks.iter().map(|t| t.velum_open).collect::<Vec<_>>(),
        b.ticks.iter().map(|t| t.velum_open).collect::<Vec<_>>()
    );
}

#[test]
fn nasal_span_opens_only_its_ticks() {
    let mut req = Request::new(
        "hello there friend",
        feel("neutral", 0.0, &[]),
        speaker(120.0, 0.0),
    );
    req.nose.push(NasalSpan {
        start_s: 0.0,
        end_s: 0.16,
    });
    let plan = plan::plan(&req, &cfg(), &inv()).unwrap();
    assert!(plan.ticks.len() > 2);
    assert!((plan.ticks[0].velum_open - 1.0).abs() < 1e-9);
    assert!((plan.ticks[1].velum_open - 1.0).abs() < 1e-9);
    assert!(plan.ticks[2].velum_open.abs() < 1e-9);
    assert_eq!(plan.ticks[0].a1_p0_direction, "rises");
    assert_eq!(plan.ticks[0].murmur, plan.ticks[0].velum_open);
    let mut c = cfg();
    c.a1_p0.db_per_velum = 2.0;
    let plan = plan::plan(&req, &c, &inv()).unwrap();
    assert!((plan.ticks[0].a1_p0_db_config - 2.0).abs() < 1e-9);
}

#[test]
fn laugh_uses_the_sex_column_and_not_the_published_bout() {
    let low = planned(
        "",
        feel("sadness", 0.2, &[]),
        speaker(120.0, 0.0),
        vec![burst("laugh", "start")],
        false,
    );
    let high = planned(
        "",
        feel("sadness", 0.2, &[]),
        speaker(200.0, 0.0),
        vec![burst("laugh", "start")],
        false,
    );
    let edge = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(165.0, 0.0),
        vec![burst("laugh", "start")],
        false,
    );
    let below = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(164.0, 0.0),
        vec![burst("laugh", "start")],
        false,
    );
    assert_eq!(low.bursts[0].category.as_deref(), Some("amusement"));
    assert_eq!(low.bursts[0].prosody, "sadness");
    assert_eq!(low.bursts[0].f0_hz, cfg().laugh.f0_low_open_hz);
    assert_eq!(high.bursts[0].f0_hz, cfg().laugh.f0_high_open_hz);
    assert_eq!(edge.bursts[0].f0_hz, cfg().laugh.f0_high_open_hz);
    assert_eq!(below.bursts[0].f0_hz, cfg().laugh.f0_low_open_hz);
    assert_eq!(low.bursts[0].calls, Some(cfg().laugh.calls_low));
    assert_eq!(high.bursts[0].calls, Some(cfg().laugh.calls_high));
    assert_eq!(low.bursts[0].f0_source, "bachorowski2001-table-ii");
    assert_eq!(
        low.bursts[0].published_bout_s,
        Some(cfg().laugh.published_bout_low_s)
    );
    let dur = low.bursts[0].end_s - low.bursts[0].start_s;
    assert!((dur - cfg().laugh.published_bout_low_s).abs() > 0.05);
    assert!(low.ticks.iter().all(|t| t.velum_open.abs() < 1e-9));
    assert!(low.bursts[0].recipe == "laugh-calls");
    on_grid(&low);
}

#[test]
fn closed_mouth_laugh_is_lower_and_a_missing_baseline_invents_neither_mean() {
    let mut ask = burst("laugh", "start");
    ask.mouth = Some("closed".into());
    let closed = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(120.0, 0.0),
        vec![ask],
        false,
    );
    assert_eq!(closed.bursts[0].f0_hz, cfg().laugh.f0_low_closed_hz);
    assert!(closed.bursts[0].f0_hz < cfg().laugh.f0_low_open_hz);

    let missing = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(0.0, 0.0),
        vec![burst("laugh", "start")],
        false,
    );
    assert_eq!(missing.bursts[0].f0_hz, 0.0);
    assert_eq!(missing.bursts[0].f0_source, "baseline-missing");
    assert_eq!(missing.bursts[0].calls, Some(cfg().laugh.calls_unset));
    assert_ne!(missing.bursts[0].calls, Some(cfg().laugh.calls_low));
    assert!(missing
        .gaps
        .iter()
        .any(|g| g.contains("not given a Bachorowski")));
}

#[test]
fn laugh_refuses_a_category_that_is_not_amusement() {
    let mut ask = burst("laugh", "start");
    ask.category = Some("sadness".into());
    let mut req = Request::new("", feel("neutral", 0.0, &[]), speaker(120.0, 0.0));
    req.bursts.push(ask);
    let err = plan::plan(&req, &cfg(), &inv()).unwrap_err();
    assert!(err.contains("amusement"));
}

#[test]
fn sigh_stays_a_detector_and_the_four_share_one_shape() {
    let mut relief = burst("sigh", "start");
    relief.category = Some("relief".into());
    let mut sadness = burst("sigh", "start");
    sadness.category = Some("sadness".into());
    let a = planned(
        "",
        feel("neutral", 0.4, &[]),
        speaker(180.0, 0.0),
        vec![relief],
        false,
    );
    let b = planned(
        "",
        feel("neutral", 0.4, &[]),
        speaker(180.0, 0.0),
        vec![sadness],
        false,
    );
    assert_eq!(a.bursts[0].category.as_deref(), Some("relief"));
    assert_eq!(a.bursts[0].candidates, SIGH_CANDIDATES);
    assert_eq!(a.bursts[0].recipe, b.bursts[0].recipe);
    assert_eq!(a.bursts[0].f0_hz, b.bursts[0].f0_hz);
    assert_eq!(a.bursts[0].level_dbfs, b.bursts[0].level_dbfs);
    assert_eq!(a.bursts[0].end_s, b.bursts[0].end_s);
    assert!(!a.bursts[0].opens_velum);

    let open = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(180.0, 0.0),
        vec![burst("sigh", "start")],
        false,
    );
    assert!(open.bursts[0].category.is_none());
    assert_eq!(open.bursts[0].candidates, SIGH_CANDIDATES);

    let mut bad = burst("sigh", "start");
    bad.category = Some("anger".into());
    let mut req = Request::new("", feel("neutral", 0.0, &[]), speaker(180.0, 0.0));
    req.bursts.push(bad);
    assert!(plan::plan(&req, &cfg(), &inv()).is_err());
}

#[test]
fn unnamed_gestures_stay_unnamed_and_hum_opens_the_velum() {
    for gesture in ["moan", "gasp", "hum", "mm"] {
        let plan = planned(
            "",
            feel("pain", 0.5, &[]),
            speaker(150.0, 0.0),
            vec![burst(gesture, "start")],
            false,
        );
        assert!(plan.bursts[0].category.is_none(), "{gesture}");
        assert!(plan.bursts[0].candidates.is_empty(), "{gesture}");
        assert_eq!(plan.bursts[0].prosody, "pain");
    }
    let hum = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(150.0, 0.0),
        vec![burst("hum", "start")],
        false,
    );
    let mm = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(150.0, 0.0),
        vec![burst("mm", "start")],
        false,
    );
    assert!(hum
        .ticks
        .iter()
        .any(|t| t.gesture.as_deref() == Some("hum") && t.velum_open > 0.99));
    assert!(mm
        .ticks
        .iter()
        .any(|t| t.gesture.as_deref() == Some("mm") && t.velum_open > 0.99));
    assert!(hum.bursts[0].opens_velum && mm.bursts[0].opens_velum);
    assert_eq!(hum.bursts[0].f0_hz, 150.0);
    assert_eq!(hum.bursts[0].f0_source, "speaker-baseline");
}

#[test]
fn valence_and_dominance_do_not_move_a_knob() {
    let mut low = feel("neutral", 0.3, &[]);
    low.valence = -1.0;
    low.dominance = -1.0;
    let mut high = feel("neutral", 0.3, &[]);
    high.valence = 1.0;
    high.dominance = 1.0;
    let a = planned(
        "",
        low,
        speaker(120.0, 0.2),
        vec![burst("laugh", "start")],
        false,
    );
    let b = planned(
        "",
        high,
        speaker(120.0, 0.2),
        vec![burst("laugh", "start")],
        false,
    );
    assert_eq!(a.bursts[0].f0_hz, b.bursts[0].f0_hz);
    assert_eq!(a.bursts[0].level_dbfs, b.bursts[0].level_dbfs);
    assert_eq!(a.bursts[0].end_s, b.bursts[0].end_s);
    assert_eq!(a.ticks[0].velum_open, b.ticks[0].velum_open);
}

#[test]
fn intensity_scales_level_only_through_the_config_slope() {
    let mut c = cfg();
    c.level.db_per_intensity = 10.0;
    let quiet = plan::plan(
        &Request::new("", feel("neutral", 0.0, &[]), speaker(120.0, 0.0)),
        &c,
        &inv(),
    );
    let mut loud_req = Request::new("", feel("neutral", 1.0, &[]), speaker(120.0, 0.0));
    loud_req.bursts.push(burst("gasp", "start"));
    let mut quiet_req = Request::new("", feel("neutral", 0.0, &[]), speaker(120.0, 0.0));
    quiet_req.bursts.push(burst("gasp", "start"));
    let _ = quiet;
    let loud = plan::plan(&loud_req, &c, &inv()).unwrap();
    let quiet = plan::plan(&quiet_req, &c, &inv()).unwrap();
    assert!((loud.bursts[0].level_dbfs - quiet.bursts[0].level_dbfs - 10.0).abs() < 1e-9);
    let rendered_l = synth::render(&loud, 16_000.0, &[], &c).unwrap();
    let rendered_q = synth::render(&quiet, 16_000.0, &[], &c).unwrap();
    let lr = synth::rms(&rendered_l.bursts);
    let qr = synth::rms(&rendered_q.bursts);
    assert!(lr > qr * 1.5);
}

#[test]
fn feeling_places_a_laugh_only_from_the_burst_track() {
    let prosody = planned(
        "hello",
        feel("amusement", 0.8, &[]),
        speaker(120.0, 0.0),
        vec![],
        true,
    );
    assert!(prosody.bursts.is_empty());

    let burst_track = planned(
        "",
        feel("neutral", 0.5, &[("amusement", 0.4)]),
        speaker(120.0, 0.0),
        vec![],
        true,
    );
    assert_eq!(burst_track.bursts.len(), 1);
    assert_eq!(burst_track.bursts[0].gesture, "laugh");
    assert_eq!(burst_track.bursts[0].category.as_deref(), Some("amusement"));

    let off = planned(
        "",
        feel("neutral", 0.5, &[("amusement", 0.4)]),
        speaker(120.0, 0.0),
        vec![],
        false,
    );
    assert!(off.bursts.is_empty());

    let tie = planned(
        "",
        feel("neutral", 0.5, &[("amusement", 0.4), ("sadness", 0.4)]),
        speaker(120.0, 0.0),
        vec![],
        true,
    );
    assert!(tie.bursts.is_empty());
    assert!(tie.gaps.iter().any(|g| g.contains("tie")));

    let other = planned(
        "",
        feel("neutral", 0.5, &[("fear", 0.9)]),
        speaker(120.0, 0.0),
        vec![],
        true,
    );
    assert!(other.bursts.is_empty());
    assert!(other.gaps.iter().any(|g| g.contains("fear")));
}

#[test]
fn bursts_do_not_overlap_and_unknown_gestures_fail() {
    let plan = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(120.0, 0.0),
        vec![burst("sigh", "start"), burst("gasp", "start")],
        false,
    );
    assert!(plan.bursts[1].start_s >= plan.bursts[0].end_s - 1e-9);
    let mut req = Request::new("", feel("neutral", 0.0, &[]), speaker(120.0, 0.0));
    req.bursts.push(burst("breath", "start"));
    assert!(plan::plan(&req, &cfg(), &inv()).is_err());
}

#[test]
fn decoder_tick_and_worker_json_are_read_not_reinvented() {
    let tick = json!({
        "arousal": 0.2,
        "valence": 0.1,
        "dominance": 0.0,
        "intensity": 0.4,
        "category": "amusement",
        "blend": [{"id": "neutral", "w": 0.6}, {"id": "amusement", "w": 0.4}],
        "burst": [{"id": "amusement", "w": 0.0}]
    });
    let parsed = feeling::from_value(&tick).unwrap();
    assert_eq!(parsed.category, "amusement");
    assert!((parsed.intensity - 0.4).abs() < 1e-9);

    let clash = json!({
        "category": "sadness",
        "blend": [{"id": "amusement", "w": 1.0}]
    });
    assert!(feeling::from_value(&clash).is_err());
    let named_tie = json!({
        "category": "sadness",
        "blend": [{"id": "sadness", "w": 0.4}, {"id": "fear", "w": 0.4}]
    });
    assert_eq!(feeling::from_value(&named_tie).unwrap().category, "sadness");
    let open_tie = json!({
        "blend": [{"id": "sadness", "w": 0.4}, {"id": "fear", "w": 0.4}]
    });
    assert!(feeling::from_value(&open_tie).is_err());

    let gap: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/laughter-worker-gap.json")).unwrap();
    let reading = read::parse_reading(&gap).unwrap();
    assert!(reading.events.is_empty());
    assert!(reading.gap.unwrap().contains("GALION_LAUGHTER_REPO"));

    let event: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/laughter-worker-event.json")).unwrap();
    let reading = read::parse_reading(&event).unwrap();
    assert_eq!(reading.events.len(), 1);
    assert_eq!(reading.events[0].gesture, "laugh");
    assert_eq!(reading.events[0].category.as_deref(), Some("amusement"));
    assert!((reading.events[0].start_s - 0.24).abs() < 1e-9);

    let track: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/decode-burst.json")).unwrap();
    let reading = read::parse_reading(&track).unwrap();
    assert!(reading.events.is_empty());
    assert_eq!(reading.burst_top.as_deref(), Some("amusement"));
    assert!(
        (reading
            .weights
            .iter()
            .find(|w| w.id == "amusement")
            .unwrap()
            .w
            - 0.2)
            .abs()
            < 1e-9
    );

    let ticks = json!({
        "ticks": [
            {"burst": [{"id": "amusement", "w": 1.0}]},
            {"burst": [{"id": "amusement", "w": 0.0}]}
        ]
    });
    let reading = read::parse_reading(&ticks).unwrap();
    assert!((reading.weights[0].w - 0.5).abs() < 1e-9);

    let sigh =
        json!({"source": "events", "events": [{"start_s": 0.0, "end_s": 0.4, "kind": "sigh"}]});
    let reading = read::parse_reading(&sigh).unwrap();
    assert!(reading.events[0].category.is_none());
    assert_eq!(reading.events[0].candidates, SIGH_CANDIDATES);
}

#[test]
fn adopting_a_detected_laugh_keeps_the_detector_span() {
    let text = include_str!("../fixtures/intent-laugh.json");
    let mut intent = Intent::parse(text).unwrap();
    intent.bursts.clear();
    intent.adopt_reading = true;
    intent.reading =
        Some(serde_json::from_str(include_str!("../fixtures/laughter-worker-event.json")).unwrap());
    let (plan, reading) = contract::plan_intent(&intent, &cfg(), &inv()).unwrap();
    assert!(reading.unwrap().events.len() == 1);
    assert_eq!(plan.bursts.len(), 1);
    assert_eq!(plan.bursts[0].timing, "detected");
    assert!((plan.bursts[0].start_s - 0.24).abs() < 1e-9);
    assert!((plan.bursts[0].end_s - 0.88).abs() < 1e-9);
    assert_eq!(plan.bursts[0].category.as_deref(), Some("amusement"));
    assert_eq!(plan.bursts[0].f0_hz, cfg().laugh.f0_low_open_hz);
}

#[test]
fn intent_fixture_laugh_is_amusement_and_does_not_nasalize() {
    let intent = Intent::parse(include_str!("../fixtures/intent-laugh.json")).unwrap();
    let (plan, _) = contract::plan_intent(&intent, &cfg(), &inv()).unwrap();
    assert_eq!(plan.bursts.len(), 1);
    assert_eq!(plan.bursts[0].gesture, "laugh");
    assert_eq!(plan.bursts[0].category.as_deref(), Some("amusement"));
    assert!(plan.bursts[0].start_s >= plan.speech_s - 1e-9);
    assert!(plan
        .ticks
        .iter()
        .all(|t| t.velum_open.abs() < 1e-9 || t.gesture.is_some()));
    assert!(plan
        .ticks
        .iter()
        .filter(|t| t.gesture.is_none())
        .all(|t| t.velum_open.abs() < 1e-9));
    let patch = handoff::profile_patch(&plan);
    assert_eq!(patch["nose"]["status"], "planned");
    assert_eq!(patch["nose"]["source"], "galion-nose");
    assert!(patch["bursts"]["events"].as_array().unwrap().len() == 1);
    let body = serde_json::to_string(&plan).unwrap();
    assert!(!body.contains("\"samples\""));
}

#[test]
fn murmur_follows_velum_and_a_reference_replaces_the_recipe() {
    let rest = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(0.0, 1.0),
        vec![],
        false,
    );
    let closed = planned(
        "",
        feel("neutral", 0.0, &[]),
        speaker(0.0, 0.0),
        vec![],
        false,
    );
    let open_audio = synth::render(&rest, 24_000.0, &[], &cfg()).unwrap();
    let shut_audio = synth::render(&closed, 24_000.0, &[], &cfg()).unwrap();
    let open_rms = synth::rms(&open_audio.nose);
    assert!((open_rms - cfg().murmur.rms_at_open).abs() < 1e-3);
    assert_eq!(synth::rms(&shut_audio.nose), 0.0);
    assert!(shut_audio.nose.iter().all(|s| *s == 0.0));

    let laugh = planned(
        "",
        feel("neutral", 0.2, &[]),
        speaker(120.0, 0.0),
        vec![burst("laugh", "start")],
        false,
    );
    let again = synth::render(&laugh, 16_000.0, &[], &cfg()).unwrap();
    let once = synth::render(&laugh, 16_000.0, &[], &cfg()).unwrap();
    assert_eq!(once.bursts, again.bursts);
    assert_eq!(once.nose, again.nose);
    assert_eq!(once.burst_events[0].source, "synthesized");

    let reference = [0.0, 1.0, 0.0, -1.0];
    let rendered = synth::render(
        &laugh,
        16_000.0,
        &[GestureRef {
            gesture: "laugh",
            samples: &reference,
            sample_rate: 16_000.0,
        }],
        &cfg(),
    )
    .unwrap();
    assert_eq!(rendered.burst_events[0].source, "reference");
    assert!(rendered.bursts.iter().any(|s| *s > 0.0));
    assert!(rendered.bursts.iter().any(|s| *s < 0.0));
    let target = synth::db_to_rms(laugh.bursts[0].level_dbfs);
    assert!((synth::rms(&rendered.bursts) - target).abs() / target < 0.05);
}

#[test]
fn wav_roundtrip_stays_outside_the_tree() {
    let dir = std::env::temp_dir().join("galion-nose-synthetic");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ref.wav");
    let samples: Vec<f64> = (0..160).map(|i| (i as f64 * 0.1).sin() * 0.25).collect();
    galion_nose::wav::write_wav_mono_i16(&path, 16_000, &samples).unwrap();
    let back = galion_nose::wav::read_wav(&path).unwrap();
    assert_eq!(back.sample_rate, 16_000.0);
    assert_eq!(back.samples.len(), samples.len());
    for (a, b) in samples.iter().zip(&back.samples) {
        assert!((a - b).abs() < 1.0 / 32768.0 + 1e-6);
    }
    std::fs::remove_dir_all(&dir).unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../");
    assert!(!path.starts_with(repo.canonicalize().unwrap_or(repo)));
}
