// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! The library source does not carry a stock comfort line, a model client,
//! or a foreign Windows drive.

use std::fs;
use std::path::Path;

#[test]
fn sources_have_no_scripted_comfort_line() {
    let lines = [
        "I hear you",
        "That sounds so hard",
        "I'm here for you no matter what",
        "I feel that in my soul",
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect(&root, &mut files);
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        let lower = text.to_ascii_lowercase();
        for line in lines {
            assert!(
                !lower.contains(&line.to_ascii_lowercase()),
                "{} contains a scripted comfort line",
                path.display()
            );
        }
    }
}

#[test]
fn the_manifest_does_not_pull_a_model_or_an_http_client() {
    let toml = include_str!("../Cargo.toml");
    for banned in [
        "reqwest", "ureq", "hyper", "openai", "candle", "ort", "tract", "burn", "llama", "ggml",
    ] {
        assert!(!toml.contains(banned), "Cargo.toml names {banned}");
    }
}

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}
