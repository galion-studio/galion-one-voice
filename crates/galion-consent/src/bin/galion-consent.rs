// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! `galion-consent fields | measure --persona P | check --persona P`.
//!
//! Same commands, output lines and exit codes as the retired
//! `scripts/clone/consent.py` (tools/legacy/consent.py.bak). Rust only.
//! `check` is the clone-time gate; it is not called per turn.

use galion_consent::{evaluate, inside_tree, persona_of, sha256, wav_info, Env, DURATION_MAX, DURATION_MIN, REQUIRED};
use std::process::ExitCode;

fn measure(env: &Env, persona: &str) -> u8 {
    let Some(name) = persona_of(persona) else {
        eprintln!("measure: persona must be samantha or jarvis");
        return 2;
    };
    let Some(store) = env.store.as_ref() else {
        eprintln!("measure: GALION_CLIP_STORE is unset. No default path.");
        return 2;
    };
    if inside_tree(store, &env.repo) {
        eprintln!("measure: the store is inside the git tree. Move it outside.");
        return 2;
    }
    let wav = store.join(name).join("reference.wav");
    if !wav.is_file() {
        eprintln!("measure: reference.wav is not in the store. This script does not fetch one.");
        return 2;
    }
    let Some((seconds, channels, width)) = wav_info(&wav) else {
        eprintln!("measure: need a 16-bit PCM wav. This tree does not convert it.");
        return 1;
    };
    if channels != 1 || width != 2 {
        eprintln!("measure: need mono 16-bit PCM. This tree does not remix it.");
        return 1;
    }
    println!("duration_s: {seconds:.3}");
    match sha256::file_hex(&wav) {
        Ok(hex) => println!("sha256: {hex}"),
        Err(_) => return 1,
    }
    if !(DURATION_MIN..=DURATION_MAX).contains(&seconds) {
        eprintln!("measure: duration is outside 8–12 seconds. About 10 seconds is the clip. This tree does not trim.");
        return 1;
    }
    0
}

fn run(argv: &[String]) -> u8 {
    if argv.is_empty() || argv[0] == "-h" || argv[0] == "--help" {
        eprintln!("usage: galion-consent fields | galion-consent measure --persona samantha|jarvis | galion-consent check --persona samantha|jarvis");
        return 2;
    }
    let cmd = argv[0].as_str();
    if cmd == "fields" {
        for key in REQUIRED {
            println!("{key}");
        }
        return 0;
    }
    if cmd != "measure" && cmd != "check" {
        eprintln!("consent: unknown command");
        return 2;
    }
    let mut persona = String::new();
    let mut args = argv[1..].iter();
    while let Some(item) = args.next() {
        match (item.as_str(), args.clone().next()) {
            ("--persona", Some(p)) => {
                persona = p.clone();
                args.next();
            }
            _ => {
                eprintln!("consent: unknown argument");
                return 2;
            }
        }
    }
    let env = Env::process();
    if cmd == "measure" {
        return measure(&env, &persona);
    }
    let gate = evaluate(&persona, &env, true);
    let lowered = persona.trim().to_lowercase();
    let name = persona_of(&persona).map(str::to_string).unwrap_or(if lowered.is_empty() { "persona".into() } else { lowered });
    if gate.ok {
        let flag = if gate.public { "yes" } else { "no" };
        println!("galion-clip-ready: {name} {:.3} public={flag}", gate.seconds.unwrap_or(0.0));
        return 0;
    }
    println!("galion-clip-absent: {name} {}", gate.reason);
    1
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(run(&argv))
}
