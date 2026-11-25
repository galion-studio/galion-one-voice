// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Piper glue. The protocol tests use a fake runtime and no model.
//! The stock-voice measurement downloads `en_US-ljspeech-medium` only when
//! `HORBM_PIPER_FETCH=1`, into the cache directory, and never into the repo.
//! That pack is the open MIT Piper voice (public-domain LJSpeech). It is not
//! a clone. Samantha and Aria are refused. No audio file is written.

#![cfg(all(feature = "piper", unix))]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use galion_emotion_core::{Breath, MomentState};
use horbm::Pad;
use horbm_duplex::{
    discover_piper, measure_piper, piper_cache_dir, PiperMouth, PiperPaths, STOCK_VOICE,
};
use horbm_voice::{
    aria_baseline, instant, samantha_baseline, MomentVoice, Mouth, MouthPull, SpokenLine,
    FRAME_SAMPLES, SAMPLE_RATE,
};

const ONNX_SHA: &str = "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22";
const JSON_SHA: &str = "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860";
const RUNTIME_SHA: &str = "a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992";
const VOICE_URL: &str =
    "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium";
const RUNTIME_URL: &str =
    "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz";

#[test]
fn a_persona_pack_is_refused_before_spawn() {
    match PiperMouth::open(&PiperPaths {
        runtime: PathBuf::from("piper"),
        model: PathBuf::from("samantha.onnx"),
        config: PathBuf::from("samantha.onnx.json"),
        voice: "samantha".into(),
    }) {
        Err(err) => assert!(err.contains("refusing"), "{err}"),
        Ok(_) => panic!("samantha pack was opened"),
    }
    match PiperMouth::open(&PiperPaths {
        runtime: PathBuf::from("piper"),
        model: PathBuf::from("aria.onnx"),
        config: PathBuf::from("aria.onnx.json"),
        voice: "aria".into(),
    }) {
        Err(err) => assert!(err.contains("refusing"), "{err}"),
        Ok(_) => panic!("aria pack was opened"),
    }
}

#[test]
fn sentences_stream_on_the_frame_clock_and_barge_in_drops_them() {
    let dir = std::env::temp_dir().join(format!("horbm-piper-mouth-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let runtime = dir.join("piper");
    std::fs::write(&runtime, FAKE_PIPER).unwrap();
    let mut perm = std::fs::metadata(&runtime).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(0o755);
    }
    std::fs::set_permissions(&runtime, perm).unwrap();
    let model = dir.join(format!("{STOCK_VOICE}.onnx"));
    let config = dir.join(format!("{STOCK_VOICE}.onnx.json"));
    std::fs::write(&model, b"not-a-model").unwrap();
    std::fs::write(&config, br#"{"audio":{"sample_rate":16000}}"#).unwrap();
    let before = names(&dir);

    let paths = PiperPaths {
        runtime,
        model,
        config,
        voice: STOCK_VOICE.into(),
    };
    let mut mouth = PiperMouth::open(&paths).expect("fake piper");
    let pid = mouth.pid();
    assert!(mouth.process_alive());
    assert_eq!(mouth.spawn_noise(), "0.00");
    assert_eq!(mouth.id(), "piper");

    let calm = held(samantha_baseline().pos);
    let hot = held([0.9, 0.95, 0.85, 0.4]);
    assert!(!calm.parked);
    assert!(!hot.parked);
    let aria = instant(
        aria_baseline(),
        MomentState::at([0.9, 0.95, 0.85, 0.4]),
        Breath::rest(),
    );
    assert!(aria.parked);

    let parked = speak(&mut mouth, aria, "Hello there.");
    let steered = speak(&mut mouth, hot, "Hello there.");
    assert_eq!(mouth.pid(), pid, "a new sentence spawned another process");
    assert_ne!(
        peak(&parked),
        peak(&steered),
        "moment steering did not change the sentence"
    );
    assert_eq!(mouth.spawn_noise(), "0.00");
    assert!(mouth.controls().noise_scale > 0.0 || mouth.controls().length_scale != 1.0);

    mouth.feel(calm);
    mouth.start(&line("One. Two more words."));
    let mut first = true;
    let mut frames = 0u32;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if Instant::now() > deadline {
            panic!("sentences did not finish");
        }
        match mouth.pull() {
            MouthPull::Frame(frame) => {
                assert_eq!(frame.samples.len(), FRAME_SAMPLES);
                assert_eq!(frame.sample_rate, SAMPLE_RATE);
                if frame.first {
                    assert!(first, "more than one first frame");
                    first = false;
                }
                frames = frames.saturating_add(1);
                assert_eq!(mouth.last(), frame.samples.as_slice());
            }
            MouthPull::Idle => std::thread::sleep(Duration::from_millis(1)),
            MouthPull::Done => break,
        }
    }
    assert!(!first, "no audible frame");
    assert!(frames > 2, "frames {frames}");
    assert_eq!(mouth.sentences_started(), 4);

    mouth.feel(calm);
    mouth.start(&line("Hello there."));
    let ready = Instant::now() + Duration::from_secs(3);
    loop {
        if Instant::now() > ready {
            panic!("no frame before cancel");
        }
        match mouth.pull() {
            MouthPull::Frame(frame) => {
                assert!(frame.first);
                break;
            }
            MouthPull::Idle => std::thread::sleep(Duration::from_millis(1)),
            MouthPull::Done => panic!("the sentence ended before a frame"),
        }
    }
    assert!(mouth.speaking());
    let started = Instant::now();
    mouth.cancel();
    assert!(!mouth.speaking());
    assert!(mouth.last().is_empty());
    match mouth.pull() {
        MouthPull::Frame(frame) => panic!("cancel left {} samples", frame.samples.len()),
        MouthPull::Idle | MouthPull::Done => {}
    }
    let barge_ms = started.elapsed().as_millis();
    assert!(
        barge_ms <= 160,
        "barge-in to silence {barge_ms} ms, budget 160"
    );
    assert_eq!(mouth.pid(), pid);
    assert!(mouth.process_alive());

    mouth.start(&line("slow down please"));
    match mouth.pull() {
        MouthPull::Idle => {}
        MouthPull::Frame(_) => panic!("the slow sentence already had a frame"),
        MouthPull::Done => panic!("the slow sentence ended before any audio"),
    }
    let started = Instant::now();
    mouth.cancel();
    assert!(!mouth.speaking());
    match mouth.pull() {
        MouthPull::Frame(_) => panic!("in-flight audio was queued after cancel"),
        MouthPull::Idle | MouthPull::Done => {}
    }
    let inflight_ms = started.elapsed().as_millis();
    assert!(
        inflight_ms <= 160,
        "in-flight barge-in took {inflight_ms} ms"
    );
    let drain = Instant::now() + Duration::from_secs(3);
    while mouth.busy() {
        if Instant::now() > drain {
            panic!("cancelled sentence did not finish in the worker");
        }
        mouth.feel(calm);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(mouth.process_alive());
    let after = speak(&mut mouth, calm, "After.");
    assert!(peak(&after) > 0);
    assert_eq!(mouth.pid(), pid);

    let after_names = names(&dir);
    assert_eq!(
        before, after_names,
        "the mouth wrote a file: {after_names:?}"
    );
    drop(mouth);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stock_voice_reports_ttfa_barge_and_rtf() {
    let Some(paths) = stock_or_fetch() else {
        eprintln!("skip: stock piper voice not cached; set HORBM_PIPER_FETCH=1 to download");
        return;
    };
    let stats = measure_piper(&paths).expect("measure");
    assert!(stats.cpu);
    assert_eq!(stats.voice, STOCK_VOICE);
    assert!(stats.runs >= 10, "runs {}", stats.runs);
    assert!(stats.ttfa_p50_ms > 0);
    assert!(stats.ttfa_p50_ms <= stats.ttfa_p95_ms);
    assert!(
        stats.barge_to_silence_ms <= 160,
        "barge_to_silence_ms {}",
        stats.barge_to_silence_ms
    );
    assert!(
        stats.rtf.is_finite() && stats.rtf > 0.0,
        "rtf {}",
        stats.rtf
    );
    let json = stats.json();
    assert!(json.contains("\"cpu\":true"));
    assert!(json.contains("\"clock\":\"wall\""));
    assert!(!json.contains("sample"));
    eprintln!("{}", stats.text());
}

fn stock_or_fetch() -> Option<PiperPaths> {
    if let Some(paths) = discover_piper() {
        return Some(paths);
    }
    if std::env::var("HORBM_PIPER_FETCH").ok().as_deref() != Some("1") {
        return None;
    }
    Some(fetch_stock().expect("download the stock piper voice"))
}

fn fetch_stock() -> Result<PiperPaths, String> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("the pinned piper runtime is linux x86_64 only".into());
    }
    let cache = piper_cache_dir();
    let tts = cache.join("tts");
    let runtime_dir = cache.join("runtime");
    std::fs::create_dir_all(&tts).map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&runtime_dir).map_err(|err| err.to_string())?;
    let model = tts.join(format!("{STOCK_VOICE}.onnx"));
    let config = tts.join(format!("{STOCK_VOICE}.onnx.json"));
    fetch_file(&format!("{VOICE_URL}/{STOCK_VOICE}.onnx"), &model, ONNX_SHA)?;
    fetch_file(
        &format!("{VOICE_URL}/{STOCK_VOICE}.onnx.json"),
        &config,
        JSON_SHA,
    )?;
    let bin = runtime_dir.join("piper").join("piper");
    if !bin.is_file() {
        let tar = cache.join("piper_linux_x86_64.tar.gz");
        fetch_file(RUNTIME_URL, &tar, RUNTIME_SHA)?;
        let status = Command::new("tar")
            .args(["xzf"])
            .arg(&tar)
            .arg("-C")
            .arg(&runtime_dir)
            .status()
            .map_err(|err| err.to_string())?;
        if !status.success() {
            return Err("tar could not unpack the piper runtime".into());
        }
        let _ = std::fs::remove_file(&tar);
    }
    discover_piper().ok_or("stock voice is in the cache but was not discovered".into())
}

fn fetch_file(url: &str, dest: &Path, sha: &str) -> Result<(), String> {
    if dest.is_file() && file_sha(dest)?.eq_ignore_ascii_case(sha) {
        return Ok(());
    }
    let part = dest.with_extension("download");
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&part)
        .arg(url)
        .status()
        .map_err(|err| format!("curl: {err}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(format!("curl failed for {url}"));
    }
    let got = file_sha(&part)?;
    if !got.eq_ignore_ascii_case(sha) {
        let bytes = std::fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "sha256 mismatch for {url} ({bytes} bytes, got {got})"
        ));
    }
    std::fs::rename(&part, dest).map_err(|err| err.to_string())?;
    Ok(())
}

fn file_sha(path: &Path) -> Result<String, String> {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .map_err(|err| format!("sha256sum: {err}"))?;
    if !output.status.success() {
        return Err("sha256sum failed".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .next()
        .map(str::to_string)
        .ok_or("sha256sum returned no hash".into())
}

fn held(pos: [f32; 4]) -> horbm_voice::VoiceControls {
    let mut voice = MomentVoice::new(samantha_baseline());
    let moment = MomentState::at(pos);
    voice.tick(moment, Breath::rest());
    voice.tick(moment, Breath::rest());
    voice.applied()
}

fn line(text: &str) -> SpokenLine {
    SpokenLine {
        text: text.into(),
        mood: Pad::RESTING,
        rate: 1.0,
        energy: 1.0,
        pitch_st: 0.0,
        pause_ms: 0.0,
    }
}

fn speak(mouth: &mut PiperMouth, controls: horbm_voice::VoiceControls, text: &str) -> Vec<i16> {
    mouth.feel(controls);
    mouth.start(&line(text));
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if Instant::now() > deadline {
            panic!(
                "timeout speaking ({})",
                mouth.last_error().unwrap_or("no error")
            );
        }
        match mouth.pull() {
            MouthPull::Frame(frame) => out.extend_from_slice(&frame.samples),
            MouthPull::Idle => std::thread::sleep(Duration::from_millis(1)),
            MouthPull::Done => break,
        }
    }
    assert!(mouth.last_error().is_none());
    out
}

fn peak(samples: &[i16]) -> i16 {
    samples.iter().map(|sample| sample.abs()).max().unwrap_or(0)
}

fn names(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut walk = vec![dir.to_path_buf()];
    while let Some(next) = walk.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk.push(path);
            } else if let Some(name) = path.file_name() {
                found.push(name.to_string_lossy().into_owned());
            }
        }
    }
    found.sort();
    found
}

const FAKE_PIPER: &str = r#"#!/usr/bin/env python3
import json, struct, sys, time
count = 4000
while True:
    line = sys.stdin.buffer.readline()
    if not line:
        break
    text = json.loads(line.decode()).get("text", "")
    amp = 800 + (len(text) % 40) * 20
    if "slow" in text:
        time.sleep(0.3)
    pcm = struct.pack("<" + str(count) + "h", *([amp] * count))
    sys.stdout.buffer.write(pcm)
    sys.stdout.buffer.flush()
    sys.stderr.write("Real-time factor: 0.20 (infer=0.01 sec, audio=0.25 sec)\n")
    sys.stderr.flush()
"#;
