// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Print a probe, or speak one line and print timing.
//! This example does not write a wav.

use samantha_voice_clone::{
    probe_compute, run_repo_stub_probe, run_self_check, Pad, SamanthaTts, Tts,
};
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        eprintln!(
            "usage: speak --probe | speak --self-check | speak \"sentence\"\n\
             GALION_SAMANTHA_REF must be a local wav whose sha256 starts with 10ac44dc.\n\
             This program prints counts and timings. It does not write audio."
        );
        return if args.is_empty() {
            ExitCode::from(2)
        } else {
            ExitCode::SUCCESS
        };
    }
    if args[0] == "--probe" {
        let probe = probe_compute();
        println!(
            "device={} note={} raw={} elapsed_us={}",
            probe.kind.as_str(),
            probe.note,
            probe.raw,
            probe.elapsed_us
        );
        return ExitCode::SUCCESS;
    }
    if args[0] == "--self-check" {
        let probe = probe_compute();
        println!(
            "probe device={} note={} raw={} elapsed_us={}",
            probe.kind.as_str(),
            one_line(&probe.note),
            probe.raw,
            probe.elapsed_us
        );
        match run_self_check() {
            Ok(check) => {
                println!(
                    "self-check reuse={} mode={} exaggeration={} cfg={} seed0={} seed1={} stub_samples={} stub_ttfa_us={} wall_us={} device_probe={} device_note={} auto_device={} resample_out={}",
                    check.reuse,
                    check.mode,
                    check.exaggeration,
                    check.cfg,
                    check.seed0,
                    check.seed1,
                    check.stub_samples,
                    check.stub_ttfa_us,
                    check.wall_us,
                    check.device_probe,
                    one_line(&check.device_note),
                    check.auto_device,
                    check.resample_out
                );
            }
            Err(err) => {
                eprintln!("self-check failed: {err}");
                return ExitCode::from(1);
            }
        }
        match run_repo_stub_probe() {
            Ok(probe) => {
                println!(
                    "repo-stub exit={} pcm_bytes={} first_sample={} wall_us={} controls={}",
                    probe.exit_code,
                    probe.pcm_bytes,
                    probe.first_sample,
                    probe.wall_us,
                    one_line(&probe.controls_line)
                );
            }
            Err(err) => {
                eprintln!("repo stub probe failed: {err}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    let text = args.join(" ");
    let mut voice = match SamanthaTts::open_from_env() {
        Ok(voice) => voice,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(2);
        }
    };
    match voice.synthesize(&text, &Pad::RESTING) {
        Ok(speech) => {
            println!(
                "recipe={} device={} cached={} samples={} frames={} tail={} took_us={} rate={} channels={} bits={}",
                speech.recipe,
                speech.device,
                speech.cached,
                speech.samples.len(),
                speech.full_frames().count(),
                speech.tail().len(),
                speech.took_us,
                speech.sample_rate_hz,
                speech.channels,
                speech.bits
            );
            for sentence in &speech.sentences {
                println!(
                    "sentence index={} seed={} cached={} samples={} worker_ttfa_us={} ref_prefix={}",
                    sentence.index,
                    sentence.seed,
                    sentence.cached,
                    sentence.samples.len(),
                    sentence
                        .worker_ttfa_us
                        .map(|us| us.to_string())
                        .unwrap_or_else(|| "none".into()),
                    &sentence.reference_sha256[..8]
                );
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
