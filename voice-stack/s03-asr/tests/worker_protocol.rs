// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::process::Command;

use galion_s03_asr::{final_text, Asr, AsrKind, VoskWorkerAsr};

#[test]
fn worker_adapter_reads_partial_then_final() {
    let mut bin = std::env::current_exe().expect("test executable");
    bin.pop();
    bin.pop();
    let name = if cfg!(windows) {
        "s03-asr-protocol-double.exe"
    } else {
        "s03-asr-protocol-double"
    };
    bin.push(name);
    let mut engine = VoskWorkerAsr::from_command(Command::new(&bin)).unwrap();
    assert_eq!(engine.name(), "vosk-worker:double");
    assert!(engine.push_pcm(&[0, 1, -1, 2]).unwrap().is_empty());
    let events = engine.finish().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, AsrKind::Partial);
    assert_eq!(events[0].text, "hello there");
    assert_eq!(events[1].kind, AsrKind::Final);
    assert_eq!(events[1].text, "hello there");
    assert_eq!(events[1].words.len(), 2);
    assert_eq!(events[1].words[0].word, "hello");
    assert!((events[1].words[1].conf - 0.75).abs() < 1e-6);
    assert_eq!(final_text(&events), "hello there");
    assert!(engine.finish().unwrap().is_empty());
}
