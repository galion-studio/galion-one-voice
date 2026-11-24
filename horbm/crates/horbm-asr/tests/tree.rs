// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Default build links no recognizer. The vosk feature links galion-sam-asr only.

use std::path::PathBuf;
use std::process::Command;

fn horbm_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates")
        .parent()
        .expect("horbm")
        .to_path_buf()
}

fn tree(extra: &[&str]) -> String {
    let target = std::env::temp_dir().join(format!("horbm-asr-tree-{}", extra.len()));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut args = vec![
        "tree",
        "-p",
        "horbm-asr",
        "--prefix",
        "none",
        "--edges",
        "normal",
    ];
    args.extend_from_slice(extra);
    let output = Command::new(cargo)
        .current_dir(horbm_dir())
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_NET_OFFLINE", "true")
        .args(&args)
        .output()
        .unwrap_or_else(|err| panic!("cargo tree: {err}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "cargo tree failed\n{stderr}");
    String::from_utf8(output.stdout).expect("tree utf-8")
}

fn assert_no_forbidden_runtime(tree: &str) {
    for line in tree.lines() {
        let lower = line.to_ascii_lowercase();
        for needle in [
            "whisper", "sherpa", "ggml", "candle", "llama", "tract", "burn",
        ] {
            assert!(!lower.contains(needle), "{line}");
        }
        let has_ort = lower
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|word| word == "ort");
        assert!(!has_ort, "{line}");
    }
}

#[test]
fn the_default_tree_is_empty_and_vosk_links_only_the_existing_engine() {
    let plain = tree(&[]);
    assert!(
        plain.lines().any(|line| line.starts_with("horbm-asr")),
        "{plain}"
    );
    assert!(!plain.to_ascii_lowercase().contains("vosk"), "{plain}");
    assert!(!plain.contains("galion-sam-asr"), "{plain}");
    assert_no_forbidden_runtime(&plain);

    let featured = tree(&["--features", "vosk"]);
    assert!(
        featured.lines().any(|line| line.contains("galion-sam-asr")),
        "{featured}"
    );
    assert_no_forbidden_runtime(&featured);
}
