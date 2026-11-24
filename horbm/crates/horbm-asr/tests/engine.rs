// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The real engine, opened without weights. Compiled with `--features vosk`.

#![cfg(feature = "vosk")]

use horbm_asr::{open_vosk, VoskConfig};

#[test]
fn a_missing_model_is_an_error_not_a_line() {
    let missing = std::env::temp_dir().join("horbm-asr-absent-model");
    let err = open_vosk(VoskConfig {
        model_dir: missing,
        lib_path: None,
        threads: 1,
    })
    .err()
    .expect("missing model");
    let msg = err.to_string();
    assert!(msg.contains("No transcript was invented"), "{msg}");
    assert!(!msg.to_ascii_lowercase().contains("kettle"), "{msg}");
    assert!(!msg.contains("://"), "{msg}");
}
