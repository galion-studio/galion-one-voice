// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Optional stages. In-crate modules are cfg-gated. Separate crates are
//! probed when their source file exists.

use crate::probe::{self, ear_eot, ear_lib, ear_listen, piper_mouth};
use crate::report::Report;
use crate::Sense;

pub fn fill(report: &mut Report, sense: Sense, script: &str) -> Result<(), String> {
    push_timed(report, "dialogue", dialogue_ns());
    push_timed(report, "person_memory", memory_ns(script));
    push_timed(report, "mood_into_voice", mood_ns());
    prosody(report, sense)?;
    acoustic(
        report,
        sense,
        "end_of_turn",
        ear_eot().is_file(),
        eot_manifest(),
    )?;
    acoustic(
        report,
        sense,
        "listener_feeling",
        ear_listen().is_file(),
        listen_manifest(),
    )?;
    piper(report)?;
    Ok(())
}

fn push_timed(report: &mut Report, name: &str, ns: Option<u64>) {
    match ns {
        Some(ns) => {
            report.push(name, "real");
            report.push(format!("{name}_ns"), ns.to_string());
        }
        None => report.push(name, "stub"),
    }
}

fn prosody(report: &mut Report, sense: Sense) -> Result<(), String> {
    if !ear_lib().is_file() {
        let value = if sense == Sense::On { "stub" } else { "off" };
        report.push("prosody", value);
        return Ok(());
    }
    let cargo = ear_cargo();
    let probe = probe::run_named("prosody", sense, &cargo)?;
    apply(report, &probe, &["prosody", "prosody_ns"])?;
    if sense == Sense::Off && report.get("prosody_ns").is_some() {
        return Err("prosody was timed while sensing is off".into());
    }
    if sense == Sense::Off && report.get("prosody") != Some("off") {
        return Err("prosody off switch did not report off".into());
    }
    Ok(())
}

fn acoustic(
    report: &mut Report,
    sense: Sense,
    name: &str,
    present: bool,
    cargo: String,
) -> Result<(), String> {
    if !present {
        report.push(name, "stub");
        return Ok(());
    }
    if sense == Sense::Off {
        report.push(name, "off");
        return Ok(());
    }
    let ns_key = format!("{name}_ns");
    let probe = probe::run_named(name_to_file(name), sense, &cargo)?;
    apply(report, &probe, &[name, ns_key.as_str()])?;
    Ok(())
}

fn name_to_file(name: &str) -> &str {
    match name {
        "end_of_turn" => "eot",
        "listener_feeling" => "listen",
        other => other,
    }
}

fn piper(report: &mut Report) -> Result<(), String> {
    if !piper_mouth().is_file() {
        report.push("piper_duplex", "stub");
        return Ok(());
    }
    let probe = probe::run_named("piper", Sense::On, &piper_cargo())?;
    apply(report, &probe, &["piper_duplex", "piper_duplex_ns"])?;
    Ok(())
}

fn apply(report: &mut Report, probe: &probe::Probe, allow: &[&str]) -> Result<(), String> {
    let mut seen = false;
    for (key, value) in &probe.lines {
        if allow.iter().any(|item| item == key) {
            report.push(key, value);
            seen = true;
        }
    }
    if !seen {
        return Err("probe result was not an allowed field".into());
    }
    Ok(())
}

fn cargo_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn ear_cargo() -> String {
    let path = cargo_path(
        ear_lib()
            .parent()
            .and_then(|dir| dir.parent())
            .unwrap_or(std::path::Path::new(".")),
    );
    format!(
        r#"# generated at test time
[package]
name = "horbm-e2e-probe"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
horbm-ear = {{ path = "{path}" }}
"#
    )
}

fn eot_manifest() -> String {
    ear_cargo()
}

fn listen_manifest() -> String {
    ear_cargo()
}

fn piper_cargo() -> String {
    let path = cargo_path(
        piper_mouth()
            .parent()
            .and_then(|dir| dir.parent())
            .unwrap_or(std::path::Path::new(".")),
    );
    format!(
        r#"# generated at test time
[package]
name = "horbm-e2e-probe"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
horbm-duplex = {{ path = "{path}", features = ["piper"] }}
"#
    )
}

#[cfg(has_dialogue)]
fn dialogue_ns() -> Option<u64> {
    let started = std::time::Instant::now();
    let closed = horbm::flag_granted(Some("deny"));
    let unset = horbm::flag_granted(None);
    let cap = horbm::RING_CAP;
    std::hint::black_box((closed, unset, cap));
    Some(elapsed_ns(started))
}

#[cfg(not(has_dialogue))]
fn dialogue_ns() -> Option<u64> {
    None
}

#[cfg(has_memory)]
fn memory_ns(script: &str) -> Option<u64> {
    let path = std::env::temp_dir().join(format!("horbm-e2e-memory-{}", std::process::id()));
    let started = std::time::Instant::now();
    let store = horbm::EpisodicStore::open(&path, false);
    let about = horbm::about_the_person(script);
    let writes = store.writes();
    std::hint::black_box((about, writes));
    assert_eq!(writes, 0, "person memory wrote while remember is deny");
    Some(elapsed_ns(started))
}

#[cfg(not(has_memory))]
fn memory_ns(_script: &str) -> Option<u64> {
    None
}

#[cfg(has_moment_voice)]
fn mood_ns() -> Option<u64> {
    let started = std::time::Instant::now();
    let baseline = horbm_voice::samantha_baseline();
    assert!(
        horbm_voice::is_samantha(&baseline),
        "moment voice baseline is not samantha"
    );
    let mut voice = horbm_voice::MomentVoice::new(baseline);
    let moment = galion_emotion_core::MomentState::at(baseline.pos);
    let controls = voice.tick(moment, galion_emotion_core::Breath::rest());
    assert!(
        controls.in_bounds(),
        "moment voice controls left their bounds"
    );
    std::hint::black_box(controls);
    Some(elapsed_ns(started))
}

#[cfg(not(has_moment_voice))]
fn mood_ns() -> Option<u64> {
    None
}

#[cfg(any(has_dialogue, has_memory, has_moment_voice))]
fn elapsed_ns(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}
