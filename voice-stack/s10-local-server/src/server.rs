// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Axum process. `127.0.0.1` only. `/health`, the UI directory, and
//! `/v1/stream`. TTS slices leave on an 80 ms clock. Mood ticks on the same
//! period. Nothing in this file dials out.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant as Wall;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Json;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval_at, sleep_until, Instant as Tick, MissedTickBehavior};
use tower_http::services::ServeDir;

use crate::audio::{self, FRAME_MS, MIC_RATE};
use crate::bind::{origin_is_local, require_ipv4_loopback, NotLoopback};
use crate::pipeline::{Pipeline, TurnProduct};
use crate::protocol::{
    self, decode_frame, encode_frame, parse_client, stamp, AudioFrame, ClientControl, FLAG_END,
    FLAG_START, KIND_MIC, KIND_TTS,
};

const MAX_SESSIONS: usize = 8;

#[derive(Debug)]
pub enum ServeError {
    NotLoopback(NotLoopback),
    Bind(std::io::Error),
    Serve(std::io::Error),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::NotLoopback(e) => write!(f, "{e}"),
            ServeError::Bind(e) => write!(f, "bind: {e}"),
            ServeError::Serve(e) => write!(f, "serve: {e}"),
        }
    }
}

impl std::error::Error for ServeError {}

/// How `GALION_SAMANTHA_REF` looks, without the path itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefState {
    Unset,
    Set,
    Missing,
}

impl RefState {
    pub fn as_str(self) -> &'static str {
        match self {
            RefState::Unset => "unset",
            RefState::Set => "set",
            RefState::Missing => "missing",
        }
    }

    /// `exists` is `Path::is_file` on the value. The path is not copied into the state.
    pub fn classify(value: Option<&str>, exists: bool) -> Self {
        match value.map(str::trim).filter(|s| !s.is_empty()) {
            None => RefState::Unset,
            Some(_) if exists => RefState::Set,
            Some(_) => RefState::Missing,
        }
    }

    pub fn from_env() -> Self {
        match std::env::var("GALION_SAMANTHA_REF") {
            Err(_) => RefState::Unset,
            Ok(value) => {
                let trimmed = value.trim();
                let exists = !trimmed.is_empty() && std::path::Path::new(trimmed).is_file();
                // Drop the path before returning. Metadata only; the bytes stay on disk.
                Self::classify(Some(&value), exists)
            }
        }
    }
}

pub type PipelineFactory = Arc<dyn Fn() -> Box<dyn Pipeline> + Send + Sync>;

#[derive(Clone)]
pub struct AppState {
    pub bound: SocketAddr,
    pub factory: PipelineFactory,
    pub samantha_ref: RefState,
    pub sessions: Arc<AtomicUsize>,
}

#[derive(Serialize)]
struct HealthBody {
    ok: bool,
    service: &'static str,
    protocol: &'static str,
    bind: String,
    loopback: bool,
    frame_ms: u32,
    mic_rate: u32,
    tts_rate: u32,
    samantha_ref: &'static str,
}

pub struct Running {
    pub addr: SocketAddr,
    join: JoinHandle<()>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.join.abort();
    }
}

pub async fn start(
    addr: SocketAddr,
    ui_dir: PathBuf,
    factory: PipelineFactory,
    samantha_ref: RefState,
) -> Result<Running, ServeError> {
    require_ipv4_loopback(addr).map_err(ServeError::NotLoopback)?;
    let listener = TcpListener::bind(addr).await.map_err(ServeError::Bind)?;
    let bound = listener.local_addr().map_err(ServeError::Bind)?;
    require_ipv4_loopback(bound).map_err(ServeError::NotLoopback)?;
    let state = AppState {
        bound,
        factory,
        samantha_ref,
        sessions: Arc::new(AtomicUsize::new(0)),
    };
    let app = router(state, ui_dir);
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(Running { addr: bound, join })
}

/// Bind and serve until ctrl-c. Used by the binary.
pub async fn serve_until_ctrl_c(
    addr: SocketAddr,
    ui_dir: PathBuf,
    factory: PipelineFactory,
    samantha_ref: RefState,
) -> Result<(), ServeError> {
    require_ipv4_loopback(addr).map_err(ServeError::NotLoopback)?;
    let listener = TcpListener::bind(addr).await.map_err(ServeError::Bind)?;
    let bound = listener.local_addr().map_err(ServeError::Bind)?;
    require_ipv4_loopback(bound).map_err(ServeError::NotLoopback)?;
    eprintln!(
        "samantha-local listening on http://{bound}/  (loopback only, protocol {})",
        protocol::PROTOCOL
    );
    let state = AppState {
        bound,
        factory,
        samantha_ref,
        sessions: Arc::new(AtomicUsize::new(0)),
    };
    let app = router(state, ui_dir);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(ServeError::Serve)
}

fn router(state: AppState, ui_dir: PathBuf) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/stream", get(stream))
        .with_state(state)
        .fallback_service(ServeDir::new(ui_dir))
}

async fn health(State(app): State<AppState>) -> impl IntoResponse {
    let body = HealthBody {
        ok: true,
        service: "samantha-local",
        protocol: protocol::PROTOCOL,
        bind: app.bound.to_string(),
        loopback: true,
        frame_ms: FRAME_MS,
        mic_rate: crate::audio::MIC_RATE,
        tts_rate: crate::audio::TTS_RATE,
        samantha_ref: app.samantha_ref.as_str(),
    };
    (
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(body),
    )
}

async fn stream(State(app): State<AppState>, headers: HeaderMap, ws: WebSocketUpgrade) -> Response {
    if app.sessions.load(Ordering::Relaxed) >= MAX_SESSIONS {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !origin_is_local(origin) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let sessions = app.sessions.clone();
    let factory = app.factory.clone();
    ws.on_upgrade(move |socket| async move {
        sessions.fetch_add(1, Ordering::Relaxed);
        session(socket, factory).await;
        sessions.fetch_sub(1, Ordering::Relaxed);
    })
    .into_response()
}

struct Down {
    msg: Message,
}

async fn session(socket: WebSocket, factory: PipelineFactory) {
    let mut pipe = factory();
    let info = pipe.info();
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Down>(64);
    let writer = tokio::spawn(async move {
        while let Some(down) = rx.recv().await {
            if sink.send(down.msg).await.is_err() {
                break;
            }
        }
    });

    let started = Wall::now();
    let send_json = |tx: &mpsc::Sender<Down>, v: Value| {
        let line = stamp(v, elapsed_ms(started));
        let tx = tx.clone();
        async move {
            tx.send(Down {
                msg: Message::Text(line),
            })
            .await
            .is_ok()
        }
    };

    if !send_json(
        &tx,
        protocol::ready_json(&info.ear, &info.mouth, &info.mind),
    )
    .await
    {
        return;
    }

    let mut mood_tick = interval_at(
        Tick::now() + Duration::from_millis(u64::from(FRAME_MS)),
        Duration::from_millis(u64::from(FRAME_MS)),
    );
    mood_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut pending: VecDeque<Vec<u8>> = VecDeque::new();
    let mut audio_deadline: Option<Tick> = None;
    let mut seq: u32 = 0;
    let mut speaking = false;

    loop {
        let deadline = audio_deadline;
        tokio::select! {
            incoming = stream.next() => {
                match incoming {
                    None | Some(Err(_)) => break,
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Ping(p))) => {
                        if tx.send(Down { msg: Message::Pong(p) }).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Text(text))) => {
                        if !handle_text(
                            &text,
                            &mut pipe,
                            &tx,
                            started,
                            &mut pending,
                            &mut audio_deadline,
                            &mut seq,
                            &mut speaking,
                        ).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if !handle_binary(
                            &bytes,
                            &mut pipe,
                            &tx,
                            started,
                            &mut pending,
                            &mut audio_deadline,
                            &mut seq,
                            &mut speaking,
                        ).await {
                            break;
                        }
                    }
                }
            }
            _ = mood_tick.tick() => {
                let (pad, tick) = pipe.tick_mood();
                let state = if speaking { "speaking" } else { pipe.phase() };
                let line = stamp(protocol::mood_json(pad, tick, state), elapsed_ms(started));
                if tx.try_send(Down { msg: Message::Text(line) }).is_err() {
                    // The writer is gone, or the queue is full. A full queue
                    // drops this tick. The next one still fires.
                    if tx.is_closed() {
                        break;
                    }
                }
            }
            _ = sleep_until(deadline.unwrap_or_else(|| Tick::now() + Duration::from_secs(3600))), if deadline.is_some() => {
                audio_deadline = None;
                if let Some(frame) = pending.pop_front() {
                    let ended = frame_has_end(&frame);
                    if tx.send(Down { msg: Message::Binary(frame) }).await.is_err() {
                        break;
                    }
                    if pending.is_empty() {
                        speaking = false;
                        let reason = if ended { Some("end") } else { None };
                        if !send_json(&tx, protocol::state_json("idle", reason)).await {
                            break;
                        }
                    } else {
                        audio_deadline = Some(Tick::now() + Duration::from_millis(u64::from(FRAME_MS)));
                    }
                }
            }
        }
    }
    drop(tx);
    let _ = writer.await;
}

fn elapsed_ms(started: Wall) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn frame_has_end(bytes: &[u8]) -> bool {
    decode_frame(bytes)
        .map(|f| f.flags & FLAG_END != 0)
        .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
async fn handle_text(
    text: &str,
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    started: Wall,
    pending: &mut VecDeque<Vec<u8>>,
    audio_deadline: &mut Option<Tick>,
    seq: &mut u32,
    speaking: &mut bool,
) -> bool {
    match parse_client(text) {
        Err(error) => send_value(tx, started, protocol::error_json(&error)).await,
        Ok(ClientControl::Hello) => send_value(tx, started, protocol::hello_json()).await,
        Ok(ClientControl::Ping { t_ms }) => {
            send_value(tx, started, protocol::pong_json(t_ms)).await
        }
        Ok(ClientControl::Unknown { kind }) => {
            send_value(
                tx,
                started,
                protocol::error_json(&format!("unknown type {kind}")),
            )
            .await
        }
        Ok(ClientControl::Barge) => {
            barge(pipe, tx, started, pending, audio_deadline, speaking).await
        }
        Ok(ClientControl::Text { text }) => {
            let product = pipe.push_text(&text);
            emit_turn(product, tx, started, pending, audio_deadline, seq, speaking).await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_binary(
    bytes: &[u8],
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    started: Wall,
    pending: &mut VecDeque<Vec<u8>>,
    audio_deadline: &mut Option<Tick>,
    seq: &mut u32,
    speaking: &mut bool,
) -> bool {
    let frame = match decode_frame(bytes) {
        Ok(frame) => frame,
        Err(error) => {
            return send_value(tx, started, protocol::error_json(&error.to_string())).await
        }
    };
    if frame.kind != KIND_MIC {
        return send_value(
            tx,
            started,
            protocol::error_json("client frames must be mic"),
        )
        .await;
    }
    if frame.sample_rate != MIC_RATE {
        return send_value(tx, started, protocol::error_json("mic rate must be 16000")).await;
    }
    if frame.flags & protocol::FLAG_BARGE != 0 {
        if !barge(pipe, tx, started, pending, audio_deadline, speaking).await {
            return false;
        }
    }
    let product = pipe.push_mic(&frame.pcm);
    emit_turn(product, tx, started, pending, audio_deadline, seq, speaking).await
}

async fn barge(
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    started: Wall,
    pending: &mut VecDeque<Vec<u8>>,
    audio_deadline: &mut Option<Tick>,
    speaking: &mut bool,
) -> bool {
    pending.clear();
    *audio_deadline = None;
    *speaking = false;
    pipe.barge();
    send_value(tx, started, protocol::state_json("idle", Some("barge"))).await
}

async fn emit_turn(
    product: TurnProduct,
    tx: &mpsc::Sender<Down>,
    started: Wall,
    pending: &mut VecDeque<Vec<u8>>,
    audio_deadline: &mut Option<Tick>,
    seq: &mut u32,
    speaking: &mut bool,
) -> bool {
    if !product.pcm.is_empty() {
        pending.clear();
        *audio_deadline = None;
    }
    for event in product.events {
        if !send_value(tx, started, event).await {
            return false;
        }
    }
    if product.pcm.is_empty() {
        return true;
    }
    let mut frames = encode_slices(&product.pcm, product.rate, seq);
    if !send_value(tx, started, protocol::state_json("speaking", None)).await {
        return false;
    }
    *speaking = true;
    if let Some(first) = frames.pop_front() {
        if tx
            .send(Down {
                msg: Message::Binary(first),
            })
            .await
            .is_err()
        {
            return false;
        }
    }
    if frames.is_empty() {
        *speaking = false;
        return send_value(tx, started, protocol::state_json("idle", Some("end"))).await;
    }
    *pending = frames;
    *audio_deadline = Some(Tick::now() + Duration::from_millis(u64::from(FRAME_MS)));
    true
}

fn encode_slices(pcm: &[i16], rate: u32, seq: &mut u32) -> VecDeque<Vec<u8>> {
    let slices = audio::slice_pcm(pcm, rate, FRAME_MS);
    let n = slices.len();
    let mut out = VecDeque::with_capacity(n);
    for (i, samples) in slices.into_iter().enumerate() {
        *seq = seq.saturating_add(1);
        let mut flags = 0;
        if i == 0 {
            flags |= FLAG_START;
        }
        if i + 1 == n {
            flags |= FLAG_END;
        }
        let frame = AudioFrame {
            kind: KIND_TTS,
            flags,
            sample_rate: rate,
            sequence: *seq,
            pcm: samples,
        };
        match encode_frame(&frame) {
            Ok(bytes) => out.push_back(bytes),
            Err(_) => break,
        }
    }
    out
}

async fn send_value(tx: &mpsc::Sender<Down>, started: Wall, v: Value) -> bool {
    let line = stamp(v, elapsed_ms(started));
    tx.send(Down {
        msg: Message::Text(line),
    })
    .await
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_classify_hides_the_path() {
        assert_eq!(RefState::classify(None, false), RefState::Unset);
        assert_eq!(RefState::classify(Some("  "), true), RefState::Unset);
        assert_eq!(
            RefState::classify(Some("G:\\galion\\ref.wav"), true),
            RefState::Set
        );
        assert_eq!(
            RefState::classify(Some("/var/lib/ref.wav"), false),
            RefState::Missing
        );
        assert!(!RefState::Set.as_str().contains("galion"));
        assert_eq!(RefState::Set.as_str(), "set");
        assert_eq!(RefState::Missing.as_str(), "missing");
    }
}
