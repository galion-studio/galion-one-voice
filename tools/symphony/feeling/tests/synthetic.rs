// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic intent only. No voice audio, no download, no timing measurement.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use galion_feeling::{alpha, Config, Feeling, Inventory, TICK_MS};
use galion_feeling::{parse_frames, AxisTarget, IntentFrame, PinIntent, FRAME_SCHEMA as CONTRACT};

fn bundled() -> (Config, Inventory) {
    let cfg = Config::load(&Config::bundled_path()).unwrap();
    let inv = Inventory::load(&Inventory::bundled_path()).unwrap();
    (cfg, inv)
}

fn blank(t_ms: u32) -> IntentFrame {
    IntentFrame {
        schema: None,
        t_ms,
        axes: Vec::new(),
        pins: Vec::new(),
        blend: None,
        track: None,
        energy: None,
        fatigue: None,
        environment: None,
        listener_distance_m: None,
        release: false,
    }
}

fn with_axis(t_ms: u32, name: &str, value: f64) -> IntentFrame {
    let mut frame = blank(t_ms);
    frame.axes.push(AxisTarget {
        name: name.to_string(),
        value,
    });
    frame
}

fn with_pin(t_ms: u32, id: &str, w: f64) -> IntentFrame {
    let mut frame = blank(t_ms);
    frame.pins.push(PinIntent {
        id: id.to_string(),
        w,
    });
    frame
}

fn hold_axis(name: &str, value: f64, n: u32) -> Vec<IntentFrame> {
    (0..n)
        .map(|i| with_axis(i * TICK_MS, name, value))
        .collect()
}

fn hold_pin(id: &str, n: u32) -> Vec<IntentFrame> {
    (0..n).map(|i| with_pin(i * TICK_MS, id, 1.0)).collect()
}

fn run(frames: &[IntentFrame], cfg: &Config, inv: &Inventory) -> galion_feeling::Run {
    Feeling::run(frames, cfg, inv).unwrap()
}

#[test]
fn the_clock_is_80_ms_and_the_schemas_are_versioned() {
    let (cfg, inv) = bundled();
    assert_eq!(TICK_MS, 80);
    assert_eq!(cfg.tick_ms, 80);
    let out = run(&[blank(0), blank(80)], &cfg, &inv);
    assert_eq!(out.schema, "galion-feeling/1");
    assert_eq!(out.contract, CONTRACT);
    assert_eq!(out.tick_ms, 80);
    assert_eq!(out.frames[0].t_ms, 0);
    assert_eq!(out.frames[1].t_ms, 80);
    assert!(out.frames[0].nearest.is_none());
}

#[test]
fn one_tick_moves_toward_the_target_and_does_not_arrive() {
    let (cfg, inv) = bundled();
    let out = run(&hold_axis("valence", 1.0, 1), &cfg, &inv);
    let v = out.frames[0].axis("valence");
    assert!(v > 0.0 && v < 0.5, "valence {v}");
    assert!((out.frames[0].goal_axis("valence") - 1.0).abs() < 1e-12);
    let a = alpha(cfg.inertia_half_life_s, 0.08);
    let expect = 1.0 - 0.5_f64.sqrt();
    assert!((a - expect).abs() < 1e-12, "alpha {a}");
}

#[test]
fn a_held_target_is_reached_and_a_partial_blend_stays_between() {
    let (cfg, inv) = bundled();
    let full = run(&hold_axis("valence", 1.0, 80), &cfg, &inv);
    let v = full.frames[79].axis("valence");
    assert!((v - 1.0).abs() < 0.05, "valence {v}");

    let mut mid = vec![with_axis(0, "valence", 1.0)];
    mid[0].blend = Some(0.5);
    for i in 1..80 {
        mid.push(blank(i * TICK_MS));
    }
    let half = run(&mid, &cfg, &inv);
    let g = half.frames[79].goal_axis("valence");
    let now = half.frames[79].axis("valence");
    assert!((g - 0.5).abs() < 1e-12, "goal {g}");
    assert!((now - 0.5).abs() < 0.05, "now {now}");
}

#[test]
fn mood_keeps_the_moment_up_after_the_goal_is_released() {
    let (cfg, inv) = bundled();
    let mut plain = cfg.clone();
    plain.linger = 0.0;
    let mut lingered_frames = hold_axis("valence", 1.0, 80);
    lingered_frames.push(blank(80 * TICK_MS));
    lingered_frames[80].release = true;
    let lingered = run(&lingered_frames, &cfg, &inv);
    let plain_run = run(&lingered_frames, &plain, &inv);
    let peak_l = lingered.frames[79].axis("valence");
    let peak_p = plain_run.frames[79].axis("valence");
    let after_l = lingered.frames[80].axis("valence");
    let after_p = plain_run.frames[80].axis("valence");
    assert!(peak_l > 0.9 && peak_p > 0.9, "peaks {peak_l} {peak_p}");
    assert!(after_l > after_p, "linger {after_l} plain {after_p}");
    assert!(after_p > 0.0 && after_l < peak_l);
}

#[test]
fn anger_and_fear_share_valence_and_not_approach() {
    let (cfg, inv) = bundled();
    let anger = run(&hold_pin("anger", 8), &cfg, &inv);
    let fear = run(&hold_pin("fear", 8), &cfg, &inv);
    let a = &anger.frames[7];
    let f = &fear.frames[7];
    assert!(a.axis("valence") < 0.0 && f.axis("valence") < 0.0);
    assert!(
        a.axis("approach") > 0.05,
        "anger approach {}",
        a.axis("approach")
    );
    assert!(
        f.axis("approach") < -0.05,
        "fear approach {}",
        f.axis("approach")
    );
    assert!(a.axis("arousal") > 0.05);
    assert_eq!(f.axis("arousal"), 0.0);
    assert_eq!(a.axis("dominance"), 0.0);
    assert_eq!(a.controls.blend[0].id, "anger");
    assert_eq!(a.lungs.category, "anger");
    assert_eq!(f.lungs.category, "fear");
    assert_eq!(a.track, "prosody");
    assert!(a.nearest.as_ref().unwrap().id == "anger");
    assert!(!a.nearest.as_ref().unwrap().assigned);
}

#[test]
fn sadness_is_low_arousal_and_contentment_is_pleasant() {
    let (cfg, inv) = bundled();
    let sad = run(&hold_pin("sadness", 8), &cfg, &inv)
        .frames
        .pop()
        .unwrap();
    let calm = run(&hold_pin("contentment", 8), &cfg, &inv)
        .frames
        .pop()
        .unwrap();
    let anger = run(&hold_pin("anger", 8), &cfg, &inv).frames.pop().unwrap();
    assert!(sad.axis("valence") < 0.0 && sad.axis("arousal") < 0.0);
    assert!(calm.axis("valence") > 0.0 && calm.axis("arousal") < 0.0);
    assert!(anger.axis("arousal") > sad.axis("arousal"));
}

#[test]
fn an_unplaced_pin_is_readable_and_does_not_move_the_space() {
    let (cfg, inv) = bundled();
    let out = run(&hold_pin("embarrassment", 4), &cfg, &inv);
    let last = &out.frames[3];
    assert_eq!(last.axis("valence"), 0.0);
    assert_eq!(last.axis("arousal"), 0.0);
    assert_eq!(last.axis("approach"), 0.0);
    assert!(!last.pins[0].placed);
    assert_eq!(last.controls.blend[0].id, "embarrassment");
    assert_eq!(last.lungs.category, "embarrassment");
    assert!(last.nearest.is_none());
}

#[test]
fn an_explicit_axis_overrides_the_pin_on_that_axis_only() {
    let (cfg, inv) = bundled();
    let mut frames = hold_pin("anger", 40);
    for frame in &mut frames {
        frame.axes.push(AxisTarget {
            name: "valence".into(),
            value: 0.5,
        });
    }
    let last = run(&frames, &cfg, &inv).frames.pop().unwrap();
    assert!(
        last.axis("valence") > 0.3,
        "valence {}",
        last.axis("valence")
    );
    assert!(
        last.axis("arousal") > 0.5,
        "arousal {}",
        last.axis("arousal")
    );
    assert_eq!(last.lungs.category, "anger");
}

#[test]
fn two_pins_blend_and_a_tie_does_not_name_the_breath() {
    let (cfg, inv) = bundled();
    let mut frames = Vec::new();
    for i in 0..80 {
        let mut frame = with_pin(i * TICK_MS, "anger", 1.0);
        frame.pins.push(PinIntent {
            id: "sadness".into(),
            w: 1.0,
        });
        frames.push(frame);
    }
    let last = run(&frames, &cfg, &inv).frames.pop().unwrap();
    assert!(
        last.axis("valence") < -0.8,
        "valence {}",
        last.axis("valence")
    );
    assert!(
        last.axis("approach") > 0.8,
        "approach {}",
        last.axis("approach")
    );
    assert!(
        last.axis("arousal").abs() < 0.15,
        "arousal {}",
        last.axis("arousal")
    );
    assert_eq!(last.lungs.category, "neutral");
    assert!(last.open.iter().any(|s| s == "category_tie"));
    assert!(last.controls.blend.iter().any(|b| b.id == "anger"));
    assert!(last.controls.blend.iter().any(|b| b.id == "sadness"));
}

#[test]
fn sitting_on_a_pole_without_naming_it_does_not_assign_the_category() {
    let (cfg, inv) = bundled();
    let mut frames = Vec::new();
    for i in 0..40 {
        let mut frame = with_axis(i * TICK_MS, "valence", -1.0);
        frame.axes.push(AxisTarget {
            name: "arousal".into(),
            value: 1.0,
        });
        frame.axes.push(AxisTarget {
            name: "approach".into(),
            value: 1.0,
        });
        frames.push(frame);
    }
    let last = run(&frames, &cfg, &inv).frames.pop().unwrap();
    assert_eq!(last.lungs.category, "neutral");
    assert!(last.controls.blend.is_empty());
    let near = last.nearest.unwrap();
    assert_eq!(near.id, "anger");
    assert!(!near.assigned);
}

#[test]
fn a_learned_axis_is_smoothed_and_stays_out_of_the_category_packets() {
    let (cfg, inv) = bundled();
    let out = run(&hold_axis("d4", 0.6, 80), &cfg, &inv);
    let last = &out.frames[79];
    let first = &out.frames[0];
    assert!(first.axis("d4") > 0.0 && first.axis("d4") < last.axis("d4"));
    assert!(
        (last.axis("d4") - 0.6).abs() < 0.05,
        "d4 {}",
        last.axis("d4")
    );
    assert!(!inv.is_category("d4"));
    assert!(last.controls.blend.is_empty());
    let controls = serde_json::to_value(&last.controls).unwrap();
    assert!(controls.get("d4").is_none());
    assert_eq!(last.lungs.category, "neutral");
}

#[test]
fn the_axis_cap_stops_an_unbounded_stream() {
    let (mut cfg, inv) = bundled();
    cfg.max_axes = 5;
    let mut frames = vec![with_axis(0, "d4", 0.2)];
    frames.push(with_axis(TICK_MS, "d5", 0.2));
    let err = Feeling::run(&frames, &cfg, &inv).unwrap_err();
    assert!(err.to_string().contains("cap"), "{err}");
}

#[test]
fn energy_and_fatigue_are_inertial_and_are_the_lungs_inputs() {
    let (cfg, inv) = bundled();
    let mut frames = Vec::new();
    for i in 0..40 {
        let mut frame = blank(i * TICK_MS);
        frame.energy = Some(1.0);
        frame.fatigue = Some(0.4);
        frames.push(frame);
    }
    let out = run(&frames, &cfg, &inv);
    let e0 = out.frames[0].energy;
    let e1 = out.frames[39].energy;
    assert!(e0 > 0.5 && e0 < 1.0, "energy {e0}");
    assert!(e1 > 0.9, "energy {e1}");
    assert!(out.frames[0].fatigue > 0.0 && out.frames[0].fatigue < 0.4);
    assert_eq!(out.lungs.energy, e0);
    assert_eq!(out.lungs.trajectory.len(), 40);
    assert!((out.lungs.trajectory[1].at_s - 0.08).abs() < 1e-12);
    assert_eq!(out.frames[39].lungs.category, "neutral");
    for frame in &out.frames {
        assert!((0.0..=1.0).contains(&frame.controls.intensity));
        assert!((-1.0..=1.0).contains(&frame.controls.arousal));
        assert!((-1.0..=1.0).contains(&frame.controls.valence));
        assert!((-1.0..=1.0).contains(&frame.controls.dominance));
        assert!((0.0..=1.0).contains(&frame.energy));
        assert!((0.0..=1.0).contains(&frame.fatigue));
    }
}

#[test]
fn distance_and_environment_are_recorded_and_do_not_move_valence() {
    let (cfg, inv) = bundled();
    assert!(cfg.distance_into.per_unit.is_none());
    assert!(cfg.energy_into.per_unit.is_none());
    assert!(cfg.fatigue_into.per_unit.is_none());
    let mut far = hold_pin("contentment", 6);
    far[0].listener_distance_m = Some(3.0);
    far[0].environment = Some("room".into());
    let near = run(&hold_pin("contentment", 6), &cfg, &inv);
    let far = run(&far, &cfg, &inv);
    assert_eq!(
        near.frames[5].axis("valence"),
        far.frames[5].axis("valence")
    );
    assert!(!far.frames[5].outer_applied);
    assert_eq!(far.frames[5].environment.as_deref(), Some("room"));
    assert_eq!(far.frames[5].listener_distance_m, Some(3.0));
}

#[test]
fn a_configured_coefficient_is_applied_and_the_shipped_one_is_not_a_number() {
    let (mut cfg, inv) = bundled();
    cfg.distance_into.per_unit = Some(0.5);
    let mut frames = Vec::new();
    for i in 0..80 {
        let mut frame = blank(i * TICK_MS);
        if i == 0 {
            frame.listener_distance_m = Some(1.0);
        }
        frames.push(frame);
    }
    let last = run(&frames, &cfg, &inv).frames.pop().unwrap();
    assert!(last.outer_applied);
    assert!(
        (last.axis("valence") - 0.5).abs() < 0.08,
        "valence {}",
        last.axis("valence")
    );
    assert_eq!(last.goal_axis("valence"), 0.0);

    let (mut cfg, inv) = bundled();
    cfg.fatigue_into.per_unit = Some(-0.5);
    let mut frames = Vec::new();
    for i in 0..80 {
        let mut frame = blank(i * TICK_MS);
        frame.fatigue = Some(1.0);
        frames.push(frame);
    }
    let last = run(&frames, &cfg, &inv).frames.pop().unwrap();
    assert!(last.inner_applied);
    assert!(
        last.axis("arousal") < -0.3,
        "arousal {}",
        last.axis("arousal")
    );
    assert_eq!(last.goal_axis("arousal"), 0.0);
}

#[test]
fn old_names_follow_the_spec_map_and_splits_stay_unassigned() {
    let (cfg, inv) = bundled();
    let desire = run(&hold_pin("seductive", 1), &cfg, &inv)
        .frames
        .pop()
        .unwrap();
    assert_eq!(desire.controls.blend[0].id, "desire");
    assert_eq!(desire.pins[0].alias_from.as_deref(), Some("seductive"));
    assert_eq!(desire.track, "prosody");
    assert_eq!(desire.axis("valence"), 0.0);

    let laugh = run(&hold_pin("laughter", 1), &cfg, &inv)
        .frames
        .pop()
        .unwrap();
    assert_eq!(laugh.controls.blend[0].id, "amusement");
    assert_eq!(laugh.track, "burst");
    assert_eq!(laugh.lungs.category, "amusement");
    assert_eq!(laugh.axis("valence"), 0.0);

    let tender = run(&hold_pin("tender", 8), &cfg, &inv)
        .frames
        .pop()
        .unwrap();
    assert_eq!(tender.controls.blend[0].id, "adoration");
    assert!(tender.axis("arousal") < 0.0);
    assert_eq!(tender.axis("valence"), 0.0);

    let calm = run(&hold_pin("calm", 1), &cfg, &inv).frames.pop().unwrap();
    assert!(calm.controls.blend.is_empty());
    assert_eq!(calm.lungs.category, "neutral");
    assert!(calm.open.iter().any(|s| s == "baseline:calm"));

    let happy = run(&hold_pin("happy", 1), &cfg, &inv).frames.pop().unwrap();
    assert!(happy.controls.blend.is_empty());
    assert!(happy
        .open
        .iter()
        .any(|s| s == "split:happy->amusement+interest"));

    let sigh = run(&hold_pin("sigh", 1), &cfg, &inv).frames.pop().unwrap();
    assert!(sigh.open.iter().any(|s| s.starts_with("split:sigh->")));
    assert_eq!(sigh.lungs.category, "neutral");

    let old = run(&hold_pin("disgusted", 1), &cfg, &inv)
        .frames
        .pop()
        .unwrap();
    assert_eq!(old.controls.blend[0].id, "contempt");
    assert_eq!(old.axis("valence"), 0.0);
}

#[test]
fn bad_frames_are_refused_and_a_repeat_matches() {
    let (cfg, inv) = bundled();
    let mut off = blank(40);
    off.axes.push(AxisTarget {
        name: "valence".into(),
        value: 0.0,
    });
    assert!(Feeling::run(&[off], &cfg, &inv).is_err());

    let mut wide = with_axis(0, "valence", 1.5);
    wide.t_ms = 0;
    assert!(Feeling::run(&[wide], &cfg, &inv).is_err());

    let mut hot = blank(0);
    hot.energy = Some(1.5);
    assert!(Feeling::run(&[hot], &cfg, &inv).is_err());

    let mut track = blank(0);
    track.track = Some("song".into());
    assert!(Feeling::run(&[track], &cfg, &inv).is_err());

    let mut breath = with_pin(0, "breath", 1.0);
    breath.t_ms = 0;
    assert!(Feeling::run(&[breath], &cfg, &inv).is_err());

    assert!(parse_frames(r#"{"schema":"galion-mne-contract/9","t_ms":0}"#).is_err());

    let frames = hold_pin("anger", 12);
    assert_eq!(run(&frames, &cfg, &inv), run(&frames, &cfg, &inv));
}

#[test]
fn the_inventory_is_the_file_and_the_state_is_not_that_list() {
    let (_, inv) = bundled();
    assert_eq!(inv.labels().len(), 24);
    assert!(inv.is_category("embarrassment"));
    assert!(inv.is_category("anger"));
    assert!(!inv.is_category("neutral"));
    assert!(!inv.is_category("breath"));
    assert!(!inv.is_category("guilt"));
    let (cfg, inv) = bundled();
    let out = run(&hold_axis("not_a_cowen_name", 0.2, 1), &cfg, &inv);
    assert!(out.frames[0].axis("not_a_cowen_name") > 0.0);
}

#[test]
fn the_stand_in_fixture_replays() {
    let (cfg, inv) = bundled();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/intent-standin.json");
    let text = fs::read_to_string(&path).unwrap();
    let frames = parse_frames(&text).unwrap();
    let out = run(&frames, &cfg, &inv);
    assert_eq!(out.frames.len(), 8);
    let peak = out
        .frames
        .iter()
        .map(|f| f.axis("approach"))
        .fold(0.0_f64, f64::max);
    let last = out.frames[7].axis("approach");
    assert!(peak > 0.2, "peak {peak}");
    assert!(last > 0.0 && last < peak, "last {last} peak {peak}");
    assert!(out.frames[4].axis("d4") > 0.0);
    assert_eq!(out.frames[0].environment.as_deref(), Some("quiet"));
    assert!(!out.frames[0].outer_applied);
}

#[test]
fn no_audio_files_are_in_the_feeling_tree() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let banned = ["wav", "mp3", "flac", "ogg", "aiff", "aif"];
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                assert!(
                    !banned.contains(&ext),
                    "audio file in tree: {}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn the_cli_reads_the_stand_in_fixture() {
    let bin = env!("CARGO_BIN_EXE_galion-feeling");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("fixtures/intent-standin.json");
    let config = Config::bundled_path();
    let inventory = Inventory::bundled_path();
    let body = fs::read(&fixture).unwrap();
    let output = Command::new(bin)
        .arg("--config")
        .arg(&config)
        .arg("--inventory")
        .arg(&inventory)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = output;
    use std::io::Write;
    child.stdin.take().unwrap().write_all(&body).unwrap();
    let done = child.wait_with_output().unwrap();
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&done.stdout).unwrap();
    assert_eq!(value["schema"], "galion-feeling/1");
    assert_eq!(value["contract"], CONTRACT);
    assert_eq!(value["frames"].as_array().unwrap().len(), 8);
    assert_eq!(value["lungs"]["trajectory"][0]["category"], "anger");
}
