// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Worker protocol, without libvosk and without a model.

use galion_asr_vosk::{parse_event, VoskClient, WorkerEvent};

const PLACEHOLDERS: &[&str] = &["hello", "hello how are you", "can you hear me"];

#[test]
fn protocol_double_reports_samples_not_a_placeholder() {
    let bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_galion-asr-protocol-double"));
    let mut client = VoskClient::from_bin(&bin, &[]).expect("double");
    let pcm = vec![1i16, -1, 2, -2];
    let first = client.recognize(&pcm, None).unwrap();
    assert_eq!(first.text, "n=4 grammar=0");
    assert_eq!(first.partials, vec!["frame".to_string()]);
    assert_eq!(first.words.len(), 1);
    let grammar = vec!["thanks".to_string(), "[unk]".to_string()];
    let second = client.recognize(&pcm, Some(&grammar)).unwrap();
    assert_eq!(second.text, "n=4 grammar=2");
    assert_eq!(
        client.recognize(&pcm, None).unwrap().text,
        "n=4 grammar=0",
        "same process"
    );
    for phrase in PLACEHOLDERS {
        assert_ne!(first.text, *phrase);
        assert_ne!(second.text, *phrase);
    }
}

#[test]
fn missing_model_is_an_error_line_not_a_transcript() {
    let bin = env!("CARGO_BIN_EXE_galion-asr-vosk");
    let out = std::process::Command::new(bin)
        .arg("--model")
        .arg("/no/such/galion-vosk-model")
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut events = Vec::new();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        events.push(parse_event(line).unwrap_or_else(|e| panic!("{e}: {line}")));
    }
    assert!(
        events.iter().any(|e| matches!(e, WorkerEvent::Error { error } if error.contains("No transcript was invented"))),
        "{stdout}"
    );
    assert!(events.iter().all(|e| !matches!(
        e,
        WorkerEvent::Final { .. } | WorkerEvent::Partial { .. } | WorkerEvent::Ready { .. }
    )));
    for phrase in PLACEHOLDERS {
        assert!(!stdout.contains(phrase), "{stdout}");
    }
}
