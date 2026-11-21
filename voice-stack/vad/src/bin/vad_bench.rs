// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Measure per-frame CPU time on the CC0 clips `scripts/fetch-cc0.sh` downloads.
//! Prints JSON. Does not write audio.

use std::path::{Path, PathBuf};
use std::time::Instant;

use galion_sam_vad::{
    summarize_ns, Backend, Event, FrameReport, Listen, TimeStats, TurnCfg, FRAME_SAMPLES,
    SAMPLE_RATE,
};

fn main() {
    let mut out_path: Option<PathBuf> = None;
    let mut samples: Option<PathBuf> = None;
    let mut model: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(k) = args.next() {
        match k.as_str() {
            "--out" => out_path = args.next().map(PathBuf::from),
            "--samples" => samples = args.next().map(PathBuf::from),
            "--model" => model = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                eprintln!("usage: vad-bench [--samples DIR] [--model FILE] [--out FILE]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }

    let cfg = TurnCfg::from_env();
    let dir = samples.unwrap_or_else(|| {
        galion_sam_vad::samples_dir()
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples"))
    });
    let model = model.unwrap_or_else(|| {
        std::env::var_os("GALION_SILERO_VAD")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(galion_sam_vad::bundled_model_path)
    });

    let clips = load_clips(&dir).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    if clips.is_empty() {
        eprintln!(
            "no wav files in {} — run scripts/fetch-cc0.sh",
            dir.display()
        );
        std::process::exit(2);
    }
    if !model.is_file() {
        eprintln!(
            "silero model not found at {} — run scripts/fetch-cc0.sh",
            model.display()
        );
        std::process::exit(2);
    }

    let timing_frames = repeat_until(&clips, 2000);
    let energy = measure(Listen::energy(cfg), &clips, &timing_frames);
    let silero_ear = Listen::silero(&model, cfg).unwrap_or_else(|e| {
        eprintln!("silero: {e}");
        std::process::exit(1);
    });
    let silero = measure(silero_ear, &clips, &timing_frames);

    let json = render(&dir, &model, cfg, &clips, &energy, &silero);
    if let Some(path) = &out_path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        std::fs::write(path, &json).unwrap_or_else(|e| {
            eprintln!("write {}: {e}", path.display());
            std::process::exit(1);
        });
        eprintln!("wrote {}", path.display());
    }
    println!("{json}");
}

struct Clip {
    name: String,
    rate: u32,
    n: usize,
    seconds: f64,
    frames: Vec<[i16; FRAME_SAMPLES]>,
}

struct Measured {
    backend: Backend,
    warm_cpu_ns: u64,
    model_io: Option<String>,
    events: Vec<String>,
    cpu: TimeStats,
    wall: TimeStats,
    model_cpu: TimeStats,
    windows: usize,
}

fn load_clips(dir: &Path) -> Result<Vec<Clip>, String> {
    if !dir.is_dir() {
        return Err(format!("samples dir {} is missing", dir.display()));
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("wav"))
        .collect();
    paths.sort();
    let mut clips = Vec::new();
    for path in paths {
        let wav = galion_sam_vad::read_wav(&path).map_err(|e| e.to_string())?;
        let mono = galion_sam_vad::to_mono(&wav);
        let pcm = if wav.rate == SAMPLE_RATE {
            mono
        } else {
            galion_sam_vad::resample(&mono, wav.rate, SAMPLE_RATE)
        };
        let mut frames = Vec::new();
        let mut i = 0;
        while i < pcm.len() {
            let mut frame = [0i16; FRAME_SAMPLES];
            let n = (pcm.len() - i).min(FRAME_SAMPLES);
            frame[..n].copy_from_slice(&pcm[i..i + n]);
            frames.push(frame);
            i += FRAME_SAMPLES;
        }
        let seconds = wav.samples.len() as f64 / wav.channels.max(1) as f64 / wav.rate as f64;
        clips.push(Clip {
            name: path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("clip")
                .to_string(),
            rate: wav.rate,
            n: wav.samples.len() / wav.channels.max(1) as usize,
            seconds,
            frames,
        });
    }
    Ok(clips)
}

fn repeat_until(clips: &[Clip], min_frames: usize) -> Vec<[i16; FRAME_SAMPLES]> {
    let mut out = Vec::new();
    while out.len() < min_frames {
        for c in clips {
            out.extend_from_slice(&c.frames);
        }
        if clips.iter().all(|c| c.frames.is_empty()) {
            break;
        }
    }
    out
}

fn measure(mut ear: Listen, clips: &[Clip], timing: &[[i16; FRAME_SAMPLES]]) -> Measured {
    let backend = ear.backend();
    let warm_cpu_ns = ear.warm_cpu_ns();
    let model_io = ear.model_io().map(str::to_string);
    let mut events = Vec::new();
    for clip in clips {
        ear.reset();
        let mut saw = Vec::new();
        for frame in &clip.frames {
            let r = ear.push(frame, false, "").unwrap_or_else(|e| {
                eprintln!("{backend:?} {}: {e}", clip.name);
                std::process::exit(1);
            });
            if r.event != Event::None || r.barge_in {
                saw.push(format_event(&clip.name, &r));
            }
        }
        // Trailing silence long enough to close a turn the clip itself left open.
        let hang = ear.cfg().hangover_ms + ear.cfg().frame_ms;
        let n = (hang / ear.cfg().frame_ms).max(1);
        for _ in 0..n {
            let r = ear
                .push(&[0i16; FRAME_SAMPLES], false, "")
                .unwrap_or_else(|e| {
                    eprintln!("{e}");
                    std::process::exit(1);
                });
            if r.event != Event::None || r.barge_in {
                saw.push(format_event(&clip.name, &r));
            }
        }
        if saw.is_empty() {
            saw.push(format!(
                "{{\"file\":{},\"event\":\"none\"}}",
                jstr(&clip.name)
            ));
        }
        events.extend(saw);
    }

    ear.reset();
    // One unmeasured pass so the first timed frame is not the first touch of the weights
    // beyond the load warmup. Silero's load already ran one silent window and reset state.
    if !timing.is_empty() {
        let _ = ear.push(&timing[0], false, "");
        ear.reset();
    }
    let mut cpu = Vec::with_capacity(timing.len());
    let mut wall = Vec::with_capacity(timing.len());
    let mut model_cpu = Vec::new();
    let t_all = Instant::now();
    for frame in timing {
        let r = ear.push(frame, false, "").unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        });
        cpu.push(r.cpu_ns);
        wall.push(r.wall_ns);
        if r.ran_model {
            model_cpu.push(r.model_cpu_ns);
        }
        std::hint::black_box(r.prob);
    }
    let elapsed_ms = t_all.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "{:>7}  frames {:5}  cpu_us p50 {:8.1} p95 {:8.1} max {:8.1}  windows {:4}  model_cpu_us p50 {:8.1} p95 {:8.1} max {:8.1}  wall_ms {:.1}",
        backend.name(),
        cpu.len(),
        summarize_ns(&cpu).p50_ns as f64 / 1000.0,
        summarize_ns(&cpu).p95_ns as f64 / 1000.0,
        summarize_ns(&cpu).max_ns as f64 / 1000.0,
        model_cpu.len(),
        summarize_ns(&model_cpu).p50_ns as f64 / 1000.0,
        summarize_ns(&model_cpu).p95_ns as f64 / 1000.0,
        summarize_ns(&model_cpu).max_ns as f64 / 1000.0,
        elapsed_ms
    );
    Measured {
        backend,
        warm_cpu_ns,
        model_io,
        events,
        cpu: summarize_ns(&cpu),
        wall: summarize_ns(&wall),
        model_cpu: summarize_ns(&model_cpu),
        windows: model_cpu.len(),
    }
}

fn format_event(file: &str, r: &FrameReport) -> String {
    format!(
        "{{\"file\":{},\"t_ms\":{},\"event\":\"{}\",\"barge_in\":{},\"prob\":{:.4},\"speaking\":{}}}",
        jstr(file),
        r.t_ms,
        if r.barge_in && r.event == Event::None { "barge_in" } else { r.event.name() },
        if r.barge_in { "true" } else { "false" },
        r.prob,
        if r.speaking { "true" } else { "false" }
    )
}

fn render(
    dir: &Path,
    model: &Path,
    cfg: TurnCfg,
    clips: &[Clip],
    energy: &Measured,
    silero: &Measured,
) -> String {
    let mut clips_json = String::from("[");
    for (i, c) in clips.iter().enumerate() {
        if i > 0 {
            clips_json.push(',');
        }
        clips_json.push_str(&format!(
            "{{\"file\":{},\"rate\":{},\"samples\":{},\"seconds\":{:.3},\"frames\":{}}}",
            jstr(&c.name),
            c.rate,
            c.n,
            c.seconds,
            c.frames.len()
        ));
    }
    clips_json.push(']');
    format!(
        "{{\n  \"sector\": \"vad\",\n  \"os\": \"{}\",\n  \"arch\": \"{}\",\n  \"frame_ms\": 20,\n  \"sample_rate\": 16000,\n  \"heartbeat_ms\": 80,\n  \"samples_dir\": {},\n  \"model\": {},\n  \"cfg\": {{\"onset_ms\":{},\"hangover_ms\":{},\"spec_ms\":{},\"commit_ms\":{},\"hold_ms\":{},\"barge_ms\":{},\"start_threshold\":{},\"keep_threshold\":{}}},\n  \"clips\": {clips_json},\n  \"energy\": {},\n  \"silero\": {}\n}}\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        jstr(&dir.display().to_string()),
        jstr(&model.display().to_string()),
        cfg.onset_ms,
        cfg.hangover_ms,
        cfg.spec_ms,
        cfg.commit_ms,
        cfg.hold_ms,
        cfg.barge_ms,
        fmt_f32(cfg.start_threshold),
        fmt_f32(cfg.keep_threshold),
        measured_json(energy),
        measured_json(silero),
    )
}

fn measured_json(m: &Measured) -> String {
    let io = match &m.model_io {
        Some(s) => jstr(s),
        None => "null".into(),
    };
    format!(
        "{{\"backend\":\"{}\",\"warmup_cpu_ns\":{},\"model_io\":{io},\"timing_frames\":{},\"cpu_ns\":{},\"wall_ns\":{},\"model_windows\":{},\"model_cpu_ns\":{},\"events\":[{}]}}",
        m.backend.name(),
        m.warm_cpu_ns,
        m.cpu.n,
        stats_json(&m.cpu),
        stats_json(&m.wall),
        m.windows,
        stats_json(&m.model_cpu),
        m.events.join(",")
    )
}

fn stats_json(s: &TimeStats) -> String {
    format!(
        "{{\"n\":{},\"min\":{},\"p50\":{},\"p95\":{},\"max\":{},\"mean\":{}}}",
        s.n, s.min_ns, s.p50_ns, s.p95_ns, s.max_ns, s.mean_ns
    )
}

fn fmt_f32(v: f32) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn jstr(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if c.is_control() => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
