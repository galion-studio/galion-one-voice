// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

#![cfg(feature = "real")]

use horbm_eval::{Config, Mode, Thresholds};

/// Linux x86_64 round trip. A missing model or a failed download is a skip,
/// not a score. The default `cargo test` build does not compile this test.
#[test]
fn the_real_linux_ear_and_mouth_round_trip() {
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
    let detail = asr.detail.expect("asr detail");
    assert_eq!(detail["prosody"], "on");
    assert!(detail["sensed_frames"].as_u64().unwrap_or(0) > 0);
    let wer = detail["held_out_wer"].as_f64().expect("held-out wer");
    let cap = detail["wer_max"].as_f64().expect("wer max");
    assert!(wer <= cap, "held-out wer {wer} over {cap}");
    assert!(detail["held_out_n"].as_u64().unwrap_or(0) >= 20);
    assert!(detail["script_wer"].as_f64().is_some());
    assert_eq!(detail["decode"], "free+keyword");
    assert_eq!(detail["yeah"], "yeah");
    assert_eq!(detail["sad"], "i am sad");
    let rtf = detail["rtf"].as_f64().expect("rtf");
    assert!(rtf < 1.0, "rtf {rtf}");
    assert!(detail["script_rtf"].as_f64().expect("script rtf") < 1.0);
    assert!(detail["model_bytes"].as_u64().unwrap_or(0) > 0);
    let heard = latency["backchannel_heard"].as_str().unwrap_or("");
    assert_eq!(heard, "yeah");
}
