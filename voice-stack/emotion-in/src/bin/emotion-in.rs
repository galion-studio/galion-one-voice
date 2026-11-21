// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! `emotion-in` — read prosody from a wav, from raw s16le, or print a bench.
//! Sense is denied until `--sense` or `GALION_EMOTION_SENSE=1`. Audio is not
//! written. `GALION_SAMANTHA_REF` is not read.

use galion_emotion_in::clock::FRAME_SAMPLES;
use galion_emotion_in::consent::{ConsentGate, Purpose, Session};
use galion_emotion_in::error::{EmotionError, EmotionErrorKind};
use galion_emotion_in::pitch::{self, PitchCheck};
use galion_emotion_in::report::{self, Report};
use galion_emotion_in::serve::{self, Publish};
use galion_emotion_in::stream::{self, EmotionIn};
use galion_emotion_in::wav::{self, PcmS16, WavPull};
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let code = match run(env::args().skip(1).collect()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("emotion-in: {e}");
            e.exit_code()
        }
    };
    process_exit(code);
}

fn process_exit(code: i32) -> ! {
    std::process::exit(code);
}

fn run(args: Vec<String>) -> Result<(), EmotionError> {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        println!("{}", help_text());
        return Ok(());
    }
    match args[0].as_str() {
        "bench" => cmd_bench(&args[1..]),
        "wav" => cmd_wav(&args[1..]),
        "pcm" => cmd_pcm(&args[1..]),
        "serve" => cmd_serve(&args[1..]),
        _ => {
            eprintln!("{}", help_text());
            Err(EmotionError::new(
                EmotionErrorKind::Usage,
                "unknown command",
            ))
        }
    }
}

fn cmd_bench(args: &[String]) -> Result<(), EmotionError> {
    let n: usize = match args.first() {
        Some(s) if !s.starts_with('-') => s.parse().map_err(|_| {
            EmotionError::new(
                EmotionErrorKind::Usage,
                "bench frame count must be an integer",
            )
        })?,
        _ => 200,
    };
    if n == 0 || n > 5_000 {
        return Err(EmotionError::new(
            EmotionErrorKind::Usage,
            "bench frame count must be 1..=5000",
        ));
    }
    let times = stream::measure_stages(n);
    println!("synthetic stage times (180 Hz sine, amplitude 0.2, one thread)");
    println!("{}", report::stage_text(&times));
    println!("pitch checks (synthetic sines, amplitude 0.2, one 80 ms frame at 24 kHz)");
    for c in pitch::sine_checks(&[100.0, 140.0, 180.0, 220.0, 300.0]) {
        println!("  {}", pitch_line(c));
    }
    println!("scenarios (synthetic, not recordings)");
    for scene in stream::synthetic_scenarios()? {
        println!("  {}", scene_line(scene.name, &scene.report));
    }
    Ok(())
}

fn pitch_line(c: PitchCheck) -> String {
    format!(
        "target {:>6.1} Hz  measured {:>8.3} Hz  err_hz {:>+7.3}  err_pct {:>+6.3}  harm {:.3}",
        c.target_hz,
        c.measured_hz,
        c.error_hz(),
        c.error_pct(),
        c.harmonicity
    )
}

fn scene_line(name: &str, report: &Report) -> String {
    let p = &report.reading.prosody;
    let m = &report.reading.mapped;
    format!(
        "{name:<18} f0 {f0:8.3} range {range:7.3} rms {rms:.4} dbfs {db:7.2} art {art:.3} speech {speech:.3} pause {pause:.3} long {long:4} ms nuclei {nuclei} voiced_frames {vf} conf {conf:.3} press {press:.3} estimate a {ea:.3} v {ev:.3} d {ed:.3} target a {ta:.3} v {tv:.3} d {td:.3} {class} push_p95 {p95} us",
        f0 = p.f0_hz,
        range = p.f0_range_hz,
        rms = p.rms,
        db = p.rms_dbfs,
        art = p.articulation_hz,
        speech = p.speech_rate_hz,
        pause = p.pause_ratio,
        long = p.longest_pause_ms,
        nuclei = p.nuclei,
        vf = p.voiced_frames,
        conf = m.confidence,
        press = m.anger_press,
        ea = m.estimate.arousal,
        ev = m.estimate.valence,
        ed = m.estimate.dominance,
        ta = m.target.arousal,
        tv = m.target.valence,
        td = m.target.dominance,
        class = m.classifier.as_str(),
        p95 = report.push_p95_us,
    )
}

fn cmd_wav(args: &[String]) -> Result<(), EmotionError> {
    let path = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .ok_or(EmotionError::new(
            EmotionErrorKind::Usage,
            "wav needs a path",
        ))?;
    let session = session_from(args);
    let report = wav::analyse_wav_path(Path::new(path), &session)?;
    println!("{}", report::report_json(&report));
    Ok(())
}

fn cmd_pcm(args: &[String]) -> Result<(), EmotionError> {
    let rate = required_u32(args, "--rate")?;
    let session = session_from(args);
    session.permit(Purpose::Sense)?;
    #[cfg(windows)]
    binary_stdin();
    let stdin = io::stdin();
    let mut pull = PcmS16::open(stdin.lock(), rate)?;
    let mut engine = EmotionIn::new();
    let mut frame = [0.0f32; FRAME_SAMPLES];
    let mut times = Vec::new();
    let stream_lines = flag(args, "--stream");
    let mut frames = 0u32;
    while pull.next_frame(&mut frame)? {
        let started = Instant::now();
        let reading = engine.push_frame(&session, &frame)?;
        times.push(report::elapsed_us(started));
        frames += 1;
        if stream_lines {
            println!("{}", report::reading_json(&reading));
            let _ = io::stdout().flush();
        }
        frame = [0.0; FRAME_SAMPLES];
    }
    pull.wipe();
    if !stream_lines {
        let report = Report::from_reading(engine.reading(), frames, 0, &times);
        println!("{}", report::report_json(&report));
    }
    Ok(())
}

fn cmd_serve(args: &[String]) -> Result<(), EmotionError> {
    let bind = opt(args, "--bind").unwrap_or("127.0.0.1:8768");
    let listener = serve::bind_loopback(bind)?;
    let addr = listener.local_addr().map_err(|_| {
        EmotionError::new(EmotionErrorKind::Bind, "could not read the bound address")
    })?;
    let publish = Arc::new(Mutex::new(Publish::EMPTY));
    let shared = Arc::clone(&publish);
    thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { break };
            let snap = shared.lock().map(|g| *g).unwrap_or(Publish::EMPTY);
            let _ = serve::handle_client(&mut stream, &snap);
        }
    });
    eprintln!("emotion-in http://{addr}/");
    if let Some(path) = opt(args, "--wav") {
        let session = session_from(args);
        session.permit(Purpose::Sense)?;
        let file = File::open(path)
            .map_err(|_| EmotionError::new(EmotionErrorKind::BadAudio, "could not open wav"))?;
        let mut pull = WavPull::open(file)?;
        let mut engine = EmotionIn::new();
        let mut frame = [0.0f32; FRAME_SAMPLES];
        let mut times = Vec::new();
        let pace = flag(args, "--pace");
        let mut frames = 0u32;
        while pull.next_frame(&mut frame)? {
            let started = Instant::now();
            let reading = engine.push_frame(&session, &frame)?;
            times.push(report::elapsed_us(started));
            frames += 1;
            if let Ok(mut slot) = publish.lock() {
                *slot = Publish {
                    reading,
                    push_median_us: report::percentile(&times, 50),
                    push_p95_us: report::percentile(&times, 95),
                    dropped_tail_samples: pull.dropped_tail_samples(),
                    frames,
                };
            }
            frame = [0.0; FRAME_SAMPLES];
            if pace {
                thread::sleep(Duration::from_millis(u64::from(
                    galion_emotion_in::FRAME_MS,
                )));
            }
        }
        let dropped = pull.dropped_tail_samples();
        pull.wipe();
        if let Ok(mut slot) = publish.lock() {
            slot.dropped_tail_samples = dropped;
            slot.frames = frames;
        }
    }
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn session_from(args: &[String]) -> Session {
    let env = env::var("GALION_EMOTION_SENSE").ok();
    Session::from_flag_and_env(flag(args, "--sense"), env.as_deref())
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

fn required_u32(args: &[String], name: &str) -> Result<u32, EmotionError> {
    opt(args, name)
        .ok_or(EmotionError::new(EmotionErrorKind::Usage, "missing --rate"))?
        .parse()
        .map_err(|_| EmotionError::new(EmotionErrorKind::Usage, "--rate must be an integer"))
}

fn help_text() -> &'static str {
    "\
galion-emotion-in — prosody on the 80 ms clock, as a HORBM Pad mood target

  emotion-in bench [frames]
  emotion-in wav <file.wav> --sense
  emotion-in pcm --rate <hz> --sense [--stream]
  emotion-in serve --bind 127.0.0.1:8768 [--wav <file.wav> --sense] [--pace]

Sense is denied until --sense or GALION_EMOTION_SENSE=1. That grant stays in
this process. Audio is not written. One frame of scratch is wiped after use.
GALION_SAMANTHA_REF is not read. The page binds loopback only.

Linux mic (24 kHz mono s16le):
  arecord -f S16_LE -r 24000 -c 1 -t raw | emotion-in pcm --rate 24000 --sense --stream

Windows, from the clone (this machine's home is often G:\\galion):
  cargo run --release --manifest-path voice-stack\\emotion-in\\Cargo.toml -- wav C:\\path\\clip.wav --sense
  ffmpeg -f dshow -i audio=Microphone -ac 1 -ar 24000 -f s16le - | emotion-in.exe pcm --rate 24000 --sense --stream
"
}

#[cfg(windows)]
fn binary_stdin() {
    extern "C" {
        fn _setmode(fd: i32, mode: i32) -> i32;
    }
    unsafe {
        let _ = _setmode(0, 0x8000);
    }
}
