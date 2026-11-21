// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::process::Command;

use galion_sam_asr::{final_text, Asr, AsrKind, VoskWorkerAsr};

#[test]
fn worker_adapter_reads_partial_then_final() {
    let name = if cfg!(windows) {
        "s03-asr-protocol-double.exe"
    } else {
        "s03-asr-protocol-double"
    };
    let exe = std::env::current_exe().expect("test executable");
    let mut dir = exe.clone();
    let bin = loop {
        if !dir.pop() {
            panic!("protocol double not found above {}", exe.display());
        }
        let candidate = dir.join(name);
        if candidate.is_file() {
            break candidate;
        }
    };
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
