// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Reply-latency harness. Prints one JSON object of times this process measured.
//!
//! The model is a loopback stub. It is not a paid call and it is not the live
//! Hermes process. Piper is measured only when the A21 pack is on disk.
//! Missing stages are null. This program does not invent a latency.

use galion_cascade::{HermesLlm, LLM_NAME};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const STUB_GAP_MS: u64 = 300;
const FIRST: &str = "She died this morning.";
const SECOND: &str = " I will stay with that.";

fn main() {
    let gap = Duration::from_millis(STUB_GAP_MS);
    let url = spawn_stub(gap);
    let llm = HermesLlm::new(&url, LLM_NAME, "", Duration::from_secs(5));

    let t0 = Instant::now();
    let buffered = llm.complete("My mother died this morning.");
    let buffered_llm_ms = ms(t0.elapsed());
    let buffered_ok = buffered.contains("She died") && buffered.contains("stay");

    let t1 = Instant::now();
    let mut first_sentence = None;
    let streamed = llm.complete_stream("My mother died this morning.", &mut |delta| {
        if first_sentence.is_none() && delta.contains('.') {
            first_sentence = Some(ms(t1.elapsed()));
        }
    });
    let stream_done_ms = ms(t1.elapsed());
    let stream_ok = streamed.contains("She died") && streamed.contains("stay");
    let stream_first_ms = first_sentence.unwrap_or(stream_done_ms);

    let asr = measure_asr();
    let mut piper = measure_piper();
    let buffered_audio = piper
        .as_mut()
        .and_then(|session| e2e_buffered(&llm, session));
    let stream_audio = piper.as_mut().and_then(|session| e2e_stream(&llm, session));

    let measured = buffered_ok && stream_ok && stream_first_ms + 100.0 < buffered_llm_ms;
    let root = repo_root();
    let piper_block = match &piper {
        Some(p) => format!(
            "{{\"measured\":true,\"runtime\":{},\"cold_spawn_to_pcm_ms\":{:.3},\"warm_write_to_pcm_ms\":{:.3},\"warm_samples_ms\":{},\"n_warm\":{}}}",
            json_str(&p.runtime),
            p.cold_ms,
            p.warm_median_ms,
            json_nums(&p.warm_samples_ms),
            p.warm_samples_ms.len()
        ),
        None => "{\"measured\":false,\"runtime\":null,\"cold_spawn_to_pcm_ms\":null,\"warm_write_to_pcm_ms\":null,\"warm_samples_ms\":null,\"n_warm\":0,\"reason\":\"A21 Piper pack is not on disk\"}".to_string(),
    };
    let note = "Loopback stub on this CPU. Not a paid model and not a live Hermes process. \
The stub holds the second sentence for stub_gap_ms. buffered_llm_ms waits for that hold. \
stream_first_sentence_ms is the first sentence mark. Piper fields are this process. null means that stage was not run.";
    println!(
        "{{\n  \"measured\": {measured},\n  \"paid\": false,\n  \"persona\": \"samantha\",\n  \"host\": {host},\n  \"repo\": {repo},\n  \"asr\": {asr},\n  \"hermes\": {{\n    \"transport\": \"loopback-stub\",\n    \"stub_gap_ms\": {STUB_GAP_MS},\n    \"buffered_llm_ms\": {buffered_llm_ms:.3},\n    \"stream_first_sentence_ms\": {stream_first_ms:.3},\n    \"stream_done_ms\": {stream_done_ms:.3},\n    \"buffered_text_ok\": {buffered_ok},\n    \"stream_text_ok\": {stream_ok}\n  }},\n  \"piper\": {piper_block},\n  \"reply\": {{\n    \"buffered_first_audio_ms\": {buf_a},\n    \"stream_first_audio_ms\": {str_a},\n    \"note\": \"One clock each: stub then warm Piper first PCM. null when Piper was not measured.\"\n  }},\n  \"note\": {note_json}\n}}",
        measured = measured,
        host = json_str(&host_line()),
        repo = json_str(&root.display().to_string()),
        asr = asr,
        buf_a = num_or_null(buffered_audio),
        str_a = num_or_null(stream_audio),
        note_json = json_str(note),
    );
    if !measured {
        std::process::exit(1);
    }
}

struct PiperSession {
    runtime: String,
    cold_ms: f64,
    warm_median_ms: f64,
    warm_samples_ms: Vec<f64>,
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    gen: Arc<AtomicUsize>,
    pcm_at: Arc<Mutex<Vec<(usize, Instant)>>>,
    rtf: Arc<AtomicUsize>,
    last_out: Arc<Mutex<Instant>>,
}

impl Drop for PiperSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn measure_piper() -> Option<PiperSession> {
    let (bin, model) = piper_paths()?;
    let started = Instant::now();
    let gen = Arc::new(AtomicUsize::new(0));
    let pcm_at = Arc::new(Mutex::new(Vec::new()));
    let rtf = Arc::new(AtomicUsize::new(0));
    let last_out = Arc::new(Mutex::new(Instant::now()));
    let gen_r = gen.clone();
    let pcm_r = pcm_at.clone();
    let last_r = last_out.clone();
    let rtf_r = rtf.clone();
    let mut child = Command::new(&bin)
        .arg("--model")
        .arg(&model)
        .args([
            "--json-input",
            "--output_raw",
            "--sentence_silence",
            "0",
            "--noise_scale",
            "0.667",
            "--noise_w",
            "0.8",
            "--length_scale",
            "1.0",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let stdin = child.stdin.take()?;
    let mut stdout = child.stdout.take()?;
    let mut stderr = child.stderr.take()?;
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut reported = usize::MAX;
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    *last_r.lock().unwrap() = Instant::now();
                    let g = gen_r.load(Ordering::SeqCst);
                    if reported != g {
                        reported = g;
                        pcm_r.lock().unwrap().push((g, Instant::now()));
                    }
                }
            }
        }
    });
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let mut acc = String::new();
        loop {
            match stderr.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                    let n_rtf = acc.matches("Real-time factor").count();
                    rtf_r.store(n_rtf, Ordering::SeqCst);
                }
            }
        }
    });
    let mut session = PiperSession {
        runtime: bin.display().to_string(),
        cold_ms: 0.0,
        warm_median_ms: 0.0,
        warm_samples_ms: Vec::new(),
        child,
        stdin,
        gen,
        pcm_at,
        rtf,
        last_out: last_out.clone(),
    };
    let cold = session.say_until_pcm("Hello.", started)?;
    session.cold_ms = cold;
    session.wait_quiet();
    let mut warm = Vec::new();
    for text in ["I hear you.", "Okay.", "Yes.", "Thanks.", "Good."] {
        let t = Instant::now();
        session.gen.fetch_add(1, Ordering::SeqCst);
        let ms = session.say_until_pcm(text, t)?;
        warm.push(ms);
        session.wait_quiet();
    }
    warm.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    session.warm_median_ms = warm[warm.len() / 2];
    session.warm_samples_ms = warm;
    Some(session)
}

impl PiperSession {
    fn say_until_pcm(&mut self, text: &str, t0: Instant) -> Option<f64> {
        let g = self.gen.load(Ordering::SeqCst);
        let line = format!("{{\"text\":\"{text}\"}}\n");
        self.stdin.write_all(line.as_bytes()).ok()?;
        self.stdin.flush().ok()?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if Instant::now() > deadline {
                return None;
            }
            if let Some((_, at)) = self.pcm_at.lock().unwrap().iter().find(|(n, _)| *n == g) {
                return Some(ms(at.duration_since(t0)));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn wait_quiet(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let need = self.gen.load(Ordering::SeqCst) + 1;
        while self.rtf.load(Ordering::SeqCst) < need && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let quiet_deadline = Instant::now() + Duration::from_millis(400);
        while Instant::now() < quiet_deadline {
            let idle = self.last_out.lock().unwrap().elapsed() > Duration::from_millis(40);
            if idle {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn write_line(&mut self, text: &str) -> Option<()> {
        let line = format!(
            "{{\"text\":\"{}\"}}\n",
            text.replace('\\', " ").replace('"', " ")
        );
        self.stdin.write_all(line.as_bytes()).ok()?;
        self.stdin.flush().ok()
    }
}

fn e2e_buffered(llm: &HermesLlm, piper: &mut PiperSession) -> Option<f64> {
    piper.wait_quiet();
    let t = Instant::now();
    let text = llm.complete("My mother died this morning.");
    if !text.contains("She died") {
        return None;
    }
    piper.gen.fetch_add(1, Ordering::SeqCst);
    piper.write_line(FIRST)?;
    let g = piper.gen.load(Ordering::SeqCst);
    let at = wait_pcm(&piper.pcm_at, g)?;
    Some(ms(at.duration_since(t)))
}

fn e2e_stream(llm: &HermesLlm, piper: &mut PiperSession) -> Option<f64> {
    piper.wait_quiet();
    let t = Instant::now();
    let mut hit: Option<Instant> = None;
    let text = llm.complete_stream("My mother died this morning.", &mut |delta| {
        if hit.is_none() && delta.contains('.') {
            piper.gen.fetch_add(1, Ordering::SeqCst);
            let g = piper.gen.load(Ordering::SeqCst);
            if piper.write_line(delta.trim()).is_some() {
                hit = wait_pcm(&piper.pcm_at, g);
            }
        }
    });
    if !text.contains("She died") {
        return None;
    }
    hit.map(|at| ms(at.duration_since(t)))
}

fn wait_pcm(pcm_at: &Mutex<Vec<(usize, Instant)>>, g: usize) -> Option<Instant> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if Instant::now() > deadline {
            return None;
        }
        if let Some((_, at)) = pcm_at.lock().unwrap().iter().find(|(n, _)| *n == g) {
            return Some(*at);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn measure_asr() -> String {
    use galion_cascade::{AsrMode, PhoneTurnAsr};
    let pcm = galion_cascade::stub_fixture_pcm16("hello how are you");
    let mut asr = PhoneTurnAsr::new(AsrMode::Stub);
    let mut samples = Vec::new();
    for _ in 0..21 {
        let t = Instant::now();
        let (text, src) = asr.transcribe(&pcm).expect("stub fixture");
        assert_eq!(text, "hello how are you");
        assert_eq!(src.id(), "stub-fixture");
        samples.push(ms(t.elapsed()));
    }
    samples.remove(0);
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = samples[samples.len() / 2];
    let vosk = repo_root().join("weights/asr/vosk-model-small-en-us-0.15");
    let vosk_note = if vosk.is_dir() {
        "vosk model directory is present; this row is still the stub fixture, not a Vosk decode"
    } else {
        "vosk model is not on disk; this row is the stub fixture decode, not speech recognition"
    };
    format!(
        "{{\"engine\":\"stub-fixture\",\"measured\":true,\"n\":{},\"median_ms\":{:.3},\"min_ms\":{:.3},\"max_ms\":{:.3},\"note\":{}}}",
        samples.len(),
        median,
        samples[0],
        samples[samples.len() - 1],
        json_str(vosk_note)
    )
}

fn spawn_stub(gap: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for _ in 0..8 {
            let Ok((mut sock, _)) = listener.accept() else {
                break;
            };
            let req = read_http(&mut sock);
            if req.contains("\"stream\":true") {
                let _ = sock.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                );
                let _ = sock.write_all(sse(FIRST).as_bytes());
                let _ = sock.flush();
                std::thread::sleep(gap);
                let _ = sock.write_all(sse(SECOND).as_bytes());
                let _ = sock.write_all(b"data: [DONE]\n\n");
                let _ = sock.flush();
            } else {
                std::thread::sleep(gap);
                let body = format!(
                    "{{\"choices\":[{{\"message\":{{\"content\":{}}}}}]}}",
                    json_str(&format!("{FIRST}{SECOND}"))
                );
                let resp = format!(
                    "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes());
            }
        }
    });
    format!("http://{addr}/v1/chat/completions")
}

fn sse(text: &str) -> String {
    format!(
        "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\n",
        json_str(text)
    )
}

fn read_http(sock: &mut std::net::TcpStream) -> String {
    let _ = sock.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; 8192];
    let mut got = Vec::new();
    loop {
        match sock.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                got.extend_from_slice(&buf[..n]);
                if let Some(end) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&got[..end]);
                    let len = head.lines().find_map(|line| {
                        let (k, v) = line.split_once(':')?;
                        if k.eq_ignore_ascii_case("content-length") {
                            v.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    });
                    if got.len() >= end + 4 + len.unwrap_or(0) {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&got).into_owned()
}

fn piper_paths() -> Option<(PathBuf, PathBuf)> {
    let root = repo_root();
    let bin = std::env::var("GALION_A21_RUNTIME")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            let legacy = root.join("weights/runtime/piper/piper");
            if legacy.is_file() {
                Some(legacy)
            } else {
                None
            }
        })?;
    let model = std::env::var("GALION_TTS_MODEL")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            let model = root.join("weights/tts/en_US-ljspeech-medium.onnx");
            if model.is_file() {
                Some(model)
            } else {
                None
            }
        })?;
    Some((bin, model))
}

fn repo_root() -> PathBuf {
    std::env::var("GALION_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn host_line() -> String {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let uname = Command::new("uname").args(["-sr"]).output();
    let kernel = uname
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    format!("{kernel} nproc={n}")
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if c.is_control() => o.push(' '),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn json_nums(v: &[f64]) -> String {
    let parts: Vec<String> = v.iter().map(|n| format!("{n:.3}")).collect();
    format!("[{}]", parts.join(","))
}

fn num_or_null(v: Option<f64>) -> String {
    match v {
        Some(n) => format!("{n:.3}"),
        None => "null".into(),
    }
}
