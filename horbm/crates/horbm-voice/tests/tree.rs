// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The default cargo tree links neither Piper, nor Vosk, nor ort.

use std::path::PathBuf;
use std::process::Command;

fn tree(dir: &std::path::Path, package: &str) -> String {
    let target = std::env::temp_dir().join(format!("horbm-ear-tree-{package}"));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", &target)
        .args([
            "tree", "-p", package, "--prefix", "none", "--edges", "normal",
        ])
        .output()
        .unwrap_or_else(|err| panic!("cargo tree {package}: {err}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo tree -p {package} failed\n{stderr}"
    );
    String::from_utf8(output.stdout).expect("tree utf-8")
}

fn assert_light(tree: &str, package: &str) {
    for line in tree.lines() {
        let lower = line.to_ascii_lowercase();
        assert!(
            !lower.contains("piper") && !lower.contains("vosk"),
            "{package} tree contains a heavy engine: {line}"
        );
        let has_ort = lower
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|word| word == "ort");
        assert!(!has_ort, "{package} tree contains ort: {line}");
    }
}

#[test]
fn default_trees_have_no_piper_vosk_or_ort() {
    let voice = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let horbm = voice.parent().unwrap().parent().unwrap();
    let root = horbm.parent().unwrap();
    let ear = tree(horbm, "horbm-voice");
    assert_light(&ear, "horbm-voice");
    assert!(
        ear.lines().any(|line| line.starts_with("horbm-voice")),
        "{ear}"
    );
    let server = tree(&root.join("voice-stack"), "samantha-local-server");
    assert_light(&server, "samantha-local-server");
    println!("ear gate: cargo_tree piper=absent vosk=absent ort=absent result=pass");
}
