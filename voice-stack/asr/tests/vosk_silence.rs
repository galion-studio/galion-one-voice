// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use galion_sam_asr::{final_text, run_stream, VoskAsr, HEARTBEAT_SAMPLES};

#[test]
#[ignore = "needs libvosk and vosk-model-small-en-us-0.15"]
fn one_second_of_silence_is_not_a_sentence() {
    let mut engine = VoskAsr::open_from_env().expect("vosk model");
    let silence = vec![0i16; 16_000];
    let report = run_stream(&mut engine, &silence, HEARTBEAT_SAMPLES).unwrap();
    let text = final_text(&report.events);
    assert!(
        text.is_empty(),
        "silence produced {text:?}. No sentence should be invented."
    );
}
