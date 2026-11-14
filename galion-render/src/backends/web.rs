//! Optional web console surface, std only (no tokio/axum/serde).
//!
//! `present` stores the latest frame as JSON in a single-slot mailbox (latest wins,
//! O(1), never blocks on clients). `open` optionally binds a tiny blocking HTTP server
//! on its own thread that answers:
//!   GET /dot    -> the one dot for the latest frame ([`crate::dot::DotParams::to_json`])
//!   GET /health -> "ok"
//!   POST /mic   -> browser mic level JSON (rms); GET /mic -> last posted. The parsed
//!                  level is also exposed to the host loop via [`WebSurface::mic_tap`]
//!                  (UX-01: the browser mic drives the you band; no audio leaves the page)
//!   POST /turn  -> one finished user utterance (WAV or raw PCM16LE). Stored in a bounded
//!                  [`TurnInbox`]; the host polls it with [`TurnInbox::take`] (never blocks).
//!                  Answers `202 {"id":N,"bytes":B,"format":"wav"|"pcm16"}`.
//!                  `X-Galion-Proceed: auto` is allowed. It is not a second route.
//!   GET /reply/<id>.wav -> reply audio the host published with [`ReplyBox::set_reply`];
//!                  `audio/wav`, 404 when unknown/evicted.
//!   GET / , /screen, /one, /one/ -> the caller's single-screen HTML page, only when one was set with
//!                      [`WebSurface::with_page`] (V1-01; the page itself is owned by
//!                      galion-console, this crate only serves the bytes); 404 otherwise
//!   GET /research, /research/ -> the Galion Research desk, same gate as /one.
//!   GET /research/hermes, GET /research/history, POST /research/ask -> Hermes
//!                      research app. The gateway key stays on the server.
//!   POST /research/sources, GET /research/sources -> files this visitor attached.
//!                      A random id in the `galion_research` cookie or the
//!                      `X-Galion-Research` header scopes that list. The id is
//!                      minted here; an id this process did not mint is ignored.
//!                      It is refused in the query string. Another visitor's
//!                      files are not listed and cannot be cited. Count, stored
//!                      size, and idle time are capped.
//! `/dot` also carries `"mode"` (idle|listening|speaking|rika), `"lean"` (Rika lean-in 0..1),
//! `"level"` (0..1, the band driving the state), `"e"` (empathy weight 0..1),
//! `"engine"` (one Luti step), and `"emotion"` (sense, label, valence, arousal, lean)
//! so the page can animate on its own clock. The circle fill stays the locked pigment.
//! When the host has published a reply, `/dot` additionally carries
//! `"reply":{"id":"<id>","url":"/reply/<id>.wav","ms":<duration ms>[,"hub_texture":<bool>]}` (latest reply only;
//! the page should play it once per new `id`). Absent when no reply was set / after
//! [`ReplyBox::clear_latest`].
//!
//! ## HTTP contract for hosts (e.g. `dot_console_demo`)
//! ```text
//! let web   = WebSurface::serve(addr).with_page(SCREEN_HTML);
//! let mic   = web.mic_tap();      // POST /mic   -> RMS level
//! let turns = web.turn_inbox();   // POST /turn  -> queued Turn (take() is non-blocking)
//! let reply = web.reply_box();    // GET /reply/<id>.wav + /dot "reply"
//! // host loop, each tick:
//! if let Some(t) = turns.take() {           // oldest pending turn, or None
//!     let pcm = t.pcm16();                  // Option<(rate, channels, Vec<i16>)>
//!     /* ASR/LLM/TTS live elsewhere */
//!     reply.set_reply(t.id.to_string(), encode_wav_pcm16(24_000, 1, &tts), ms);
//! }
//! ```
//! `POST /turn` body: either a RIFF/WAVE file (detected by magic, any Content-Type) or raw
//! little-endian mono PCM16 (`Content-Type: audio/L16` / `application/octet-stream`;
//! sample rate from `?rate=N`, `X-Sample-Rate: N` or `audio/L16;rate=N` (that precedence), default 16000; channels from
//! `?channels=N` / `;channels=N`, default 1). Max body [`MAX_BODY_BYTES`]; larger -> 413,
//! empty -> 400, missing Content-Length on POST /turn -> 411. `Expect: 100-continue` is honored.
//! The inbox keeps at most [`MAX_PENDING_TURNS`] (oldest dropped). Turn ids are a
//! per-surface counter starting at 1.
//!
//! UX-01: one short-lived thread per connection (capped), so one slow client (e.g. a
//! stalled tunnel socket) can no longer stall `/dot` for everyone else.
//! FP-02: the old `/frame` (full VizFrame JSON for a web console that never existed) is gone.
//! `VizFrame::to_json` stays for other callers.
//! **Loopback by default.** `serve` refuses any non-loopback address at `open()`
//! (`RenderError::Unsupported`). `0.0.0.0` is accepted only when
//! `GALION_ALLOW_PUBLIC_BIND=1` and auth is on (`GALION_REQUIRE_SESSION=1` plus an
//! exact `GALION_CORS_ORIGINS` list). This tree does not turn that flag on.
//! The HTML/JS console itself is owned by galion-console and is not embedded here.
//!
//! ## V2-S3 edge
//! `GALION_CORS_ORIGINS` unset → `Access-Control-Allow-Origin: *` (V1 local demo).
//! Staging must set exact origins; a literal `*` in the variable is not the wildcard.
//! A foreign `Origin` gets **403** and no allow-origin header. There is no WebSocket.
//! The phone path's session token is minted by `POST /session` and returned in the
//! **body** (`transport: body`). It is refused in the query string. Later audio posts
//! send `X-Galion-Session` (binary body) or `"session"` in the JSON body.

use crate::frame::VizFrame;
use crate::surface::{RenderError, RenderSurface, SurfaceCaps, SurfaceKind};
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Max concurrent connection threads; beyond this a request is answered inline.
pub const MAX_CONN_THREADS: usize = 32;
/// Max accepted request body (POST /turn audio). 4 MiB ≈ 131 s of 16 kHz mono PCM16.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// Max request head (request line + headers).
pub const MAX_HEAD_BYTES: usize = 16 * 1024;
/// Pending turns kept in the inbox; when full the oldest is dropped.
pub const MAX_PENDING_TURNS: usize = 8;
/// Reply WAVs kept addressable under `/reply/<id>.wav` (oldest evicted).
pub const MAX_STORED_REPLIES: usize = 8;
/// CORS `Access-Control-Allow-Headers` sent on every response (incl. OPTIONS preflight).
pub const CORS_ALLOW_HEADERS: &str =
    "Content-Type, X-Turn-Id, X-Sample-Rate, X-Barge-In, X-Galion-Proceed, X-Galion-Session, ngrok-skip-browser-warning";

pub use super::edge::{
    bind_decision, cors_allow_origin, parse_json_string_field, AI_DISCLOSURE_EN, AI_DISCLOSURE_HR,
};
use super::edge::{
    check_origin, mint_token, presented_session, query_carries_token, route_is_sensitive,
    route_needs_session, strip_ai_disclosure, wipe_string, zeroise_bytes, EdgeCfg, OriginCheck,
    SessionStore,
};
/// Whole-request read deadline (head + body).
const REQ_DEADLINE: Duration = Duration::from_secs(10);

/// Read side of the browser mic level (`POST /mic {"rms":x}`), for the host loop.
#[derive(Clone, Debug, Default)]
pub struct MicTap(Arc<Mutex<Option<(f32, Instant)>>>);

impl MicTap {
    /// Latest posted RMS (0..=1) if it arrived within `max_age`, else `None` (mic off,
    /// page closed, tunnel down). Never blocks for long; never panics on poison.
    pub fn latest(&self, max_age: Duration) -> Option<f32> {
        let g = self.0.lock().ok()?;
        let (rms, at) = (*g)?;
        (at.elapsed() <= max_age).then_some(rms)
    }
    /// Store a level (tests / embedding).
    pub fn set(&self, rms: f32) {
        if let Ok(mut g) = self.0.lock() { *g = Some((rms, Instant::now())); }
    }
}

/// `{"rms":0.0123,...}` -> 0.0123 (finite, clamped to 0..=1). Tiny parser, no serde.
pub fn parse_rms(body: &str) -> Option<f32> {
    let i = body.find("\"rms\"")? + 5;
    let rest = body[i..].trim_start().strip_prefix(':')?.trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))).unwrap_or(rest.len());
    let v: f32 = rest[..end].parse().ok()?;
    v.is_finite().then(|| v.clamp(0.0, 1.0))
}

// ---------------------------------------------------------------------------------------
// Turns (POST /turn) — audio in, host polls.
// ---------------------------------------------------------------------------------------

/// How the bytes of a [`Turn`] are encoded. This crate does no ASR; it only labels bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnFormat {
    /// A complete RIFF/WAVE file (detected by magic). Use [`decode_wav_pcm16`] / [`Turn::pcm16`].
    Wav,
    /// Raw little-endian signed 16-bit PCM, interleaved when `channels > 1`.
    Pcm16 { rate: u32, channels: u16 },
}

/// One user utterance posted by the page.
#[derive(Clone, Debug)]
pub struct Turn {
    /// Per-surface id, starting at 1. Use it (as a string) for the matching reply id.
    pub id: u64,
    pub format: TurnFormat,
    /// Exact request body bytes.
    pub bytes: Vec<u8>,
    pub received_at: Instant,
}

impl Turn {
    /// Decode to `(sample_rate, channels, samples)` for PCM16 WAV or raw PCM16; `None` for
    /// unsupported WAV encodings (float, 24-bit, compressed) or malformed bodies.
    pub fn pcm16(&self) -> Option<(u32, u16, Vec<i16>)> {
        match self.format {
            TurnFormat::Wav => decode_wav_pcm16(&self.bytes),
            TurnFormat::Pcm16 { rate, channels } => Some((rate, channels, le_i16(&self.bytes))),
        }
    }

    /// Overwrite the utterance bytes. Also run from [`Drop`].
    pub fn wipe(&mut self) {
        zeroise_bytes(&mut self.bytes);
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        self.wipe();
    }
}

#[derive(Debug, Default)]
struct InboxState { q: VecDeque<Turn>, dropped: u64 }

/// Bounded FIFO of posted turns shared between the HTTP threads (push) and the host loop
/// (take). Every method takes a short mutex only; none waits for new data.
#[derive(Clone, Debug, Default)]
pub struct TurnInbox { st: Arc<Mutex<InboxState>>, next_id: Arc<AtomicU64> }

impl TurnInbox {
    /// Enqueue bytes as a new turn and return its id (also used by tests / embedding).
    /// When [`MAX_PENDING_TURNS`] are already pending the oldest is dropped.
    pub fn push(&self, format: TurnFormat, mut bytes: Vec<u8>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut g) = self.st.lock() {
            while g.q.len() >= MAX_PENDING_TURNS {
                g.q.pop_front();
                g.dropped += 1;
            }
            g.q.push_back(Turn { id, format, bytes, received_at: Instant::now() });
        } else {
            zeroise_bytes(&mut bytes);
        }
        id
    }
    /// Oldest pending turn, removing it. Non-blocking; `None` when nothing is pending.
    pub fn take(&self) -> Option<Turn> { self.st.lock().ok()?.q.pop_front() }
    /// Newest pending turn, discarding any older ones (for "only answer the last thing said").
    pub fn take_latest(&self) -> Option<Turn> {
        let mut g = self.st.lock().ok()?;
        let t = g.q.pop_back();
        let n = g.q.len() as u64;
        g.q.clear();
        g.dropped += n;
        t
    }
    /// Number of turns waiting.
    pub fn pending(&self) -> usize { self.st.lock().map(|g| g.q.len()).unwrap_or(0) }
    /// Turns dropped because the inbox overflowed (or by `take_latest`).
    pub fn dropped(&self) -> u64 { self.st.lock().map(|g| g.dropped).unwrap_or(0) }
    /// Shared id clock so a typed line and a mic turn cannot reuse the same id.
    pub fn id_clock(&self) -> Arc<AtomicU64> { self.next_id.clone() }
}

/// One persona reply waiting to be spoken (Piper / flite), not a second page.
#[derive(Clone, Debug)]
pub struct SpokenLine {
    pub id: u64,
    pub reply: String,
    pub silence: bool,
    pub barge: bool,
}

impl Drop for SpokenLine {
    fn drop(&mut self) {
        let mut v = std::mem::take(&mut self.reply).into_bytes();
        zeroise_bytes(&mut v);
    }
}

#[derive(Debug, Default)]
struct SayState { q: VecDeque<SpokenLine> }

/// Typed turns from `POST /v2/say`. The phone host speaks `reply` and publishes a WAV.
#[derive(Clone, Debug)]
pub struct SayInbox { st: Arc<Mutex<SayState>>, next_id: Arc<AtomicU64> }

impl SayInbox {
    pub fn sharing(next_id: Arc<AtomicU64>) -> Self {
        Self { st: Arc::new(Mutex::new(SayState::default())), next_id }
    }
    /// Queue a line. Empty / silence still gets an id so barge-in can cancel playback.
    pub fn push(&self, reply: String, silence: bool, barge: bool) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut g) = self.st.lock() {
            while g.q.len() >= MAX_PENDING_TURNS { g.q.pop_front(); }
            g.q.push_back(SpokenLine { id, reply, silence, barge });
        }
        id
    }
    /// Newest line, dropping any older ones.
    pub fn take_latest(&self) -> Option<SpokenLine> {
        let mut g = self.st.lock().ok()?;
        let t = g.q.pop_back();
        g.q.clear();
        t
    }
}

// ---------------------------------------------------------------------------------------
// Replies (GET /reply/<id>.wav + /dot "reply").
// ---------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct ReplyState { store: VecDeque<(String, Arc<Vec<u8>>, u32)>, latest: Option<(String, u32)>, hub: Option<bool> }

/// Host-published reply audio. Clone freely; the HTTP threads read it.
#[derive(Clone, Debug, Default)]
pub struct ReplyBox(Arc<Mutex<ReplyState>>);

/// Reply ids are URL-safe tokens: 1..=64 chars of `[A-Za-z0-9_-]`.
pub fn valid_reply_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

impl ReplyBox {
    /// Publish `wav` (a complete WAV file, served verbatim) under `id`, make it the
    /// latest reply advertised in `/dot`, and keep the last [`MAX_STORED_REPLIES`]
    /// addressable. Re-using an id replaces it. Returns `false` (and stores nothing) if
    /// `id` fails [`valid_reply_id`]. `ms` is the duration hint shown in `/dot`
    /// (see [`wav_duration_ms`]).
    pub fn set_reply(&self, id: impl Into<String>, wav: Vec<u8>, ms: u32) -> bool {
        let id = id.into();
        if !valid_reply_id(&id) { return false; }
        let Ok(mut g) = self.0.lock() else { return false };
        let mut kept = VecDeque::new();
        for (k, mut wav, ms) in std::mem::take(&mut g.store) {
            if k == id {
                if let Some(buf) = Arc::get_mut(&mut wav) {
                    zeroise_bytes(buf);
                }
            } else {
                kept.push_back((k, wav, ms));
            }
        }
        g.store = kept;
        while g.store.len() >= MAX_STORED_REPLIES {
            if let Some((_, mut wav, _)) = g.store.pop_front() {
                if let Some(buf) = Arc::get_mut(&mut wav) {
                    zeroise_bytes(buf);
                }
            }
        }
        g.store.push_back((id.clone(), Arc::new(wav), ms));
        g.latest = Some((id, ms));
        g.hub = None;
        true
    }
    /// Like [`ReplyBox::set_reply`], and records whether the Jarvis hub texture is
    /// mixed into this WAV. `/dot` then carries `"hub_texture":<hub>` on the reply.
    pub fn set_reply_hub(&self, id: impl Into<String>, wav: Vec<u8>, ms: u32, hub: bool) -> bool {
        if !self.set_reply(id, wav, ms) { return false; }
        if let Ok(mut g) = self.0.lock() { g.hub = Some(hub); }
        true
    }
    /// Stop advertising a reply in `/dot` (stored WAVs stay fetchable until evicted).
    pub fn clear_latest(&self) { if let Ok(mut g) = self.0.lock() { g.latest = None; } }
    /// Stored WAV bytes for `id`.
    pub fn get(&self, id: &str) -> Option<Arc<Vec<u8>>> {
        self.0.lock().ok()?.store.iter().find(|(k, _, _)| k == id).map(|(_, w, _)| w.clone())
    }
    /// `{"id":"..","url":"/reply/...wav","ms":N}` for the latest reply, if any.
    pub fn latest_json(&self) -> Option<String> {
        let g = self.0.lock().ok()?;
        let (id, ms) = g.latest.as_ref()?;
        Some(match g.hub {
            Some(h) => format!("{{\"id\":\"{id}\",\"url\":\"/reply/{id}.wav\",\"ms\":{ms},\"hub_texture\":{h}}}"),
            None => format!("{{\"id\":\"{id}\",\"url\":\"/reply/{id}.wav\",\"ms\":{ms}}}"),
        })
    }
}

/// Live affect riding on `/dot`. `listen.emotion` is whatever the heartbeat
/// just wrote (galion-console `viz_from_heartbeat` ← `AffectFeed`). Label
/// `"neutral"` with zero valence and arousal is the untouched default, so
/// `sense` is false — the page then holds the neutral-warm empathy prior.
fn emotion_json(f: &VizFrame) -> String {
    let e = &f.listen.emotion;
    let valence = if e.valence.is_finite() { e.valence.clamp(-1.0, 1.0) } else { 0.0 };
    let arousal = if e.intensity.is_finite() { e.intensity.clamp(0.0, 1.0) } else { 0.0 };
    let lean = crate::dot::lean_in_of(f);
    let sense = e.label != "neutral" || valence != 0.0 || arousal != 0.0;
    let mut label = String::new();
    for c in e.label.chars() {
        if label.len() >= 24 { break; }
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' { label.push(c); }
    }
    if label.is_empty() { label.push_str("neutral"); }
    format!(
        "{{\"sense\":{},\"label\":\"{label}\",\"valence\":{valence:.3},\"arousal\":{arousal:.3},\"lean\":{lean:.3}}}",
        if sense { "true" } else { "false" }
    )
}

/// Insert `"reply":{..}` into a `/dot` JSON object (no-op when `reply` is `None`).
pub fn dot_with_reply(dot: &str, reply: Option<&str>) -> String {
    let Some(r) = reply else { return dot.to_string() };
    let t = dot.trim_end();
    let Some(inner) = t.strip_suffix('}') else { return dot.to_string() };
    let sep = if inner.trim_end().ends_with('{') { "" } else { "," };
    format!("{inner}{sep}\"reply\":{r}}}")
}

// ---------------------------------------------------------------------------------------
// WAV helpers (PCM16 only; std-only, no deps). Multichannel-aware minimal set used by the
// HTTP surface. For mono decode / 16→24 kHz resampling / reply encoding see [`crate::wav`].
// ---------------------------------------------------------------------------------------

fn le_i16(b: &[u8]) -> Vec<i16> { b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect() }

/// `true` when `b` starts with a RIFF/WAVE header.
pub fn is_wav(b: &[u8]) -> bool { b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WAVE" }

fn wav_fmt_and_data(b: &[u8]) -> Option<(u16, u16, u32, u16, &[u8])> {
    if !is_wav(b) { return None; }
    let (mut i, mut fmt) = (12usize, None);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().ok()?) as usize;
        let start = i + 8;
        // Streaming writers may leave the data size as 0 / 0xFFFFFFFF: clamp to what we have.
        let end = start.saturating_add(len).min(b.len());
        if id == b"fmt " && end - start >= 16 {
            let f = &b[start..end];
            let tag = u16::from_le_bytes([f[0], f[1]]);
            let ch = u16::from_le_bytes([f[2], f[3]]);
            let rate = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
            let bits = u16::from_le_bytes([f[14], f[15]]);
            fmt = Some((tag, ch, rate, bits));
        } else if id == b"data" {
            let (tag, ch, rate, bits) = fmt?;
            let data = if len == 0 { &b[start..] } else { &b[start..end] };
            return Some((tag, ch, rate, bits, data));
        }
        i = start.saturating_add(len).saturating_add(len & 1);
    }
    None
}

/// Decode a PCM16 WAV (format tag 1 or WAVE_FORMAT_EXTENSIBLE) to
/// `(sample_rate, channels, interleaved samples)`.
pub fn decode_wav_pcm16(b: &[u8]) -> Option<(u32, u16, Vec<i16>)> {
    let (tag, ch, rate, bits, data) = wav_fmt_and_data(b)?;
    ((tag == 1 || tag == 0xFFFE) && bits == 16 && ch > 0 && rate > 0).then(|| (rate, ch, le_i16(data)))
}

/// Duration of a WAV's data chunk in ms (any PCM bit depth), for the `/dot` hint.
pub fn wav_duration_ms(b: &[u8]) -> Option<u32> {
    let (_, ch, rate, bits, data) = wav_fmt_and_data(b)?;
    let bytes_per_s = rate as u64 * ch as u64 * (bits as u64 / 8).max(1);
    (bytes_per_s > 0).then(|| (data.len() as u64 * 1000 / bytes_per_s) as u32)
}

/// Encode interleaved PCM16 samples as a canonical 44-byte-header WAV.
pub fn encode_wav_pcm16(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    v.extend_from_slice(&(channels * 2).to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples { v.extend_from_slice(&s.to_le_bytes()); }
    v
}

// ---------------------------------------------------------------------------------------
// HTTP request reading.
// ---------------------------------------------------------------------------------------

struct Request {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    /// Minted research visitor, set only for a research route.
    research_session: Option<String>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

type HttpErr = (&'static str, &'static str);

fn find_head_end(b: &[u8]) -> Option<usize> { b.windows(4).position(|w| w == b"\r\n\r\n") }

/// Read one request: head until CRLFCRLF (<= [`MAX_HEAD_BYTES`]), then exactly
/// `Content-Length` body bytes (<= `max_body`) with looped reads and an overall deadline.
fn read_request<S: Read + Write>(s: &mut S, max_body: usize) -> Result<Request, HttpErr> {
    let deadline = Instant::now() + REQ_DEADLINE;
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 16 * 1024];
    let head_end = loop {
        if let Some(i) = find_head_end(&buf) { break i; }
        if buf.len() > MAX_HEAD_BYTES { return Err(("431 Request Header Fields Too Large", "head too large")); }
        if Instant::now() > deadline { return Err(("408 Request Timeout", "timeout")); }
        match s.read(&mut chunk) {
            Ok(0) => return Err(("400 Bad Request", "incomplete head")),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(("408 Request Timeout", "timeout")),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut parts = lines.next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/");
    let (path, query) = match target.split_once('?') { Some((p, q)) => (p.to_string(), q.to_string()), None => (target.to_string(), String::new()) };
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let mut req = Request { method, path, query, headers, body: Vec::new(), research_session: None };
    if req.header("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
        return Err(("411 Length Required", "chunked bodies unsupported; send Content-Length"));
    }
    let len = match req.header("content-length") {
        Some(v) => v.parse::<usize>().map_err(|_| ("400 Bad Request", "bad content-length"))?,
        None => 0,
    };
    if len > max_body { return Err(("413 Payload Too Large", "body too large")); }
    let mut body = buf.split_off(head_end + 4);
    if len > 0 && body.len() < len && req.header("expect").is_some_and(|v| v.eq_ignore_ascii_case("100-continue")) {
        let _ = s.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
    }
    body.reserve(len.saturating_sub(body.len()));
    while body.len() < len {
        if Instant::now() > deadline { return Err(("408 Request Timeout", "timeout")); }
        let want = (len - body.len()).min(chunk.len());
        match s.read(&mut chunk[..want]) {
            Ok(0) => return Err(("400 Bad Request", "incomplete body")),
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(("408 Request Timeout", "timeout")),
        }
    }
    body.truncate(len);
    req.body = body;
    Ok(req)
}

/// `rate=24000` / `channels=2` from a query string or a `audio/L16;rate=..` content type.
fn param_u32(src: &str, key: &str) -> Option<u32> {
    src.split(['&', ';']).filter_map(|kv| kv.trim().split_once('=')).find(|(k, _)| k.trim().eq_ignore_ascii_case(key)).and_then(|(_, v)| v.trim().parse().ok())
}

fn classify_turn(req: &Request) -> TurnFormat {
    if is_wav(&req.body) { return TurnFormat::Wav; }
    let ct = req.header("content-type").unwrap_or("");
    let hdr_rate = req.header("x-sample-rate").and_then(|v| v.trim().parse().ok());
    let rate = param_u32(&req.query, "rate").or(hdr_rate).or_else(|| param_u32(ct, "rate")).filter(|r| (1..=384_000).contains(r)).unwrap_or(16_000);
    let ch = param_u32(&req.query, "channels").or_else(|| param_u32(ct, "channels")).filter(|c| (1..=8).contains(c)).unwrap_or(1) as u16;
    TurnFormat::Pcm16 { rate, channels: ch }
}

// ---------------------------------------------------------------------------------------
// Surface.
// ---------------------------------------------------------------------------------------

/// Everything the connection threads share.
#[derive(Clone)]
struct Shared {
    latest_dot: Arc<Mutex<String>>,
    latest_mic: Arc<Mutex<String>>,
    mic: MicTap,
    turns: TurnInbox,
    says: SayInbox,
    replies: ReplyBox,
    page: Option<&'static str>,
    v2_page: Option<&'static str>,
    nebula_vocabulary: Option<&'static str>,
    floor: Arc<Mutex<crate::v2_persona::Floor>>,
    edge: EdgeCfg,
    sessions: Arc<SessionStore>,
    /// First accepted `/turn` of this process already carried the Art. 50 line.
    disclosure_sent: Arc<AtomicBool>,
    research: Arc<Mutex<ResearchDesk>>,
}

/// Unit tests pin the V1 demo defaults. A developer shell that exports
/// `GALION_CORS_ORIGINS` must not flip parallel tests. The demo binary is not
/// built with `cfg(test)` and reads the environment in [`initial_edge`].
fn initial_edge() -> EdgeCfg {
    if cfg!(test) { EdgeCfg::v1_demo() } else { EdgeCfg::from_env() }
}

pub struct WebSurface {
    bind: Option<SocketAddr>,
    sh: Shared,
    stop: Arc<AtomicBool>,
    local_addr: Option<SocketAddr>,
    worker: Option<JoinHandle<()>>,
    /// Witness integrator. One Luti step per `present`, at 240 Hz `dt`.
    /// Not the phone's display loop, and not `Heartbeat::step`.
    engine: crate::OrbEngine,
}

impl WebSurface {
    /// Mailbox only (no socket) — for embedding into another server or for tests.
    pub fn mailbox() -> Self {
        let edge = initial_edge();
        let turns = TurnInbox::default();
        let says = SayInbox::sharing(turns.id_clock());
        let sh = Shared {
            latest_dot: Arc::new(Mutex::new("{}".into())),
            latest_mic: Arc::new(Mutex::new("{\"rms\":0}".into())),
            mic: MicTap::default(),
            turns,
            says,
            replies: ReplyBox::default(),
            page: None,
            v2_page: None,
            nebula_vocabulary: None,
            floor: Arc::new(Mutex::new(crate::v2_persona::Floor::new())),
            sessions: Arc::new(SessionStore::new(edge.session_ttl)),
            disclosure_sent: Arc::new(AtomicBool::new(false)),
            research: Arc::new(Mutex::new(ResearchDesk::new())),
            edge,
        };
        Self {
            bind: None,
            sh,
            stop: Arc::new(AtomicBool::new(false)),
            local_addr: None,
            worker: None,
            engine: crate::OrbEngine::idle(),
        }
    }
    /// Serve on `addr` (use "127.0.0.1:0" for an ephemeral port).
    pub fn serve(addr: SocketAddr) -> Self {
        let mut s = Self::mailbox();
        s.bind = Some(addr);
        s
    }
    /// Serve `html` at `/`, `/screen`, `/one`, and `/one/` (set before `open()`).
    /// `GET /research` uses the same gate and serves the research prompt.
    pub fn with_page(mut self, html: &'static str) -> Self {
        self.sh.page = Some(html);
        self
    }
    /// `GET /nebula/vocabulary.mjs`. The one-dot page imports `drawNebula` from here.
    pub fn with_nebula_vocabulary(mut self, js: &'static str) -> Self {
        self.sh.nebula_vocabulary = Some(js);
        self
    }
    /// Body of `GET /v2`, which redirects to `/`. Set before `open()`.
    pub fn with_v2_page(mut self, html: &'static str) -> Self {
        self.sh.v2_page = Some(html);
        self
    }
    /// Typed lines from `POST /v2/say` (persona reply text for Piper).
    pub fn say_inbox(&self) -> SayInbox { self.sh.says.clone() }
    /// Instruction-layer floor. The phone host feeds mic transcripts through it.
    pub fn persona_floor(&self) -> Arc<Mutex<crate::v2_persona::Floor>> { self.sh.floor.clone() }
    /// Exact `GALION_CORS_ORIGINS` value. Empty keeps the V1 wildcard.
    /// A literal `*` is not the wildcard.
    pub fn with_cors_origins(mut self, origins: impl Into<String>) -> Self {
        self.sh.edge.cors_origins = origins.into();
        self
    }
    /// `GALION_REQUIRE_SESSION`. An exact CORS list also requires a session.
    pub fn with_require_session(mut self, on: bool) -> Self {
        self.sh.edge.require_session = on;
        self
    }
    /// `GALION_ALLOW_PUBLIC_BIND`. `open` still refuses `0.0.0.0` without auth.
    pub fn with_allow_public_bind(mut self, on: bool) -> Self {
        self.sh.edge.allow_public_bind = on;
        self
    }
    /// Art. 50 disclosure. The demo default is on.
    pub fn with_disclosure(mut self, on: bool) -> Self {
        self.sh.edge.disclosure = on;
        self
    }
    /// Override the session lifetime (tests). The env var is whole seconds, clamped.
    pub fn with_session_ttl(mut self, ttl: Duration) -> Self {
        self.sh.edge.session_ttl = ttl;
        self.sh.sessions = Arc::new(SessionStore::new(ttl));
        self
    }
    /// How long a research visitor's files stay after the last request (tests).
    pub fn with_research_idle(self, idle: Duration) -> Self {
        if let Ok(mut desk) = self.sh.research.lock() {
            desk.idle = idle;
        }
        self
    }
    /// How many research visitors are kept in memory (tests). At least one.
    pub fn with_research_session_cap(self, cap: usize) -> Self {
        if let Ok(mut desk) = self.sh.research.lock() {
            desk.max_sessions = cap.max(1);
        }
        self
    }
    /// Latest `/dot` JSON exactly as served (includes `"reply"` when one is published).
    pub fn latest_dot_json(&self) -> String {
        let d = self.sh.latest_dot.lock().map(|g| g.clone()).unwrap_or_default();
        dot_with_reply(&d, self.sh.replies.latest_json().as_deref())
    }
    pub fn local_addr(&self) -> Option<SocketAddr> { self.local_addr }
    /// Shared handle to the browser mic level; take it before boxing the surface.
    pub fn mic_tap(&self) -> MicTap { self.sh.mic.clone() }
    /// Shared handle to posted turns (`POST /turn`); take it before boxing the surface.
    pub fn turn_inbox(&self) -> TurnInbox { self.sh.turns.clone() }
    /// Shared handle for publishing reply audio; take it before boxing the surface.
    pub fn reply_box(&self) -> ReplyBox { self.sh.replies.clone() }
    /// Attach a Hermes client to the persona floor. The next `/v2/say` content
    /// turn uses it, and the floor lock is not held during that call.
    pub fn set_hermes(&self, client: galion_cascade::HermesLlm) {
        if let Ok(mut floor) = self.sh.floor.lock() {
            floor.set_hermes(client);
        }
    }
    /// Hermes client for `POST /research/ask`. The key stays inside the client.
    pub fn set_research_hermes(&self, client: galion_cascade::HermesLlm) {
        if let Ok(mut desk) = self.sh.research.lock() {
            desk.client = Some(client);
        }
    }
}

const V2_REDIRECT_HTML: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"0; url=/\"><title>Galion Voice</title><script>location.replace(\"/\");</script></head><body><p><a href=\"/\">Galion Voice</a></p></body></html>";

/// Galion Research. Served at `/research` and `/research/` when a product page is set,
/// the same gate as `/one`.
const RESEARCH_HTML: &str = include_str!("../../../crates/galion-console/assets/research.html");

struct ResearchNote {
    id: u64,
    question: String,
    status: &'static str,
    reply: String,
    citations: Vec<(u32, String)>,
    found: Vec<String>,
    inferred: Vec<String>,
    unknown: Vec<String>,
    at: Instant,
}

/// A file the user attached. `kind` is `file` or `image`. Image pixels are not read.
struct DeskSource {
    id: u32,
    name: String,
    text: String,
    kind: &'static str,
}

/// One visitor. Files and the notes that cite them live here, not on the desk.
struct ResearchSession {
    sources: Vec<DeskSource>,
    notes: VecDeque<ResearchNote>,
    next_source: u32,
    next_id: u64,
    touched: Instant,
    /// Last standing for this visitor. `idle` until an ask finishes.
    status: &'static str,
}

impl ResearchSession {
    fn fresh(now: Instant) -> Self {
        Self {
            sources: Vec::new(),
            notes: VecDeque::new(),
            next_source: 0,
            next_id: 0,
            touched: now,
            status: "idle",
        }
    }
}

/// Idle research files are dropped. A visitor who returns after this starts empty.
const RESEARCH_SESSION_IDLE: Duration = Duration::from_secs(30 * 60);
/// Live research visitors kept in memory. The least recently used is dropped.
const MAX_RESEARCH_SESSIONS: usize = 64;
/// Notes kept per visitor. Oldest dropped. Citations live in these notes.
const MAX_RESEARCH_NOTES: usize = 32;
const RESEARCH_COOKIE: &str = "galion_research";
const RESEARCH_HEADER: &str = "x-galion-research";

struct ResearchDesk {
    client: Option<galion_cascade::HermesLlm>,
    sessions: HashMap<String, ResearchSession>,
    idle: Duration,
    max_sessions: usize,
}

impl ResearchDesk {
    fn new() -> Self {
        Self {
            client: if cfg!(test) { None } else { galion_cascade::HermesLlm::attached_from_env() },
            sessions: HashMap::new(),
            idle: RESEARCH_SESSION_IDLE,
            max_sessions: MAX_RESEARCH_SESSIONS,
        }
    }
}

impl Drop for ResearchDesk {
    fn drop(&mut self) {
        let ids: Vec<String> = self.sessions.keys().cloned().collect();
        for id in ids {
            drop_research_session(self, &id);
        }
    }
}

fn session_gateway_status(desk: &ResearchDesk, sess: &ResearchSession) -> &'static str {
    if sess.status == "Running" {
        return "Running";
    }
    if desk.client.is_none() {
        return "Unreachable";
    }
    if sess.status == "idle" {
        "Idle"
    } else {
        sess.status
    }
}

fn research_sources_json(sources: &[DeskSource]) -> String {
    let items: Vec<String> = sources.iter().map(|s| {
        format!(
            "{{\"id\":{},\"name\":{},\"kind\":\"{}\",\"chars\":{}}}",
            s.id,
            galion_cascade::hermes::json_string(&s.name),
            s.kind,
            s.text.chars().count()
        )
    }).collect();
    format!(
        "{{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"sources\":[{}]}}",
        items.join(",")
    )
}

fn empty_sources_json() -> String {
    "{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"sources\":[]}".into()
}

fn clean_source_name(raw: &str) -> Option<String> {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if base.is_empty() || base == "." || base == ".." {
        return None;
    }
    let mut out = String::new();
    for c in base.chars() {
        if c.is_control() || matches!(c, '<' | '>' | '"' | '\\') {
            continue;
        }
        out.push(c);
        if out.chars().count() >= 80 {
            break;
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() { None } else { Some(out) }
}

fn is_image_name(name: &str) -> bool {
    let low = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg", ".heic", ".avif", ".tif", ".tiff"]
        .iter()
        .any(|ext| low.ends_with(ext))
}

/// A JSON string field, scanned so a value cannot be read as a key. `cap` is in chars.
fn json_object_string(body: &str, key: &str, cap: usize) -> Option<String> {
    let needle = format!("\"{key}\"");
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;
    while i < chars.len() {
        let (byte_i, c) = chars[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            let slice = &body[byte_i..];
            if let Some(after) = slice.strip_prefix(&needle) {
                let after = after.trim_start();
                if let Some(after) = after.strip_prefix(':') {
                    return parse_json_string_capped(after.trim_start(), cap);
                }
            }
            in_string = true;
        }
        i += 1;
    }
    None
}

fn parse_json_string_capped(s: &str, cap: usize) -> Option<String> {
    let mut chars = s.chars();
    if chars.next() != Some('"') {
        return None;
    }
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '/' => out.push('/'),
                'u' => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        hex.push(chars.next()?);
                    }
                    let code = u32::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
                other => out.push(other),
            }
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
        if out.chars().count() > cap {
            return Some(out.chars().take(cap).collect());
        }
    }
    None
}

fn session_chars(sources: &[DeskSource]) -> usize {
    sources.iter().map(|s| s.text.chars().count()).sum()
}

fn wipe_source(src: &mut DeskSource) {
    wipe_string(&mut src.name);
    wipe_string(&mut src.text);
}

fn wipe_note(note: &mut ResearchNote) {
    wipe_string(&mut note.question);
    wipe_string(&mut note.reply);
    for (_, text) in &mut note.citations {
        wipe_string(text);
    }
    for line in &mut note.found {
        wipe_string(line);
    }
    for line in &mut note.inferred {
        wipe_string(line);
    }
    for line in &mut note.unknown {
        wipe_string(line);
    }
}

fn drop_research_session(desk: &mut ResearchDesk, id: &str) {
    let Some((mut key, mut sess)) = desk.sessions.remove_entry(id) else { return };
    for src in &mut sess.sources {
        wipe_source(src);
    }
    sess.sources.clear();
    for note in &mut sess.notes {
        wipe_note(note);
    }
    sess.notes.clear();
    wipe_string(&mut key);
}

fn expire_idle(desk: &mut ResearchDesk, now: Instant) {
    let idle = desk.idle;
    let dead: Vec<String> = desk
        .sessions
        .iter()
        .filter(|(_, sess)| now.saturating_duration_since(sess.touched) >= idle)
        .map(|(id, _)| id.clone())
        .collect();
    for id in dead {
        drop_research_session(desk, &id);
    }
}

fn evict_over_cap(desk: &mut ResearchDesk, keep: &str) {
    let cap = desk.max_sessions.max(1);
    while desk.sessions.len() > cap {
        let oldest = desk
            .sessions
            .iter()
            .filter(|(id, _)| id.as_str() != keep)
            .min_by_key(|(_, sess)| sess.touched)
            .map(|(id, _)| id.clone());
        let Some(oldest) = oldest else { break };
        drop_research_session(desk, &oldest);
    }
}

/// 32 bytes, hex. Same shape as the phone token, a different store.
fn research_id_ok(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn mint_research_id() -> String {
    let mut id = mint_token();
    id.make_ascii_lowercase();
    id
}

/// Reuse `presented` only when this process minted it and it has not gone idle.
/// Anything else is a new id. An unminted id never selects another visitor.
fn ensure_research_session(desk: &mut ResearchDesk, presented: &[String]) -> String {
    let now = Instant::now();
    expire_idle(desk, now);
    for id in presented {
        if research_id_ok(id) && desk.sessions.contains_key(id) {
            if let Some(sess) = desk.sessions.get_mut(id) {
                sess.touched = now;
            }
            evict_over_cap(desk, id);
            return id.clone();
        }
    }
    let mut id = mint_research_id();
    while desk.sessions.contains_key(&id) {
        id = mint_research_id();
    }
    desk.sessions.insert(id.clone(), ResearchSession::fresh(now));
    evict_over_cap(desk, &id);
    id
}

/// In-process path. The HTTP gate has already minted `id`, or a test named one.
fn touch_named_session(desk: &mut ResearchDesk, id: &str) -> bool {
    if !research_id_ok(id) {
        return false;
    }
    let now = Instant::now();
    expire_idle(desk, now);
    let sess = desk.sessions.entry(id.to_string()).or_insert_with(|| ResearchSession::fresh(now));
    sess.touched = now;
    evict_over_cap(desk, id);
    desk.sessions.contains_key(id)
}

fn cookie_value(header: Option<&str>, name: &str) -> Option<String> {
    let header = header?;
    for part in header.split(';') {
        let (k, v) = part.trim().split_once('=')?;
        if k.trim().eq_ignore_ascii_case(name) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Header first, then the cookie. Only well-formed ids. The desk still
/// ignores an id it did not mint.
fn research_candidates(req: &Request) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(h) = req.header(RESEARCH_HEADER) {
        let t = h.trim().to_ascii_lowercase();
        if research_id_ok(&t) {
            out.push(t);
        }
    }
    if let Some(c) = cookie_value(req.header("cookie"), RESEARCH_COOKIE) {
        let t = c.trim().to_ascii_lowercase();
        if research_id_ok(&t) && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

fn research_route(method: &str, path: &str) -> bool {
    if method == "OPTIONS" {
        return false;
    }
    let method = if method == "HEAD" { "GET" } else { method };
    matches!(
        (method, path),
        ("GET", "/research" | "/research/" | "/research/hermes" | "/research/history" | "/research/sources")
            | ("POST", "/research/ask" | "/research/sources")
    )
}

fn research_set_cookie(id: &str, idle: Duration) -> String {
    let age = idle.as_secs().max(1);
    format!("Set-Cookie: {RESEARCH_COOKIE}={id}; Path=/research; HttpOnly; SameSite=Lax; Max-Age={age}\r\n")
}

fn bind_research_session(desk: &Arc<Mutex<ResearchDesk>>, req: &Request) -> Result<(String, String), ()> {
    let presented = research_candidates(req);
    let mut d = desk.lock().map_err(|_| ())?;
    let id = ensure_research_session(&mut d, &presented);
    let cookie = research_set_cookie(&id, d.idle);
    Ok((id, cookie))
}

fn research_add_source(desk: &Arc<Mutex<ResearchDesk>>, session: &str, raw: &str) -> Result<String, &'static str> {
    let name = json_object_string(raw, "name", 160).ok_or("name")?;
    let name = clean_source_name(&name).ok_or("name")?;
    let kind_raw = json_object_string(raw, "kind", 16).unwrap_or_default();
    let kind = if kind_raw == "image" || is_image_name(&name) { "image" } else { "file" };
    let text = if kind == "image" {
        String::new()
    } else {
        json_object_string(raw, "text", galion_cascade::hermes::MAX_RESEARCH_SOURCE_CHARS).unwrap_or_default()
    };
    let Ok(mut d) = desk.lock() else { return Err("research") };
    if !touch_named_session(&mut d, session) {
        return Err("research");
    }
    let Some(sess) = d.sessions.get_mut(session) else { return Err("research") };
    let incoming = text.chars().count();
    let char_cap = galion_cascade::hermes::MAX_RESEARCH_SOURCES
        * galion_cascade::hermes::MAX_RESEARCH_SOURCE_CHARS;
    while !sess.sources.is_empty()
        && (sess.sources.len() >= galion_cascade::hermes::MAX_RESEARCH_SOURCES
            || session_chars(&sess.sources) + incoming > char_cap)
    {
        let mut old = sess.sources.remove(0);
        wipe_source(&mut old);
    }
    sess.next_source = sess.next_source.saturating_add(1);
    let id = sess.next_source.max(1);
    sess.sources.push(DeskSource { id, name, text, kind });
    Ok(research_sources_json(&sess.sources))
}

fn json_list(items: &[String]) -> String {
    items.iter().map(|s| galion_cascade::hermes::json_string(s)).collect::<Vec<_>>().join(",")
}

fn research_status_json(desk: &ResearchDesk, sess: Option<&ResearchSession>) -> String {
    let key = desk.client.as_ref().map(|c| c.key_state()).unwrap_or("none");
    let model = desk.client.as_ref().map(|c| c.label().to_string()).unwrap_or_else(|| galion_cascade::LLM_NAME.into());
    let status = sess.map(|s| session_gateway_status(desk, s)).unwrap_or_else(|| {
        if desk.client.is_none() { "Unreachable" } else { "Idle" }
    });
    let history = sess.map(|s| s.notes.len()).unwrap_or(0);
    format!(
        "{{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"voice\":\"galion-research\",\"product\":\"galion-one\",\"model\":{},\"key\":\"{key}\",\"status\":\"{status}\",\"history\":{history}}}",
        galion_cascade::hermes::json_string(&model),
    )
}

fn research_note_json(note: &ResearchNote) -> String {
    let cites: Vec<String> = note.citations.iter().map(|(n, text)| {
        format!("{{\"n\":{n},\"text\":{}}}", galion_cascade::hermes::json_string(text))
    }).collect();
    format!(
        "{{\"id\":{},\"status\":\"{}\",\"age_s\":{},\"question\":{},\"reply\":{},\"citations\":[{}],\"found\":[{}],\"inferred\":[{}],\"unknown\":[{}]}}",
        note.id,
        note.status,
        note.at.elapsed().as_secs(),
        galion_cascade::hermes::json_string(&note.question),
        galion_cascade::hermes::json_string(&note.reply),
        cites.join(","),
        json_list(&note.found),
        json_list(&note.inferred),
        json_list(&note.unknown)
    )
}

fn research_history_json(notes: &VecDeque<ResearchNote>) -> String {
    let items: Vec<String> = notes.iter().rev().map(research_note_json).collect();
    format!("{{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"items\":[{}]}}", items.join(","))
}

fn research_unreachable_json() -> String {
    "{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"status\":\"Unreachable\",\"reply\":\"The research path is not reachable from here.\",\"citations\":[]}".into()
}

fn research_ask(desk: &Arc<Mutex<ResearchDesk>>, session: &str, question: &str) -> String {
    let (id, client, model_fallback, sources) = {
        let Ok(mut d) = desk.lock() else {
            return research_unreachable_json();
        };
        let store = touch_named_session(&mut d, session);
        let model = d.client.as_ref().map(|c| c.label().to_string()).unwrap_or_else(|| galion_cascade::LLM_NAME.into());
        let client = d.client.clone();
        let (note_id, sources) = if store {
            let Some(sess) = d.sessions.get_mut(session) else {
                return research_unreachable_json();
            };
            sess.status = "Running";
            sess.next_id = sess.next_id.saturating_add(1);
            let note_id = sess.next_id;
            let sources: Vec<galion_cascade::hermes::ResearchSource> = sess.sources.iter().map(|s| {
                galion_cascade::hermes::ResearchSource { id: s.id, name: s.name.clone(), text: s.text.clone() }
            }).collect();
            (note_id, sources)
        } else {
            (0, Vec::new())
        };
        (note_id, client, model, sources)
    };
    let turn = match client {
        Some(c) => c.research_turn(question, &sources),
        None => galion_cascade::hermes::ResearchAnswer {
            text: galion_cascade::UNREACHABLE.into(),
            citations: Vec::new(),
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: Vec::new(),
            standing: "Unreachable",
        },
    };
    let status = turn.standing;
    let reply = if status == "Unreachable" {
        "The research path is not reachable from here. Local. What is already known stays marked local.".to_string()
    } else {
        turn.text
    };
    let citations = if status == "Unreachable" || status == "Denied" { Vec::new() } else { turn.citations };
    let mut found = if status == "Found" { turn.found } else { Vec::new() };
    let mut inferred = if status == "Found" || status == "Inferred" { turn.inferred } else { Vec::new() };
    let mut unknown = if status == "Found" || status == "Inferred" || status == "Unknown" { turn.unknown } else { Vec::new() };
    if status == "Unreachable" {
        found.clear();
        inferred.clear();
        unknown.clear();
        unknown.push(reply.clone());
    }
    if status == "Found" && found.is_empty() && !reply.is_empty() {
        found.push(reply.clone());
    }
    if status == "Inferred" && inferred.is_empty() && !reply.is_empty() {
        inferred.push(reply.clone());
    }
    if status == "Unknown" && unknown.is_empty() && !reply.is_empty() {
        unknown.push(reply.clone());
    }
    let note = ResearchNote {
        id,
        question: question.to_string(),
        status,
        reply,
        citations,
        found,
        inferred,
        unknown,
        at: Instant::now(),
    };
    let body = format!(
        "{{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"model\":{},{}",
        galion_cascade::hermes::json_string(&model_fallback),
        research_note_json(&note).trim_start_matches('{')
    );
    if let Ok(mut d) = desk.lock() {
        if let Some(sess) = d.sessions.get_mut(session) {
            sess.status = status;
            sess.touched = Instant::now();
            sess.notes.push_back(note);
            while sess.notes.len() > MAX_RESEARCH_NOTES {
                if let Some(mut old) = sess.notes.pop_front() {
                    wipe_note(&mut old);
                }
            }
        }
    }
    body
}

const SECURITY_HEADERS: &str = concat!(
    "X-Content-Type-Options: nosniff\r\n",
    "Referrer-Policy: no-referrer\r\n",
    "Permissions-Policy: microphone=(self)\r\n",
    "Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline' blob:; style-src 'self' 'unsafe-inline'; connect-src 'self'; worker-src 'self' blob:; img-src 'self' data: blob:; media-src 'self' blob:; base-uri 'none'; frame-ancestors 'none'\r\n",
);

enum Acao {
    /// Echo this origin, or omit the header when `None` (same-origin / no Origin).
    Keep(Option<String>),
    /// Foreign Origin, or a token placed in the query string. No allow-origin header.
    Omit,
    /// The request never parsed, so there is no Origin to judge.
    Unscoped,
}

fn format_acao(origin: Option<String>) -> String {
    match origin {
        Some(o) => format!("Access-Control-Allow-Origin: {o}\r\nVary: Origin\r\n"),
        None => String::new(),
    }
}

fn handle(mut stream: std::net::TcpStream, sh: &Shared) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(2000)));
    let (status, ctype, mut body, acao, cookie) = match read_request(&mut stream, MAX_BODY_BYTES) {
        Err((st, msg)) => (st, "text/plain", msg.as_bytes().to_vec(), Acao::Unscoped, None),
        Ok(mut req) => dispatch(&mut req, sh),
    };
    let cors = match acao {
        Acao::Omit => String::new(),
        Acao::Keep(o) => format_acao(o),
        Acao::Unscoped => {
            if sh.edge.wildcard() { format_acao(Some("*".to_string())) } else { String::new() }
        }
    };
    let location = if status.starts_with("302") { "Location: /\r\n" } else { "" };
    let set_cookie = cookie.unwrap_or_default();
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{location}{cors}{set_cookie}Access-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: {CORS_ALLOW_HEADERS}\r\n{SECURITY_HEADERS}Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
    // The socket has the bytes. Do not leave a token, a session id, or a WAV in this buffer.
    wipe_string(&mut head);
    zeroise_bytes(&mut body);
}

fn dispatch(req: &mut Request, sh: &Shared) -> (&'static str, &'static str, Vec<u8>, Acao, Option<String>) {
    if query_carries_token(&req.query) {
        zeroise_bytes(&mut req.body);
        return ("400 Bad Request", "text/plain", b"session token must not be in the URL".to_vec(), Acao::Omit, None);
    }
    let origin = req.header("origin").map(str::to_string);
    let sensitive = route_is_sensitive(&req.method, &req.path);
    match check_origin(origin.as_deref(), &sh.edge.cors_origins, sensitive) {
        OriginCheck::Reject => {
            zeroise_bytes(&mut req.body);
            ("403 Forbidden", "text/plain", b"origin rejected".to_vec(), Acao::Omit, None)
        }
        OriginCheck::Allow { acao } => {
            if route_needs_session(&req.method, &req.path) && sh.edge.session_required() {
                let mut presented = presented_session(
                    req.header("x-galion-session"),
                    req.header("content-type"),
                    &req.body,
                );
                let ok = presented.as_deref().is_some_and(|t| sh.sessions.check(t));
                if let Some(ref mut p) = presented {
                    wipe_string(p);
                }
                if !ok {
                    zeroise_bytes(&mut req.body);
                    return ("401 Unauthorized", "text/plain", b"session required".to_vec(), Acao::Keep(acao), None);
                }
            }
            let cookie = if sh.page.is_some() && research_route(&req.method, &req.path) {
                match bind_research_session(&sh.research, req) {
                    Ok((id, cookie)) => {
                        req.research_session = Some(id);
                        Some(cookie)
                    }
                    Err(()) => {
                        return ("500 Internal Server Error", "text/plain", b"research".to_vec(), Acao::Keep(acao), None);
                    }
                }
            } else {
                None
            };
            let (st, ct, body) = route(req, sh);
            (st, ct, body, Acao::Keep(acao), cookie)
        }
    }
}

/// Mic JSON is re-served on `GET /mic`. Drop the session field before that.
fn mic_store_body(cleaned: &str) -> String {
    if !cleaned.starts_with('{') || cleaned.len() > 512 || cleaned.contains("\"session\"") {
        if let Some(rms) = parse_rms(cleaned) {
            return format!("{{\"rms\":{rms}}}");
        }
        return "{\"raw\":true}".to_string();
    }
    cleaned.to_string()
}

/// `/v2/cc0` reply: licence name, link, and this session's acceptance.
fn cc0_json(accepted: bool) -> String {
    format!(
        "{{\"licence\":\"{}\",\"url\":\"{}\",\"accepted\":{accepted}}}",
        galion_consent::CC0_LICENCE_NAME,
        galion_consent::CC0_LICENCE_URL
    )
}

fn route(req: &mut Request, sh: &Shared) -> (&'static str, &'static str, Vec<u8>) {
    let head = req.method == "HEAD";
    let method = if head { "GET" } else { req.method.as_str() };
    let txt = |s: &str| s.as_bytes().to_vec();
    let (st, ct, body) = match (method, req.path.as_str()) {
        ("OPTIONS", _) => ("204 No Content", "text/plain", Vec::new()),
        ("POST", "/session") => {
            if req.body.is_empty() {
                return ("400 Bad Request", "text/plain", txt("empty body"));
            }
            if req.body.len() > 512 {
                zeroise_bytes(&mut req.body);
                return ("400 Bad Request", "text/plain", txt("session body too large"));
            }
            let mut token = sh.sessions.mint();
            let ttl_s = sh.edge.session_ttl.as_secs();
            let json = format!("{{\"token\":\"{token}\",\"ttl_s\":{ttl_s},\"transport\":\"body\"}}");
            wipe_string(&mut token);
            zeroise_bytes(&mut req.body);
            ("200 OK", "application/json", json.into_bytes())
        }
        ("POST", "/mic") => {
            let mut owned = String::from_utf8_lossy(&req.body).into_owned();
            let cleaned = owned.trim();
            let store = mic_store_body(cleaned);
            if let Some(r) = parse_rms(cleaned) { sh.mic.set(r); }
            if let Ok(mut g) = sh.latest_mic.lock() { *g = store; }
            wipe_string(&mut owned);
            zeroise_bytes(&mut req.body);
            ("204 No Content", "text/plain", Vec::new())
        }
        ("GET", "/v2/cc0") => {
            let mut s = presented_session(req.header("x-galion-session"), None, &[]);
            let accepted = s.as_deref().is_some_and(|t| galion_consent::cc0_acceptances().accepted(t));
            if let Some(ref mut t) = s { wipe_string(t); }
            ("200 OK", "application/json", cc0_json(accepted).into_bytes())
        }
        // CC0 licence registration (clone time). In memory; the token is
        // hashed, never stored. Body: {"accept":true|false} (+ "session" or the header).
        ("POST", "/v2/cc0") => {
            let mut s = presented_session(req.header("x-galion-session"), req.header("content-type"), &req.body);
            let raw = String::from_utf8_lossy(&req.body).into_owned();
            let accept = crate::v2_persona::json_field_bool(&raw, "accept");
            zeroise_bytes(&mut req.body);
            let Some(mut token) = s.take() else {
                return ("400 Bad Request", "text/plain", txt("session required for CC0 acceptance"));
            };
            // Registration time: accept + one clone gate run (Rust, in
            // process). Turns never re-check CC0.
            if accept { galion_consent::cc0_register(&token, &galion_consent::Env::process()); } else { galion_consent::cc0_unregister(&token); }
            let accepted = galion_consent::cc0_acceptances().accepted(&token);
            wipe_string(&mut token);
            ("200 OK", "application/json", cc0_json(accepted).into_bytes())
        }
        ("GET", "/v2/decisions") => (
            "200 OK",
            "application/json",
            crate::v2_persona::decisions_json().into_bytes(),
        ),
        ("POST", "/v2/decide") => {
            let raw = String::from_utf8_lossy(&req.body).into_owned();
            zeroise_bytes(&mut req.body);
            match crate::v2_persona::decision_reply(&raw) {
                Some(json) => ("200 OK", "application/json", json.into_bytes()),
                None => ("400 Bad Request", "text/plain", txt("decisions")),
            }
        }
        ("POST", "/v2/say") => {
            if req.body.is_empty() {
                return ("400 Bad Request", "text/plain", txt("empty body"));
            }
            let raw = String::from_utf8_lossy(&req.body);
            let text = crate::v2_persona::json_field_string(&raw, "text").unwrap_or_default();
            let barge = crate::v2_persona::json_field_bool(&raw, "barge");
            let e = crate::v2_persona::json_field_f32(&raw, "e");
            let sense = crate::v2_persona::json_field_bool(&raw, "sense");
            zeroise_bytes(&mut req.body);
            // Classify under the lock, then drop it before the model call.
            // A 3 s Hermes timeout must not stall GET /v2/state or /dot.
            let prepared = match sh.floor.lock() {
                Ok(mut floor) => Some(floor.begin_say(&text, barge, e, sense)),
                Err(_) => None,
            };
            let Some(prepared) = prepared else {
                return (
                    "200 OK",
                    "application/json",
                    txt("{\"persona\":\"samantha\",\"reply\":\"\",\"silence\":true,\"film_voice\":false}"),
                );
            };
            let model = prepared.model_reply();
            let json = match sh.floor.lock() {
                Ok(mut floor) => floor.finish_say_json(prepared, model),
                Err(_) => "{\"persona\":\"samantha\",\"reply\":\"\",\"silence\":true,\"film_voice\":false}".to_string(),
            };
            let silence = crate::v2_persona::json_field_bool(&json, "silence");
            let reply = crate::v2_persona::json_field_string(&json, "reply").unwrap_or_default();
            let barge_out = crate::v2_persona::json_field_bool(&json, "barge");
            let quiet = silence || reply.trim().is_empty();
            sh.says.push(if quiet { String::new() } else { reply }, quiet, barge_out);
            zeroise_bytes(&mut req.body);
            ("200 OK", "application/json", json.into_bytes())
        }
        ("POST", "/research/ask") if sh.page.is_some() => {
            let raw = String::from_utf8_lossy(&req.body).into_owned();
            zeroise_bytes(&mut req.body);
            let Some(session) = req.research_session.clone() else {
                return ("500 Internal Server Error", "text/plain", txt("research"));
            };
            let Some(question) = crate::v2_persona::json_field_string(&raw, "question") else {
                return ("400 Bad Request", "text/plain", txt("question"));
            };
            let question = question.trim().to_string();
            if question.is_empty() {
                return ("400 Bad Request", "text/plain", txt("question"));
            }
            let json = research_ask(&sh.research, &session, &question);
            ("200 OK", "application/json", json.into_bytes())
        }
        ("POST", "/research/sources") if sh.page.is_some() => {
            let raw = String::from_utf8_lossy(&req.body).into_owned();
            zeroise_bytes(&mut req.body);
            let Some(session) = req.research_session.clone() else {
                return ("500 Internal Server Error", "text/plain", txt("research"));
            };
            match research_add_source(&sh.research, &session, &raw) {
                Ok(json) => ("200 OK", "application/json", json.into_bytes()),
                Err("name") => ("400 Bad Request", "text/plain", txt("name")),
                Err(_) => ("500 Internal Server Error", "text/plain", txt("research")),
            }
        }
        ("POST", "/turn") => {
            if req.header("content-length").is_none() {
                zeroise_bytes(&mut req.body);
                return ("411 Length Required", "text/plain", txt("Content-Length required"));
            }
            if req.body.is_empty() { return ("400 Bad Request", "text/plain", txt("empty body")); }
            let fmt = classify_turn(req);
            let n = req.body.len();
            let bytes = std::mem::take(&mut req.body);
            let id = sh.turns.push(fmt, bytes);
            let f = match fmt { TurnFormat::Wav => "wav", TurnFormat::Pcm16 { .. } => "pcm16" };
            let json = if sh.edge.disclosure && !sh.disclosure_sent.swap(true, Ordering::SeqCst) {
                format!(
                    "{{\"id\":{id},\"bytes\":{n},\"format\":\"{f}\",\"ai_disclosure\":{{\"article\":\"50\",\"hr\":\"{AI_DISCLOSURE_HR}\",\"en\":\"{AI_DISCLOSURE_EN}\",\"audible\":true,\"visible\":true,\"first\":true}}}}"
                )
            } else {
                format!("{{\"id\":{id},\"bytes\":{n},\"format\":\"{f}\"}}")
            };
            ("202 Accepted", "application/json", json.into_bytes())
        }
        ("GET", p) => {
            if let Some(id) = p.strip_prefix("/reply/").and_then(|r| r.strip_suffix(".wav")) {
                match sh.replies.get(id) {
                    Some(w) if valid_reply_id(id) => ("200 OK", "audio/wav", w.as_ref().clone()),
                    _ => ("404 Not Found", "text/plain", txt("not found")),
                }
            } else { match p {
                "/nebula/vocabulary.mjs" => match sh.nebula_vocabulary {
                    Some(js) => ("200 OK", "text/javascript; charset=utf-8", js.as_bytes().to_vec()),
                    None => ("404 Not Found", "text/plain", txt("not found")),
                },
                "/" | "/screen" | "/one" | "/one/" if sh.page.is_some() => {
                    let html = sh.page.unwrap_or_default();
                    let page = if sh.edge.disclosure { html.to_string() } else { strip_ai_disclosure(html) };
                    ("200 OK", "text/html; charset=utf-8", page.into_bytes())
                }
                "/research/hermes" if sh.page.is_some() => {
                    let Some(session) = req.research_session.as_deref() else {
                        return ("500 Internal Server Error", "text/plain", txt("research"));
                    };
                    let Ok(desk) = sh.research.lock() else {
                        return ("500 Internal Server Error", "text/plain", txt("research"));
                    };
                    let body = research_status_json(&desk, desk.sessions.get(session));
                    ("200 OK", "application/json", body.into_bytes())
                }
                "/research/history" if sh.page.is_some() => {
                    let Some(session) = req.research_session.as_deref() else {
                        return ("500 Internal Server Error", "text/plain", txt("research"));
                    };
                    let Ok(desk) = sh.research.lock() else {
                        return ("500 Internal Server Error", "text/plain", txt("research"));
                    };
                    let notes = desk.sessions.get(session).map(|s| research_history_json(&s.notes)).unwrap_or_else(|| {
                        "{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"items\":[]}".into()
                    });
                    ("200 OK", "application/json", notes.into_bytes())
                }
                "/research/sources" if sh.page.is_some() => {
                    let Some(session) = req.research_session.as_deref() else {
                        return ("500 Internal Server Error", "text/plain", txt("research"));
                    };
                    let Ok(desk) = sh.research.lock() else {
                        return ("500 Internal Server Error", "text/plain", txt("research"));
                    };
                    let body = desk.sessions.get(session).map(|s| research_sources_json(&s.sources)).unwrap_or_else(empty_sources_json);
                    ("200 OK", "application/json", body.into_bytes())
                }
                "/research" | "/research/" if sh.page.is_some() => {
                    ("200 OK", "text/html; charset=utf-8", RESEARCH_HTML.as_bytes().to_vec())
                }
                "/dot" => {
                    let d = sh.latest_dot.lock().map(|g| g.clone()).unwrap_or_default();
                    let (persona, mood, from_turn) = sh.floor.lock().map(|g| (g.id().to_string(), g.mood().to_string(), g.mood_from_turn())).unwrap_or_else(|_| ("samantha".into(), "calm".into(), false));
                    // Mood is attached only after a turn, and then emotion.label matches it.
                    // `reply` stays the last field.
                    let mood_field = if from_turn { Some(mood.as_str()) } else { None };
                    let body = dot_with_reply(&crate::v2_persona::annotate_dot(&d, &persona, mood_field), sh.replies.latest_json().as_deref());
                    ("200 OK", "application/json", body.into_bytes())
                }
                "/v2" | "/v2/" => {
                    let html = sh.v2_page.unwrap_or(V2_REDIRECT_HTML);
                    ("302 Found", "text/html; charset=utf-8", html.as_bytes().to_vec())
                }
                "/v2/state" => {
                    let json = sh.floor.lock().map(|g| g.state_json()).unwrap_or_else(|_| "{}".into());
                    ("200 OK", "application/json", json.into_bytes())
                }
                "/mic" => ("200 OK", "application/json", sh.latest_mic.lock().map(|g| g.clone()).unwrap_or_else(|_| "{\"rms\":0}".into()).into_bytes()),
                "/health" => ("200 OK", "text/plain", txt("ok")),
                _ => ("404 Not Found", "text/plain", txt("not found")),
            } }
        }
        _ => ("404 Not Found", "text/plain", txt("not found")),
    };
    if head { (st, ct, Vec::new()) } else { (st, ct, body) }
}

impl RenderSurface for WebSurface {
    fn kind(&self) -> SurfaceKind { SurfaceKind::Web }
    fn caps(&self) -> SurfaceCaps { SurfaceCaps { color: true, pixels: true, max_fps: 12.5, size: (0, 0) } }
    fn open(&mut self) -> Result<(), RenderError> {
        let Some(addr) = self.bind else { return Ok(()) };
        if self.worker.is_some() { return Ok(()); }
        if let Err(msg) = bind_decision(
            addr.ip(),
            self.sh.edge.allow_public_bind,
            &self.sh.edge.cors_origins,
            self.sh.edge.require_session,
        ) {
            return Err(RenderError::Unsupported(msg));
        }
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        self.local_addr = Some(listener.local_addr()?);
        self.stop.store(false, Ordering::SeqCst);
        let (sh, stop) = (self.sh.clone(), self.stop.clone());
        self.worker = Some(std::thread::spawn(move || {
            let live = Arc::new(AtomicUsize::new(0));
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((s, _)) => {
                        let _ = s.set_nonblocking(false);
                        let _ = s.set_nodelay(true);
                        let _ = s.set_write_timeout(Some(Duration::from_millis(1000)));
                        if live.load(Ordering::SeqCst) >= MAX_CONN_THREADS {
                            handle(s, &sh);
                            continue;
                        }
                        live.fetch_add(1, Ordering::SeqCst);
                        let (c, l) = (sh.clone(), live.clone());
                        let spawned = std::thread::Builder::new().name("galion-web-conn".into()).spawn(move || {
                            handle(s, &c);
                            l.fetch_sub(1, Ordering::SeqCst);
                        });
                        if spawned.is_err() { live.fetch_sub(1, Ordering::SeqCst); }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(4))
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        }));
        Ok(())
    }
    fn present(&mut self, f: &VizFrame) -> Result<(), RenderError> {
        let dot = crate::dot::DotParams::from_frame(f);
        let hz = crate::DisplayHz::Hz240;
        let sample = self.engine.step_toward_dot(&dot, hz.dt_s());
        let mut d = dot.to_json(f.t_ms);
        if d.ends_with('}') {
            d.pop();
            let level = match crate::dot::DotState::from_frame(f) {
                crate::dot::DotState::Speaking => f.speak.level,
                crate::dot::DotState::Listening => f.listen.level,
                crate::dot::DotState::Idle => 0.0,
            };
            let level = if level.is_finite() { level.clamp(0.0, 1.0) } else { 0.0 };
            let empathy = if f.empathy.is_finite() { f.empathy.clamp(0.0, 1.0) } else { 0.0 };
            d.push_str(&format!(
                ",\"mode\":\"{}\",\"lean\":{:.3},\"level\":{:.3},\"e\":{:.3},\"engine\":{},\"emotion\":{}}}",
                crate::dot::screen_mode(f),
                crate::dot::lean_in_of(f),
                level,
                empathy,
                sample.account.to_json(hz.hz()),
                emotion_json(f)
            ));
        }
        if let Ok(mut g) = self.sh.latest_dot.lock() { *g = d; }
        Ok(())
    }
    fn close(&mut self) -> Result<(), RenderError> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.worker.take() { let _ = h.join(); }
        Ok(())
    }
}

impl Drop for WebSurface {
    fn drop(&mut self) { let _ = self.close(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_ask_reply_is_one_json_object() {
        let desk = Arc::new(Mutex::new(ResearchDesk::new()));
        let body = research_ask(&desk, &"11".repeat(32), "Is the reply one object?");
        assert!(body.starts_with('{') && body.ends_with("]}") && !body.ends_with("]}}"), "{body}");
        let (mut depth, mut in_str, mut esc, mut closed_at) = (0i32, false, false, None);
        for (i, c) in body.char_indices() {
            if in_str {
                if esc { esc = false } else if c == '\\' { esc = true } else if c == '"' { in_str = false }
                continue;
            }
            match c {
                '"' => in_str = true,
                '{' | '[' => depth += 1,
                '}' | ']' => {
                    depth -= 1;
                    if depth == 0 { closed_at = Some(i); }
                }
                _ => {}
            }
        }
        assert_eq!(depth, 0, "{body}");
        assert_eq!(closed_at, Some(body.len() - 1), "{body}");
    }

    #[test]
    fn no_frame_route() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        w.present(&VizFrame::default()).unwrap();
        let mut s = std::net::TcpStream::connect(w.local_addr().unwrap()).unwrap();
        s.write_all(b"GET /frame HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 404"), "{out}");
        w.close().unwrap();
    }

    #[test]
    fn refuses_public_bind() {
        let mut w = WebSurface::serve("0.0.0.0:0".parse().unwrap()).with_allow_public_bind(false);
        let msg = w.open().unwrap_err().to_string();
        assert!(msg.contains("localhost-only"), "{msg}");
        assert!(w.local_addr().is_none());
    }

    #[test]
    fn refuses_public_bind_when_flagged_without_auth() {
        let mut w = WebSurface::serve("0.0.0.0:0".parse().unwrap())
            .with_allow_public_bind(true)
            .with_cors_origins("*")
            .with_require_session(true);
        let msg = w.open().unwrap_err().to_string();
        assert!(msg.contains("exact") && msg.contains("GALION_REQUIRE_SESSION"), "{msg}");
        let mut w = WebSurface::serve("0.0.0.0:0".parse().unwrap())
            .with_allow_public_bind(true)
            .with_cors_origins("https://voice.example");
        let msg = w.open().unwrap_err().to_string();
        assert!(msg.contains("GALION_REQUIRE_SESSION"), "{msg}");
        assert!(w.local_addr().is_none(), "flag without auth must not listen");
    }

    fn get(addr: SocketAddr, path: &str) -> String {
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn page_only_when_set_and_rika_mode() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        assert!(get(w.local_addr().unwrap(), "/").starts_with("HTTP/1.1 404"));
        assert!(get(w.local_addr().unwrap(), "/research").starts_with("HTTP/1.1 404"));
        w.close().unwrap();
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        for p in ["/", "/screen", "/one", "/one/", "/?kiosk=1"] {
            let r = get(a, p);
            assert!(r.starts_with("HTTP/1.1 200") && r.contains("text/html") && r.contains("<canvas>"), "{r}");
        }
        let decisions = get(a, "/v2/decisions");
        assert!(decisions.contains("galion-session-decision/1") && decisions.contains("\"id\":\"here\""), "{decisions}");
        let body = r#"{"answers":[{"id":"english","yes":true},{"id":"quiet","yes":false},{"id":"here","yes":true}]}"#;
        let mut s = std::net::TcpStream::connect(a).unwrap();
        s.write_all(
            format!(
                "POST /v2/decide HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.contains("HTTP/1.1 200") && out.contains("Keep replies in English: yes"), "{out}");
        let partial = r#"{"answers":[{"id":"here","yes":true}]}"#;
        let mut s = std::net::TcpStream::connect(a).unwrap();
        s.write_all(
            format!(
                "POST /v2/decide HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{partial}",
                partial.len()
            )
            .as_bytes(),
        )
        .unwrap();
        let mut bad = String::new();
        s.read_to_string(&mut bad).unwrap();
        assert!(bad.starts_with("HTTP/1.1 400"), "{bad}");
        let research = get(a, "/research");
        assert!(research.starts_with("HTTP/1.1 200") && research.contains("Galion Research"), "{research}");
        let mut f = VizFrame::default();
        f.speak.emotion = crate::frame::Emotion { label: "lean-in", intensity: 0.5, valence: 0.0 };
        w.present(&f).unwrap();
        let r = get(a, "/dot");
        assert!(r.contains("\"state\":\"idle\"") && r.contains("\"mode\":\"rika\"") && r.contains("\"lean\":0.500"), "{r}");
        assert!(r.contains("\"e\":0.000") && r.contains("\"dot\":\"#"), "{r}");
        assert!(r.contains("\"engine\":{\"profile\":\"luti\",\"hz\":240"), "{r}");
        assert!(r.contains("\"emotion\"") && r.contains("\"sense\":false"), "{r}");
        assert!(!r.contains("#B695CF"), "empathy hue is not a circle fill");
        assert!(r.trim_end().ends_with('}'));
        assert!(r.contains("\"persona\":\"samantha\""), "{r}");
        w.close().unwrap();
    }

    #[test]
    fn research_route_serves_the_prompt() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let one = get(a, "/one");
        assert!(one.starts_with("HTTP/1.1 200") && one.contains("<canvas>") && !one.contains("id=\"prompt\""), "{one}");
        for p in ["/research", "/research/", "/research?kiosk=1"] {
            let r = get(a, p);
            assert!(r.starts_with("HTTP/1.1 200") && r.contains("text/html"), "{p} {r}");
            let body = r.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
            for needle in [
                "<title>Galion Research</title>",
                "data-product=\"research\"",
                "id=\"prompt\"",
                "id=\"voice\"",
                "id=\"voice-dot\"",
                "id=\"voice-name\"",
                "aria-label=\"Voice\"",
                "function markInner",
                "edge:\"#E8E8ED\"",
                "Galion Research is the Hermes research app, a voice of Galion One",
                "id=\"hermes-status\"",
                "id=\"history\"",
                "data-app=\"hermes\"",
                "/research/ask",
                "/research/hermes",
                "/research/history",
                "/research/sources",
                "readAsText",
                "data-voice=\"galion-research\"",
                "id=\"add-file\"",
                "id=\"add-image\"",
                "id=\"sources\"",
                "id=\"answer\"",
                "id=\"citations\"",
                "id=\"to-one\"",
                "href=\"/one\"",
                "No source yet.",
                "None in hand.",
                "galion.app/research",
                "Ultra $220/€220 per month",
                "id=\"sources-local\"",
                "id=\"sources-reading\"",
                "id=\"sources-arriving\"",
                "id=\"notebook\"",
                "id=\"found\"",
                "id=\"inferred\"",
                "id=\"unknown\"",
                "id=\"experiment\"",
                "Ask first",
                "data-voice-to-voice=\"on\"",
                "id=\"ui-toggle\"",
                "id=\"helpers\"",
                "ratio:\"5:4\"",
                "Local. What is already known stays marked local.",
                "hz:432",
                "hz:216",
                "hz:540",
                "hz:648",
                "G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network",
                "Developed and distributed by Galion Labs",
                "Perpetual Open MIT License",
                "Copyright © 2026 Galion Labs. All rights reserved.",
                "Permission is hereby granted, free of charge",
                "Copyright (c) 2026 Maciej Grajczyk",
                "#0B0F14",
                "#58C4FF",
                "#B8E8FF",
                "#0B5FA5",
                "#F5F5F7",
            ] {
                assert!(body.contains(needle), "{p} missing {needle}");
            }
            assert!(body.contains("<canvas id=\"voice-dot\""), "the restyled Voice V2 dot is the entry");
            assert!(!body.contains("id=\"s\""), "research is not the full-screen one-dot page");
            assert!(!body.contains("data-mark=\"orb\""), "the full-screen orb stays on /one");
            let id = research_cookie(&r);
            assert!(!body.contains(&id), "the session id stays out of the page");
            assert!(r.to_ascii_lowercase().contains("httponly"), "{r}");
            assert!(r.contains("Path=/research"), "{r}");
        }
        w.close().unwrap();
    }

    fn post_json(addr: SocketAddr, path: &str, body: &str) -> String {
        call(addr, "POST", path, "", body)
    }

    fn call(addr: SocketAddr, method: &str, path: &str, headers: &str, body: &str) -> String {
        let len = if method == "POST" {
            format!("Content-Type: application/json\r\nContent-Length: {}\r\n", body.len())
        } else {
            String::new()
        };
        let req = format!("{method} {path} HTTP/1.1\r\nHost: x\r\n{headers}{len}\r\n{body}");
        String::from_utf8(raw(addr, req.as_bytes())).unwrap_or_default()
    }

    fn research_cookie(resp: &str) -> String {
        let line = resp.lines().find(|l| l.trim().len() >= 11 && l.trim()[..11].eq_ignore_ascii_case("set-cookie:")).unwrap_or_else(|| panic!("no research cookie in {resp}"));
        let rest = line.split_once(':').unwrap().1.trim();
        let id = rest.strip_prefix("galion_research=").unwrap_or_else(|| panic!("cookie {rest}")).split(';').next().unwrap().trim();
        assert_eq!(id.len(), 64, "{id}");
        assert!(research_id_ok(id), "{id}");
        id.to_string()
    }

    fn research_cookie_header(id: &str) -> String {
        format!("Cookie: {RESEARCH_COOKIE}={id}\r\n")
    }

    #[test]
    fn research_ask_reaches_hermes_without_leaking_the_key() {
        let key = "test-gateway-key-not-for-git";
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let off = get(a, "/research/hermes");
        assert!(off.contains("\"app\":\"hermes\"") && off.contains("\"key\":\"none\"") && off.contains("\"status\":\"Unreachable\""), "{off}");
        assert!(!off.contains(key) && !off.contains("Bearer") && !off.contains("GALION_LLM_KEY"), "{off}");
        let who = research_cookie_header(&research_cookie(&off));
        let denied = call(a, "POST", "/research/ask", &who, r#"{"question":"Where is the source?"}"#);
        assert!(denied.contains("HTTP/1.1 200") && denied.contains("The research path is not reachable from here."), "{denied}");
        assert!(!denied.contains(key), "{denied}");
        let hist = call(a, "GET", "/research/history", &who, "");
        assert!(hist.contains("Where is the source?") && hist.contains("\"status\":\"Unreachable\""), "{hist}");
        let empty = post_json(a, "/research/ask", r#"{"question":"  "}"#);
        assert!(empty.starts_with("HTTP/1.1 400"), "{empty}");
        w.close().unwrap();

        let payload = "{\"choices\":[{\"message\":{\"content\":\"The wash is amber to violet.\\n[1] Galion brand note\"}}]}";
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else { return };
            let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = [0u8; 8192];
            let mut got = Vec::new();
            loop {
                match sock.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        got.extend_from_slice(&buf[..n]);
                        if got.windows(4).any(|w| w == b"\r\n\r\n") { break; }
                    }
                }
            }
            let resp = format!(
                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.set_research_hermes(galion_cascade::HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            "hermes-agent",
            key,
            Duration::from_millis(1000),
        ));
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let ready = get(a, "/research/hermes");
        assert!(ready.contains("\"key\":\"set\"") && ready.contains("\"status\":\"Idle\"") && ready.contains("hermes-agent"), "{ready}");
        assert!(!ready.contains(key), "{ready}");
        let who = research_cookie_header(&research_cookie(&ready));
        let asked = call(a, "POST", "/research/ask", &who, r#"{"question":"What colour is the wash?"}"#);
        assert!(asked.contains("HTTP/1.1 200") && asked.contains("\"status\":\"Unknown\""), "{asked}");
        assert!(asked.contains("amber to violet"), "{asked}");
        assert!(!asked.contains("Galion brand note"), "an unprovided source must not be cited: {asked}");
        assert!(asked.contains("\"citations\":[]"), "{asked}");
        assert!(!asked.contains("NotFound") && !asked.contains("\"status\":\"Returned\""), "{asked}");
        assert!(asked.contains("\"app\":\"hermes\""), "{asked}");
        assert!(!asked.contains(key) && !asked.contains("Bearer"), "{asked}");
        let hist = call(a, "GET", "/research/history", &who, "");
        assert!(hist.contains("What colour is the wash?") && hist.contains("\"status\":\"Unknown\""), "{hist}");
        assert!(!hist.contains(key) && !hist.contains("Galion brand note"), "{hist}");
        w.close().unwrap();
    }

    #[test]
    fn research_sources_are_the_only_citations() {
        let key = "test-gateway-key-not-for-git";
        let content = "The wash is amber.\\nFound\\n- The wash is amber. [2]\\n- A paper measured violet. [3]\\nInferred\\n- A cooler mix would read as night.\\nUnknown\\n- The hex is not in the note.\\n[2] wash-note.txt\\n[3] Invented Journal 2024";
        let payload = format!("{{\"choices\":[{{\"message\":{{\"content\":\"{content}\"}}}}]}}");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        let seen_w = seen.clone();
        std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else { return };
            let _ = sock.set_read_timeout(Some(Duration::from_millis(800)));
            let mut buf = [0u8; 8192];
            let mut got = Vec::new();
            let mut header_end = None;
            loop {
                match sock.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        got.extend_from_slice(&buf[..n]);
                        if header_end.is_none() {
                            header_end = got.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
                        }
                        if let Some(end) = header_end {
                            let head = String::from_utf8_lossy(&got[..end]);
                            let len = head.lines().find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                if !k.trim().eq_ignore_ascii_case("content-length") {
                                    return None;
                                }
                                v.trim().parse::<usize>().ok()
                            }).unwrap_or(0);
                            if got.len() >= end + len {
                                break;
                            }
                        }
                    }
                }
            }
            if let Ok(s) = String::from_utf8(got) {
                *seen_w.lock().unwrap() = s;
            }
            let resp = format!(
                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.set_research_hermes(galion_cascade::HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            "hermes-agent",
            key,
            Duration::from_millis(1000),
        ));
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let empty = get(a, "/research/sources");
        assert!(empty.contains("\"sources\":[]"), "{empty}");
        let who = research_cookie_header(&research_cookie(&empty));
        let unnamed = call(a, "POST", "/research/sources", &who, r#"{"text":"no name"}"#);
        assert!(unnamed.starts_with("HTTP/1.1 400"), "{unnamed}");
        let image = call(a, "POST", "/research/sources", &who, r#"{"name":"../shot.png","kind":"image","text":"pixels"}"#);
        assert!(image.contains("HTTP/1.1 200") && image.contains("shot.png") && image.contains("\"kind\":\"image\"") && image.contains("\"chars\":0"), "{image}");
        assert!(!image.contains("../"), "{image}");
        let file = call(
            a,
            "POST",
            "/research/sources",
            &who,
            r#"{"name":"notes/wash-note.txt","kind":"file","text":"The wash is amber."}"#,
        );
        assert!(file.contains("wash-note.txt") && file.contains("\"kind\":\"file\"") && file.contains("\"chars\":18") && file.contains("\"id\":2"), "{file}");
        assert!(!file.contains("notes/"), "{file}");
        let asked = call(a, "POST", "/research/ask", &who, r#"{"question":"What colour is the wash?"}"#);
        assert!(asked.contains("HTTP/1.1 200") && asked.contains("\"status\":\"Found\""), "{asked}");
        assert!(asked.contains("wash-note.txt") && asked.contains("\"n\":2"), "{asked}");
        assert!(asked.contains("amber"), "{asked}");
        assert!(asked.contains("cooler") || asked.contains("night"), "{asked}");
        assert!(!asked.contains("Invented Journal"), "a source that was not attached must not be cited: {asked}");
        assert!(!asked.contains("\"n\":3"), "{asked}");
        assert!(!asked.contains("NotFound"), "{asked}");
        assert!(!asked.contains(key) && !asked.contains("Bearer"), "{asked}");
        let upstream = seen.lock().unwrap().clone();
        assert!(upstream.contains("wash-note.txt"), "{upstream}");
        assert!(upstream.contains("The wash is amber."), "{upstream}");
        assert!(!upstream.contains("Invented Journal"), "{upstream}");
        assert!(upstream.contains("Authorization: Bearer "), "{upstream}");
        let hist = call(a, "GET", "/research/history", &who, "");
        assert!(hist.contains("\"status\":\"Found\"") && !hist.contains("Invented Journal"), "{hist}");
        w.close().unwrap();
    }

    #[test]
    fn research_desk_keeps_the_latest_eight_sources() {
        let desk = Arc::new(Mutex::new(ResearchDesk::new()));
        let sid = "aa".repeat(32);
        let other = "bb".repeat(32);
        research_add_source(&desk, &other, r#"{"name":"kept.txt","text":"other-visitor"}"#).unwrap();
        for i in 1..=9 {
            let body = format!(r#"{{"name":"n{i}.txt","text":"t{i}"}}"#);
            research_add_source(&desk, &sid, &body).unwrap();
        }
        let long = "x".repeat(2_500);
        let clipped = research_add_source(&desk, &sid, &format!(r#"{{"name":"long.txt","text":"{long}"}}"#)).unwrap();
        assert!(clipped.contains("\"chars\":2000"), "{clipped}");
        let d = desk.lock().unwrap();
        let mine = &d.sessions[&sid];
        assert_eq!(mine.sources.len(), 8);
        assert_eq!(mine.sources[0].name, "n3.txt");
        assert_eq!(mine.sources[6].name, "n9.txt");
        assert_eq!(mine.sources[7].name, "long.txt");
        assert_eq!(mine.sources[7].text.chars().count(), 2_000);
        let kept = &d.sessions[&other];
        assert_eq!(kept.sources.len(), 1);
        assert_eq!(kept.sources[0].name, "kept.txt");
        assert_eq!(kept.sources[0].text, "other-visitor");
        let total: usize = mine.sources.iter().map(|s| s.text.chars().count()).sum();
        assert!(total <= galion_cascade::hermes::MAX_RESEARCH_SOURCES * galion_cascade::hermes::MAX_RESEARCH_SOURCE_CHARS);
    }

    #[test]
    fn research_sources_are_per_session() {
        let desk = Arc::new(Mutex::new(ResearchDesk::new()));
        let a = "aa".repeat(32);
        let b = "bb".repeat(32);
        let added = research_add_source(&desk, &a, r#"{"name":"alpha.txt","text":"alpha-only-sentence"}"#).unwrap();
        assert!(added.contains("alpha.txt"), "{added}");
        assert!(!added.contains("alpha-only-sentence") && !added.contains("beta"), "{added}");
        research_add_source(&desk, &b, r#"{"name":"beta.txt","text":"beta-only-sentence"}"#).unwrap();
        let ask_a = research_ask(&desk, &a, "What is in alpha?");
        let ask_b = research_ask(&desk, &b, "What is in beta?");
        assert!(!ask_a.contains("beta-only-sentence") && !ask_a.contains("beta.txt"), "{ask_a}");
        assert!(!ask_b.contains("alpha-only-sentence") && !ask_b.contains("alpha.txt"), "{ask_b}");
        let d = desk.lock().unwrap();
        let hist_a = research_history_json(&d.sessions[&a].notes);
        let hist_b = research_history_json(&d.sessions[&b].notes);
        assert!(hist_a.contains("What is in alpha?") && !hist_a.contains("beta"), "{hist_a}");
        assert!(hist_b.contains("What is in beta?") && !hist_b.contains("alpha"), "{hist_b}");
        assert_eq!(d.sessions[&a].sources[0].text, "alpha-only-sentence");
        assert_eq!(d.sessions[&b].sources[0].text, "beta-only-sentence");
        drop(d);
        for i in 0..33 {
            research_ask(&desk, &a, &format!("note {i}"));
        }
        let d = desk.lock().unwrap();
        assert_eq!(d.sessions[&a].notes.len(), MAX_RESEARCH_NOTES);
        assert_eq!(d.sessions[&a].notes.front().unwrap().question, "note 1");
        assert!(d.sessions[&b].notes.len() < MAX_RESEARCH_NOTES);
    }

    #[test]
    fn research_session_drops_idle_files_and_extra_visitors() {
        let desk = Arc::new(Mutex::new(ResearchDesk::new()));
        {
            let mut d = desk.lock().unwrap();
            d.idle = Duration::from_secs(60);
            d.max_sessions = 2;
        }
        let a = "aa".repeat(32);
        let b = "bb".repeat(32);
        let c = "cc".repeat(32);
        research_add_source(&desk, &a, r#"{"name":"a.txt","text":"alpha-only-sentence"}"#).unwrap();
        {
            let mut d = desk.lock().unwrap();
            d.sessions.get_mut(&a).unwrap().touched = Instant::now() - Duration::from_secs(30);
        }
        research_add_source(&desk, &b, r#"{"name":"b.txt","text":"beta-only-sentence"}"#).unwrap();
        {
            let mut d = desk.lock().unwrap();
            d.sessions.get_mut(&b).unwrap().touched = Instant::now() - Duration::from_secs(20);
        }
        research_add_source(&desk, &c, r#"{"name":"c.txt","text":"gamma-only-sentence"}"#).unwrap();
        {
            let d = desk.lock().unwrap();
            assert!(d.sessions.get(&a).is_none(), "the quietest visitor is dropped at the cap");
            assert!(d.sessions[&b].sources.iter().any(|s| s.text == "beta-only-sentence"));
            assert!(d.sessions[&c].sources.iter().any(|s| s.text == "gamma-only-sentence"));
            assert!(d.sessions.values().all(|s| s.sources.iter().all(|src| src.text != "alpha-only-sentence")));
        }
        {
            let mut d = desk.lock().unwrap();
            let old = Instant::now().checked_sub(Duration::from_secs(61)).unwrap();
            d.sessions.get_mut(&b).unwrap().touched = old;
            expire_idle(&mut d, Instant::now());
            assert!(d.sessions.get(&b).is_none());
            assert!(d.sessions[&c].sources.iter().any(|s| s.text == "gamma-only-sentence"));
            assert!(d.sessions.values().all(|s| s.sources.iter().all(|src| src.text != "beta-only-sentence")));
        }
        let forged = "dd".repeat(32);
        let mut d = desk.lock().unwrap();
        let id = ensure_research_session(&mut d, &[forged.clone()]);
        assert_ne!(id, forged, "an id this desk did not mint is ignored");
        assert!(d.sessions.get(&forged).is_none());
        assert!(d.sessions[&id].sources.is_empty());
    }

    #[test]
    fn research_http_sessions_do_not_share_files() {
        let key = "test-gateway-key-not-for-git";
        let content = "Found\\n- A colour is named. [1]\\n[1] alpha.txt\\n[2] beta.txt";
        let payload = format!("{{\"choices\":[{{\"message\":{{\"content\":\"{content}\"}}}}]}}");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_w = seen.clone();
        std::thread::spawn(move || {
            for _ in 0..4 {
                let Ok((mut sock, _)) = listener.accept() else { break };
                let _ = sock.set_read_timeout(Some(Duration::from_millis(800)));
                let mut buf = [0u8; 8192];
                let mut got = Vec::new();
                let mut header_end = None;
                loop {
                    match sock.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            got.extend_from_slice(&buf[..n]);
                            if header_end.is_none() {
                                header_end = got.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
                            }
                            if let Some(end) = header_end {
                                let head = String::from_utf8_lossy(&got[..end]);
                                let len = head.lines().find_map(|l| {
                                    let (k, v) = l.split_once(':')?;
                                    if !k.trim().eq_ignore_ascii_case("content-length") {
                                        return None;
                                    }
                                    v.trim().parse::<usize>().ok()
                                }).unwrap_or(0);
                                if got.len() >= end + len {
                                    break;
                                }
                            }
                        }
                    }
                }
                if let Ok(s) = String::from_utf8(got) {
                    seen_w.lock().unwrap().push(s);
                }
                let resp = format!(
                    "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = sock.write_all(resp.as_bytes());
            }
        });
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.set_research_hermes(galion_cascade::HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            "hermes-agent",
            key,
            Duration::from_millis(1000),
        ));
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let minted = get(a, "/research/sources");
        let id_a = research_cookie(&minted);
        let cookie_a = research_cookie_header(&id_a);
        let posted = call(a, "POST", "/research/sources", &cookie_a, r#"{"name":"alpha.txt","text":"alpha-only-sentence"}"#);
        assert!(posted.contains("alpha.txt") && !posted.contains("beta"), "{posted}");
        let other = get(a, "/research/sources");
        let id_b = research_cookie(&other);
        assert_ne!(id_a, id_b);
        assert!(other.contains("\"sources\":[]") && !other.contains("alpha"), "{other}");
        let cookie_b = research_cookie_header(&id_b);
        let posted_b = call(a, "POST", "/research/sources", &cookie_b, r#"{"name":"beta.txt","text":"beta-only-sentence"}"#);
        assert!(posted_b.contains("beta.txt") && !posted_b.contains("alpha"), "{posted_b}");
        let header_a = format!("{RESEARCH_HEADER}: {id_a}\r\n");
        let via_header = call(a, "GET", "/research/sources", &header_a, "");
        assert!(via_header.contains("alpha.txt") && !via_header.contains("beta"), "{via_header}");
        let via_b = call(a, "GET", "/research/sources", &cookie_b, "");
        assert!(via_b.contains("beta.txt") && !via_b.contains("alpha"), "{via_b}");
        let forged = "dd".repeat(32);
        let stranger = call(a, "GET", "/research/sources", &format!("{RESEARCH_HEADER}: {forged}\r\n"), "");
        assert!(stranger.contains("\"sources\":[]"), "{stranger}");
        assert!(!stranger.contains("alpha-only-sentence") && !stranger.contains("beta-only-sentence"), "{stranger}");
        assert!(!stranger.contains("alpha.txt") && !stranger.contains("beta.txt"), "{stranger}");
        let stranger_id = research_cookie(&stranger);
        assert_ne!(stranger_id, forged);
        assert_ne!(stranger_id, id_a);
        assert_ne!(stranger_id, id_b);
        let both = format!("{RESEARCH_HEADER}: {forged}\r\n{cookie_a}");
        let kept = call(a, "GET", "/research/sources", &both, "");
        assert!(kept.contains("alpha.txt") && !kept.contains("beta.txt"), "{kept}");
        assert_eq!(research_cookie(&kept), id_a);
        let in_url = call(a, "GET", &format!("/research/sources?{RESEARCH_COOKIE}={id_a}"), &cookie_a, "");
        assert!(in_url.starts_with("HTTP/1.1 400"), "{in_url}");
        let still = call(a, "GET", "/research/sources", &cookie_a, "");
        assert!(still.contains("alpha.txt") && !still.contains("beta"), "{still}");
        let ask_a = call(a, "POST", "/research/ask", &cookie_a, r#"{"question":"What does alpha say?"}"#);
        let ask_b = call(a, "POST", "/research/ask", &cookie_b, r#"{"question":"What does beta say?"}"#);
        assert!(ask_a.contains("\"status\":\"Found\"") && ask_a.contains("alpha.txt"), "{ask_a}");
        assert!(!ask_a.contains("beta.txt") && !ask_a.contains("beta-only-sentence"), "{ask_a}");
        assert!(!ask_b.contains("alpha.txt") && !ask_b.contains("alpha-only-sentence"), "{ask_b}");
        assert!(ask_b.contains("\"citations\":[]"), "{ask_b}");
        let hist_a = call(a, "GET", "/research/history", &cookie_a, "");
        let hist_b = call(a, "GET", "/research/history", &cookie_b, "");
        assert!(hist_a.contains("What does alpha say?") && hist_a.contains("alpha.txt"), "{hist_a}");
        assert!(!hist_a.contains("beta.txt") && !hist_a.contains("beta-only-sentence") && !hist_a.contains("What does beta say?"), "{hist_a}");
        assert!(hist_b.contains("What does beta say?"), "{hist_b}");
        assert!(!hist_b.contains("alpha.txt") && !hist_b.contains("alpha-only-sentence") && !hist_b.contains("What does alpha say?"), "{hist_b}");
        let upstream = seen.lock().unwrap().clone();
        assert!(upstream.iter().any(|u| u.contains("alpha-only-sentence") && !u.contains("beta-only-sentence")), "{upstream:?}");
        assert!(upstream.iter().any(|u| u.contains("beta-only-sentence") && !u.contains("alpha-only-sentence")), "{upstream:?}");
        assert!(!ask_a.contains(key) && !ask_b.contains(key), "{ask_a} {ask_b}");
        w.close().unwrap();
    }

    #[test]
    fn research_http_session_expires_when_idle() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap())
            .with_page("<!doctype html><canvas></canvas>")
            .with_research_idle(Duration::from_millis(200));
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let minted = get(a, "/research/sources");
        let id = research_cookie(&minted);
        let cookie = research_cookie_header(&id);
        let posted = call(a, "POST", "/research/sources", &cookie, r#"{"name":"idle.txt","text":"idle-only-sentence"}"#);
        assert!(posted.contains("idle.txt"), "{posted}");
        std::thread::sleep(Duration::from_millis(350));
        let later = call(a, "GET", "/research/sources", &cookie, "");
        assert!(later.contains("\"sources\":[]"), "{later}");
        assert!(!later.contains("idle.txt") && !later.contains("idle-only-sentence"), "{later}");
        assert_ne!(research_cookie(&later), id);
        let again = call(a, "GET", "/research/sources", &cookie, "");
        assert!(!again.contains("idle-only-sentence") && again.contains("\"sources\":[]"), "{again}");
        w.close().unwrap();
    }

    #[test]
    fn cc0_checkbox_is_per_session_and_in_memory() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas id=\"s\"></canvas>");
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let post = |hdr: &str, body: &str| {
            let mut s = std::net::TcpStream::connect(a).unwrap();
            s.write_all(format!("POST /v2/cc0 HTTP/1.1\r\nHost: x\r\n{hdr}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        };
        let info = get(a, "/v2/cc0");
        assert!(info.contains("CC0 1.0 Universal") && info.contains("creativecommons.org/publicdomain/zero/1.0"), "{info}");
        assert!(info.contains("\"accepted\":false"), "{info}");
        assert!(post("", "{\"accept\":true}").contains("400"), "no session, no acceptance");
        let tok = "web-cc0-test-token";
        let off = post(&format!("X-Galion-Session: {tok}\r\n"), "{\"accept\":false}");
        assert!(off.contains("\"accepted\":false"), "{off}");
        assert!(!galion_consent::cc0_acceptances().accepted(tok), "unticked = refused");
        let on = post(&format!("X-Galion-Session: {tok}\r\n"), "{\"accept\":true}");
        assert!(on.contains("\"accepted\":true") && !on.contains(tok), "{on}");
        assert!(galion_consent::cc0_acceptances().accepted(tok), "ticked = allowed");
        assert!(!galion_consent::cc0_acceptances().accepted("another-session"));
        post(&format!("X-Galion-Session: {tok}\r\n"), "{\"accept\":false}");
        assert!(!galion_consent::cc0_acceptances().accepted(tok));
        w.close().unwrap();
    }

    #[test]
    fn v2_say_routes_personas_and_keeps_the_hold() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas id=\"s\"></canvas>");
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let page = get(a, "/v2");
        assert!(page.contains("HTTP/1.1 302"), "{page}");
        assert!(page.to_ascii_lowercase().contains("location: /"), "{page}");
        let state = get(a, "/v2/state");
        assert!(state.contains("\"rights\":\"hold\"") && state.contains("\"film_voice\":false"), "{state}");
        assert!(state.contains("\"mouth\":\"piper\""), "{state}");
        assert!(state.contains("\"warmth_e0\":0.62") && state.contains("\"pre_pause_e0\":80"), "{state}");
        assert!(state.contains("\"llm\":\"hermes-agent\""), "{state}");
        let mut hot = VizFrame::default();
        hot.listen.emotion = crate::frame::Emotion { label: "tense", intensity: 0.82, valence: -0.6 };
        w.present(&hot).unwrap();
        let before = get(a, "/dot");
        assert!(before.contains("\"label\":\"tense\""), "{before}");
        assert!(!before.contains("\"mood\""), "{before}");
        let body = b"{\"text\":\"My mother died this morning.\"}";
        let mut s = std::net::TcpStream::connect(a).unwrap();
        s.write_all(format!("POST /v2/say HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes()).unwrap();
        s.write_all(body).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.contains("HTTP/1.1 200"), "{out}");
        assert!(out.contains("model can't be reached"), "{out}");
        assert!(!out.contains("Your mother died this morning."), "{out}");
        assert!(out.contains("\"mood\":\"grief\""));
        assert!(out.contains("\"llm\":\"hermes-agent\""));
        assert!(!out.to_ascii_lowercase().contains("i hear you"));
        assert!(out.contains("\"film_voice\":false"));
        let dot = get(a, "/dot");
        assert!(dot.contains("\"persona\":\"samantha\""), "{dot}");
        assert!(dot.contains("\"mood\":\"grief\""), "{dot}");
        assert!(dot.contains("\"label\":\"grief\""), "{dot}");
        assert!(dot.contains("\"valence\":-0.550"), "{dot}");
        assert!(!dot.contains("\"label\":\"tense\""), "{dot}");
        let line = w.say_inbox().take_latest().expect("reply queued for Piper");
        assert_eq!(line.reply, "model can't be reached");
        assert!(!line.silence);
        w.close().unwrap();
    }

    #[test]
    fn v2_say_does_not_hold_the_floor_during_the_model_call() {
        use std::sync::atomic::AtomicBool;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let model = listener.local_addr().unwrap();
        let release = Arc::new(AtomicBool::new(false));
        let started = Arc::new(AtomicBool::new(false));
        let release_w = release.clone();
        let started_w = started.clone();
        std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else { return };
            let mut buf = [0u8; 8192];
            let _ = sock.read(&mut buf);
            started_w.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(2);
            while !release_w.load(Ordering::SeqCst) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            let payload = "{\"choices\":[{\"message\":{\"content\":\"She died this morning.\"}}]}";
            let resp = format!(
                "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.set_hermes(galion_cascade::HermesLlm::new(
            &format!("http://{model}/v1/chat/completions"),
            "hermes-agent",
            "",
            Duration::from_millis(2000),
        ));
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let say = std::thread::spawn(move || {
            let body = b"{\"text\":\"My mother died this morning.\"}";
            let mut s = std::net::TcpStream::connect(a).unwrap();
            s.write_all(format!(
                "POST /v2/say HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                body.len()
            ).as_bytes()).unwrap();
            s.write_all(body).unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        });
        let wait_deadline = Instant::now() + Duration::from_secs(2);
        while !started.load(Ordering::SeqCst) && Instant::now() < wait_deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(started.load(Ordering::SeqCst), "model call did not start");
        let t = Instant::now();
        let state = get(a, "/v2/state");
        assert!(
            t.elapsed() < Duration::from_millis(400),
            "state blocked on the model call: {:?} {state}",
            t.elapsed()
        );
        assert!(state.contains("\"llm\":\"hermes-agent\""), "{state}");
        release.store(true, Ordering::SeqCst);
        let out = say.join().unwrap();
        assert!(out.contains("She died this morning."), "{out}");
        assert!(!out.to_ascii_lowercase().contains("i hear you"), "{out}");
        w.close().unwrap();
    }

    #[test]
    fn dot_emotion_tracks_each_presented_frame() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        w.present(&VizFrame::default()).unwrap();
        let quiet = get(a, "/dot");
        assert!(quiet.contains("\"sense\":false") && quiet.contains("\"label\":\"neutral\""), "{quiet}");
        let mut hot = VizFrame::default();
        hot.t_ms = 800;
        hot.listen.emotion = crate::frame::Emotion { label: "tense", intensity: 0.82, valence: -0.6 };
        w.present(&hot).unwrap();
        let live = get(a, "/dot");
        assert!(live.contains("\"sense\":true"), "{live}");
        assert!(live.contains("\"label\":\"tense\""), "{live}");
        assert!(live.contains("\"valence\":-0.600"), "{live}");
        assert!(live.contains("\"arousal\":0.820"), "{live}");
        assert!(!quiet.contains("\"label\":\"tense\""));
        let mut next = hot;
        next.t_ms = 1600;
        next.listen.emotion = crate::frame::Emotion { label: "warm", intensity: 0.31, valence: 0.4 };
        w.present(&next).unwrap();
        let later = get(a, "/dot");
        assert!(later.contains("\"label\":\"warm\"") && later.contains("\"arousal\":0.310"), "{later}");
        assert!(!later.contains("\"label\":\"tense\""));
        assert!(!later.contains("\"mood\""), "heartbeat emotion stays until a turn sets mood: {later}");
        w.close().unwrap();
    }

    #[test]
    fn mic_post_reaches_tap_and_parser_is_strict() {
        assert_eq!(parse_rms("{\"rms\":0.0420,\"t\":1}"), Some(0.042));
        assert_eq!(parse_rms("{\"rms\": 3}"), Some(1.0));
        assert_eq!(parse_rms("{\"rms\":\"x\"}"), None);
        assert_eq!(parse_rms("{\"lvl\":0.1}"), None);
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let tap = w.mic_tap();
        assert_eq!(tap.latest(Duration::from_secs(1)), None);
        w.open().unwrap();
        let body = "{\"rms\":0.0500,\"t\":1}";
        let mut s = std::net::TcpStream::connect(w.local_addr().unwrap()).unwrap();
        s.write_all(format!("POST /mic HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 204"), "{out}");
        assert_eq!(tap.latest(Duration::from_secs(1)), Some(0.05));
        assert_eq!(tap.latest(Duration::from_nanos(0)), None, "stale level is dropped");
        w.close().unwrap();
    }

    #[test]
    fn slow_client_does_not_block_dot() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        // Open a socket and send nothing (a stalled tunnel connection).
        let _stall = std::net::TcpStream::connect(a).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        let t = Instant::now();
        let r = get(a, "/health");
        assert!(r.contains("ok"));
        assert!(t.elapsed() < Duration::from_millis(300), "health took {:?}", t.elapsed());
        w.close().unwrap();
    }

    #[test]
    fn serves_dot() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        let mut f = VizFrame::default();
        f.speak.active = true;
        w.present(&f).unwrap();
        let mut s = std::net::TcpStream::connect(w.local_addr().unwrap()).unwrap();
        s.write_all(b"GET /dot HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.contains("\"state\":\"speaking\"") && out.contains("\"level\":"), "{out}");
        assert!(out.contains("\"e\":0.000"), "{out}");
        assert!(out.contains("\"engine\":{\"profile\":\"luti\",\"hz\":240,\"frames\":1"), "{out}");
        assert!(out.contains("\"cost_hint\":48"), "{out}");
        assert!(out.contains("\"settle_err\":"), "{out}");
        let body = out.split("\r\n\r\n").nth(1).unwrap_or("");
        let mut depth = 0i32;
        for c in body.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0, "{body}");
        }
        assert_eq!(depth, 0, "{body}");
        w.present(&f).unwrap();
        let again = get(w.local_addr().unwrap(), "/dot");
        assert!(again.contains("\"frames\":2"), "{again}");
        assert!(again.contains("\"state\":\"speaking\""), "{again}");
        w.close().unwrap();
    }

    fn raw(addr: SocketAddr, req: &[u8]) -> Vec<u8> {
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        s.write_all(req).unwrap();
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        out
    }
    fn split_resp(r: &[u8]) -> (String, Vec<u8>) {
        let i = find_head_end(r).expect("head");
        (String::from_utf8_lossy(&r[..i]).into_owned(), r[i + 4..].to_vec())
    }

    #[test]
    fn mic_body_split_across_writes_is_read_fully() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let tap = w.mic_tap();
        w.open().unwrap();
        let body = "{\"rms\":0.2500}";
        let mut s = std::net::TcpStream::connect(w.local_addr().unwrap()).unwrap();
        s.write_all(format!("POST /mic HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes()).unwrap();
        s.flush().unwrap();
        std::thread::sleep(Duration::from_millis(50));
        s.write_all(&body.as_bytes()[..6]).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        s.write_all(&body.as_bytes()[6..]).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 204"), "{out}");
        assert_eq!(tap.latest(Duration::from_secs(1)), Some(0.25));
        w.close().unwrap();
    }

    #[test]
    fn turn_wav_large_body_chunked_writes() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let inbox = w.turn_inbox();
        w.open().unwrap();
        let samples: Vec<i16> = (0..160_000).map(|i| ((i * 37) % 20000 - 10000) as i16).collect(); // 10 s @16k
        let wav = encode_wav_pcm16(16_000, 1, &samples);
        assert!(wav.len() > 300_000);
        let mut s = std::net::TcpStream::connect(w.local_addr().unwrap()).unwrap();
        s.write_all(format!("POST /turn HTTP/1.1\r\nHost: x\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\n\r\n", wav.len()).as_bytes()).unwrap();
        for c in wav.chunks(7_001) { s.write_all(c).unwrap(); }
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 202") && out.contains("\"id\":1") && out.contains("\"format\":\"wav\""), "{out}");
        assert_eq!(inbox.pending(), 1);
        let t = inbox.take().unwrap();
        assert_eq!((t.id, t.format, t.bytes.len()), (1, TurnFormat::Wav, wav.len()));
        let (rate, ch, pcm) = t.pcm16().unwrap();
        assert_eq!((rate, ch), (16_000, 1));
        assert_eq!(pcm, samples);
        assert!(inbox.take().is_none(), "take is non-blocking and empties the inbox");
        w.close().unwrap();
    }

    #[test]
    fn turn_raw_pcm_with_rate_and_expect_continue() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let inbox = w.turn_inbox();
        w.open().unwrap();
        let pcm: Vec<u8> = [1i16, -2, 300, -32768].iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut s = std::net::TcpStream::connect(w.local_addr().unwrap()).unwrap();
        s.write_all(format!("POST /turn?rate=24000 HTTP/1.1\r\nHost: x\r\nContent-Type: application/octet-stream\r\nExpect: 100-continue\r\nContent-Length: {}\r\n\r\n", pcm.len()).as_bytes()).unwrap();
        let mut first = [0u8; 25];
        s.read_exact(&mut first).unwrap();
        assert_eq!(&first, b"HTTP/1.1 100 Continue\r\n\r\n");
        s.write_all(&pcm).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 202") && out.contains("\"format\":\"pcm16\""), "{out}");
        let t = inbox.take().unwrap();
        assert_eq!(t.format, TurnFormat::Pcm16 { rate: 24_000, channels: 1 });
        assert_eq!(t.pcm16().unwrap().2, vec![1, -2, 300, -32768]);
        // audio/L16 params in the content type
        let r = raw(w.local_addr().unwrap(), b"POST /turn HTTP/1.1\r\nContent-Type: audio/L16; rate=8000; channels=2\r\nContent-Length: 4\r\n\r\n\x01\x00\x02\x00");
        assert!(split_resp(&r).0.starts_with("HTTP/1.1 202"));
        assert_eq!(inbox.take().unwrap().format, TurnFormat::Pcm16 { rate: 8_000, channels: 2 });
        w.close().unwrap();
    }

    #[test]
    fn turn_rejects_oversize_empty_and_missing_length() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let inbox = w.turn_inbox();
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let big = format!("POST /turn HTTP/1.1\r\nContent-Length: {}\r\n\r\n", MAX_BODY_BYTES + 1);
        assert!(split_resp(&raw(a, big.as_bytes())).0.starts_with("HTTP/1.1 413"));
        assert!(split_resp(&raw(a, b"POST /turn HTTP/1.1\r\nContent-Length: 0\r\n\r\n")).0.starts_with("HTTP/1.1 400"));
        assert!(split_resp(&raw(a, b"POST /turn HTTP/1.1\r\n\r\n")).0.starts_with("HTTP/1.1 411"));
        assert!(split_resp(&raw(a, b"POST /turn HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n")).0.starts_with("HTTP/1.1 411"));
        assert_eq!(inbox.pending(), 0);
        w.close().unwrap();
    }

    #[test]
    fn cors_preflight_and_x_sample_rate() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let inbox = w.turn_inbox();
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let (h, _) = split_resp(&raw(a, b"OPTIONS /turn HTTP/1.1\r\nOrigin: https://x.ngrok.app\r\n\r\n"));
        assert!(h.starts_with("HTTP/1.1 204"), "{h}");
        for k in ["Content-Type", "X-Turn-Id", "X-Sample-Rate", "X-Barge-In", "X-Galion-Proceed", "ngrok-skip-browser-warning"] { assert!(h.contains(k), "{k} missing: {h}"); }
        let r = raw(a, b"POST /turn HTTP/1.1\r\nX-Sample-Rate: 48000\r\nContent-Length: 2\r\n\r\n\x00\x00");
        assert!(split_resp(&r).0.starts_with("HTTP/1.1 202"));
        assert_eq!(inbox.take().unwrap().format, TurnFormat::Pcm16 { rate: 48_000, channels: 1 });
        w.close().unwrap();
    }

    #[test]
    fn inbox_is_bounded_and_ids_increase() {
        let ib = TurnInbox::default();
        for _ in 0..(MAX_PENDING_TURNS + 3) { ib.push(TurnFormat::Wav, vec![0]); }
        assert_eq!(ib.pending(), MAX_PENDING_TURNS);
        assert_eq!(ib.dropped(), 3);
        assert_eq!(ib.take().unwrap().id, 4);
        let last = ib.take_latest().unwrap();
        assert_eq!(last.id, (MAX_PENDING_TURNS + 3) as u64);
        assert_eq!(ib.pending(), 0);
    }

    #[test]
    fn reply_served_and_advertised_in_dot() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let rb = w.reply_box();
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        w.present(&VizFrame::default()).unwrap();
        assert!(get(a, "/reply/7.wav").starts_with("HTTP/1.1 404"));
        assert!(!get(a, "/dot").contains("\"reply\""));
        let wav = encode_wav_pcm16(24_000, 1, &vec![0i16; 12_000]);
        assert_eq!(wav_duration_ms(&wav), Some(500));
        assert!(!rb.set_reply("../x", wav.clone(), 1), "unsafe id rejected");
        assert!(rb.set_reply("7", wav.clone(), 500));
        let (h, b) = split_resp(&raw(a, b"GET /reply/7.wav HTTP/1.1\r\n\r\n"));
        assert!(h.starts_with("HTTP/1.1 200") && h.contains("Content-Type: audio/wav"), "{h}");
        assert_eq!(b, wav);
        let d = get(a, "/dot");
        assert!(d.contains(",\"reply\":{\"id\":\"7\",\"url\":\"/reply/7.wav\",\"ms\":500}}"), "{d}");
        assert!(d.contains("\"mode\":"));
        rb.clear_latest();
        assert!(!get(a, "/dot").contains("\"reply\""));
        assert!(split_resp(&raw(a, b"GET /reply/7.wav HTTP/1.1\r\n\r\n")).0.starts_with("HTTP/1.1 200"), "still fetchable after clear_latest");
        for i in 0..MAX_STORED_REPLIES { rb.set_reply(format!("r{i}"), vec![1], 0); }
        assert!(get(a, "/reply/7.wav").starts_with("HTTP/1.1 404"), "evicted");
        w.close().unwrap();
    }

    #[test]
    fn dot_reply_splice_and_wav_helpers() {
        assert_eq!(dot_with_reply("{}", Some("{\"id\":\"1\"}")), "{\"reply\":{\"id\":\"1\"}}");
        assert_eq!(dot_with_reply("{\"a\":1}", Some("{}")), "{\"a\":1,\"reply\":{}}");
        assert_eq!(dot_with_reply("{\"a\":1}", None), "{\"a\":1}");
        assert!(decode_wav_pcm16(b"RIFF\0\0\0\0WAVE").is_none());
        assert!(decode_wav_pcm16(b"not a wav").is_none());
        let w = encode_wav_pcm16(8_000, 2, &[1, 2, 3, 4]);
        assert_eq!(decode_wav_pcm16(&w), Some((8_000, 2, vec![1, 2, 3, 4])));
        assert_eq!(param_u32("rate=48000&channels=1", "rate"), Some(48_000));
        assert_eq!(param_u32("audio/L16; rate=8000", "rate"), Some(8_000));
    }

    #[test]
    fn cors_allowlist_is_exact_and_star_is_default() {
        assert_eq!(cors_allow_origin(None, ""), Some("*".into()));
        assert_eq!(cors_allow_origin(Some("https://a.ngrok-free.dev"), ""), Some("*".into()));
        assert_eq!(cors_allow_origin(Some("https://a.ngrok-free.dev"), "*"), None, "star only when the list is unset");
        assert_eq!(
            cors_allow_origin(Some("https://voice.example"), "https://voice.example, http://127.0.0.1:7878"),
            Some("https://voice.example".into())
        );
        assert_eq!(cors_allow_origin(Some("http://127.0.0.1:7878"), "https://voice.example, http://127.0.0.1:7878"), Some("http://127.0.0.1:7878".into()));
        assert_eq!(cors_allow_origin(Some("https://evil.example"), "https://voice.example"), None);
        assert_eq!(cors_allow_origin(Some("https://voice.example.evil"), "https://voice.example"), None);
        assert_eq!(cors_allow_origin(None, "https://voice.example"), None);
    }

    #[test]
    fn responses_carry_edge_security_headers() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_cors_origins("");
        w.open().unwrap();
        let r = get(w.local_addr().unwrap(), "/health");
        assert!(r.contains("X-Content-Type-Options: nosniff"), "{r}");
        assert!(r.contains("Referrer-Policy: no-referrer"), "{r}");
        assert!(r.contains("Permissions-Policy: microphone=(self)"), "{r}");
        assert!(r.contains("Content-Security-Policy:"), "{r}");
        assert!(r.contains("Cache-Control: no-store"), "{r}");
        assert!(r.contains("Access-Control-Allow-Origin: *"), "{r}");
        assert!(r.contains("X-Galion-Session"), "{r}");
        w.close().unwrap();
    }

    fn exchange(addr: SocketAddr, req: &[u8]) -> (String, String) {
        let (h, b) = split_resp(&raw(addr, req));
        (h, String::from_utf8_lossy(&b).into_owned())
    }

    #[test]
    fn foreign_origin_is_rejected_without_acao() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap())
            .with_cors_origins("https://voice.example, http://127.0.0.1:7878");
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let (h, body) = exchange(a, b"GET /health HTTP/1.1\r\nOrigin: https://evil.example\r\n\r\n");
        assert!(h.starts_with("HTTP/1.1 403"), "{h}");
        assert_eq!(body, "origin rejected");
        assert!(!h.contains("Access-Control-Allow-Origin"), "{h}");
        assert!(h.contains("X-Content-Type-Options: nosniff"), "{h}");
        assert!(h.contains("Content-Security-Policy:"), "{h}");
        assert!(h.contains("Cache-Control: no-store"), "{h}");
        let (h, _) = exchange(a, b"OPTIONS /turn HTTP/1.1\r\nOrigin: https://voice.example.evil\r\n\r\n");
        assert!(h.starts_with("HTTP/1.1 403") && !h.contains("Access-Control-Allow-Origin"), "{h}");
        let (h, _) = exchange(a, b"GET /health HTTP/1.1\r\nOrigin: https://voice.example\r\n\r\n");
        assert!(h.starts_with("HTTP/1.1 200"), "{h}");
        assert!(h.contains("Access-Control-Allow-Origin: https://voice.example\r\n"), "{h}");
        assert!(!h.contains("Access-Control-Allow-Origin: *"), "{h}");
        let (h, _) = exchange(a, b"GET /health HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(h.starts_with("HTTP/1.1 200") && !h.contains("Access-Control-Allow-Origin"), "no Origin, no echo: {h}");
        let (h, _) = exchange(a, b"POST /turn HTTP/1.1\r\nContent-Length: 2\r\n\r\n\x00\x00");
        assert!(h.starts_with("HTTP/1.1 403"), "sensitive route without Origin: {h}");
        w.close().unwrap();
    }

    #[test]
    fn session_token_is_in_body_not_url() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap())
            .with_cors_origins("https://voice.example")
            .with_session_ttl(Duration::from_secs(120));
        let inbox = w.turn_inbox();
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let (h, _) = exchange(a, b"POST /turn?token=abc HTTP/1.1\r\nOrigin: https://voice.example\r\nContent-Length: 2\r\n\r\n\x00\x00");
        assert!(h.starts_with("HTTP/1.1 400"), "{h}");
        assert!(!h.contains("Access-Control-Allow-Origin"), "query token is not echoed: {h}");
        assert_eq!(inbox.pending(), 0);
        let (h, body) = exchange(
            a,
            b"POST /session HTTP/1.1\r\nOrigin: https://voice.example\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"op\":\"open\"}",
        );
        assert!(h.starts_with("HTTP/1.1 200"), "{h} {body}");
        assert!(body.contains("\"transport\":\"body\""), "{body}");
        let token = parse_json_string_field(&body, "token").expect("token in body");
        assert_eq!(token.len(), 64);
        assert!(!body.contains('?') && !h.contains(&token));
        let (h, _) = exchange(
            a,
            b"POST /turn HTTP/1.1\r\nOrigin: https://voice.example\r\nContent-Type: application/octet-stream\r\nContent-Length: 2\r\n\r\n\x01\x00",
        );
        assert!(h.starts_with("HTTP/1.1 401"), "allow-list requires a session: {h}");
        assert!(h.contains("Access-Control-Allow-Origin: https://voice.example"), "{h}");
        let req = format!(
            "POST /turn HTTP/1.1\r\nOrigin: https://voice.example\r\nX-Galion-Session: {token}\r\nContent-Type: application/octet-stream\r\nContent-Length: 2\r\n\r\n"
        );
        let mut bytes = req.into_bytes();
        bytes.extend_from_slice(&[0x02, 0x00]);
        let (h, turn_body) = exchange(a, &bytes);
        assert!(h.starts_with("HTTP/1.1 202"), "{h} {turn_body}");
        assert!(turn_body.contains(AI_DISCLOSURE_HR) && turn_body.contains(AI_DISCLOSURE_EN), "{turn_body}");
        assert!(turn_body.contains("\"article\":\"50\""), "{turn_body}");
        let mic = format!(
            "{{\"rms\":0.2,\"session\":\"{token}\"}}"
        );
        let req = format!(
            "POST /mic HTTP/1.1\r\nOrigin: https://voice.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{mic}",
            mic.len()
        );
        let (h, _) = exchange(a, req.as_bytes());
        assert!(h.starts_with("HTTP/1.1 204"), "{h}");
        let stored = get(a, "/mic");
        assert!(!stored.contains(&token), "token must not be re-served: {stored}");
        assert!(stored.contains("\"rms\":"));
        let again = format!(
            "POST /turn HTTP/1.1\r\nOrigin: https://voice.example\r\nX-Galion-Session: {token}\r\nContent-Length: 2\r\n\r\n"
        );
        let mut bytes = again.into_bytes();
        bytes.extend_from_slice(&[0, 0]);
        let (_, second) = exchange(a, &bytes);
        assert!(!second.contains("ai_disclosure"), "disclosure is the first turn only: {second}");
        w.close().unwrap();
    }

    #[test]
    fn session_token_expires_and_v1_still_accepts_a_turn() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap())
            .with_require_session(true)
            .with_session_ttl(Duration::from_millis(40));
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let (h, _) = exchange(a, b"POST /turn HTTP/1.1\r\nContent-Length: 2\r\n\r\n\x00\x00");
        assert!(h.starts_with("HTTP/1.1 401"), "{h}");
        let (h, body) = exchange(a, b"POST /session HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"op\":\"open\"}");
        assert!(h.starts_with("HTTP/1.1 200"), "{h} {body}");
        let token = parse_json_string_field(&body, "token").unwrap();
        let req = format!("POST /turn HTTP/1.1\r\nX-Galion-Session: {token}\r\nContent-Length: 2\r\n\r\n");
        let mut bytes = req.into_bytes();
        bytes.extend_from_slice(&[0, 1]);
        assert!(exchange(a, &bytes).0.starts_with("HTTP/1.1 202"));
        std::thread::sleep(Duration::from_millis(80));
        let req = format!("POST /turn HTTP/1.1\r\nX-Galion-Session: {token}\r\nContent-Length: 2\r\n\r\n");
        let mut bytes = req.into_bytes();
        bytes.extend_from_slice(&[0, 1]);
        assert!(exchange(a, &bytes).0.starts_with("HTTP/1.1 401"), "expired");
        w.close().unwrap();

        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let inbox = w.turn_inbox();
        w.open().unwrap();
        let (h, body) = exchange(w.local_addr().unwrap(), b"POST /turn HTTP/1.1\r\nContent-Length: 2\r\n\r\n\x00\x00");
        assert!(h.starts_with("HTTP/1.1 202"), "V1 wildcard does not require a session: {h} {body}");
        assert!(body.contains(AI_DISCLOSURE_HR));
        assert_eq!(inbox.pending(), 1);
        w.close().unwrap();
    }

    #[test]
    fn disclosure_flag_off_strips_the_page_and_the_first_turn() {
        let page = "<!--galion-ai-disclosure--><p id=\"ai-disclosure\">AI</p><!--/galion-ai-disclosure--><canvas id=\"s\">";
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap())
            .with_disclosure(false)
            .with_page(page);
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let r = get(a, "/");
        assert!(r.contains("<canvas id=\"s\">"), "{r}");
        assert!(!r.contains("ai-disclosure"), "{r}");
        let (_, body) = exchange(a, b"POST /turn HTTP/1.1\r\nContent-Length: 2\r\n\r\n\x00\x00");
        assert!(!body.contains("ai_disclosure") && !body.contains(AI_DISCLOSURE_EN), "{body}");
        w.close().unwrap();
    }

    #[test]
    fn turn_body_is_zeroised_on_wipe() {
        let mut t = Turn {
            id: 1,
            format: TurnFormat::Pcm16 { rate: 16_000, channels: 1 },
            bytes: b"RIFFPCM_SECRET_0123456789".to_vec(),
            received_at: Instant::now(),
        };
        t.wipe();
        assert!(t.bytes.iter().all(|&b| b == 0));
        assert!(!t.bytes.is_empty());
    }

    #[test]
    fn reply_box_marks_hub_texture_only_when_published_with_it() {
        let b = ReplyBox::default();
        assert!(b.set_reply("a1", b"RIFF0000WAVE".to_vec(), 120));
        assert_eq!(b.latest_json().unwrap(), r#"{"id":"a1","url":"/reply/a1.wav","ms":120}"#);
        assert!(b.set_reply_hub("j2", b"RIFF0000WAVE".to_vec(), 80, true));
        assert_eq!(b.latest_json().unwrap(), r#"{"id":"j2","url":"/reply/j2.wav","ms":80,"hub_texture":true}"#);
        assert!(b.set_reply_hub("s3", b"RIFF0000WAVE".to_vec(), 80, false));
        assert!(b.latest_json().unwrap().ends_with(r#""hub_texture":false}"#));
        assert!(b.set_reply("s4", b"RIFF0000WAVE".to_vec(), 80));
        assert!(!b.latest_json().unwrap().contains("hub_texture"));
        assert!(!b.set_reply_hub("bad id", Vec::new(), 1, true));
    }

}
