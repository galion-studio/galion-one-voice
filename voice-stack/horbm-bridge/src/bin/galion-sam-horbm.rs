// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Local driver for the reply bridge. No audio, no network.
//!
//! ```text
//! galion-sam-horbm tick
//! galion-sam-horbm chunks The kettle is on. It can wait.
//! galion-sam-horbm say the kettle is on
//! galion-sam-horbm pace
//! ```
//!
//! `say` and `chunks` read stdin when no words are given.
//! With `--features horbm` and `GALION_HORBM_MEMORY`, `GALION_HORBM_KNOWLEDGE`,
//! and `GALION_HORBM_CASES` all set, `say` opens that brain. Otherwise it
//! uses the fake echo. This binary does not read `GALION_SAMANTHA_REF`.

use std::io::{self, Read};
use std::thread;
use std::time::{Duration, Instant};

use galion_sam_horbm::{
    speakable_chunks, time_ns, FakeEngine, MoodTick, ReplyEngine, Session, FRAME_MS,
};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        std::process::exit(2);
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "help" | "-h" | "--help" => usage(),
        "tick" => tick(),
        "chunks" => {
            let text = words_or_stdin(&args);
            print_chunks(&text);
        }
        "say" => say(&words_or_stdin(&args)),
        "pace" => pace(),
        other => {
            eprintln!("unknown command {other}");
            usage();
            std::process::exit(2);
        }
    }
}

fn usage() {
    eprintln!(
        "galion-sam-horbm {{tick|chunks|say|pace}}\n\
         tick    time one mood frame, one chunk split, and one fake turn\n\
         chunks  split reply text into speakable sentences\n\
         say     one turn on the fake engine, or on horbm when the feature and paths are set\n\
         pace    run the 80 ms mood thread and print the gaps it actually woke on\n\
         This crate does not read GALION_SAMANTHA_REF and does not play audio."
    );
}

fn words_or_stdin(args: &[String]) -> String {
    if !args.is_empty() {
        return args.join(" ");
    }
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).ok();
    buf.trim().to_string()
}

fn tick() {
    let host = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let frame = time_ns(2_000, || {
        let mut engine = FakeEngine::new();
        let pad = std::hint::black_box(engine.tick());
        std::hint::black_box(pad);
    });
    print_sample(&host, "empty_tick", &frame);

    let line = "The kettle is on. It can wait while the water cools.";
    let chunks = time_ns(2_000, || {
        let out = std::hint::black_box(speakable_chunks(line));
        std::hint::black_box(out);
    });
    print_sample(&host, "speakable_chunks", &chunks);

    let turn = time_ns(2_000, || {
        let mut engine = FakeEngine::new();
        let reply = std::hint::black_box(engine.turn("The kettle is on."));
        std::hint::black_box(reply);
    });
    print_sample(&host, "fake_turn", &turn);
    println!(
        "{{\"host\":{host:?},\"frame_ms\":{FRAME_MS},\"note\":\"wall time of the bridge only\"}}"
    );
}

fn print_sample(host: &str, op: &str, sample: &galion_sam_horbm::SampleNs) {
    println!(
        "{{\"host\":{host:?},\"op\":{op:?},\"n\":{},\"warmup\":64,\"min_ns\":{},\"p50_ns\":{},\"p95_ns\":{},\"max_ns\":{}}}",
        sample.n, sample.min_ns, sample.p50_ns, sample.p95_ns, sample.max_ns
    );
}

fn print_chunks(text: &str) {
    let chunks = speakable_chunks(text);
    println!("chunks {}", chunks.len());
    for chunk in chunks {
        println!("{}\t{}\t{}", chunk.index, chunk.words, chunk.text);
    }
}

fn say(text: &str) {
    #[cfg(feature = "horbm")]
    if let Some(engine) = horbm_from_env() {
        println!("engine: horbm");
        say_with(engine, text);
        return;
    }
    println!("engine: fake");
    say_with(FakeEngine::new(), text);
}

fn say_with<E: ReplyEngine>(engine: E, text: &str) {
    let mut session = Session::new(engine);
    let started = Instant::now();
    let spoken = session.turn(text);
    let wall_us = started.elapsed().as_micros();
    println!("reply: {}", spoken.reply.text);
    println!(
        "mood: arousal={:.4} valence={:.4} dominance={:.4}",
        spoken.reply.mood.arousal, spoken.reply.mood.valence, spoken.reply.mood.dominance
    );
    println!(
        "sources: {}",
        if spoken.reply.sources.is_empty() {
            "(none)".to_string()
        } else {
            spoken.reply.sources.join(", ")
        }
    );
    println!(
        "took_us: {} lookup_us: {} wall_us: {wall_us}",
        spoken.reply.took_us, spoken.reply.lookup_us
    );
    println!("chunks: {}", spoken.chunks.len());
    for chunk in &spoken.chunks {
        println!("  [{}] {}", chunk.index, chunk.text);
    }
}

fn pace() {
    let host = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let mut session = Session::new(FakeEngine::new());
    let loud = session.turn("hey!");
    let mut gaps = Vec::new();
    let mut prev = Instant::now();
    for _ in 0..8 {
        let target = prev + Duration::from_millis(u64::from(FRAME_MS));
        let now = Instant::now();
        if now < target {
            thread::sleep(target - now);
        }
        let woke = Instant::now();
        gaps.push(woke.saturating_duration_since(prev).as_micros());
        prev = woke;
        session.tick();
    }
    let mood = session.mood();
    println!(
        "{{\"host\":{host:?},\"op\":\"session_tick_gap\",\"frames\":{},\"start_arousal\":{},\"gaps_us\":{gaps:?},\"frame_ms\":{FRAME_MS},\"mood\":{{\"arousal\":{},\"valence\":{},\"dominance\":{}}}}}",
        session.frames(),
        loud.reply.mood.arousal,
        mood.arousal,
        mood.valence,
        mood.dominance
    );

    let tick = match MoodTick::spawn(FakeEngine::new()) {
        Ok(tick) => tick,
        Err(err) => {
            eprintln!("mood thread did not start: {err}");
            std::process::exit(1);
        }
    };
    thread::sleep(Duration::from_millis(500));
    let frames = tick.frames();
    println!(
        "{{\"host\":{host:?},\"op\":\"mood_thread_frames_in_500ms\",\"frames\":{frames},\"frame_ms\":{FRAME_MS}}}"
    );
}

#[cfg(feature = "horbm")]
fn horbm_from_env() -> Option<galion_sam_horbm::HorbmEngine> {
    use std::path::Path;
    let memory = std::env::var("GALION_HORBM_MEMORY").ok()?;
    let knowledge = std::env::var("GALION_HORBM_KNOWLEDGE").ok()?;
    let cases = std::env::var("GALION_HORBM_CASES").ok()?;
    if memory.is_empty() || knowledge.is_empty() || cases.is_empty() {
        return None;
    }
    println!("horbm_memory: {memory}");
    println!("horbm_knowledge: {knowledge}");
    println!("horbm_cases: {cases}");
    Some(galion_sam_horbm::HorbmEngine::open(
        Path::new(&memory),
        Path::new(&knowledge),
        Path::new(&cases),
    ))
}
