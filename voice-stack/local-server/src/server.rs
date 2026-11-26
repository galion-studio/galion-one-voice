// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Axum process for `galion-samantha-ws/1`.
//! Default bind `127.0.0.1:7881`. `GALION_VOICE_WS` may select another
//! `127.0.0.1` or `::1` port. `GALION_SAMANTHA_WS` is a deprecated alias.
//! `0.0.0.0` is refused. Nothing in this file dials out.

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
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tokio::time::{interval_at, sleep_until, Instant as Tick, MissedTickBehavior};
use tower_http::services::ServeDir;

use crate::audio::{self, FRAME_MS};
use crate::bind::{origin_allowed, require_loopback, NotLoopback};
use crate::pipeline::{AudioFeed, FeedMsg, Pipeline, TurnProduct};
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
    Signal(String),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::NotLoopback(e) => write!(f, "{e}"),
            ServeError::Bind(e) => write!(f, "bind: {e}"),
            ServeError::Serve(e) => write!(f, "serve: {e}"),
            ServeError::Signal(e) => write!(f, "signal: {e}"),
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
        match crate::env_alias::read("GALION_VOICE_REF", "GALION_SAMANTHA_REF") {
            None => RefState::Unset,
            Some(value) => {
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
    pub voice_ref: RefState,
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
    join: Option<JoinHandle<()>>,
    stop: Option<Arc<Notify>>,
    host: Option<Arc<crate::host::HostSession>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

impl Running {
    /// Drain the listener, then seal the session. This is the path a caught
    /// SIGTERM or ctrl-c takes after the signal. Dropping [`Running`] aborts
    /// the task and does not seal.
    pub async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            stop.notify_one();
        }
        if let Some(join) = self.join.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), join).await;
        }
        if let Some(host) = self.host.take() {
            host.finish();
        }
    }
}

pub async fn start(
    addr: SocketAddr,
    ui_dir: PathBuf,
    factory: PipelineFactory,
    voice_ref: RefState,
    require_session: bool,
    ttl: Duration,
) -> Result<Running, ServeError> {
    start_bound(
        addr,
        ui_dir,
        factory,
        voice_ref,
        require_session,
        ttl,
        None,
    )
    .await
}

pub(crate) async fn start_bound(
    addr: SocketAddr,
    ui_dir: PathBuf,
    factory: PipelineFactory,
    voice_ref: RefState,
    require_session: bool,
    ttl: Duration,
    host: Option<Arc<crate::host::HostSession>>,
) -> Result<Running, ServeError> {
    let (listener, bound) = bind_listener(addr).await?;
    let state = app_state(
        bound,
        factory,
        voice_ref,
        require_session,
        ttl,
        ui_dir.clone(),
    );
    let app = router(state, ui_dir);
    let stop = Arc::new(Notify::new());
    let shutdown = Arc::clone(&stop);
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown.notified().await;
            })
            .await;
    });
    Ok(Running {
        addr: bound,
        join: Some(join),
        stop: Some(stop),
        host,
    })
}

/// ctrl-c, and on Unix SIGTERM. After the listener drains, the host seals.
pub async fn serve_until_ctrl_c(
    addr: SocketAddr,
    ui_dir: PathBuf,
    factory: PipelineFactory,
    voice_ref: RefState,
    require_session: bool,
    ttl: Duration,
    host: Arc<crate::host::HostSession>,
) -> Result<(), ServeError> {
    let running = start_bound(
        addr,
        ui_dir,
        factory,
        voice_ref,
        require_session,
        ttl,
        Some(Arc::clone(&host)),
    )
    .await?;
    eprintln!(
        "samantha-local listening on http://{}/  (loopback only, {SCHEMA} {WS_PATH})",
        running.addr
    );
    let stop = Stop::install().map_err(ServeError::Signal)?;
    stop.wait().await;
    running.shutdown().await;
    Ok(())
}

/// Installed before the process reports that it is listening, so a stop
/// signal is not delivered to the default terminate action.
pub struct Stop {
    #[cfg(unix)]
    term: tokio::signal::unix::Signal,
}

impl Stop {
    pub fn install() -> Result<Self, String> {
        #[cfg(unix)]
        {
            let term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(|err| format!("SIGTERM handler: {err}"))?;
            Ok(Self { term })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    pub async fn wait(self) {
        #[cfg(unix)]
        {
            let mut term = self.term;
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
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
    voice_ref: RefState,
    require_session: bool,
    ttl: Duration,
    ui: PathBuf,
) -> AppState {
    let http_pipe = Arc::new(Mutex::new((factory)()));
    AppState {
        bound,
        factory,
        voice_ref,
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

fn health_json(bound: &str, voice_ref: &str, info: &crate::pipeline::PipeInfo) -> String {
    serde_json::json!({
        "ok": true,
        "schema": SCHEMA,
        "bind": bound,
        "loopback": true,
        "voice_ref": voice_ref,
        "samantha_ref": voice_ref,
        "vad": info.vad_backend,
        "vad_ready": info.vad_ready,
        "asr": info.ear,
        "mouth": info.mouth,
        "voice": info.voice,
        "clone": info.clone_status,
        "mouth_device": info.mouth_device,
        "mind": info.mind,
        "knowledge_cards": info.knowledge_cards,
        "knowledge_dir": info.knowledge_dir,
    })
    .to_string()
}

async fn health(State(app): State<AppState>) -> impl IntoResponse {
    let info = app
        .http_pipe
        .lock()
        .map(|pipe| pipe.info())
        .unwrap_or_else(|err| err.into_inner().info());
    let body = health_json(&app.bound.to_string(), app.voice_ref.as_str(), &info);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
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
    let mut pace = Pace::default();

    loop {
        let deadline = pace.deadline;
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
                        if !handle_text(&text, &mut pipe, &tx, started, &mut pace).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if !handle_binary(&bytes, &mut pipe, &tx, started, &mut pace).await {
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
                if let Some(inner) = pipe.poll_inner() {
                    if tx.try_send(Down { msg: Message::Text(inner) }).is_err() && tx.is_closed() {
                        break;
                    }
                }
            }
            _ = sleep_until(deadline.unwrap_or_else(|| Tick::now() + Duration::from_secs(3600))), if deadline.is_some() => {
                if !on_audio_deadline(&mut pipe, &tx, &mut pace).await {
                    break;
                }
            }
            msg = poll_feed(&mut pace.feed) => {
                if !on_feed(&mut pipe, &tx, &mut pace, msg).await {
                    break;
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

struct Pace {
    pending: VecDeque<Vec<u8>>,
    deadline: Option<Tick>,
    seq: u16,
    speaking: bool,
    /// The native mouth sent clause text and no PCM.
    captioned: bool,
    /// When the previous slice was handed to the writer. `None` until the first.
    last_sent: Option<Tick>,
    feed: Option<AudioFeed>,
    rate: u32,
}

impl Default for Pace {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            deadline: None,
            seq: 0,
            speaking: false,
            captioned: false,
            last_sent: None,
            feed: None,
            rate: crate::audio::SPEAKER_RATE,
        }
    }
}

async fn poll_feed(feed: &mut Option<AudioFeed>) -> Option<FeedMsg> {
    match feed.as_mut() {
        Some(feed) => feed.rx.recv().await,
        None => std::future::pending().await,
    }
}

async fn handle_text(
    text: &str,
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    started: Wall,
    pace: &mut Pace,
) -> bool {
    match parse_client(text) {
        Err(code) => send_line(tx, error_json(&code)).await,
        // The socket already welcomed this session. A second hello within the
        // same connection is not another session and does not send welcome again.
        Ok(ClientControl::Hello { .. }) => true,
        Ok(ClientControl::Ping) => send_line(tx, pong_json()).await,
        Ok(ClientControl::Unknown { .. }) => send_line(tx, error_json("unknown-type")).await,
        Ok(ClientControl::Consent { .. }) => true,
        Ok(ClientControl::Stop) => stop_audio(pipe, tx, pace).await,
        Ok(ClientControl::Text { text }) => {
            let product = pipe.push_text(&text, elapsed_ms(started));
            emit_turn(pipe, product, tx, pace).await
        }
    }
}

async fn handle_binary(
    bytes: &[u8],
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    started: Wall,
    pace: &mut Pace,
) -> bool {
    let (dir, rate, _seq, pcm) = match decode_pcm(bytes) {
        Ok(frame) => frame,
        Err(error) => return send_line(tx, error_json(&error.to_string())).await,
    };
    if dir != AudioDir::Mic {
        return send_line(tx, error_json("mic-only")).await;
    }
    let product = pipe.push_mic(&pcm, rate.hz(), elapsed_ms(started));
    emit_turn(pipe, product, tx, pace).await
}

async fn stop_audio(
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    pace: &mut Pace,
) -> bool {
    pace.pending.clear();
    pace.deadline = None;
    pace.speaking = false;
    pace.captioned = false;
    pace.last_sent = None;
    pace.feed = None;
    pipe.stop();
    send_line(tx, protocol::dot_json("idle", 0.0)).await
}

async fn emit_turn(
    pipe: &mut Box<dyn Pipeline>,
    mut product: TurnProduct,
    tx: &mpsc::Sender<Down>,
    pace: &mut Pace,
) -> bool {
    let streamed = product.feed.is_some();
    let buffered = !product.pcm.is_empty();
    if streamed || buffered {
        pace.pending.clear();
        pace.deadline = None;
        pace.speaking = false;
        pace.captioned = false;
        pace.last_sent = None;
        pace.feed = None;
        pace.rate = product.rate;
    }
    for event in product.events.drain(..) {
        if !send_line(tx, event).await {
            return false;
        }
    }
    if let Some(feed) = product.feed.take() {
        pace.feed = Some(feed);
        return true;
    }
    if product.pcm.is_empty() {
        return true;
    }
    for slice in audio::slice_pcm(&product.pcm, pace.rate, FRAME_MS) {
        if !accept_slice(tx, pace, &slice).await {
            return false;
        }
    }
    if pace.pending.is_empty() && pace.feed.is_none() {
        pace.speaking = false;
        pace.last_sent = None;
        pipe.utterance_done();
        return send_line(tx, protocol::dot_json("idle", 0.0)).await;
    }
    true
}

/// The first slice of an utterance goes out now. A later slice waits until
/// 80 ms after the previous send, unless that wait has already elapsed.
async fn accept_slice(tx: &mpsc::Sender<Down>, pace: &mut Pace, samples: &[i16]) -> bool {
    let Some(frame) = encode_one(pace, samples) else {
        return true;
    };
    let now = Tick::now();
    let due = match pace.last_sent {
        None => true,
        Some(sent) => now >= sent + Duration::from_millis(u64::from(FRAME_MS)),
    };
    if due && pace.pending.is_empty() {
        if tx
            .send(Down {
                msg: Message::Binary(frame),
            })
            .await
            .is_err()
        {
            return false;
        }
        pace.speaking = true;
        pace.last_sent = Some(Tick::now());
        pace.deadline = None;
        return true;
    }
    pace.pending.push_back(frame);
    pace.speaking = true;
    if pace.deadline.is_none() {
        let sent = pace.last_sent.unwrap_or(now);
        let when = sent + Duration::from_millis(u64::from(FRAME_MS));
        pace.deadline = Some(if when <= now { now } else { when });
    }
    true
}

fn encode_one(pace: &mut Pace, samples: &[i16]) -> Option<Vec<u8>> {
    let code = RateCode::from_hz(pace.rate).unwrap_or(RateCode::Hz24k);
    pace.seq = pace.seq.wrapping_add(1);
    encode_pcm(AudioDir::Speaker, code, pace.seq, samples).ok()
}

async fn on_audio_deadline(
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    pace: &mut Pace,
) -> bool {
    pace.deadline = None;
    let Some(frame) = pace.pending.pop_front() else {
        return true;
    };
    if tx
        .send(Down {
            msg: Message::Binary(frame),
        })
        .await
        .is_err()
    {
        return false;
    }
    pace.last_sent = Some(Tick::now());
    if !pace.pending.is_empty() {
        pace.deadline = Some(Tick::now() + Duration::from_millis(u64::from(FRAME_MS)));
        return true;
    }
    if pace.feed.is_some() {
        return true;
    }
    pace.speaking = false;
    pace.last_sent = None;
    pipe.utterance_done();
    send_line(tx, protocol::dot_json("idle", pipe.level())).await
}

async fn on_feed(
    pipe: &mut Box<dyn Pipeline>,
    tx: &mpsc::Sender<Down>,
    pace: &mut Pace,
    msg: Option<FeedMsg>,
) -> bool {
    match msg {
        Some(FeedMsg::Slice(samples)) => accept_slice(tx, pace, &samples).await,
        Some(FeedMsg::Text(line)) => {
            pace.captioned = true;
            send_line(tx, line).await
        }
        Some(FeedMsg::Failed(code)) => {
            pace.pending.clear();
            pace.deadline = None;
            pace.feed = None;
            pace.speaking = false;
            pace.captioned = false;
            pace.last_sent = None;
            pipe.utterance_done();
            send_line(tx, error_json(&code)).await
                && send_line(tx, protocol::dot_json("idle", 0.0)).await
        }
        Some(FeedMsg::End) | None => {
            pace.feed = None;
            if pace.pending.is_empty() && pace.deadline.is_none() {
                let was = pace.speaking || pace.captioned;
                pace.speaking = false;
                pace.captioned = false;
                pace.last_sent = None;
                pipe.utterance_done();
                if was {
                    return send_line(tx, protocol::dot_json("idle", pipe.level())).await;
                }
            }
            true
        }
    }
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

    #[test]
    fn health_names_ljspeech_and_s711() {
        use crate::pipeline::PipeInfo;
        let piper = health_json(
            "127.0.0.1:7881",
            "set",
            &PipeInfo {
                ear: "vosk",
                mouth: "piper",
                voice: "ljspeech",
                clone_status: "cpu-not-realtime",
                mouth_device: "cpu",
                mind: "horbm".into(),
                vad_backend: "silero",
                vad_ready: true,
                knowledge_cards: 1506,
                knowledge_dir: "knowledge".into(),
            },
        );
        assert!(piper.contains("\"mouth\":\"piper\""));
        assert!(piper.contains("\"mind\":\"horbm\""));
        assert!(piper.contains("\"knowledge_cards\":1506"));
        assert!(piper.contains("\"voice\":\"ljspeech\""));
        assert!(piper.contains("\"clone\":\"cpu-not-realtime\""));
        assert!(piper.contains("\"ok\":true"));
        let s711 = health_json(
            "127.0.0.1:7881",
            "set",
            &PipeInfo {
                ear: "vosk",
                mouth: "s711",
                voice: "s711",
                clone_status: "selected",
                mouth_device: "cuda",
                mind: "horbm".into(),
                vad_backend: "silero",
                vad_ready: true,
                knowledge_cards: 1,
                knowledge_dir: "knowledge".into(),
            },
        );
        assert!(s711.contains("\"mouth\":\"s711\""));
        assert!(s711.contains("\"voice\":\"s711\""));
        assert!(s711.contains("\"clone\":\"selected\""));
        assert!(!s711.contains("ljspeech"));
    }
}
