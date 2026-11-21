// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `galion-audio-io list | measure | play <wav>`
//!
//! `list` prints the host and the devices cpal can see.
//! `measure` prints JSON. The software block always runs. The live block
//! records a capture-to-callback figure only when a device opens.
//! `play` writes a WAV the caller already has to the default output.
//! It does not look for a voice reference.

use galion_audio_io::{
    host_report, measure_all, open, read_wav, Direction, SessionConfig, PLAYBACK_HZ,
};
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("list") => cmd_list(),
        Some("measure") => cmd_measure(),
        Some("play") => cmd_play(args.next()),
        _ => {
            eprintln!("usage: galion-audio-io <list|measure|play <wav>>");
            ExitCode::from(2)
        }
    }
}

fn cmd_list() -> ExitCode {
    let report = host_report();
    println!("host: {}", report.host);
    println!("available: {}", report.available.join(", "));
    println!("pulse_or_pipewire_plugin_file: {}", report.pulse_plugin);
    if let Some(err) = &report.error {
        println!("error: {err}");
    }
    if report.devices.is_empty() {
        println!("devices: 0");
    }
    for device in &report.devices {
        let marks = format!(
            "{}{}{}{}",
            if device.input { "in " } else { "" },
            if device.output { "out " } else { "" },
            if device.default_input || device.default_output {
                "default "
            } else {
                ""
            },
            if device.pulse_name { "pulse " } else { "" }
        );
        println!(
            "{marks}{name} in_hz={in_hz} out_hz={out_hz}",
            name = device.name,
            in_hz = device
                .input_rate
                .map(|r| r.to_string())
                .unwrap_or_else(|| "-".into()),
            out_hz = device
                .output_rate
                .map(|r| r.to_string())
                .unwrap_or_else(|| "-".into()),
        );
    }
    ExitCode::SUCCESS
}

fn cmd_measure() -> ExitCode {
    let report = measure_all();
    println!("{}", galion_audio_io::full_json(&report));
    ExitCode::SUCCESS
}

fn cmd_play(path: Option<String>) -> ExitCode {
    let Some(path) = path else {
        eprintln!("usage: galion-audio-io play <wav>");
        return ExitCode::from(2);
    };
    let path = PathBuf::from(path);
    let wav = match read_wav(&path) {
        Ok(wav) => wav,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    let session = match open(&SessionConfig::from_env(Direction::Playback)) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("no output device: {err}");
            return ExitCode::from(1);
        }
    };
    let playback = session.playback();
    let mut offset = 0usize;
    while offset < wav.samples.len() {
        let channels = wav.channels.max(1) as usize;
        let frames_left = (wav.samples.len() - offset) / channels;
        let free = playback.free();
        if free < 64 {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        let src_frames = ((free as u64 * u64::from(wav.rate)) / u64::from(PLAYBACK_HZ))
            .saturating_sub(8)
            .max(1) as usize;
        let src_frames = src_frames.min(frames_left).max(1);
        let end = offset + src_frames * channels;
        match playback.push_pcm16(&wav.samples[offset..end], wav.rate) {
            Ok(0) => thread::sleep(Duration::from_millis(10)),
            Ok(_) => offset = end,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::from(1);
            }
        }
    }
    let timeout = Duration::from_secs(30);
    let start = std::time::Instant::now();
    while playback.queued() > 0 && start.elapsed() < timeout {
        thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(Duration::from_millis(200));
    if let Some(err) = session.last_error() {
        eprintln!("stream error: {err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
