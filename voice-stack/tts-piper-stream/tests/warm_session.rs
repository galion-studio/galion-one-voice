// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One fake Piper process, two speaks, clause PCM matches the stand-in,
//! and clause 2 is submitted before clause 1's callback.

use galion_tts_piper::{
    prepare_text, split_clauses, stand_in_pcm, IoMode, Pad, PiperTts, PiperVoice, StreamEvent, Tts,
};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> Tmp {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("galion-tts-piper-{n}"));
    std::fs::create_dir_all(&dir).unwrap();
    Tmp(dir)
}

fn voice(dir: &Path, io: IoMode) -> PiperVoice {
    let model = dir.join("voice.onnx");
    let config = dir.join("voice.onnx.json");
    std::fs::write(&model, b"not-a-model").unwrap();
    std::fs::write(&config, br#"{"audio":{"sample_rate":22050}}"#).unwrap();
    let runtime = PathBuf::from(env!("CARGO_BIN_EXE_fake-piper"));
    PiperVoice::from_paths(runtime, model, config)
        .unwrap()
        .with_io(io)
}

fn modes() -> Vec<IoMode> {
    if cfg!(target_os = "linux") {
        vec![IoMode::Poll, IoMode::Thread]
    } else {
        vec![IoMode::Thread]
    }
}

fn pos(events: &[StreamEvent], submit: bool, sentence: u32) -> usize {
    events
        .iter()
        .position(|e| match e {
            StreamEvent::Submit { sentence: s, .. } if submit && *s == sentence => true,
            StreamEvent::Callback { sentence: s, .. } if !submit && *s == sentence => true,
            _ => false,
        })
        .expect("event")
}

#[test]
fn warm_process_streams_clauses_before_the_callback() {
    for io in modes() {
        let dir = scratch();
        let mut tts = PiperTts::spawn(voice(&dir.0, io)).unwrap();
        let text = "I am here. I can hear you.";
        let clauses = split_clauses(&prepare_text(text));
        assert_eq!(clauses.len(), 2, "{io:?}");
        let mut got = Vec::new();
        let report = tts
            .speak(text, Pad::RESTING, &mut |c| got.push(c.samples.to_vec()))
            .unwrap();
        assert_eq!(got.len(), 2, "{io:?}");
        assert_eq!(got[0], stand_in_pcm(&clauses[0]), "{io:?} clause 0");
        assert_eq!(got[1], stand_in_pcm(&clauses[1]), "{io:?} clause 1");
        assert!(
            pos(&report.events, true, 1) < pos(&report.events, false, 0),
            "{io:?} {:?}",
            report.events
        );
        assert!(report.cold, "{io:?}");
        assert_eq!(report.pid, tts.pid(), "{io:?}");

        let mut second = Vec::new();
        let warm = tts
            .speak("Second line.", Pad::RESTING, &mut |c| {
                second.extend_from_slice(c.samples)
            })
            .unwrap();
        assert!(!warm.cold, "{io:?}");
        assert_eq!(warm.pid, report.pid, "{io:?} pid changed");
        assert_ne!(
            second[0], got[0][0],
            "{io:?} second speak replayed the first PCM"
        );
        assert!(tts.pid().is_some());
    }
}

#[test]
fn quotes_roundtrip_and_tags_are_not_sent() {
    let dir = scratch();
    let mut tts = PiperTts::spawn(voice(&dir.0, IoMode::Auto)).unwrap();
    let text = "say \"hi\" \\ now";
    let mut got = Vec::new();
    tts.speak(text, Pad::RESTING, &mut |c| {
        got.extend_from_slice(c.samples)
    })
    .unwrap();
    assert_eq!(got, stand_in_pcm(text));

    let tagged = "Hello [laugh] there.";
    let expect = split_clauses(&prepare_text(tagged));
    let mut got = Vec::new();
    tts.speak(tagged, Pad::RESTING, &mut |c| {
        got.extend_from_slice(c.samples)
    })
    .unwrap();
    assert_eq!(got, stand_in_pcm(&expect[0]));
    assert!(tts.speak("[laugh]", Pad::RESTING, &mut |_| {}).is_err());
}
