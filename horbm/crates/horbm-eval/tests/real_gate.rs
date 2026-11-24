// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

#![cfg(feature = "real")]

use horbm_eval::{Config, Mode, Thresholds};

/// Linux x86_64 round trip. A missing model or a failed download is a skip,
/// not a score. The default `cargo test` build does not compile this test.
///
/// `CARGO_BIN_EXE_galion-asr-vosk` is the worker Cargo built for this test.
/// A clean target directory still has the binary, because this package builds
/// it when the `real` feature is on.
#[test]
fn the_real_linux_ear_and_mouth_round_trip() {
    let vosk_bin = env!("CARGO_BIN_EXE_galion-asr-vosk");
    assert!(
        std::path::Path::new(vosk_bin).is_file(),
        "galion-asr-vosk was not built at {vosk_bin}. No transcript was invented."
    );
    std::env::set_var("GALION_VOSK_BIN", vosk_bin);
    let report = horbm_eval::run_with(Config {
        research_dir: None,
        thresholds: Thresholds::embedded(),
        mode: Mode::Real,
        prosody: true,
    });
    if report.skipped {
        eprintln!(
            "ignored: {}",
            report.error.as_deref().unwrap_or("real ear skipped")
        );
        return;
    }
    eprintln!("{}", report.to_json());
    assert!(report.pass, "{}", report.to_json());
    assert_eq!(report.mode, "real");
    assert_eq!(report.research_dir, "unset");
    assert!(!report.skipped);
    let latency = report.scores.latency.detail.expect("latency detail");
    assert_eq!(latency["clock"], "wall");
    let asr = report.scores.asr.expect("asr score");
    assert!(asr.pass, "{:?}", asr.failures);
    let prosody = asr.detail.expect("asr detail");
    assert_eq!(prosody["prosody"], "on");
    assert!(prosody["sensed_frames"].as_u64().unwrap_or(0) > 0);
}
