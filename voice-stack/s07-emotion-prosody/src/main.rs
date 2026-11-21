// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Print Samantha's speaking parameters as the mood slews.
//!
//! No audio. `GALION_SAMANTHA_REF` is not opened.

use samantha_emotion_prosody::{measure_ticks, Pad, ProsodyClock, TICK_MS};
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("samantha-prosody: {err}");
            eprintln!("usage: samantha-prosody [--json] [--ticks N] [--from A,V,D] [--target A,V,D] [--bench N]");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let mut json = false;
    let mut ticks: u32 = 1;
    let mut from = Pad::RESTING;
    let mut target = Pad::RESTING;
    let mut bench: Option<u32> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--help" | "-h" => {
                println!(
                    "samantha-prosody [--json] [--ticks N] [--from A,V,D] [--target A,V,D] [--bench N]\n\
                     \n\
                     Prints one line per {TICK_MS} ms tick while the voice slews from --from to --target.\n\
                     Default is one tick at HORBM rest (arousal 0.3, valence 0.25, dominance 0.5).\n\
                     --bench N times N ticks and prints the mean. It does not synthesise audio."
                );
                return Ok(());
            }
            "--json" => json = true,
            "--ticks" => ticks = parse_u32(next(args, &mut i)?, "--ticks")?,
            "--bench" => bench = Some(parse_u32(next(args, &mut i)?, "--bench")?),
            "--from" => from = parse_pad(next(args, &mut i)?)?,
            "--target" => target = parse_pad(next(args, &mut i)?)?,
            other if other.starts_with('-') => return Err(format!("unknown flag {other}")),
            other => return Err(format!("unexpected argument {other}")),
        }
        i += 1;
    }

    if let Some(n) = bench {
        if n == 0 {
            return Err("--bench needs a positive count".to_string());
        }
        let elapsed = measure_ticks(n);
        let mean_ns = elapsed.as_nanos() as f64 / f64::from(n);
        println!(
            "bench ticks={n} elapsed_us={} mean_ns={mean_ns:.1} tick_budget_us={}",
            elapsed.as_micros(),
            u64::from(TICK_MS) * 1000
        );
        return Ok(());
    }

    if ticks == 0 {
        return Err("--ticks needs a positive count".to_string());
    }

    let mut clock = ProsodyClock::new();
    clock.set_target(from.clamped());
    for _ in 0..64 {
        clock.tick();
    }
    clock.set_target(target);
    for n in 0..ticks {
        let params = clock.tick();
        let applied = clock.applied();
        if json {
            println!("{}", json_line(n, applied, params));
        } else {
            println!("{}", plain_line(n, applied, params));
        }
    }
    Ok(())
}

fn next<'a>(args: &'a [String], i: &mut usize) -> Result<&'a str, String> {
    let flag = args[*i].as_str();
    *i += 1;
    args.get(*i)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_u32(text: &str, flag: &str) -> Result<u32, String> {
    text.parse::<u32>()
        .map_err(|_| format!("{flag} wants an integer, got {text}"))
}

fn parse_pad(text: &str) -> Result<Pad, String> {
    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() != 3 {
        return Err(format!("pad wants arousal,valence,dominance, got {text}"));
    }
    let num = |s: &str, name: &str| {
        s.trim()
            .parse::<f32>()
            .map_err(|_| format!("{name} is not a number in {text}"))
    };
    Ok(Pad {
        arousal: num(parts[0], "arousal")?,
        valence: num(parts[1], "valence")?,
        dominance: num(parts[2], "dominance")?,
    })
}

fn plain_line(n: u32, pad: Pad, params: samantha_emotion_prosody::SpeakingParams) -> String {
    let tag = params.tag.map(|t| t.token()).unwrap_or("-");
    format!(
        "t={n} ms={} a={:.3} v={:.3} d={:.3} ex={:.4} ls={:.4} ns={:.4} nw={:.4} sil={:.4} pause_ms={:.1} pitch_st={:.3} energy={:.4} rate={:.4} worker_rate={:.4} tag={tag}",
        u64::from(n) * u64::from(TICK_MS),
        pad.arousal,
        pad.valence,
        pad.dominance,
        params.exaggeration,
        params.length_scale,
        params.noise_scale,
        params.noise_w,
        params.sentence_silence_s,
        params.pre_pause_ms,
        params.pitch_shift_st,
        params.energy,
        params.rate,
        params.worker_rate,
    )
}

fn json_line(n: u32, pad: Pad, params: samantha_emotion_prosody::SpeakingParams) -> String {
    let tag = match params.tag {
        Some(t) => format!("\"{}\"", t.token()),
        None => "null".to_string(),
    };
    format!(
        "{{\"t_ms\":{},\"pad\":{{\"arousal\":{:.4},\"valence\":{:.4},\"dominance\":{:.4}}},\
         \"exaggeration\":{:.4},\"tag\":{tag},\"tag_on_piper\":false,\"tag_on_clone\":{},\
         \"prosody\":{{\"length_scale\":{:.4},\"noise_scale\":{:.4},\"noise_w\":{:.4},\
         \"sentence_silence_s\":{:.4},\"pre_pause_ms\":{},\"pitch_st\":{:.4},\"energy\":{:.4},\
         \"rate\":{:.4},\"worker_rate\":{:.4}}}}}",
        u64::from(n) * u64::from(TICK_MS),
        pad.arousal,
        pad.valence,
        pad.dominance,
        params.exaggeration,
        params.tag.is_some(),
        params.length_scale,
        params.noise_scale,
        params.noise_w,
        params.sentence_silence_s,
        params.pre_pause_ms_i(),
        params.pitch_shift_st,
        params.energy,
        params.rate,
        params.worker_rate,
    )
}
