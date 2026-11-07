//! Nebula Galion Voice V2 binary.
//!
//! Headless frames need no display. A window build is `--features window`
//! (or `--features gles` on a Pi). Laptop and desktop default to the normal
//! pacing tier in `quality.rs`.

use std::path::PathBuf;

use nebula_voice_v2::demo::frame_at;
use nebula_voice_v2::frame::{drive_from, parse_frame};
use nebula_voice_v2::quality::{
    budget_for, parse_device_alias, parse_quality, DeviceClass, GpuClass, QualityMode, TierBudget,
};
use nebula_voice_v2::render::{render, write_png};
use nebula_voice_v2::shade::Voice;
use nebula_voice_v2::sim::Sim;

fn main() {
    if let Err(err) = run() {
        eprintln!("nebula-voice-v2: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }
    let mut headless = false;
    let mut frames = 4u32;
    let mut out = PathBuf::from("nebula-frames");
    let mut voice = None;
    let mut quality = QualityMode::Normal;
    let mut device = DeviceClass::Laptop;
    let mut width = 1280u32;
    let mut height = 720u32;
    let mut seed = 7u32;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--headless" => headless = true,
            "--stdin" => return run_stdin(),
            "--demo" => headless = true,
            "--frames" => frames = take(&args, &mut i)?.parse().map_err(|e| format!("{e}"))?,
            "--out" => out = PathBuf::from(take(&args, &mut i)?),
            "--voice" => voice = Voice::parse(&take(&args, &mut i)?),
            "--quality" => {
                let raw = take(&args, &mut i)?;
                quality = parse_quality(&raw).ok_or_else(|| format!("unknown quality {raw}"))?;
                if let Some(class) = parse_device_alias(&raw) {
                    device = class;
                }
            }
            "--width" => width = take(&args, &mut i)?.parse().map_err(|e| format!("{e}"))?,
            "--height" => height = take(&args, &mut i)?.parse().map_err(|e| format!("{e}"))?,
            "--seed" => seed = take(&args, &mut i)?.parse().map_err(|e| format!("{e}"))?,
            "--udp" | "--ws" => {
                let addr = take(&args, &mut i)?;
                return Err(format!("{arg} {addr} is accepted by a window build; this process has no display loop. Use --stdin for NDJSON."));
            }
            other => return Err(format!("unknown argument {other}")),
        }
        i += 1;
    }
    if !headless {
        print_help();
        return Err("no window in this build. Pass --headless, or rebuild with --features window.".into());
    }
    let gpu = GpuClass::Integrated;
    let budget = budget_for(quality, device, 60.0, gpu);
    let voices = match voice {
        Some(v) => vec![v],
        None => vec![Voice::Samantha, Voice::Jarvis],
    };
    for voice in voices {
        render_voice(voice, frames, &out, seed, width, height, &budget)?;
    }
    Ok(())
}

fn render_voice(voice: Voice, frames: u32, out: &PathBuf, seed: u32, width: u32, height: u32, budget: &TierBudget) -> Result<(), String> {
    let mut sim = Sim::create(seed, budget.particle_ceiling.max(budget.particles));
    let label = match voice {
        Voice::Samantha => "samantha",
        Voice::Jarvis => "jarvis",
    };
    let mode = format!("{:?}", budget.mode).to_ascii_lowercase();
    for index in 0..frames {
        let time = index as f32 * (1.0 / budget.fps_target.max(1.0)) * 24.0 + 1.5;
        let frame = frame_at(voice, time);
        let drive = drive_from(&frame, budget.particles, budget.octaves);
        for _ in 0..8 {
            sim.step(&drive, 1.0 / budget.fps_target.max(1.0));
        }
        let image = render(&sim, &frame, width, height, budget, true);
        let path = out.join(format!("{label}-{mode}-{index:02}.png"));
        write_png(&path, &image).map_err(|e| e.to_string())?;
        println!("{} {}x{} {}", path.display(), image.width, image.height, mode);
    }
    Ok(())
}

fn run_stdin() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut voice = Voice::Samantha;
    for line in stdin.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let frame = parse_frame(&line, voice).map_err(|e| e.0)?;
        voice = frame.voice;
        let top = nebula_voice_v2::frame::dominant(&frame, 3);
        let names: Vec<&str> = top.iter().map(|l| l.emotion).collect();
        println!("{{\"voice\":\"{:?}\",\"dominant\":{}}}", voice, serde_json::to_string(&names).unwrap_or_else(|_| "[]".into()));
    }
    Ok(())
}

fn take<'a>(args: &'a [String], i: &mut usize) -> Result<&'a str, String> {
    *i += 1;
    args.get(*i).map(|s| s.as_str()).ok_or_else(|| "missing value".into())
}

fn print_help() {
    eprintln!(
        "Nebula Galion Voice V2\n\
         --headless --frames N --out DIR --voice samantha|jarvis --quality pi|normal|high|ultra\n\
         --quality phone|laptop|desktop keeps normal pacing and sets the device class\n\
         --stdin reads newline-delimited JSON frames\n\
         Window: cargo run --features window -- --demo"
    );
}
