// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! A mic take whose brain turn is held must not pin the runtime worker.
//!
//! Open frames stay on the session task. Only the closing frame's brain
//! runs on the blocking pool. The gates below are failure fuses. The
//! success path wakes on a channel or a socket read. It does not sleep,
//! and it does not change the 640 ms hang.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use samantha_local_server::protocol::{encode_pcm, AudioDir, RateCode, SCHEMA, WS_PATH};
use samantha_local_server::reply::{Pad, ReplyEngine, TurnOut};
use samantha_local_server::server::PipelineFactory;
use samantha_local_server::{start, FakePipeline, Pipeline, RefState, MIC_FRAME_SAMPLES};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Sock = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Upper bound for a step that should already be ready. A blocked worker
/// trips this on the client runtime, which then releases the gate.
const FUSE: Duration = Duration::from_secs(3);

/// Two frames at this level open the energy gate.
const LOUD: i16 = 8_000;
/// Quiet frames after the open. 32 × 20 ms is the 640 ms hang.
const HANG_FRAMES: u16 = 32;

struct Hold {
    entered: std::sync::mpsc::Sender<()>,
    release: Arc<Mutex<Option<std::sync::mpsc::Receiver<()>>>>,
}

impl ReplyEngine for Hold {
    fn name(&self) -> &str {
        "hold"
    }

    fn turn(&mut self, _heard: &str) -> Result<TurnOut, String> {
        let _ = self.entered.send(());
        let rx = self
            .release
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();
        if let Some(rx) = rx {
            let _ = rx.recv();
        }
        Ok(held_turn())
    }
}

fn held_turn() -> TurnOut {
    TurnOut {
        reply: "The fence is loose.".into(),
        intent_json: "null".into(),
        arm_json: "null".into(),
        propensity: 0.0,
        evidence: Vec::new(),
        sources: Vec::new(),
        violations: 1,
        lookup_us: 0,
        took_us: 1,
        mood: Pad::RESTING,
        empathy_e: 0.0,
        prosody_rate: 1.0,
        prosody_energy: 1.0,
        prosody_pitch_st: 0.0,
        prosody_pause_ms: 0.0,
        first_clause: String::new(),
        rest: String::new(),
    }
}

struct Release(Option<std::sync::mpsc::Sender<()>>);

impl Drop for Release {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}

fn hello() -> String {
    format!(r#"{{"schema":"{SCHEMA}","type":"hello","session":null}}"#)
}

fn ping() -> String {
    format!(r#"{{"schema":"{SCHEMA}","type":"ping"}}"#)
}

fn mic_frame(sample: i16, seq: u16) -> Result<Vec<u8>, String> {
    let pcm = [sample; MIC_FRAME_SAMPLES];
    encode_pcm(AudioDir::Mic, RateCode::Hz16k, seq, &pcm).map_err(|err| err.to_string())
}

async fn connect(addr: SocketAddr) -> Result<Sock, String> {
    let url = format!("ws://{addr}{WS_PATH}");
    match tokio::time::timeout(FUSE, connect_async(url)).await {
        Ok(Ok((ws, _))) => Ok(ws),
        Ok(Err(err)) => Err(format!("{err}")),
        Err(_) => Err("connect timed out".into()),
    }
}

async fn send_text(ws: &mut Sock, text: &str) -> Result<(), String> {
    match tokio::time::timeout(FUSE, ws.send(Message::Text(text.into()))).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(format!("{err}")),
        Err(_) => Err("send timed out".into()),
    }
}

async fn send_bin(ws: &mut Sock, bytes: Vec<u8>) -> Result<(), String> {
    match tokio::time::timeout(FUSE, ws.send(Message::Binary(bytes))).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(format!("{err}")),
        Err(_) => Err("send timed out".into()),
    }
}

async fn recv(ws: &mut Sock, bound: Duration) -> Option<Message> {
    match tokio::time::timeout(bound, ws.next()).await {
        Ok(Some(Ok(msg))) => Some(msg),
        _ => None,
    }
}

async fn until_type(ws: &mut Sock, kind: &str, bound: Duration) -> Result<Vec<Value>, String> {
    let deadline = Instant::now() + bound;
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        let left = deadline.saturating_duration_since(Instant::now());
        let Some(msg) = recv(ws, left).await else {
            break;
        };
        let Message::Text(body) = msg else {
            continue;
        };
        let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        let hit = value.get("type").and_then(Value::as_str) == Some(kind);
        seen.push(value);
        if hit {
            return Ok(seen);
        }
    }
    Err(format!("no {kind} frame"))
}

fn has_type(frames: &[Value], kind: &str) -> bool {
    frames
        .iter()
        .any(|value| value.get("type").and_then(Value::as_str) == Some(kind))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn held_mic_take_keeps_the_worker_free() {
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release = Arc::new(Mutex::new(Some(release_rx)));
    let factory: PipelineFactory = Arc::new(move || {
        Box::new(FakePipeline::scripted(
            "northwind",
            Hold {
                entered: entered_tx.clone(),
                release: Arc::clone(&release),
            },
        )) as Box<dyn Pipeline>
    });
    let running = start(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        ui_dir(),
        factory,
        RefState::Unset,
        false,
        Duration::from_secs(120),
    )
    .await
    .unwrap();
    let addr = running.addr;
    let client = std::thread::spawn(move || drive_held_take(addr, entered_rx, release_tx));
    let report = tokio::task::spawn_blocking(move || client.join())
        .await
        .unwrap();
    running.shutdown().await;
    report.unwrap().unwrap();
}

fn drive_held_take(
    addr: SocketAddr,
    entered_rx: std::sync::mpsc::Receiver<()>,
    release_tx: std::sync::mpsc::Sender<()>,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("{err}"))?;
    rt.block_on(held_take_session(addr, entered_rx, release_tx))
}

async fn held_take_session(
    addr: SocketAddr,
    entered_rx: std::sync::mpsc::Receiver<()>,
    release_tx: std::sync::mpsc::Sender<()>,
) -> Result<(), String> {
    let mut release = Release(Some(release_tx));
    let mut socket_a = connect(addr).await?;
    send_text(&mut socket_a, &hello()).await?;
    // Two loud frames open the take. The quiet run is the 640 ms hang.
    // The closing frame is the last send, so the session stops reading
    // only after every frame is already in its socket.
    let mut seq = 0u16;
    for _ in 0..2u16 {
        send_bin(&mut socket_a, mic_frame(LOUD, seq)?).await?;
        seq = seq.saturating_add(1);
    }
    for _ in 0..HANG_FRAMES {
        send_bin(&mut socket_a, mic_frame(0, seq)?).await?;
        seq = seq.saturating_add(1);
    }
    tokio::task::spawn_blocking(move || entered_rx.recv_timeout(FUSE))
        .await
        .map_err(|err| format!("{err}"))?
        .map_err(|_| "turn did not reach the gate".to_string())?;

    let welcome = until_type(&mut socket_a, "welcome", FUSE).await?;
    if has_type(&welcome, "turn") {
        return Err("turn frame arrived while the turn was held".into());
    }

    let mut socket_b = connect(addr).await?;
    send_text(&mut socket_b, &hello()).await?;
    until_type(&mut socket_b, "welcome", FUSE).await?;
    send_text(&mut socket_b, &ping()).await?;
    until_type(&mut socket_b, "pong", FUSE).await?;

    if let Some(tx) = release.0.take() {
        let _ = tx.send(());
    }
    let turn = until_type(&mut socket_a, "turn", FUSE).await?;
    let reply = turn
        .iter()
        .find(|value| value.get("type").and_then(Value::as_str) == Some("turn"))
        .and_then(|value| value.get("reply"))
        .and_then(Value::as_str);
    if reply != Some("The fence is loose.") {
        return Err("turn frame did not carry the held reply".into());
    }
    Ok(())
}
