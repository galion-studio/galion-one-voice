// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Cold and warm time-to-first-audio, plus real-time factor, on a real Piper child.
//! Prints one JSON object. Exit 0 when measured, 2 when the voice or runtime
//! is not on disk, 1 on a runtime failure. Does not write audio unless `--wav`
//! is passed, and that file must not be committed.

use galion_tts_piper::{frames_80ms, verify_voice, Pad, PiperTts, PiperVoice, Tts};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut wav: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--wav" => wav = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                eprintln!("piper-ttfa [--wav FILE]");
                eprintln!("Measures cold and warm TTFA on the resolved Piper voice. --wav writes s16le of the stream utterance and must not be committed.");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let voice = match galion_tts_piper::resolve() {
        Ok(v) => v,
        Err(e) => {
            println!("{}", missing(&e.to_string()));
            std::process::exit(2);
        }
    };
    let t_sha = Instant::now();
    if let Err(e) = verify_voice(&voice) {
        println!("{}", missing(&e.to_string()));
        std::process::exit(1);
    }
    let sha_ms = ms(t_sha.elapsed());
    match run(&voice, sha_ms, wav.as_deref()) {
        Ok(body) => {
            println!("{body}");
            std::process::exit(0);
        }
        Err(e) => {
            println!("{}", missing(&e.to_string()));
            std::process::exit(1);
        }
    }
}

fn run(voice: &PiperVoice, sha_ms: u32, wav: Option<&std::path::Path>) -> Result<String, String> {
    let t0 = Instant::now();
    let mut tts = PiperTts::spawn(voice.clone()).map_err(|e| e.to_string())?;
    let spawn_return_ms = ms(t0.elapsed());
    let cold = tts
        .speak("Hello.", Pad::RESTING, &mut |_| {})
        .map_err(|e| e.to_string())?;
    let mut warm_ttfa = Vec::new();
    let mut warm_piper = Vec::new();
    let mut warm_wall = Vec::new();
    let mut warm_audio = 0u32;
    let mut last_warm_pcm_samples = 0u32;
    for _ in 0..5 {
        let r = tts
            .speak("Hello.", Pad::RESTING, &mut |_| {})
            .map_err(|e| e.to_string())?;
        warm_ttfa.push(r.first_pcm_byte_ms);
        if let Some(rtf) = r.detail.first().and_then(|d| d.piper_rtf) {
            warm_piper.push(rtf);
        }
        if let Some(rtf) = r.wall_rtf {
            warm_wall.push(rtf);
        }
        warm_audio = r.audio_ms;
        last_warm_pcm_samples = r.samples;
    }
    let mut stream_pcm = Vec::new();
    let stream = tts
        .speak("I am here. I can hear you.", Pad::RESTING, &mut |c| {
            stream_pcm.extend_from_slice(c.samples)
        })
        .map_err(|e| e.to_string())?;
    if let Some(path) = wav {
        write_s16(path, &stream_pcm).map_err(|e| e.to_string())?;
        eprintln!(
            "wrote {} bytes s16le {} Hz to {} (do not commit)",
            stream_pcm.len() * 2,
            stream.sample_rate,
            path.display()
        );
    }
    let submit1 = event_ms(&stream.events, true, 1);
    let callback0 = event_ms(&stream.events, false, 0);
    let frames = frames_80ms(&stream_pcm, stream.sample_rate);
    let full = frames
        .iter()
        .filter(|f| f.filled == galion_tts_piper::HEARTBEAT_FRAME)
        .count();
    let tail = frames.last().map(|f| f.filled).unwrap_or(0);
    let mut warm_sorted = warm_ttfa.clone();
    let warm_median = median_u32(&mut warm_sorted);
    let mut piper_sorted = warm_piper.clone();
    let piper_median = median_f32(&mut piper_sorted);
    let mut wall_sorted = warm_wall.clone();
    let wall_median = median_f32(&mut wall_sorted);
    let cold_rtf = cold.detail.first().and_then(|d| d.piper_rtf);
    let cold_wall = cold.wall_rtf;
    Ok(format!(
        concat!(
            "{{\n",
            "  \"measured\": true,\n",
            "  \"voice\": \"en_US-ljspeech-medium\",\n",
            "  \"runtime\": \"{runtime}\",\n",
            "  \"runtime_path\": \"{runtime_path}\",\n",
            "  \"model_path\": \"{model_path}\",\n",
            "  \"sample_rate\": {rate},\n",
            "  \"io\": \"{io}\",\n",
            "  \"drain_confirm_ms\": {drain},\n",
            "  \"host_cpu\": \"{cpu}\",\n",
            "  \"host_cpus\": {cpus},\n",
            "  \"loadavg\": \"{load}\",\n",
            "  \"build\": \"{build}\",\n",
            "  \"sha_verify_ms\": {sha},\n",
            "  \"spawn_return_ms\": {spawn_ms},\n",
            "  \"cold_spawn_to_first_pcm_ms\": {cold_spawn},\n",
            "  \"cold_submit_to_first_pcm_ms\": {cold_submit},\n",
            "  \"cold_piper_rtf\": {cold_piper},\n",
            "  \"cold_wall_rtf\": {cold_wall},\n",
            "  \"cold_audio_ms\": {cold_audio},\n",
            "  \"warm_submit_to_first_pcm_ms\": {warm_list},\n",
            "  \"warm_submit_to_first_pcm_median_ms\": {warm_med},\n",
            "  \"warm_piper_rtf\": {warm_piper},\n",
            "  \"warm_piper_rtf_median\": {warm_piper_med},\n",
            "  \"warm_wall_rtf\": {warm_wall},\n",
            "  \"warm_wall_rtf_median\": {warm_wall_med},\n",
            "  \"warm_audio_ms\": {warm_audio},\n",
            "  \"warm_samples\": {warm_samples},\n",
            "  \"warm_under_80ms\": {under},\n",
            "  \"stream_sentences\": {stream_n},\n",
            "  \"stream_first_pcm_ms\": {stream_first},\n",
            "  \"stream_second_submit_ms\": {submit1},\n",
            "  \"stream_first_callback_ms\": {callback0},\n",
            "  \"stream_second_submitted_before_callback\": {overlap},\n",
            "  \"stream_wall_ms\": {stream_wall},\n",
            "  \"stream_heartbeat_full_frames\": {full},\n",
            "  \"stream_heartbeat_tail_samples\": {tail},\n",
            "  \"short_text\": \"Hello.\",\n",
            "  \"stream_text\": \"I am here. I can hear you.\",\n",
            "  \"note\": \"cold_spawn_to_first_pcm_ms and cold_wall_rtf include model load. cold_piper_rtf is the child infer/audio for that first line. warm_submit_to_first_pcm_ms is the warm time-to-first-audio. Piper VITS emits a whole clause at once. warm_under_80ms compares this host warm median to the 80 ms heartbeat and is not a portable guarantee. wall_rtf is speak wall divided by audio duration. piper_rtf is the child infer/audio line.\"\n",
            "}}\n"
        ),
        runtime = json_str(&voice.runtime.file_name().and_then(|s| s.to_str()).unwrap_or("piper")),
        runtime_path = json_str(&voice.runtime.display().to_string()),
        model_path = json_str(&voice.model.display().to_string()),
        rate = voice.sample_rate,
        io = tts.io_name(),
        drain = if tts.io_name() == "thread" { galion_tts_piper::thread_drain_confirm_ms() } else { 0 },
        cpu = json_str(&cpu_model()),
        cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        load = json_str(&loadavg()),
        build = if cfg!(debug_assertions) { "debug" } else { "release" },
        sha = sha_ms,
        spawn_ms = spawn_return_ms,
        cold_spawn = cold.cold_spawn_to_first_pcm_ms.map(|n| n.to_string()).unwrap_or_else(|| "null".into()),
        cold_submit = cold.first_pcm_byte_ms,
        cold_piper = num_opt(cold_rtf),
        cold_wall = num_opt(cold_wall),
        cold_audio = cold.audio_ms,
        warm_list = json_u32s(&warm_ttfa),
        warm_med = warm_median,
        warm_piper = json_f32s(&warm_piper),
        warm_piper_med = num_opt(piper_median),
        warm_wall = json_f32s(&warm_wall),
        warm_wall_med = num_opt(wall_median),
        warm_audio = warm_audio,
        warm_samples = last_warm_pcm_samples,
        under = if warm_median <= 80 { "true" } else { "false" },
        stream_n = stream.sentences,
        stream_first = stream.first_pcm_byte_ms,
        submit1 = submit1.map(|n| n.to_string()).unwrap_or_else(|| "null".into()),
        callback0 = callback0.map(|n| n.to_string()).unwrap_or_else(|| "null".into()),
        overlap = match (submit1, callback0) {
            (Some(s), Some(c)) if s <= c => "true",
            (Some(_), Some(_)) => "false",
            _ => "false",
        },
        stream_wall = stream.wall_ms,
        full = full,
        tail = tail,
    ))
}

fn event_ms(events: &[galion_tts_piper::StreamEvent], submit: bool, sentence: u32) -> Option<u32> {
    events.iter().find_map(|e| match e {
        galion_tts_piper::StreamEvent::Submit { sentence: s, ms } if submit && *s == sentence => {
            Some(*ms)
        }
        galion_tts_piper::StreamEvent::Callback { sentence: s, ms }
            if !submit && *s == sentence =>
        {
            Some(*ms)
        }
        _ => None,
    })
}

fn write_s16(path: &std::path::Path, pcm: &[i16]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    for s in pcm {
        f.write_all(&s.to_le_bytes())?;
    }
    Ok(())
}

fn missing(reason: &str) -> String {
    format!(
        "{{\"measured\": false, \"reason\": \"{}\"}}\n",
        json_str(reason)
    )
}

fn ms(d: std::time::Duration) -> u32 {
    d.as_millis().min(u128::from(u32::MAX)) as u32
}

fn median_u32(v: &mut [u32]) -> u32 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[v.len() / 2]
}

fn median_f32(v: &mut [f32]) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[v.len() / 2])
}

fn num_opt(v: Option<f32>) -> String {
    match v {
        Some(n) if n.is_finite() => format!("{n:.4}"),
        _ => "null".into(),
    }
}

fn json_u32s(v: &[u32]) -> String {
    let body = v
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

fn json_f32s(v: &[f32]) -> String {
    let body = v
        .iter()
        .map(|n| format!("{n:.4}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

fn json_str(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn cpu_model() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in s.lines() {
                if let Some(rest) = line.strip_prefix("model name") {
                    return rest
                        .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
                        .trim()
                        .to_string();
                }
            }
        }
    }
    "unknown".into()
}

fn loadavg() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "unknown".into())
}
