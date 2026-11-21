// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Boots the loopback server on the galion-samantha-ws/1 socket and drives a
//! scripted brain. The ear does not recognise speech.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use samantha_local_server::protocol::{
    decode_pcm, encode_pcm, AudioDir, RateCode, SCHEMA, WS_PATH,
};
use samantha_local_server::reply::{Pad, ReplyEngine, TurnOut};
use samantha_local_server::server::PipelineFactory;
use samantha_local_server::{start, FakePipeline, RefState, SPEAKER_RATE};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

struct Fixed;

impl ReplyEngine for Fixed {
    fn name(&self) -> &str {
        "fixed"
    }
    fn turn(&mut self, _heard: &str) -> Result<TurnOut, String> {
        let started = Instant::now();
        Ok(TurnOut {
            reply: "The fence is loose.".into(),
            intent_json: "null".into(),
            arm_json: "null".into(),
            propensity: 0.0,
            evidence: vec!["card:fixture".into()],
            sources: vec!["fixture".into()],
            violations: 0,
            lookup_us: 7,
            took_us: u32::try_from(started.elapsed().as_micros()).unwrap_or(0),
            mood: Pad::RESTING,
        })
    }
}

fn factory() -> PipelineFactory {
    Arc::new(|| Box::new(FakePipeline::scripted("hello from the mic", Fixed)))
}

fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}

async fn boot(addr: &str) -> samantha_local_server::Running {
    start(
        addr.parse().unwrap(),
        ui_dir(),
        factory(),
        RefState::Unset,
        false,
        Duration::from_secs(120),
    )
    .await
    .unwrap()
}

struct HttpResp {
    status: u16,
    head: String,
    body: String,
    elapsed: Duration,
}

async fn http(addr: SocketAddr, method: &str, path: &str, body: &str) -> HttpResp {
    let started = Instant::now();
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let host = match addr.ip() {
        std::net::IpAddr::V4(_) => "127.0.0.1".to_string(),
        std::net::IpAddr::V6(_) => "[::1]".to_string(),
    };
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    let elapsed = started.elapsed();
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut parts = text.splitn(2, "\r\n\r\n");
    let head = parts.next().unwrap_or("").to_string();
    let body = parts.next().unwrap_or("").to_string();
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);
    HttpResp {
        status,
        head,
        body,
        elapsed,
    }
}

fn mic_bytes(sample: i16, seq: u16) -> Vec<u8> {
    encode_pcm(AudioDir::Mic, RateCode::Hz16k, seq, &vec![sample; 320]).unwrap()
}

type Sock = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn connect(addr: SocketAddr, origin: &str) -> Result<Sock, String> {
    let host = match addr.ip() {
        std::net::IpAddr::V4(ip) => format!("{ip}"),
        std::net::IpAddr::V6(_) => "[::1]".into(),
    };
    let mut request = format!(
        "ws://{host}:{}/{}",
        addr.port(),
        WS_PATH.trim_start_matches('/')
    )
    .into_client_request()
    .map_err(|e| e.to_string())?;
    let value = origin.parse().map_err(
        |e: tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue| e.to_string(),
    )?;
    request.headers_mut().insert("Origin", value);
    match connect_async(request).await {
        Ok((ws, _)) => Ok(ws),
        Err(e) => Err(e.to_string()),
    }
}

async fn recv(ws: &mut Sock, timeout: Duration) -> Option<Message> {
    match tokio::time::timeout(timeout, ws.next()).await {
        Ok(Some(Ok(msg))) => Some(msg),
        _ => None,
    }
}

fn median(mut v: Vec<u128>) -> u128 {
    v.sort_unstable();
    v[v.len() / 2]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refuses_non_loopback_and_accepts_ipv6_loopback() {
    let ui = ui_dir();
    for raw in ["0.0.0.0:9", "192.168.1.20:7881"] {
        let addr: SocketAddr = raw.parse().unwrap();
        let err = match start(
            addr,
            ui.clone(),
            factory(),
            RefState::Unset,
            false,
            Duration::from_secs(120),
        )
        .await
        {
            Ok(_) => panic!("{raw} was accepted"),
            Err(err) => err,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("127.0.0.1") || msg.contains("::1"),
            "{raw}: {msg}"
        );
    }
    let v6 = boot("[::1]:0").await;
    assert!(v6.addr.ip().is_loopback());
    let health = http(v6.addr, "GET", "/health", "").await;
    assert_eq!(health.status, 200, "{}", health.body);
    assert!(health.body.contains("\"ok\":true"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn companion_socket() {
    let running = boot("127.0.0.1:0").await;
    let addr = running.addr;
    assert!(addr.ip().is_loopback());

    let mut health_us = Vec::new();
    for i in 0..21 {
        let resp = http(addr, "GET", "/health", "").await;
        assert_eq!(resp.status, 200, "{}", resp.body);
        if i == 0 {
            assert!(resp.body.contains("\"ok\":true"));
            assert!(resp.body.contains(SCHEMA));
            assert!(resp.body.contains("\"loopback\":true"));
            assert!(resp.body.contains("\"samantha_ref\":\"unset\""));
            assert!(resp
                .head
                .to_ascii_lowercase()
                .contains("cache-control: no-store"));
            assert!(!resp.body.contains("wav"));
        } else {
            health_us.push(resp.elapsed.as_micros());
        }
    }
    let index = http(addr, "GET", "/", "").await;
    assert_eq!(index.status, 200);
    assert!(index.body.contains("Galion One"));
    assert!(index.body.contains("Ask anything"));
    assert!(index.body.contains("data-mark=\"orb\""));
    assert!(index.body.contains("/v1/dot"));
    assert!(index.body.contains("PREVIEW_ONLY=false"));
    let dot_page = http(addr, "GET", "/dot/", "").await;
    assert_eq!(dot_page.status, 200, "{}", dot_page.body);
    assert!(dot_page.body.contains("app.js"));
    let js = http(addr, "GET", "/dot/app.js", "").await;
    assert!(js.body.contains("/v1/samantha"));
    assert!(js.body.contains("#F5F5F7"));
    let css = http(addr, "GET", "/dot/dot.css", "").await;
    assert!(css.body.contains("#0A0A0C"));
    let icon = http(addr, "GET", "/favicon.ico", "").await;
    assert_eq!(icon.status, 200, "{}", icon.head);
    assert!(icon.head.to_ascii_lowercase().contains("image/x-icon"));
    assert!(js.body.contains("audioWorklet"));
    assert!(!js.body.contains("createScriptProcessor"));

    let session = http(addr, "POST", "/session", "{\"op\":\"open\"}").await;
    assert_eq!(session.status, 200, "{}", session.body);
    let opened: Value = serde_json::from_str(&session.body).unwrap();
    assert_eq!(opened["transport"], "body");
    assert_eq!(opened["ttl_s"], 120);
    assert!(opened["token"].as_str().unwrap().len() >= 16);
    let leaked = http(addr, "GET", "/v1/samantha?token=nope", "").await;
    assert_eq!(leaked.status, 400, "{}", leaked.body);

    let denied = connect(addr, "https://evil.example").await;
    assert!(denied.is_err(), "{denied:?}");

    let mut ws = connect(addr, "http://127.0.0.1:7878")
        .await
        .expect("one-dot origin");
    ws.send(Message::Text(format!(
        r#"{{"schema":"{SCHEMA}","type":"hello","session":null}}"#
    )))
    .await
    .unwrap();
    let welcome = next_text(&mut ws, Duration::from_secs(2))
        .await
        .expect("welcome");
    assert_eq!(welcome["type"], "welcome");
    assert_eq!(welcome["schema"], SCHEMA);
    assert_eq!(welcome["frame_ms"], 80);
    assert_eq!(welcome["sample_rate"], 24000);
    assert_eq!(welcome["dot"], "idle");

    ws.send(Message::Text(format!(
        r#"{{"schema":"{SCHEMA}","type":"hello","session":null}}"#
    )))
    .await
    .unwrap();
    let again = Instant::now() + Duration::from_millis(400);
    while Instant::now() < again {
        if let Some(Message::Text(text)) = recv(&mut ws, Duration::from_millis(100)).await {
            let v: Value = serde_json::from_str(&text).unwrap();
            assert_ne!(
                v["type"].as_str(),
                Some("welcome"),
                "a second hello is not a second welcome"
            );
        }
    }

    ws.send(Message::Text(format!(
        r#"{{"schema":"{SCHEMA}","type":"ping"}}"#
    )))
    .await
    .unwrap();
    let pong = next_text(&mut ws, Duration::from_secs(2))
        .await
        .expect("pong");
    assert_eq!(pong["type"], "pong");
    assert_eq!(pong["schema"], SCHEMA);

    for n in 0..4u16 {
        ws.send(Message::Binary(mic_bytes(8000, n))).await.unwrap();
    }
    for n in 4..36u16 {
        ws.send(Message::Binary(mic_bytes(0, n))).await.unwrap();
    }
    let sent_end = Instant::now();

    let mut turns = Vec::new();
    let mut turn_at = None;
    let mut moods: Vec<(Instant, Value)> = Vec::new();
    let mut pcm: Vec<(Instant, (AudioDir, RateCode, u16, Vec<i16>))> = Vec::new();
    let mut errors = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && (pcm.len() < 4 || turns.is_empty()) {
        match recv(&mut ws, Duration::from_millis(200)).await {
            Some(Message::Text(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap();
                match v["type"].as_str() {
                    Some("turn") => {
                        if turn_at.is_none() {
                            turn_at = Some(Instant::now());
                        }
                        turns.push(v);
                    }
                    Some("mood") => moods.push((Instant::now(), v)),
                    Some("error") => errors.push(v),
                    _ => {}
                }
            }
            Some(Message::Binary(bytes)) => {
                pcm.push((Instant::now(), decode_pcm(&bytes).unwrap()));
            }
            _ => {}
        }
    }
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(turns.len(), 1, "{turns:?}");
    assert_eq!(turns[0]["reply"], "The fence is loose.");
    assert_eq!(turns[0]["intent"], Value::Null);
    assert_eq!(turns[0]["arm"], Value::Null);
    assert_eq!(turns[0]["sources"][0], "fixture");
    assert_eq!(turns[0]["lookup_us"], 7);
    assert!(turns[0]["mood"]["valence"].as_f64().unwrap() - 0.25 < 1e-6);
    let reply_wall_us = turn_at
        .unwrap()
        .saturating_duration_since(sent_end)
        .as_micros();
    let took_us = turns[0]["took_us"].as_u64().unwrap();

    assert_eq!(pcm.len(), 4, "slices {}", pcm.len());
    for (i, (_, (dir, rate, seq, samples))) in pcm.iter().enumerate() {
        assert_eq!(*dir, AudioDir::Speaker);
        assert_eq!(rate.hz(), SPEAKER_RATE);
        assert_eq!(*seq, (i as u16) + 1);
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        assert!(peak > 1000, "slice {i} peak {peak}");
    }
    let gaps_ms: Vec<u128> = pcm
        .windows(2)
        .map(|w| w[1].0.saturating_duration_since(w[0].0).as_millis())
        .collect();
    let gap_median = median(gaps_ms.clone());
    assert!(
        gaps_ms.iter().any(|g| *g >= 70)
            && gaps_ms.iter().all(|g| *g <= 1000)
            && (60..=200).contains(&gap_median),
        "tts gaps {gaps_ms:?}"
    );

    let mood_deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < mood_deadline && moods.len() < 3 {
        if let Some(Message::Text(text)) = recv(&mut ws, Duration::from_millis(200)).await {
            let v: Value = serde_json::from_str(&text).unwrap();
            if v["type"] == "mood" {
                moods.push((Instant::now(), v));
            }
        }
    }
    assert!(moods.len() >= 2, "{moods:?}");
    for (_, m) in &moods {
        assert_eq!(m["source"], "resting");
        assert!((m["pad"]["arousal"].as_f64().unwrap() - 0.3).abs() < 1e-5);
        assert!((m["pad"]["valence"].as_f64().unwrap() - 0.25).abs() < 1e-5);
        assert!((m["pad"]["dominance"].as_f64().unwrap() - 0.5).abs() < 1e-5);
    }
    let mood_gaps: Vec<u128> = moods
        .windows(2)
        .map(|w| w[1].0.saturating_duration_since(w[0].0).as_millis())
        .collect();
    let mood_median = if mood_gaps.is_empty() {
        0
    } else {
        median(mood_gaps.clone())
    };

    ws.send(Message::Text(format!(
        r#"{{"schema":"{SCHEMA}","type":"stop"}}"#
    )))
    .await
    .unwrap();
    let mut stopped = false;
    let stop_deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < stop_deadline && !stopped {
        if let Some(Message::Text(text)) = recv(&mut ws, Duration::from_millis(200)).await {
            let v: Value = serde_json::from_str(&text).unwrap();
            if v["type"] == "dot" && v["state"] == "idle" {
                stopped = true;
            }
        }
    }
    assert!(stopped, "stop did not return the dot to idle");

    let text_sent = Instant::now();
    ws.send(Message::Text(format!(
        r#"{{"schema":"{SCHEMA}","type":"text","text":"typed line"}}"#
    )))
    .await
    .unwrap();
    let mut typed = None;
    let mut typed_pcm_us = None;
    let type_deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < type_deadline && (typed.is_none() || typed_pcm_us.is_none()) {
        match recv(&mut ws, Duration::from_millis(200)).await {
            Some(Message::Text(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap();
                if v["type"] == "turn" {
                    typed = Some(v);
                }
            }
            Some(Message::Binary(bytes)) => {
                if typed_pcm_us.is_none() {
                    let (_, rate, _, _) = decode_pcm(&bytes).unwrap();
                    assert_eq!(rate.hz(), SPEAKER_RATE);
                    typed_pcm_us = Some(text_sent.elapsed().as_micros());
                }
            }
            _ => {}
        }
    }
    let typed = typed.expect("typed turn");
    assert_eq!(typed["reply"], "The fence is loose.");
    assert!(!typed.to_string().contains("typed line"));
    let typed_pcm_us = typed_pcm_us.expect("typed pcm");

    let line = format!(
        "MEASURE health_n={} health_min_us={} health_p50_us={} reply_wall_us={} took_us={} tts_gaps_ms={:?} tts_gap_median_ms={} mood_n={} mood_gaps_ms={:?} mood_gap_median_ms={} typed_first_pcm_us={}",
        health_us.len(),
        health_us.iter().min().copied().unwrap_or(0),
        median(health_us),
        reply_wall_us,
        took_us,
        gaps_ms,
        gap_median,
        moods.len(),
        mood_gaps,
        mood_median,
        typed_pcm_us
    );
    eprintln!("{line}");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/s10-measure.txt");
    let _ = std::fs::write(path, line);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn galion_one_text_returns_speaker_audio() {
    let running = boot("127.0.0.1:0").await;
    let said = http(
        running.addr,
        "POST",
        "/v2/say",
        "{\"text\":\"hello from the page\"}",
    )
    .await;
    assert_eq!(said.status, 200, "{}", said.body);
    assert!(said.body.contains("The fence is loose."));
    assert!(!said.body.contains("brain-unlinked"));
    let dot = http(running.addr, "GET", "/v1/dot", "").await;
    assert_eq!(dot.status, 200, "{}", dot.body);
    let state: Value = serde_json::from_str(&dot.body).expect("dot json");
    assert!(state["radius"].as_f64().is_some());
    assert_eq!(state["vad"]["ready"], true);
    let url = state["reply"]["url"].as_str().expect("reply url");
    let wav = http(running.addr, "GET", url, "").await;
    assert_eq!(wav.status, 200, "{}", wav.head);
    assert!(wav.body.starts_with("RIFF"), "speaker body was not a wav");
}

async fn next_text(ws: &mut Sock, timeout: Duration) -> Option<Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let left = deadline.saturating_duration_since(Instant::now());
        match recv(ws, left.min(Duration::from_millis(200))).await {
            Some(Message::Text(text)) => {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    return Some(v);
                }
            }
            Some(_) => continue,
            None => continue,
        }
    }
    None
}
