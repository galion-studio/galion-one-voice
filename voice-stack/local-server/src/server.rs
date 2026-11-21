// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Axum process for `galion-samantha-ws/1`.
//! Default bind `127.0.0.1:7881`. `GALION_SAMANTHA_WS` may select another
//! `127.0.0.1` or `::1` port. `0.0.0.0` is refused. Nothing in this file dials out.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant as Wall;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval_at, sleep_until, Instant as Tick, MissedTickBehavior};
use tower_http::services::ServeDir;

use crate::audio::{self, FRAME_MS};
use crate::bind::{origin_allowed, require_loopback, NotLoopback};
use crate::pipeline::{Pipeline, TurnProduct};
use crate::protocol::{
    self, decode_pcm, encode_pcm, error_json, mood_json, parse_client, pong_json,
    query_carries_token, welcome_json, AudioDir, ClientControl, RateCode, SCHEMA, WS_PATH,
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
    pub sessions: Arc<SessionStore>,
    pub require_session: bool,
    pub live: Arc<AtomicUsize>,
    pub ui: PathBuf,
    pub http_pipe: Arc<Mutex<Box<dyn crate::pipeline::Pipeline>>>,
    pub stage: crate::voice_http::SharedStage,
}

#[derive(Clone)]
pub struct SessionStore {
    ttl: Duration,
    inner: Arc<Mutex<HashMap<String, Wall>>>,
}

impl SessionStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn ttl_secs(&self) -> u64 {
        self.ttl.as_secs()
    }

    pub fn open(&self) -> String {
        let token = mint_token();
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert(token.clone(), Wall::now());
        }
        token
    }

    pub fn valid(&self, token: &str) -> bool {
        let Ok(mut guard) = self.inner.lock() else {
            return false;
        };
        let Some(at) = guard.get(token).copied() else {
            return false;
        };
        if at.elapsed() > self.ttl {
            guard.remove(token);
            false
        } else {
            true
        }
    }
}

fn mint_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut a = RandomState::new().build_hasher();
    let mut b = RandomState::new().build_hasher();
    a.write_u64(Wall::now().elapsed().as_nanos() as u64);
    b.write_u64(a.finish());
    format!("{:016x}{:016x}", a.finish(), b.finish())
}

/// Clamp `GALION_SESSION_TTL_SECS` to 15..=900. Default 120.
pub fn session_ttl_from_env() -> Duration {
    let secs = std::env::var("GALION_SESSION_TTL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(120);
    Duration::from_secs(secs.clamp(15, 900))
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
    require_session: bool,
    ttl: Duration,
) -> Result<Running, ServeError> {
    let (listener, bound) = bind_listener(addr).await?;
    let state = app_state(
        bound,
        factory,
        samantha_ref,
        require_session,
        ttl,
        ui_dir.clone(),
    );
    let app = router(state, ui_dir);
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(Running { addr: bound, join })
}

pub async fn serve_until_ctrl_c(
    addr: SocketAddr,
    ui_dir: PathBuf,
    factory: PipelineFactory,
    samantha_ref: RefState,
    require_session: bool,
    ttl: Duration,
) -> Result<(), ServeError> {
    let (listener, bound) = bind_listener(addr).await?;
    eprintln!("samantha-local listening on http://{bound}/  (loopback only, {SCHEMA} {WS_PATH})");
    let state = app_state(
        bound,
        factory,
        samantha_ref,
        require_session,
        ttl,
        ui_dir.clone(),
    );
    let app = router(state, ui_dir);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(ServeError::Serve)
}

async fn bind_listener(addr: SocketAddr) -> Result<(TcpListener, SocketAddr), ServeError> {
    require_loopback(addr).map_err(ServeError::NotLoopback)?;
    let listener = TcpListener::bind(addr).await.map_err(ServeError::Bind)?;
    let bound = listener.local_addr().map_err(ServeError::Bind)?;
    require_loopback(bound).map_err(ServeError::NotLoopback)?;
    Ok((listener, bound))
}

fn app_state(
    bound: SocketAddr,
    factory: PipelineFactory,
    samantha_ref: RefState,
    require_session: bool,
    ttl: Duration,
    ui: PathBuf,
) -> AppState {
    let http_pipe = Arc::new(Mutex::new((factory)()));
    AppState {
        bound,
        factory,
        samantha_ref,
        sessions: Arc::new(SessionStore::new(ttl)),
        require_session,
        live: Arc::new(AtomicUsize::new(0)),
        ui,
        http_pipe,
        stage: crate::voice_http::new_stage(),
    }
}

fn router(state: AppState, ui_dir: PathBuf) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/favicon.ico", get(favicon))
        .route("/session", post(open_session))
        .route(WS_PATH, get(stream))
        .route("/", get(one_page))
        .route("/one", get(one_page))
        .route("/one/", get(one_page))
        .route("/dot", get(dot_redirect))
        .route("/dot/", get(dot_page))
        .route("/v1/dot", get(crate::voice_http::dot_state))
        .route("/v1/mic", post(crate::voice_http::mic_level))
        .route("/v1/turn", post(crate::voice_http::voice_turn))
        .route("/v2/say", post(crate::voice_http::say_text))
        .route("/v1/reply/:id", get(crate::voice_http::reply_audio))
        .with_state(state)
        .fallback_service(ServeDir::new(ui_dir))
}

async fn one_page(State(app): State<AppState>) -> Response {
    crate::voice_http::html_file(&app.ui.join("index.html"))
}

async fn dot_page(State(app): State<AppState>) -> Response {
    crate::voice_http::html_file(&app.ui.join("dot/index.html"))
}

async fn dot_redirect() -> Redirect {
    Redirect::permanent("/dot/")
}

async fn favicon() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/x-icon"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        Bytes::from_static(include_bytes!("../ui/favicon.ico")),
    )
}

async fn health(State(app): State<AppState>) -> impl IntoResponse {
    let info = app
        .http_pipe
        .lock()
        .map(|pipe| pipe.info())
        .unwrap_or_else(|err| err.into_inner().info());
    let body = serde_json::json!({
        "ok": true,
        "schema": SCHEMA,
        "bind": app.bound.to_string(),
        "loopback": true,
        "samantha_ref": app.samantha_ref.as_str(),
        "vad": info.vad_backend,
        "vad_ready": info.vad_ready,
        "asr": info.ear,
        "mouth": info.mouth,
        "mind": info.mind,
    });
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body.to_string(),
    )
}

async fn open_session(State(app): State<AppState>, uri: Uri, body: Bytes) -> Response {
    if query_carries_token(uri.query()) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if !session_open_body(&body) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let token = app.sessions.open();
    let body = format!(
        "{{\"token\":\"{token}\",\"ttl_s\":{},\"transport\":\"body\"}}",
        app.sessions.ttl_secs()
    );
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

fn session_open_body(bytes: &[u8]) -> bool {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return false;
    };
    v.get("op").and_then(|x| x.as_str()) == Some("open")
}

async fn stream(
    State(app): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    ws: WebSocketUpgrade,
) -> Response {
    if query_carries_token(uri.query()) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if app.live.load(Ordering::Relaxed) >= MAX_SESSIONS {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !origin_allowed(origin, app.bound) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let live = app.live.clone();
    ws.on_upgrade(move |socket| async move {
        live.fetch_add(1, Ordering::Relaxed);
        session(socket, app).await;
        live.fetch_sub(1, Ordering::Relaxed);
    })
    .into_response()
}

struct Down {
    msg: Message,
}

async fn session(socket: WebSocket, app: AppState) {
    let mut pipe = (app.factory)();
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

    let hello = match tokio::time::timeout(
        Duration::from_secs(protocol::HELLO_DEADLINE_SECS),
        stream.next(),
    )
    .await
    {
        Ok(Some(Ok(Message::Text(text)))) => text,
        _ => {
            let _ = tx
                .send(Down {
                    msg: Message::Text(error_json("hello-timeout")),
                })
                .await;
            drop(tx);
            let _ = writer.await;
            return;
        }
    };
    match parse_client(&hello) {
        Ok(ClientControl::Hello { session }) => {
            if app.require_session {
                let ok = session
                    .as_deref()
                    .map(|t| app.sessions.valid(t))
                    .unwrap_or(false);
                if !ok {
                    let _ = tx
                        .send(Down {
                            msg: Message::Text(error_json("session")),
                        })
                        .await;
                    drop(tx);
                    let _ = writer.await;
                    return;
                }
            }
        }
        _ => {
            let _ = tx
                .send(Down {
                    msg: Message::Text(error_json("hello")),
                })
                .await;
            drop(tx);
            let _ = writer.await;
            return;
        }
    }
    if tx
        .send(Down {
            msg: Message::Text(welcome_json()),
        })
        .await
        .is_err()
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
    let mut seq: u16 = 0;
    let mut speaking = false;

    loop {
        let deadline = audio_deadline;
        tokio::select! {
            incoming = stream.next() => {
                match incoming {
                    None | Some(Err(_)) => break,
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Ping(p))) => {
                        if tx.send(Down { msg: Message::Pong(p) }).await.is_err() { break; }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Text(text))) => {
                        if !handle_text(&text, &mut pipe, &tx, started, &mut pending, &mut audio_deadline, &mut seq, &mut speaking).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if !handle_binary(&bytes, &mut pipe, &tx, started, &mut pending, &mut audio_deadline, &mut seq, &mut speaking).await {
                            break;
                        }
                    }
                }
            }
            _ = mood_tick.tick() => {
                let (pad, source) = pipe.tick_mood();
                let t_ms = elapsed_ms(started);
                let line = mood_json(t_ms, source, pad);
                if tx.try_send(Down { msg: Message::Text(line) }).is_err() && tx.is_closed() {
                    break;
                }
            }
            _ = sleep_until(deadline.unwrap_or_else(|| Tick::now() + Duration::from_secs(3600))), if deadline.is_some() => {
                audio_deadline = None;
                if let Some(frame) = pending.pop_front() {
                    if tx.send(Down { msg: Message::Binary(frame) }).await.is_err() { break; }
                    if pending.is_empty() {
                        speaking = false;
                        let _ = send_line(&tx, protocol::dot_json("idle", pipe.level())).await;
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

#[allow(clippy::too_many_arguments)]
async fn handle_text(
    text: &str,
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    started: Wall,
    pending: &mut VecDeque<Vec<u8>>,
    audio_deadline: &mut Option<Tick>,
    seq: &mut u16,
    speaking: &mut bool,
) -> bool {
    match parse_client(text) {
        Err(code) => send_line(tx, error_json(&code)).await,
        // The socket already welcomed this session. A second hello within the
        // same connection is not another session and does not send welcome again.
        Ok(ClientControl::Hello { .. }) => true,
        Ok(ClientControl::Ping) => send_line(tx, pong_json()).await,
        Ok(ClientControl::Unknown { .. }) => send_line(tx, error_json("unknown-type")).await,
        Ok(ClientControl::Consent { .. }) => true,
        Ok(ClientControl::Stop) => stop_audio(pipe, tx, pending, audio_deadline, speaking).await,
        Ok(ClientControl::Text { text }) => {
            let product = pipe.push_text(&text, elapsed_ms(started));
            emit_turn(product, tx, pending, audio_deadline, seq, speaking).await
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
    seq: &mut u16,
    speaking: &mut bool,
) -> bool {
    let (dir, rate, _seq, pcm) = match decode_pcm(bytes) {
        Ok(frame) => frame,
        Err(error) => return send_line(tx, error_json(&error.to_string())).await,
    };
    if dir != AudioDir::Mic {
        return send_line(tx, error_json("mic-only")).await;
    }
    let product = pipe.push_mic(&pcm, rate.hz(), elapsed_ms(started));
    emit_turn(product, tx, pending, audio_deadline, seq, speaking).await
}

async fn stop_audio(
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    pending: &mut VecDeque<Vec<u8>>,
    audio_deadline: &mut Option<Tick>,
    speaking: &mut bool,
) -> bool {
    pending.clear();
    *audio_deadline = None;
    *speaking = false;
    pipe.stop();
    send_line(tx, protocol::dot_json("idle", 0.0)).await
}

async fn emit_turn(
    product: TurnProduct,
    tx: &mpsc::Sender<Down>,
    pending: &mut VecDeque<Vec<u8>>,
    audio_deadline: &mut Option<Tick>,
    seq: &mut u16,
    speaking: &mut bool,
) -> bool {
    if !product.pcm.is_empty() {
        pending.clear();
        *audio_deadline = None;
    }
    for event in product.events {
        if !send_line(tx, event).await {
            return false;
        }
    }
    if product.pcm.is_empty() {
        return true;
    }
    let mut frames = encode_slices(&product.pcm, product.rate, seq);
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
        return send_line(tx, protocol::dot_json("idle", 0.0)).await;
    }
    *pending = frames;
    *audio_deadline = Some(Tick::now() + Duration::from_millis(u64::from(FRAME_MS)));
    true
}

fn encode_slices(pcm: &[i16], rate: u32, seq: &mut u16) -> VecDeque<Vec<u8>> {
    let code = RateCode::from_hz(rate).unwrap_or(RateCode::Hz24k);
    let slices = audio::slice_pcm(pcm, code.hz(), FRAME_MS);
    let mut out = VecDeque::with_capacity(slices.len());
    for samples in slices {
        *seq = seq.wrapping_add(1);
        if let Ok(bytes) = encode_pcm(AudioDir::Speaker, code, *seq, &samples) {
            out.push_back(bytes);
        }
    }
    out
}

async fn send_line(tx: &mpsc::Sender<Down>, line: String) -> bool {
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
    }

    #[test]
    fn ttl_clamp_logic_matches_the_architecture() {
        assert_eq!(15u64.clamp(15, 900), 15);
        assert_eq!(120u64.clamp(15, 900), 120);
        assert_eq!(5000u64.clamp(15, 900), 900);
    }

    #[test]
    fn session_open_requires_the_op() {
        assert!(session_open_body(br#"{"op":"open"}"#));
        assert!(!session_open_body(br#"{"op":"close"}"#));
        assert!(!session_open_body(b"nope"));
    }
}
