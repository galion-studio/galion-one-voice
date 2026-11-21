// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Boots the loopback server and drives it with a scripted ear.
//! The ear does not recognise speech. It repeats one line so the socket,
//! the 80 ms TTS pace, and the mood tick can be measured without Vosk.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use samantha_local_server::protocol::{
    decode_frame, encode_frame, AudioFrame, FLAG_END, FLAG_START, KIND_MIC, PROTOCOL,
};
use samantha_local_server::reply::{Pad, ReplyEngine, TurnOut};
use samantha_local_server::server::PipelineFactory;
use samantha_local_server::{start, FakePipeline, RefState, MIC_FRAME_SAMPLES, MIC_RATE, TTS_RATE};
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

    fn turn(&mut self, heard: &str) -> TurnOut {
        let started = Instant::now();
        let reply = format!("Okay. {heard}");
        let took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        TurnOut {
            reply,
            intent: "chat".into(),
            arm: "fake".into(),
            propensity: 0.4,
            evidence: vec!["card:fixture".into()],
            sources: vec!["fixture".into()],
            violations: 0,
            lookup_us: 7,
            took_us,
            mood: Pad {
                arousal: 0.8,
                valence: 0.7,
                dominance: 0.6,
            },
        }
    }
}

fn factory() -> PipelineFactory {
    Arc::new(|| Box::new(FakePipeline::scripted("hello from the mic", Fixed)))
}

fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}

struct HttpResp {
    status: u16,
    head: String,
    body: String,
    elapsed: Duration,
}

async fn http_get(addr: SocketAddr, path: &str) -> HttpResp {
    let started = Instant::now();
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
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

fn mic_bytes(sample: i16, seq: u32) -> Vec<u8> {
    encode_frame(&AudioFrame {
        kind: KIND_MIC,
        flags: 0,
        sample_rate: MIC_RATE,
        sequence: seq,
        pcm: vec![sample; MIC_FRAME_SAMPLES],
    })
    .unwrap()
}

type Sock = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn connect(addr: SocketAddr, origin: &str) -> Result<Sock, String> {
    let mut request = format!("ws://{addr}/v1/stream")
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

fn near(v: &Value, expected: f64) -> bool {
    v.as_f64()
        .map(|n| (n - expected).abs() < 1e-5)
        .unwrap_or(false)
}

fn p50(mut v: Vec<u128>) -> u128 {
    assert!(!v.is_empty());
    v.sort_unstable();
    v[v.len() / 2]
}

fn median_ms(mut v: Vec<u128>) -> u128 {
    assert!(!v.is_empty());
    v.sort_unstable();
    v[v.len() / 2]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refuses_non_loopback() {
    let ui = ui_dir();
    for raw in ["0.0.0.0:9", "192.168.1.20:8790", "[::1]:8790"] {
        let addr: SocketAddr = raw.parse().unwrap();
        let err = match start(addr, ui.clone(), factory(), RefState::Unset).await {
            Ok(_) => panic!("{raw} was accepted"),
            Err(err) => err,
        };
        let msg = err.to_string();
        assert!(msg.contains("127.0.0.1"), "{raw}: {msg}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_hides_ref_path() {
    let running = start(
        "127.0.0.1:0".parse().unwrap(),
        ui_dir(),
        factory(),
        RefState::Set,
    )
    .await
    .unwrap();
    let resp = http_get(running.addr, "/health").await;
    assert_eq!(resp.status, 200, "{}", resp.body);
    let json: Value = serde_json::from_str(&resp.body).unwrap();
    assert_eq!(json["samantha_ref"], "set");
    assert!(!resp.body.contains("wav"), "{}", resp.body);
    assert!(!resp.body.contains("G:"), "{}", resp.body);
    assert!(json["bind"].as_str().unwrap().starts_with("127.0.0.1:"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fake_pipeline_over_websocket() {
    let running = start(
        "127.0.0.1:0".parse().unwrap(),
        ui_dir(),
        factory(),
        RefState::Unset,
    )
    .await
    .unwrap();
    let addr = running.addr;
    assert!(addr.ip().is_loopback());

    let mut health_us = Vec::new();
    for i in 0..21 {
        let resp = http_get(addr, "/health").await;
        assert_eq!(resp.status, 200, "{}", resp.body);
        if i > 0 {
            health_us.push(resp.elapsed.as_micros());
        }
        if i == 0 {
            let json: Value = serde_json::from_str(&resp.body).unwrap();
            assert_eq!(json["ok"], true);
            assert_eq!(json["loopback"], true);
            assert_eq!(json["frame_ms"], 80);
            assert_eq!(json["mic_rate"], 16000);
            assert_eq!(json["tts_rate"], 22050);
            assert_eq!(json["protocol"], PROTOCOL);
            assert_eq!(json["samantha_ref"], "unset");
            assert_eq!(json["service"], "samantha-local");
            assert!(json["bind"].as_str().unwrap().starts_with("127.0.0.1:"));
            assert!(
                resp.head
                    .to_ascii_lowercase()
                    .contains("cache-control: no-store"),
                "{}",
                resp.head
            );
        }
    }
    let index = http_get(addr, "/").await;
    assert_eq!(index.status, 200, "{}", index.body);
    assert!(
        index.body.contains("data-mark=\"orb\""),
        "one-dot mark missing"
    );
    assert!(index.body.contains("Samantha"));
    let js = http_get(addr, "/app.js").await;
    assert_eq!(js.status, 200);
    assert!(js.body.contains("/v1/stream"));
    let css = http_get(addr, "/dot.css").await;
    assert_eq!(css.status, 200);
    assert!(css.body.contains("#0B0F14"));

    let denied = connect(addr, "https://evil.example").await;
    assert!(denied.is_err(), "foreign origin was upgraded: {denied:?}");

    let mut ws = connect(addr, "http://127.0.0.1")
        .await
        .expect("local origin");
    let ready = next_json(&mut ws, Duration::from_secs(2))
        .await
        .expect("ready");
    assert_eq!(ready["type"], "ready");
    assert_eq!(ready["protocol"], PROTOCOL);
    assert_eq!(ready["frame_ms"], 80);
    assert_eq!(ready["ear"], "scripted");
    assert_eq!(ready["mouth"], "hold");
    assert_eq!(ready["mind"], "fixed");
    assert!(near(&ready["mood_rest"]["arousal"], 0.3));
    assert!(near(&ready["mood_rest"]["valence"], 0.25));
    assert!(near(&ready["mood_rest"]["dominance"], 0.5));

    ws.send(Message::Text(r#"{"type":"ping","t_ms":42}"#.into()))
        .await
        .unwrap();
    let pong = next_json(&mut ws, Duration::from_secs(2))
        .await
        .expect("pong");
    assert_eq!(pong["type"], "pong");
    assert_eq!(pong["echo_ms"], 42);
    assert!(pong["t_ms"].as_u64().is_some());

    for n in 0..4 {
        ws.send(Message::Binary(mic_bytes(8000, n))).await.unwrap();
    }
    for n in 4..14 {
        ws.send(Message::Binary(mic_bytes(0, n))).await.unwrap();
    }
    let sent_end = Instant::now();

    let mut replies = Vec::new();
    let mut reply_at: Option<Instant> = None;
    let mut heard = Vec::new();
    let mut states = Vec::new();
    let mut errors = Vec::new();
    let mut moods: Vec<(Instant, Value)> = Vec::new();
    let mut tts: Vec<(Instant, AudioFrame)> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && (tts.len() < 4 || replies.is_empty()) {
        match recv(&mut ws, Duration::from_millis(200)).await {
            None => continue,
            Some(Message::Text(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap();
                match v["type"].as_str() {
                    Some("reply") => {
                        if reply_at.is_none() {
                            reply_at = Some(Instant::now());
                        }
                        replies.push(v);
                    }
                    Some("heard") => heard.push(v),
                    Some("state") => states.push(v),
                    Some("mood") => moods.push((Instant::now(), v)),
                    Some("error") => errors.push(v),
                    _ => {}
                }
            }
            Some(Message::Binary(bytes)) => {
                let frame = decode_frame(&bytes).expect("tts frame");
                tts.push((Instant::now(), frame));
            }
            Some(_) => {}
        }
    }
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0]["text"], "hello from the mic");
    assert_eq!(heard[0]["recognized"], false);
    assert_eq!(heard[0]["ear"], "scripted");
    assert_eq!(replies.len(), 1, "{replies:?}");
    let reply = &replies[0];
    assert_eq!(reply["text"], "Okay. hello from the mic");
    assert_eq!(reply["sources"][0], "fixture");
    assert_eq!(reply["lookup_us"], 7);
    assert_eq!(reply["mouth"], "hold");
    assert!(near(&reply["mood"]["arousal"], 0.8));
    assert!(near(&reply["mood"]["valence"], 0.7));
    assert!(near(&reply["mood"]["dominance"], 0.6));
    let reply_wall_us = reply_at
        .expect("reply time")
        .saturating_duration_since(sent_end)
        .as_micros();
    let took_us = reply["took_us"].as_u64().unwrap();
    let mouth_us = reply["mouth_us"].as_u64().unwrap();

    assert_eq!(tts.len(), 4, "paced frames: {}", tts.len());
    for (i, (_, frame)) in tts.iter().enumerate() {
        assert_eq!(frame.sample_rate, TTS_RATE);
        assert_eq!(frame.sequence, (i as u32) + 1);
        let start = frame.flags & FLAG_START != 0;
        let end = frame.flags & FLAG_END != 0;
        assert_eq!(start, i == 0);
        assert_eq!(end, i == 3);
        let peak = frame
            .pcm
            .iter()
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(peak > 1000, "slice {i} peak {peak}");
    }
    let mut gaps_ms = Vec::new();
    for pair in tts.windows(2) {
        gaps_ms.push(pair[1].0.saturating_duration_since(pair[0].0).as_millis());
    }
    // The first gap can be short: slice 0 is sent while the client is still
    // writing the tail, so it sits in the socket until the read loop starts.
    // Later gaps are the 80 ms pace.
    let tts_median_now = median_ms(gaps_ms.clone());
    assert!(
        gaps_ms.iter().any(|g| *g >= 70)
            && gaps_ms.iter().all(|g| *g <= 1000)
            && (60..=200).contains(&tts_median_now),
        "tts gaps ms {gaps_ms:?} median {tts_median_now}"
    );

    let mood_deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < mood_deadline
        && !moods
            .iter()
            .any(|(_, m)| m["arousal"].as_f64().unwrap_or(0.0) > 0.35)
    {
        if let Some(Message::Text(text)) = recv(&mut ws, Duration::from_millis(200)).await {
            let v: Value = serde_json::from_str(&text).unwrap();
            if v["type"] == "mood" {
                moods.push((Instant::now(), v));
            }
        }
    }
    assert!(
        moods
            .iter()
            .any(|(_, m)| m["arousal"].as_f64().unwrap_or(0.0) > 0.35),
        "mood did not leave rest: {moods:?}"
    );
    assert!(moods.iter().any(|(_, m)| m["frame_ms"] == 80));
    let mut mood_gaps = Vec::new();
    for pair in moods.windows(2) {
        mood_gaps.push(pair[1].0.saturating_duration_since(pair[0].0).as_millis());
    }

    ws.send(Message::Text(r#"{"type":"barge"}"#.into()))
        .await
        .unwrap();
    let mut barged = false;
    let barge_deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < barge_deadline && !barged {
        if let Some(Message::Text(text)) = recv(&mut ws, Duration::from_millis(200)).await {
            let v: Value = serde_json::from_str(&text).unwrap();
            if v["type"] == "state" && v["reason"] == "barge" {
                assert_eq!(v["state"], "idle");
                barged = true;
            }
        }
    }
    assert!(barged, "no barge state");

    let text_sent = Instant::now();
    ws.send(Message::Text(
        r#"{"type":"text","text":"typed line"}"#.into(),
    ))
    .await
    .unwrap();
    let mut typed = None;
    let mut typed_tts_at = None;
    let type_deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < type_deadline && (typed.is_none() || typed_tts_at.is_none()) {
        match recv(&mut ws, Duration::from_millis(200)).await {
            Some(Message::Text(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap();
                if v["type"] == "reply" {
                    typed = Some(v);
                }
            }
            Some(Message::Binary(bytes)) => {
                if typed_tts_at.is_none() {
                    let frame = decode_frame(&bytes).unwrap();
                    assert_eq!(frame.sample_rate, TTS_RATE);
                    typed_tts_at = Some(text_sent.elapsed().as_micros());
                }
            }
            _ => {}
        }
    }
    let typed = typed.expect("typed reply");
    assert_eq!(typed["text"], "Okay. typed line");
    let typed_tts_us = typed_tts_at.expect("typed tts");

    assert!(
        states.iter().any(|s| s["state"] == "listening"),
        "{states:?}"
    );
    assert!(
        states.iter().any(|s| s["state"] == "speaking"),
        "{states:?}"
    );

    let health_min = *health_us.iter().min().unwrap();
    let health_p50 = p50(health_us.clone());
    let tts_median = median_ms(gaps_ms.clone());
    let mood_median = if mood_gaps.is_empty() {
        0
    } else {
        median_ms(mood_gaps.clone())
    };
    let line = format!(
        "MEASURE health_n={} health_min_us={} health_p50_us={} reply_wall_us={} took_us={} mouth_us={} tts_gaps_ms={:?} tts_gap_median_ms={} mood_n={} mood_gaps_ms={:?} mood_gap_median_ms={} typed_first_tts_us={}",
        health_us.len(),
        health_min,
        health_p50,
        reply_wall_us,
        took_us,
        mouth_us,
        gaps_ms,
        tts_median,
        moods.len(),
        mood_gaps,
        mood_median,
        typed_tts_us
    );
    eprintln!("{line}");
    let measure_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/s10-measure.txt");
    if let Some(parent) = measure_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&measure_path, line).expect("write measure");
}

async fn next_json(ws: &mut Sock, timeout: Duration) -> Option<Value> {
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
