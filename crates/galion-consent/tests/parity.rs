// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Parity with the retired Python gate (tools/legacy/consent.py.bak).
//!
//! Python runs here and only here (tests). Each fixture is evaluated by the
//! old `consent.py check` and by the in-process Rust gate; the printed
//! `galion-clip-ready|absent` line must match. Two cases differ on purpose
//! and are pinned below: Rust refuses `expired` and `celebrity`, which the
//! old gate let through or called `persona`. Skips when python3 is absent.

mod common;

use common::{wav_bytes, Tree};
use galion_consent::{evaluate, persona_of, Env, Gate};
use std::fs;
use std::path::Path;
use std::process::Command;

fn python() -> bool {
    Command::new("python3").arg("-c").arg("import wave").output().map(|o| o.status.success()).unwrap_or(false)
}

fn legacy() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/legacy/consent.py.bak")
}

fn install(t: &Tree) {
    fs::copy(legacy(), t.repo.join("scripts/clone/consent.py")).unwrap();
}

fn py(t: &Tree, persona: &str, store: Option<&Path>) -> String {
    let mut cmd = Command::new("python3");
    cmd.arg(t.repo.join("scripts/clone/consent.py")).args(["check", "--persona", persona]);
    for (k, _) in std::env::vars() {
        if k.starts_with("GALION_") {
            cmd.env_remove(k);
        }
    }
    if let Some(s) = store {
        cmd.env("GALION_CLIP_STORE", s);
    }
    let out = cmd.output().expect("python3");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn line(persona: &str, g: &Gate) -> String {
    let name = persona_of(persona).map(str::to_string).unwrap_or_else(|| {
        let p = persona.trim().to_lowercase();
        if p.is_empty() { "persona".into() } else { p }
    });
    match (g.ok, g.seconds) {
        (true, Some(s)) => format!("galion-clip-ready: {name} {s:.3} public={}", if g.public { "yes" } else { "no" }),
        _ => format!("galion-clip-absent: {name} {}", g.reason),
    }
}

fn rust(t: &Tree, persona: &str, store: Option<&Path>) -> String {
    let mut env = Env::at(&t.repo);
    env.store = store.map(Path::to_path_buf);
    env.today = Some(common::TODAY);
    line(persona, &evaluate(persona, &env, true))
}

struct Case {
    name: &'static str,
    persona: &'static str,
    setup: fn(&Tree),
    want: &'static str,
}

fn edit(t: &Tree, key: &str, value: &str) {
    t.set("samantha", key, value);
}

fn cases() -> Vec<Case> {
    vec![
        Case { name: "valid samantha", persona: "samantha", setup: |t| t.place("samantha"), want: "galion-clip-ready: samantha 10.000 public=no" },
        Case { name: "valid jarvis caps", persona: " Jarvis ", setup: |t| t.place("jarvis"), want: "galion-clip-ready: jarvis 10.000 public=no" },
        Case { name: "public yes", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "public_demo", "YES"); }, want: "galion-clip-ready: samantha 10.000 public=yes" },
        Case { name: "9.5 s", persona: "samantha", setup: |t| t.place_with("samantha", &wav_bytes(22_050, 1, 2, 9.5), |_| {}), want: "galion-clip-ready: samantha 9.500 public=no" },
        Case { name: "declared within tolerance", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "duration_s", "10.2"); }, want: "galion-clip-ready: samantha 10.000 public=no" },
        Case { name: "equals form and spaced keys", persona: "samantha", setup: |t| {
            t.place("samantha");
            let p = t.folder("samantha").join("consent.txt");
            let text = fs::read_to_string(&p).unwrap().replace("public_demo: no", "Public Demo = no").replace("operator:", "Operator :");
            fs::write(p, text).unwrap();
        }, want: "galion-clip-ready: samantha 10.000 public=no" },
        Case { name: "unknown persona", persona: "bob", setup: |t| t.place("samantha"), want: "galion-clip-absent: bob persona" },
        Case { name: "empty persona", persona: "", setup: |_| {}, want: "galion-clip-absent: persona persona" },
        Case { name: "no wav", persona: "samantha", setup: |_| {}, want: "galion-clip-absent: samantha wav" },
        Case { name: "not a wav", persona: "samantha", setup: |t| t.place_with("samantha", b"not a riff file at all", |_| {}), want: "galion-clip-absent: samantha wav" },
        Case { name: "stereo", persona: "samantha", setup: |t| t.place_with("samantha", &wav_bytes(16_000, 2, 2, 10.0), |_| {}), want: "galion-clip-absent: samantha mono" },
        Case { name: "8-bit", persona: "samantha", setup: |t| t.place_with("samantha", &wav_bytes(16_000, 1, 1, 10.0), |_| {}), want: "galion-clip-absent: samantha mono" },
        Case { name: "too short", persona: "samantha", setup: |t| t.place_with("samantha", &wav_bytes(16_000, 1, 2, 6.0), |_| {}), want: "galion-clip-absent: samantha duration" },
        Case { name: "too long", persona: "samantha", setup: |t| t.place_with("samantha", &wav_bytes(16_000, 1, 2, 17.0), |_| {}), want: "galion-clip-absent: samantha duration" },
        Case { name: "declared off", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "duration_s", "10.5"); }, want: "galion-clip-absent: samantha duration" },
        Case { name: "duration not a float", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "duration_s", "ten"); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "no consent", persona: "samantha", setup: |t| { t.place("samantha"); fs::remove_file(t.folder("samantha").join("consent.txt")).unwrap(); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "placeholder", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "counsel", "n/a"); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "bracket", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "grantor", "[grantor]"); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "too long value", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "work", &"x".repeat(401)); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "persona mismatch", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "persona", "jarvis"); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "scope", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "scope", "fine-tune"); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "public maybe", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "public_demo", "maybe"); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "date not CET", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "date", "2026-11-04 09:00 UTC"); }, want: "galion-clip-absent: samantha consent" },
        Case { name: "date Zagreb", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "date", "2026-11-04 Europe/Zagreb"); }, want: "galion-clip-ready: samantha 10.000 public=no" },
        Case { name: "tampered wav", persona: "samantha", setup: |t| {
            t.place("samantha");
            let p = t.folder("samantha").join("reference.wav");
            let mut b = fs::read(&p).unwrap();
            let n = b.len() - 3;
            b[n] ^= 1;
            fs::write(p, b).unwrap();
        }, want: "galion-clip-absent: samantha sha" },
        Case { name: "sidecar missing", persona: "samantha", setup: |t| { t.place("samantha"); fs::remove_file(t.folder("samantha").join("reference.wav.sha256")).unwrap(); }, want: "galion-clip-absent: samantha sha" },
        Case { name: "declared sha short", persona: "samantha", setup: |t| { t.place("samantha"); edit(t, "sha256", "abc123"); }, want: "galion-clip-absent: samantha sha" },
        Case { name: "declared sha upper", persona: "samantha", setup: |t| {
            t.place("samantha");
            let sha = galion_consent::sha256::file_hex(&t.folder("samantha").join("reference.wav")).unwrap();
            edit(t, "sha256", &sha.to_uppercase());
        }, want: "galion-clip-ready: samantha 10.000 public=no" },
        Case { name: "lock false", persona: "samantha", setup: |t| { t.place("samantha"); common::write(&t.repo.join("LOCKS.toml"), "[formats.exceptions.v2s14_chatterbox]\nreference_clips = false\n"); }, want: "galion-clip-absent: samantha lock" },
        Case { name: "lock other table", persona: "samantha", setup: |t| { t.place("samantha"); common::write(&t.repo.join("LOCKS.toml"), "[formats]\nreference_clips = true\n"); }, want: "galion-clip-absent: samantha lock" },
        Case { name: "lock missing", persona: "samantha", setup: |t| { t.place("samantha"); fs::remove_file(t.repo.join("LOCKS.toml")).unwrap(); }, want: "galion-clip-absent: samantha lock" },
        Case { name: "worker missing", persona: "samantha", setup: |t| { t.place("samantha"); fs::remove_file(t.repo.join("scripts/clone/chatterbox_worker.py")).unwrap(); }, want: "galion-clip-absent: samantha worker" },
        Case { name: "weight size", persona: "samantha", setup: |t| { t.place("samantha"); fs::File::create(t.repo.join("weights/clone/shared/ve.safetensors")).unwrap().set_len(1).unwrap(); }, want: "galion-clip-absent: samantha weights" },
        Case { name: "weight missing", persona: "samantha", setup: |t| { t.place("samantha"); fs::remove_file(t.repo.join("weights/clone/nano/t3_nano_v1.safetensors")).unwrap(); }, want: "galion-clip-absent: samantha weights" },
    ]
}

#[test]
fn rust_gate_matches_python_gate() {
    let run_py = python();
    if !run_py {
        eprintln!("parity: python3 absent; checking Rust against the recorded Python lines only");
    }
    for c in cases() {
        let t = Tree::new("parity");
        install(&t);
        (c.setup)(&t);
        let r = rust(&t, c.persona, Some(&t.store));
        assert_eq!(r, c.want, "rust, case {}", c.name);
        if run_py {
            assert_eq!(py(&t, c.persona, Some(&t.store)), c.want, "python, case {}", c.name);
        }
    }
}

#[test]
fn store_unset_and_store_inside_repo_match() {
    let t = Tree::new("store");
    install(&t);
    assert_eq!(rust(&t, "samantha", None), "galion-clip-absent: samantha store");
    let inside = t.repo.join("clips");
    fs::create_dir_all(&inside).unwrap();
    assert_eq!(rust(&t, "jarvis", Some(&inside)), "galion-clip-absent: jarvis inside_repo");
    if python() {
        assert_eq!(py(&t, "samantha", None), "galion-clip-absent: samantha store");
        assert_eq!(py(&t, "jarvis", Some(&inside)), "galion-clip-absent: jarvis inside_repo");
    }
}

/// The two places the Rust gate is deliberately stricter than consent.py.
#[test]
fn documented_divergences() {
    let t = Tree::new("diverge");
    install(&t);
    t.place("samantha");
    t.set("samantha", "expiry", "2025-01-01");
    assert_eq!(rust(&t, "samantha", Some(&t.store)), "galion-clip-absent: samantha expired");
    assert_eq!(rust(&t, "armstrong", Some(&t.store)), "galion-clip-absent: armstrong celebrity");
    if python() {
        // consent.py never read the expiry date and had no celebrity rule.
        assert_eq!(py(&t, "samantha", Some(&t.store)), "galion-clip-ready: samantha 10.000 public=no");
        assert_eq!(py(&t, "armstrong", Some(&t.store)), "galion-clip-absent: armstrong persona");
    }
}

#[test]
fn cli_fields_match() {
    let bin = env!("CARGO_BIN_EXE_galion-consent");
    let r = Command::new(bin).arg("fields").output().unwrap();
    assert!(r.status.success());
    let want: String = galion_consent::REQUIRED.iter().map(|k| format!("{k}\n")).collect();
    assert_eq!(String::from_utf8_lossy(&r.stdout), want);
    if python() {
        let p = Command::new("python3").arg(legacy()).arg("fields").output().unwrap();
        assert_eq!(p.stdout, r.stdout);
    }
}

#[test]
fn cli_measure_matches() {
    let t = Tree::new("measure");
    t.place("jarvis");
    let bin = env!("CARGO_BIN_EXE_galion-consent");
    let r = Command::new(bin).args(["measure", "--persona", "jarvis"]).env("GALION_CLIP_STORE", &t.store).output().unwrap();
    assert_eq!(r.status.code(), Some(0));
    if python() {
        let p = Command::new("python3").arg(legacy()).args(["measure", "--persona", "jarvis"]).env("GALION_CLIP_STORE", &t.store).output().unwrap();
        assert_eq!(p.status.code(), Some(0));
        assert_eq!(p.stdout, r.stdout);
    }
}
