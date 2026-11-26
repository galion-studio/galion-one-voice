// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! A slow text turn and a held `finish` must not pin the runtime worker.
//!
//! The gates below are failure fuses. The success path wakes on a channel
//! or a socket read. It does not sleep, and it does not change the server's
//! 5 s join.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use samantha_local_server::protocol::{SCHEMA, WS_PATH};
use samantha_local_server::reply::{Pad, ReplyEngine, TurnOut};
use samantha_local_server::server::PipelineFactory;
use samantha_local_server::{
    start, start_with_host, FakePipeline, HostConfig, HostSession, Pipeline, RefState,
};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Sock = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Upper bound for a step that should already be ready. A blocked worker
/// trips this on the client runtime, which then releases the gate.
const FUSE: Duration = Duration::from_secs(3);

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

fn knowledge_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge")
}

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "galion-turn-off-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hello() -> String {
    format!(r#"{{"schema":"{SCHEMA}","type":"hello","session":null}}"#)
}

fn text_frame(text: &str) -> String {
    format!(r#"{{"schema":"{SCHEMA}","type":"text","text":"{text}"}}"#)
}

fn ping() -> String {
    format!(r#"{{"schema":"{SCHEMA}","type":"ping"}}"#)
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
async fn held_turn_keeps_the_worker_free() {
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
    let client = std::thread::spawn(move || drive_held_turn(addr, entered_rx, release_tx));
    let report = tokio::task::spawn_blocking(move || client.join())
        .await
        .unwrap();
    running.shutdown().await;
    report.unwrap().unwrap();
}

fn drive_held_turn(
    addr: SocketAddr,
    entered_rx: std::sync::mpsc::Receiver<()>,
    release_tx: std::sync::mpsc::Sender<()>,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("{err}"))?;
    rt.block_on(held_turn_session(addr, entered_rx, release_tx))
}

async fn held_turn_session(
    addr: SocketAddr,
    entered_rx: std::sync::mpsc::Receiver<()>,
    release_tx: std::sync::mpsc::Sender<()>,
) -> Result<(), String> {
    let mut release = Release(Some(release_tx));
    let mut socket_a = connect(addr).await?;
    send_text(&mut socket_a, &hello()).await?;
    send_text(&mut socket_a, &text_frame("northwind")).await?;
    tokio::task::spawn_blocking(move || entered_rx.recv_timeout(FUSE))
        .await
        .map_err(|err| format!("{err}"))?
        .map_err(|_| "turn did not reach the gate".to_string())?;

    let welcome = until_type(&mut socket_a, "welcome", FUSE).await?;
    if has_type(&welcome, "turn") {
        return Err("turn frame arrived while the turn was held".into());
    }
    let welcome = welcome
        .iter()
        .find(|value| value.get("type").and_then(Value::as_str) == Some("welcome"))
        .ok_or("welcome frame missing")?;
    if welcome.get("dot").and_then(Value::as_str) != Some("idle") {
        return Err("welcome frame did not carry the idle dot".into());
    }

    let mut socket_b = connect(addr).await?;
    send_text(&mut socket_b, &hello()).await?;
    until_type(&mut socket_b, "welcome", FUSE).await?;
    send_text(&mut socket_b, &ping()).await?;
    until_type(&mut socket_b, "pong", FUSE).await?;

    if let Some(tx) = release.0.take() {
        let _ = tx.send(());
    }
    until_type(&mut socket_a, "turn", FUSE).await?;
    until_type(&mut socket_a, "dot", FUSE).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn held_finish_does_not_stall_the_runtime() {
    let root = scratch("finish");
    let host = HostSession::boot(HostConfig::deny_under(&root), knowledge_dir()).unwrap();
    let running = start_with_host(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        ui_dir(),
        host.clone(),
        RefState::Unset,
        false,
        Duration::from_secs(120),
    )
    .await
    .unwrap();
    let mind = host.mind();
    let holding = Arc::new(AtomicBool::new(false));
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let holding_for_thread = Arc::clone(&holding);
    let holder = std::thread::spawn(move || {
        let guard = mind.lock().unwrap_or_else(|err| err.into_inner());
        holding_for_thread.store(true, Ordering::SeqCst);
        let _ = held_tx.send(());
        let _ = release_rx.recv_timeout(FUSE);
        holding_for_thread.store(false, Ordering::SeqCst);
        drop(guard);
    });
    tokio::task::spawn_blocking(move || held_rx.recv())
        .await
        .unwrap()
        .unwrap();

    let (done_tx, mut done_rx) = tokio::sync::oneshot::channel();
    let shutdown = tokio::spawn(async move {
        running.shutdown().await;
        let _ = done_tx.send(());
    });
    // Give shutdown the worker often enough to reach `finish`. An inline
    // `finish` never returns the worker while the mind lock is held.
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    let saw_hold = holding.load(Ordering::SeqCst);
    let still_finishing = done_rx.try_recv().is_err();
    let _ = release_tx.send(());
    let joined = shutdown.await;
    holder.join().unwrap();
    joined.unwrap();
    assert!(
        saw_hold && still_finishing,
        "finish occupied the only worker while the mind lock was held"
    );
    drop(host);
    let _ = std::fs::remove_dir_all(&root);
}
