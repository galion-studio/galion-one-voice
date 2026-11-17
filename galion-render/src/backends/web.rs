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
//!                  `Host: voice.galion.app` marks the turn as the voice stack. A research
//!                  cue on that turn is handed to `POST /research/ask` and spoken back.
//!                  `Host: galion.app` (the One page) is not marked, and this route does
//!                  not add a talk path for `/one`.
//!   GET /reply/<id>.wav -> reply audio the host published with [`ReplyBox::set_reply`];
//!                  `audio/wav`, 404 when unknown/evicted.
//!   GET / , /screen, /one, /one/ -> the caller's single-screen HTML page, only when one was set with
//!                      [`WebSurface::with_page`] (V1-01; the page itself is owned by
//!                      galion-console, this crate only serves the bytes); 404 otherwise
//!   GET /research, /research/ -> the Galion Research desk, same gate as /one.
//!                      Sets `galion_research`, an HttpOnly SameSite=Strict cookie.
//!                      `X-Galion-Research` carries the same id when there is no cookie jar.
//!   GET /research/hermes, GET /research/history, GET /research/progress,
//!   POST /research/ask -> Hermes research app. The gateway key stays on the server.
//!                      History, progress, and the ask see only this session's files.
//!                      Helper progress stays on the asking browser. `calls` is that ask's spend.
//!                      When Hermes does not answer, `/research/ask` answers Offline
//!                      from this session's files and notes.
//!   POST /research/sources, GET /research/sources -> files this session attached.
//!                      A citation may name only those files. Eight per session.
//!                      A file, and an idle session, are dropped after 30 minutes.
//!   DELETE /research/sources/{id} removes one of this session's files.
//!   DELETE /research/sources clears this session's files.
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
use std::collections::VecDeque;
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
    "Content-Type, X-Turn-Id, X-Sample-Rate, X-Barge-In, X-Galion-Proceed, X-Galion-Session, X-Galion-Research, ngrok-skip-browser-warning";

pub use super::edge::{
    bind_decision, cors_allow_origin, parse_json_string_field, AI_DISCLOSURE_EN, AI_DISCLOSURE_HR,
};
use super::edge::{
    check_origin, mint_token, presented_session, query_carries_token, route_is_sensitive, route_needs_session,
    strip_ai_disclosure, wipe_string, zeroise_bytes, EdgeCfg, OriginCheck, SessionStore,
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
    /// `true` when the request `Host` is the voice stack (`voice.galion.app`).
    /// The One page on `galion.app` stays `false`.
    pub voice_stack: bool,
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
    /// The turn is not the voice stack.
    pub fn push(&self, format: TurnFormat, bytes: Vec<u8>) -> u64 {
        self.push_from(format, bytes, false)
    }
    /// As [`Self::push`], with the voice-stack flag from the request host.
    pub fn push_from(&self, format: TurnFormat, mut bytes: Vec<u8>, voice_stack: bool) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut g) = self.st.lock() {
            while g.q.len() >= MAX_PENDING_TURNS {
                g.q.pop_front();
                g.dropped += 1;
            }
            g.q.push_back(Turn { id, format, bytes, received_at: Instant::now(), voice_stack });
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

/// Persona-column numbers for the mouth that speaks this line.
/// Set only when the turn had `sense` on. Pitch, loudness, and pause are
/// applied to the PCM. `rate` is carried and is not a Piper respawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouthProsody {
    pub pitch_st: f32,
    pub rate: f32,
    pub loudness: f32,
    pub pre_pause_ms: u32,
}

/// One persona reply waiting to be spoken (Piper / flite), not a second page.
#[derive(Clone, Debug)]
pub struct SpokenLine {
    pub id: u64,
    pub reply: String,
    pub silence: bool,
    pub barge: bool,
    /// `None` when sense was off, or the line is silence.
    pub mouth: Option<MouthProsody>,
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
    pub fn push(&self, reply: String, silence: bool, barge: bool, mouth: Option<MouthProsody>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut g) = self.st.lock() {
            while g.q.len() >= MAX_PENDING_TURNS { g.q.pop_front(); }
            g.q.push_back(SpokenLine { id, reply, silence, barge, mouth });
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

struct Request { method: String, path: String, query: String, headers: Vec<(String, String)>, body: Vec<u8> }

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
    let mut req = Request { method, path, query, headers, body: Vec::new() };
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
    /// The same reply `POST /research/ask` speaks. The voice stack calls this
    /// after a research cue. The key stays inside the desk.
    pub fn research_ask_fn(&self) -> std::sync::Arc<dyn Fn(&str) -> String + Send + Sync> {
        let desk = std::sync::Arc::clone(&self.sh.research);
        std::sync::Arc::new(move |question| research_reply(&desk, question))
    }
}

const V2_REDIRECT_HTML: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"0; url=/\"><title>Galion Voice</title><script>location.replace(\"/\");</script></head><body><p><a href=\"/\">Galion Voice</a></p></body></html>";

/// Galion Research. Served at `/research` and `/research/` when a product page is set,
/// the same gate as `/one`.
const RESEARCH_HTML: &str = include_str!("../../../crates/galion-console/assets/research.html");

#[derive(Clone)]
struct ResearchNote {
    id: u64,
    question: String,
    status: &'static str,
    reply: String,
    citations: Vec<(u32, String)>,
    found: Vec<String>,
    inferred: Vec<String>,
    unknown: Vec<String>,
    calls: u32,
    call_budget: u32,
    split: bool,
    helpers: Vec<galion_cascade::hermes::ResearchHelper>,
    at: Instant,
}

impl Drop for ResearchNote {
    fn drop(&mut self) {
        wipe_string(&mut self.question);
        wipe_string(&mut self.reply);
        for (_, text) in &mut self.citations {
            wipe_string(text);
        }
        for text in &mut self.found {
            wipe_string(text);
        }
        for text in &mut self.inferred {
            wipe_string(text);
        }
        for text in &mut self.unknown {
            wipe_string(text);
        }
        for helper in &mut self.helpers {
            wipe_string(&mut helper.question);
        }
    }
}

/// In-flight helper list for one browser. Another session cannot read it.
struct ResearchLive {
    phase: &'static str,
    calls: u32,
    call_budget: u32,
    helpers: Vec<galion_cascade::hermes::ResearchHelper>,
}

impl ResearchLive {
    fn idle() -> Self {
        Self { phase: "idle", calls: 0, call_budget: 0, helpers: Vec::new() }
    }
}

impl Drop for ResearchLive {
    fn drop(&mut self) {
        for helper in &mut self.helpers {
            wipe_string(&mut helper.question);
        }
    }
}

/// Writes helper progress onto one session. The desk lock is not held across a Hermes call.
struct SessionWatch {
    desk: Arc<Mutex<ResearchDesk>>,
    session: String,
}

impl Drop for SessionWatch {
    fn drop(&mut self) {
        wipe_string(&mut self.session);
    }
}

impl galion_cascade::hermes::ResearchWatch for SessionWatch {
    fn on_research(&self, event: &galion_cascade::hermes::ResearchEvent) {
        let Ok(mut d) = self.desk.lock() else { return };
        let Some(session) = d.session_mut(&self.session) else { return };
        use galion_cascade::hermes::ResearchEvent;
        match event {
            ResearchEvent::Phase(p) => {
                if *p == "planning" {
                    session.live.helpers.clear();
                }
                session.live.phase = p;
            }
            ResearchEvent::Calls(n) => session.live.calls = *n,
            ResearchEvent::Budget(n) => session.live.call_budget = *n,
            ResearchEvent::Helper(i, h) => {
                if *i >= session.live.helpers.len() {
                    session.live.helpers.resize(
                        *i + 1,
                        galion_cascade::hermes::ResearchHelper { question: String::new(), status: "queued" },
                    );
                }
                session.live.helpers[*i] = h.clone();
            }
        }
    }
}

/// A file one browser attached. `kind` is `file` or `image`. Image pixels are not read.
struct DeskSource {
    id: u32,
    name: String,
    text: String,
    kind: &'static str,
    at: Instant,
}

impl Drop for DeskSource {
    fn drop(&mut self) {
        wipe_string(&mut self.name);
        wipe_string(&mut self.text);
    }
}

/// Files and notes for one browser. The id is the cookie value, not a label.
struct ResearchSession {
    id: String,
    next_source: u32,
    sources: Vec<DeskSource>,
    notes: VecDeque<ResearchNote>,
    live: ResearchLive,
    touched: Instant,
}

impl Drop for ResearchSession {
    fn drop(&mut self) {
        wipe_string(&mut self.id);
    }
}

/// How long an attached file, a note, and an idle session stay in memory.
const RESEARCH_SESSION_TTL: Duration = Duration::from_secs(30 * 60);
/// Quiet browsers dropped when a new one arrives. The least recently used goes first.
const RESEARCH_SESSION_CAP: usize = 64;
const RESEARCH_COOKIE: &str = "galion_research";

struct ResearchDesk {
    client: Option<galion_cascade::HermesLlm>,
    next_id: u64,
    sessions: VecDeque<ResearchSession>,
    status: &'static str,
    ttl: Duration,
}

impl ResearchDesk {
    fn new() -> Self {
        Self {
            client: if cfg!(test) { None } else { galion_cascade::HermesLlm::attached_from_env() },
            next_id: 0,
            sessions: VecDeque::new(),
            status: "idle",
            ttl: RESEARCH_SESSION_TTL,
        }
    }
    fn gateway_status(&self) -> &'static str {
        if self.status == "Running" {
            return "Running";
        }
        if self.client.is_none() {
            return "Unreachable";
        }
        if self.status == "idle" {
            "Idle"
        } else {
            self.status
        }
    }
    fn cookie_line(&self, id: &str) -> String {
        let max_age = self.ttl.as_secs().max(1);
        format!("{RESEARCH_COOKIE}={id}; HttpOnly; SameSite=Strict; Path=/research; Max-Age={max_age}")
    }
    /// Drop files and notes older than the TTL, then drop sessions nobody has touched.
    fn evict(&mut self, now: Instant) {
        let ttl = self.ttl;
        for session in &mut self.sessions {
            session.sources.retain(|src| now.saturating_duration_since(src.at) < ttl);
            session.notes.retain(|note| now.saturating_duration_since(note.at) < ttl);
        }
        self.sessions.retain(|session| now.saturating_duration_since(session.touched) < ttl);
    }
    fn touch(&mut self, id: &str, now: Instant) -> bool {
        let Some(i) = self.sessions.iter().position(|s| id_eq(&s.id, id)) else { return false };
        self.sessions[i].touched = now;
        if let Some(session) = self.sessions.remove(i) {
            self.sessions.push_back(session);
        }
        true
    }
    fn mint(&mut self, now: Instant) -> String {
        while self.sessions.len() >= RESEARCH_SESSION_CAP {
            self.sessions.pop_front();
        }
        let id = mint_token();
        self.sessions.push_back(ResearchSession {
            id: id.clone(),
            next_source: 0,
            sources: Vec::new(),
            notes: VecDeque::new(),
            live: ResearchLive::idle(),
            touched: now,
        });
        id
    }
    /// Cookie first, then the header, and only when that id is still live.
    /// An unknown id is not adopted. A new id is minted instead.
    fn ensure(&mut self, cookie_id: Option<&str>, header_id: Option<&str>) -> String {
        let now = Instant::now();
        self.evict(now);
        for id in [cookie_id, header_id].into_iter().flatten() {
            if research_id_ok(id) && self.touch(id, now) {
                return id.to_string();
            }
        }
        self.mint(now)
    }
    fn session_mut(&mut self, id: &str) -> Option<&mut ResearchSession> {
        self.sessions.iter_mut().find(|s| id_eq(&s.id, id))
    }
}

fn id_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn research_id_ok(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn research_cookie_id(req: &Request) -> Option<String> {
    let header = req.header("cookie")?;
    for part in header.split(';') {
        let Some((k, v)) = part.trim().split_once('=') else { continue };
        if k.trim() != RESEARCH_COOKIE {
            continue;
        }
        let v = v.trim();
        return research_id_ok(v).then(|| v.to_string());
    }
    None
}

fn research_header_id(req: &Request) -> Option<String> {
    let v = req.header("x-galion-research")?.trim();
    research_id_ok(v).then(|| v.to_string())
}

fn wipe_opt(slot: &mut Option<String>) {
    if let Some(ref mut s) = slot {
        wipe_string(s);
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

struct NewSource {
    name: String,
    text: String,
    kind: &'static str,
}

fn parse_new_source(raw: &str) -> Result<NewSource, &'static str> {
    let name = json_object_string(raw, "name", 160).ok_or("name")?;
    let name = clean_source_name(&name).ok_or("name")?;
    let kind_raw = json_object_string(raw, "kind", 16).unwrap_or_default();
    let kind = if kind_raw == "image" || is_image_name(&name) { "image" } else { "file" };
    let text = if kind == "image" {
        String::new()
    } else {
        json_object_string(raw, "text", galion_cascade::hermes::MAX_RESEARCH_SOURCE_CHARS).unwrap_or_default()
    };
    Ok(NewSource { name, text, kind })
}

/// `(json, set-cookie)`. A bad name does not open a session.
fn research_attach(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
    raw: &str,
) -> Result<(String, String), &'static str> {
    let src = parse_new_source(raw)?;
    let Ok(mut d) = desk.lock() else { return Err("research") };
    let mut id = d.ensure(cookie_id, header_id);
    let cookie = d.cookie_line(&id);
    let Some(session) = d.session_mut(&id) else {
        wipe_string(&mut id);
        return Err("research");
    };
    if session.sources.len() >= galion_cascade::hermes::MAX_RESEARCH_SOURCES {
        session.sources.remove(0);
    }
    session.next_source = session.next_source.saturating_add(1);
    let source_id = session.next_source.max(1);
    session.sources.push(DeskSource {
        id: source_id,
        name: src.name,
        text: src.text,
        kind: src.kind,
        at: Instant::now(),
    });
    let json = research_sources_json(&session.sources);
    wipe_string(&mut id);
    Ok((json, cookie))
}

#[derive(Clone, Copy)]
enum SourceDelete {
    All,
    One(u32),
}

/// `None` is not this route. `Err` is a source id this route refuses.
fn parse_source_delete(path: &str) -> Option<Result<SourceDelete, ()>> {
    if path == "/research/sources" || path == "/research/sources/" {
        return Some(Ok(SourceDelete::All));
    }
    let id = path.strip_prefix("/research/sources/")?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    if id.len() > 10 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Some(Err(()));
    }
    let Ok(n) = id.parse::<u32>() else { return Some(Err(())) };
    if n == 0 || n.to_string() != id {
        return Some(Err(()));
    }
    Some(Ok(SourceDelete::One(n)))
}

fn research_remove(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
    target: SourceDelete,
) -> Result<(String, String), &'static str> {
    let Ok(mut d) = desk.lock() else { return Err("research") };
    let mut id = d.ensure(cookie_id, header_id);
    let cookie = d.cookie_line(&id);
    let Some(session) = d.session_mut(&id) else {
        wipe_string(&mut id);
        return Err("research");
    };
    match target {
        SourceDelete::All => session.sources.clear(),
        SourceDelete::One(source_id) => {
            let before = session.sources.len();
            session.sources.retain(|src| src.id != source_id);
            if session.sources.len() == before {
                wipe_string(&mut id);
                return Err("missing");
            }
        }
    }
    let json = research_sources_json(&session.sources);
    wipe_string(&mut id);
    Ok((json, cookie))
}

fn research_list(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
) -> Result<(String, String), ()> {
    let Ok(mut d) = desk.lock() else { return Err(()) };
    let mut id = d.ensure(cookie_id, header_id);
    let cookie = d.cookie_line(&id);
    let json = d.session_mut(&id).map(|s| research_sources_json(&s.sources)).unwrap_or_else(|| research_sources_json(&[]));
    wipe_string(&mut id);
    Ok((json, cookie))
}

fn research_touch(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
) -> Result<String, ()> {
    let Ok(mut d) = desk.lock() else { return Err(()) };
    let mut id = d.ensure(cookie_id, header_id);
    let cookie = d.cookie_line(&id);
    wipe_string(&mut id);
    Ok(cookie)
}

fn json_list(items: &[String]) -> String {
    items.iter().map(|s| galion_cascade::hermes::json_string(s)).collect::<Vec<_>>().join(",")
}

fn research_status_json(desk: &ResearchDesk, history: usize, calls: u32) -> String {
    let key = desk.client.as_ref().map(|c| c.key_state()).unwrap_or("none");
    let model = desk.client.as_ref().map(|c| c.label().to_string()).unwrap_or_else(|| galion_cascade::LLM_NAME.into());
    format!(
        "{{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"voice\":\"galion-research\",\"product\":\"galion-one\",\"model\":{},\"key\":\"{key}\",\"status\":\"{}\",\"calls\":{calls},\"history\":{history}}}",
        galion_cascade::hermes::json_string(&model),
        desk.gateway_status(),
    )
}

fn research_status_request(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
) -> Result<(String, String), ()> {
    let Ok(mut d) = desk.lock() else { return Err(()) };
    let mut id = d.ensure(cookie_id, header_id);
    let cookie = d.cookie_line(&id);
    let (history, calls) = d.session_mut(&id).map(|s| (s.notes.len(), s.live.calls)).unwrap_or((0, 0));
    let json = research_status_json(&d, history, calls);
    wipe_string(&mut id);
    Ok((json, cookie))
}

fn research_helpers_json(helpers: &[galion_cascade::hermes::ResearchHelper]) -> String {
    helpers.iter().map(|h| {
        format!(
            "{{\"question\":{},\"status\":\"{}\"}}",
            galion_cascade::hermes::json_string(&h.question),
            h.status
        )
    }).collect::<Vec<_>>().join(",")
}

fn research_progress_json(live: &ResearchLive) -> String {
    format!(
        "{{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"phase\":\"{}\",\"calls\":{},\"call_budget\":{},\"helpers\":[{}]}}",
        live.phase,
        live.calls,
        live.call_budget,
        research_helpers_json(&live.helpers)
    )
}

fn research_progress_request(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
) -> Result<(String, String), ()> {
    let Ok(mut d) = desk.lock() else { return Err(()) };
    let mut id = d.ensure(cookie_id, header_id);
    let cookie = d.cookie_line(&id);
    let json = d.session_mut(&id).map(|s| research_progress_json(&s.live)).unwrap_or_else(|| research_progress_json(&ResearchLive::idle()));
    wipe_string(&mut id);
    Ok((json, cookie))
}

fn research_note_json(note: &ResearchNote) -> String {
    let cites: Vec<String> = note.citations.iter().map(|(n, text)| {
        format!("{{\"n\":{n},\"text\":{}}}", galion_cascade::hermes::json_string(text))
    }).collect();
    format!(
        "{{\"id\":{},\"status\":\"{}\",\"age_s\":{},\"question\":{},\"reply\":{},\"citations\":[{}],\"found\":[{}],\"inferred\":[{}],\"unknown\":[{}],\"calls\":{},\"call_budget\":{},\"split\":{},\"helpers\":[{}]}}",
        note.id,
        note.status,
        note.at.elapsed().as_secs(),
        galion_cascade::hermes::json_string(&note.question),
        galion_cascade::hermes::json_string(&note.reply),
        cites.join(","),
        json_list(&note.found),
        json_list(&note.inferred),
        json_list(&note.unknown),
        note.calls,
        note.call_budget,
        if note.split { "true" } else { "false" },
        research_helpers_json(&note.helpers)
    )
}

fn research_history_json(notes: &VecDeque<ResearchNote>) -> String {
    let items: Vec<String> = notes.iter().rev().map(research_note_json).collect();
    format!("{{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"items\":[{}]}}", items.join(","))
}

fn research_history_request(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
) -> Result<(String, String), ()> {
    let Ok(mut d) = desk.lock() else { return Err(()) };
    let mut id = d.ensure(cookie_id, header_id);
    let cookie = d.cookie_line(&id);
    let json = d.session_mut(&id).map(|s| research_history_json(&s.notes)).unwrap_or_else(|| research_history_json(&VecDeque::new()));
    wipe_string(&mut id);
    Ok((json, cookie))
}

/// Spoken reply for the voice-stack handoff. No browser cookie: the note stays
/// on its own session and is not mixed into another visitor's files.
fn research_reply(desk: &Arc<Mutex<ResearchDesk>>, question: &str) -> String {
    research_commit(desk, None, None, question)
        .map(|(_, note, mut cookie)| {
            wipe_string(&mut cookie);
            note.reply.clone()
        })
        .unwrap_or_else(|_| "The research path is not reachable from here.".into())
}

/// `(model, note, set-cookie)`. Sources, notes, and the stored reply belong to one session.
fn research_commit(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
    question: &str,
) -> Result<(String, ResearchNote, String), ()> {
    let (note_id, client, model_fallback, mut sources, mut history, mut session_id, cookie) = {
        let Ok(mut d) = desk.lock() else { return Err(()) };
        let session_id = d.ensure(cookie_id, header_id);
        let cookie = d.cookie_line(&session_id);
        d.status = "Running";
        d.next_id = d.next_id.saturating_add(1);
        let call_budget = d.client.as_ref().map(|c| c.research_budget().calls).unwrap_or(0);
        let model = d.client.as_ref().map(|c| c.label().to_string()).unwrap_or_else(|| galion_cascade::LLM_NAME.into());
        let (sources, history) = d.session_mut(&session_id).map(|s| {
            s.live = ResearchLive { phase: "planning", calls: 0, call_budget, helpers: Vec::new() };
            let sources: Vec<galion_cascade::hermes::ResearchSource> = s.sources.iter().map(|src| {
                galion_cascade::hermes::ResearchSource {
                    id: src.id,
                    name: src.name.clone(),
                    text: src.text.clone(),
                }
            }).collect();
            let history: Vec<galion_cascade::offline::LocalNote> = s.notes.iter().map(|n| {
                galion_cascade::offline::LocalNote {
                    found: n.found.clone(),
                    inferred: n.inferred.clone(),
                    unknown: n.unknown.clone(),
                }
            }).collect();
            (sources, history)
        }).unwrap_or_default();
        (d.next_id, d.client.clone(), model, sources, history, session_id, cookie)
    };
    let report = match client {
        Some(c) => {
            let watch = SessionWatch { desk: Arc::clone(desk), session: session_id.clone() };
            c.research_ask_watched(question, &sources, &watch)
        }
        None => galion_cascade::hermes::ResearchReport {
            answer: galion_cascade::hermes::ResearchAnswer {
                text: galion_cascade::UNREACHABLE.into(),
                citations: Vec::new(),
                found: Vec::new(),
                inferred: Vec::new(),
                unknown: Vec::new(),
                standing: "Unreachable",
            },
            calls: 0,
            call_budget: 0,
            split: false,
            helpers: Vec::new(),
        },
    };
    let calls = report.calls;
    let call_budget = report.call_budget;
    let split = report.split;
    let helpers = report.helpers;
    let turn = report.answer;
    // Hermes did not answer. Search only this session's files and notes.
    let (model_name, note) = if turn.standing == "Unreachable" {
        let local = galion_cascade::offline::offline_research(question, &sources, &history);
        let note = ResearchNote {
            id: note_id,
            question: question.to_string(),
            status: "Offline",
            reply: local.text,
            citations: local.citations,
            found: local.found,
            inferred: local.inferred,
            unknown: local.unknown,
            calls,
            call_budget,
            split,
            helpers,
            at: Instant::now(),
        };
        ("local".to_string(), note)
    } else {
        let status = turn.standing;
        let reply = turn.text;
        let citations = if status == "Denied" { Vec::new() } else { turn.citations };
        let mut found = if status == "Found" { turn.found } else { Vec::new() };
        let mut inferred = if status == "Found" || status == "Inferred" { turn.inferred } else { Vec::new() };
        let mut unknown = if status == "Found" || status == "Inferred" || status == "Unknown" { turn.unknown } else { Vec::new() };
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
            id: note_id,
            question: question.to_string(),
            status,
            reply,
            citations,
            found,
            inferred,
            unknown,
            calls,
            call_budget,
            split,
            helpers,
            at: Instant::now(),
        };
        (model_fallback, note)
    };
    for src in &mut sources {
        wipe_string(&mut src.name);
        wipe_string(&mut src.text);
    }
    for note in &mut history {
        for text in note.found.iter_mut().chain(note.inferred.iter_mut()).chain(note.unknown.iter_mut()) {
            wipe_string(text);
        }
    }
    let status = note.status;
    let stored = note.clone();
    if let Ok(mut d) = desk.lock() {
        d.status = status;
        if let Some(session) = d.session_mut(&session_id) {
            session.live.phase = "done";
            session.live.calls = note.calls;
            session.live.call_budget = note.call_budget;
            session.live.helpers = note.helpers.clone();
            session.notes.push_back(note);
            while session.notes.len() > 32 {
                session.notes.pop_front();
            }
        }
    }
    wipe_string(&mut session_id);
    Ok((model_name, stored, cookie))
}

/// `(body, set-cookie)`. The cookie is absent only when the desk lock is gone.
fn research_ask_request(
    desk: &Arc<Mutex<ResearchDesk>>,
    cookie_id: Option<&str>,
    header_id: Option<&str>,
    question: &str,
) -> (String, Option<String>) {
    let Ok((model, note, cookie)) = research_commit(desk, cookie_id, header_id, question) else {
        return ("{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"status\":\"Unreachable\",\"reply\":\"The research path is not reachable from here.\",\"citations\":[]}".into(), None);
    };
    let body = format!(
        "{{\"schema\":\"galion-research/1\",\"app\":\"hermes\",\"model\":{},{}",
        galion_cascade::hermes::json_string(&model),
        research_note_json(&note).trim_start_matches('{')
    );
    (body, Some(cookie))
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
    let (status, ctype, mut body, acao, mut set_cookie) = match read_request(&mut stream, MAX_BODY_BYTES) {
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
    let mut cookie_line = match set_cookie.as_deref() {
        Some(c) if !c.is_empty() => format!("Set-Cookie: {c}\r\n"),
        _ => String::new(),
    };
    wipe_opt(&mut set_cookie);
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{location}{cors}{cookie_line}Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: {CORS_ALLOW_HEADERS}\r\n{SECURITY_HEADERS}Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
    // The socket has the bytes. Do not leave a token or a WAV sitting in this buffer.
    wipe_string(&mut head);
    wipe_string(&mut cookie_line);
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
            let mut set_cookie = None;
            let (st, ct, body) = route(req, sh, &mut set_cookie);
            (st, ct, body, Acao::Keep(acao), set_cookie)
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

fn route(req: &mut Request, sh: &Shared, set_cookie: &mut Option<String>) -> (&'static str, &'static str, Vec<u8>) {
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
            let said = match sh.floor.lock() {
                Ok(mut floor) => Some(floor.finish_say(prepared, model)),
                Err(_) => None,
            };
            let Some(said) = said else {
                return (
                    "200 OK",
                    "application/json",
                    txt("{\"persona\":\"samantha\",\"reply\":\"\",\"silence\":true,\"film_voice\":false}"),
                );
            };
            let json = crate::v2_persona::say_to_json(&said);
            let silence = said.silence;
            let reply = said.reply.clone();
            let barge_out = said.barge;
            let quiet = silence || reply.trim().is_empty();
            let mouth = if said.sense_applied && !quiet {
                Some(MouthProsody {
                    pitch_st: said.prosody.pitch_st,
                    rate: said.prosody.rate,
                    loudness: said.prosody.loudness,
                    pre_pause_ms: said.prosody.pre_pause_ms,
                })
            } else {
                None
            };
            sh.says.push(if quiet { String::new() } else { reply }, quiet, barge_out, mouth);
            zeroise_bytes(&mut req.body);
            ("200 OK", "application/json", json.into_bytes())
        }
        ("POST", "/research/ask") if sh.page.is_some() => {
            let raw = String::from_utf8_lossy(&req.body).into_owned();
            zeroise_bytes(&mut req.body);
            let Some(question) = crate::v2_persona::json_field_string(&raw, "question") else {
                return ("400 Bad Request", "text/plain", txt("question"));
            };
            let question = question.trim().to_string();
            if question.is_empty() {
                return ("400 Bad Request", "text/plain", txt("question"));
            }
            let mut cookie_id = research_cookie_id(req);
            let mut header_id = research_header_id(req);
            let (json, cookie) = research_ask_request(&sh.research, cookie_id.as_deref(), header_id.as_deref(), &question);
            wipe_opt(&mut cookie_id);
            wipe_opt(&mut header_id);
            if let Some(c) = cookie {
                *set_cookie = Some(c);
            }
            ("200 OK", "application/json", json.into_bytes())
        }
        ("POST", "/research/sources") if sh.page.is_some() => {
            let raw = String::from_utf8_lossy(&req.body).into_owned();
            zeroise_bytes(&mut req.body);
            let mut cookie_id = research_cookie_id(req);
            let mut header_id = research_header_id(req);
            let answered = research_attach(&sh.research, cookie_id.as_deref(), header_id.as_deref(), &raw);
            wipe_opt(&mut cookie_id);
            wipe_opt(&mut header_id);
            match answered {
                Ok((json, cookie)) => {
                    *set_cookie = Some(cookie);
                    ("200 OK", "application/json", json.into_bytes())
                }
                Err("name") => ("400 Bad Request", "text/plain", txt("name")),
                Err(_) => ("500 Internal Server Error", "text/plain", txt("research")),
            }
        }
        ("DELETE", path) if sh.page.is_some() => {
            zeroise_bytes(&mut req.body);
            match parse_source_delete(path) {
                None => ("404 Not Found", "text/plain", txt("not found")),
                Some(Err(())) => ("400 Bad Request", "text/plain", txt("id")),
                Some(Ok(target)) => {
                    let mut cookie_id = research_cookie_id(req);
                    let mut header_id = research_header_id(req);
                    let answered = research_remove(&sh.research, cookie_id.as_deref(), header_id.as_deref(), target);
                    wipe_opt(&mut cookie_id);
                    wipe_opt(&mut header_id);
                    match answered {
                        Ok((json, cookie)) => {
                            *set_cookie = Some(cookie);
                            ("200 OK", "application/json", json.into_bytes())
                        }
                        Err("missing") => ("404 Not Found", "text/plain", txt("not found")),
                        Err(_) => ("500 Internal Server Error", "text/plain", txt("research")),
                    }
                }
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
            let voice_stack = req.header("host").is_some_and(galion_cascade::research_handoff::voice_stack_host);
            let id = sh.turns.push_from(fmt, bytes, voice_stack);
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
                    let mut cookie_id = research_cookie_id(req);
                    let mut header_id = research_header_id(req);
                    let answered = research_status_request(&sh.research, cookie_id.as_deref(), header_id.as_deref());
                    wipe_opt(&mut cookie_id);
                    wipe_opt(&mut header_id);
                    match answered {
                        Ok((json, cookie)) => {
                            *set_cookie = Some(cookie);
                            ("200 OK", "application/json", json.into_bytes())
                        }
                        Err(()) => ("500 Internal Server Error", "text/plain", txt("research")),
                    }
                }
                "/research/progress" if sh.page.is_some() => {
                    let mut cookie_id = research_cookie_id(req);
                    let mut header_id = research_header_id(req);
                    let answered = research_progress_request(&sh.research, cookie_id.as_deref(), header_id.as_deref());
                    wipe_opt(&mut cookie_id);
                    wipe_opt(&mut header_id);
                    match answered {
                        Ok((json, cookie)) => {
                            *set_cookie = Some(cookie);
                            ("200 OK", "application/json", json.into_bytes())
                        }
                        Err(()) => ("500 Internal Server Error", "text/plain", txt("research")),
                    }
                }
                "/research/history" if sh.page.is_some() => {
                    let mut cookie_id = research_cookie_id(req);
                    let mut header_id = research_header_id(req);
                    let answered = research_history_request(&sh.research, cookie_id.as_deref(), header_id.as_deref());
                    wipe_opt(&mut cookie_id);
                    wipe_opt(&mut header_id);
                    match answered {
                        Ok((json, cookie)) => {
                            *set_cookie = Some(cookie);
                            ("200 OK", "application/json", json.into_bytes())
                        }
                        Err(()) => ("500 Internal Server Error", "text/plain", txt("research")),
                    }
                }
                "/research/sources" if sh.page.is_some() => {
                    let mut cookie_id = research_cookie_id(req);
                    let mut header_id = research_header_id(req);
                    let answered = research_list(&sh.research, cookie_id.as_deref(), header_id.as_deref());
                    wipe_opt(&mut cookie_id);
                    wipe_opt(&mut header_id);
                    match answered {
                        Ok((json, cookie)) => {
                            *set_cookie = Some(cookie);
                            ("200 OK", "application/json", json.into_bytes())
                        }
                        Err(()) => ("500 Internal Server Error", "text/plain", txt("research")),
                    }
                }
                "/research" | "/research/" if sh.page.is_some() => {
                    let mut cookie_id = research_cookie_id(req);
                    let mut header_id = research_header_id(req);
                    if let Ok(cookie) = research_touch(&sh.research, cookie_id.as_deref(), header_id.as_deref()) {
                        *set_cookie = Some(cookie);
                    }
                    wipe_opt(&mut cookie_id);
                    wipe_opt(&mut header_id);
                    ("200 OK", "text/html; charset=utf-8", RESEARCH_HTML.as_bytes().to_vec())
                }
                "/dot" => {
                    let d = sh.latest_dot.lock().map(|g| g.clone()).unwrap_or_default();
                    let (persona, mood, from_turn, felt) = sh.floor.lock().map(|g| (g.id().to_string(), g.mood().to_string(), g.mood_from_turn(), g.last_felt())).unwrap_or_else(|_| ("samantha".into(), "calm".into(), false, None));
                    // Mood is attached only after a turn, and then emotion.label matches it.
                    // A sensed turn also publishes feeling_e, wash_amber, and the OKLCH decision.
                    // `reply` stays the last field.
                    let mood_field = if from_turn { Some(mood.as_str()) } else { None };
                    let dotted = crate::v2_persona::annotate_dot(&d, &persona, mood_field);
                    let dotted = crate::v2_persona::annotate_feeling(&dotted, felt);
                    let body = dot_with_reply(&dotted, sh.replies.latest_json().as_deref());
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
        let (body, cookie) = research_ask_request(&desk, None, None, "Is the reply one object?");
        assert!(cookie.unwrap_or_default().contains("HttpOnly"), "ask sets the research cookie");
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
            assert_research_cookie(&r);
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
                "<details id=\"helpers\">",
                "id=\"helpers-summary\"",
                "Helpers are quiet.",
                "/research/progress",
                "id=\"silence\"",
                "id:\"question-sent\"",
                "id:\"thinking\"",
                "id:\"answer-ready\"",
                "id:\"source-attached\"",
                "id:\"source-cited\"",
                "id:\"error\"",
                "id:\"window-open\"",
                "id:\"window-close\"",
                "id:\"window-minimise\"",
                "id:\"window-settle\"",
                "var FUND=432",
                "FUND*(5/4)",
                "FUND*(3/2)",
                "FUND*2",
                "FUND/4",
                "galion-one-silence",
                "o.type=\"sine\"",
                "docs/07-sound-guide/SOUND-LANGUAGE.md",
                "Local. What is already known stays marked local.",
                "data-ui=\"minimal\"",
                "data-standard=\"432\"",
                "--ink:#E6EDF3",
                "--sky:#58C4FF",
                "id=\"offline-mark\"",
                "aria-live=\"polite\"",
                ">Offline<",
                "G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network",
                "Developed and distributed by Galion Labs",
                "Perpetual Open MIT License",
                "Copyright © 2026 Galion Labs. All rights reserved.",
                "Permission is hereby granted, free of charge",
                "Copyright (c) 2026 Galion Labs",
                "#0B0F14",
                "#58C4FF",
                "#B8E8FF",
                "#0B5FA5",
                "#F5F5F7",
            ] {
                assert!(body.contains(needle), "{p} missing {needle}");
            }
            assert!(body.contains("<details id=\"helpers\">"), "helper progress is a disclosure");
            assert!(!body.contains("<details id=\"helpers\" open"), "helpers stay collapsed until opened");
            assert!(body.contains("<canvas id=\"voice-dot\""), "the restyled Voice V2 dot is the entry");
            assert!(!body.contains("id=\"s\""), "research is not the full-screen one-dot page");
            assert!(!body.contains("data-mark=\"orb\""), "the full-screen orb stays on /one");
            assert!(!body.contains("hz:288"), "research cues stay on 2:1, 3:2, and 5:4");
            assert!(!body.contains("id:\"ask\"") && !body.contains("id:\"local\""), "the old single-tone set is gone");
            assert!(!body.contains(".wav") && !body.contains(".mp3") && !body.contains(".ogg"));
            assert!(!body.contains("type=\"square\"") && !body.contains("type=\"sawtooth\"") && !body.contains("type=\"triangle\""));
            assert!(!body.contains("howler") && !body.contains("tone.js"));
        }
        w.close().unwrap();
    }

    fn post_json(addr: SocketAddr, path: &str, body: &str) -> String {
        call(addr, "POST", path, "", Some(body))
    }

    fn call(addr: SocketAddr, method: &str, path: &str, headers: &str, body: Option<&str>) -> String {
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        let payload = body.unwrap_or("");
        let length = if body.is_some() {
            format!("Content-Type: application/json\r\nContent-Length: {}\r\n", payload.len())
        } else {
            String::new()
        };
        let req = format!("{method} {path} HTTP/1.1\r\nHost: x\r\n{headers}{length}\r\n{payload}");
        s.write_all(req.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    fn research_cookie_of(resp: &str) -> String {
        resp.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if !name.eq_ignore_ascii_case("set-cookie") {
                return None;
            }
            let pair = value.trim().split(';').next()?.trim();
            let (k, v) = pair.split_once('=')?;
            (k == "galion_research").then(|| v.to_string())
        }).unwrap_or_default()
    }

    fn session_id_from_cookie(cookie: &str) -> String {
        cookie.split(';').next().unwrap_or("").trim().strip_prefix("galion_research=").unwrap_or("").to_string()
    }

    fn cookie_header(id: &str) -> String {
        format!("Cookie: galion_research={id}\r\n")
    }

    fn assert_research_cookie(resp: &str) {
        let line = resp.lines().find(|l| l.to_ascii_lowercase().starts_with("set-cookie:")).unwrap_or("");
        assert!(line.contains("galion_research="), "{line}");
        assert!(line.contains("HttpOnly"), "{line}");
        assert!(line.contains("SameSite=Strict"), "{line}");
        assert!(line.contains("Path=/research"), "{line}");
        let id = research_cookie_of(resp);
        assert_eq!(id.len(), 64, "{id}");
        assert!(id.bytes().all(|b| b.is_ascii_hexdigit()), "{id}");
        let body = resp.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        assert!(!id.is_empty() && !body.contains(&id), "session id stays out of the body");
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
        let denied = post_json(a, "/research/ask", r#"{"question":"Where is the source?"}"#);
        assert!(denied.contains("HTTP/1.1 200") && denied.contains("\"status\":\"Offline\""), "{denied}");
        assert!(denied.contains("Offline.") && denied.contains("The research path is not reachable from here."), "{denied}");
        assert!(denied.contains("I don't have a source for that."), "{denied}");
        assert!(denied.contains("\"citations\":[]") && denied.contains("\"found\":[]"), "{denied}");
        assert!(denied.contains("\"model\":\"local\""), "{denied}");
        assert!(!denied.contains(key) && !denied.contains("Paris"), "{denied}");
        assert_research_cookie(&denied);
        let sid = research_cookie_of(&denied);
        let hist = call(a, "GET", "/research/history", &cookie_header(&sid), None);
        assert!(hist.contains("Where is the source?") && hist.contains("\"status\":\"Offline\""), "{hist}");
        let other = get(a, "/research/history");
        assert!(!other.contains("Where is the source?"), "another browser does not see this question: {other}");
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
        let asked = post_json(a, "/research/ask", r#"{"question":"What colour is the wash?"}"#);
        assert!(asked.contains("HTTP/1.1 200") && asked.contains("\"status\":\"Unknown\""), "{asked}");
        assert!(asked.contains("\"calls\":1") && asked.contains("\"split\":false"), "{asked}");
        assert!(asked.contains("amber to violet"), "{asked}");
        assert!(!asked.contains("Galion brand note"), "an unprovided source must not be cited: {asked}");
        assert!(asked.contains("\"citations\":[]"), "{asked}");
        assert!(!asked.contains("NotFound") && !asked.contains("\"status\":\"Returned\""), "{asked}");
        assert!(asked.contains("\"app\":\"hermes\""), "{asked}");
        assert!(!asked.contains(key) && !asked.contains("Bearer"), "{asked}");
        let wash_sid = research_cookie_of(&asked);
        let hist = call(a, "GET", "/research/history", &cookie_header(&wash_sid), None);
        assert!(hist.contains("What colour is the wash?") && hist.contains("\"status\":\"Unknown\""), "{hist}");
        assert!(!hist.contains(key) && !hist.contains("Galion brand note"), "{hist}");
        let other = get(a, "/research/history");
        assert!(!other.contains("What colour is the wash?"), "{other}");
        w.close().unwrap();
    }

    #[test]
    fn research_helpers_stay_in_the_asking_session() {
        let key = "test-gateway-key-not-for-git";
        let release = Arc::new(AtomicBool::new(false));
        let release_w = release.clone();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        std::thread::spawn(move || {
            let mut handled = 0usize;
            while handled < 16 {
                match listener.accept() {
                    Ok((mut sock, _)) => {
                        handled += 1;
                        let _ = sock.set_nonblocking(false);
                        let release_c = release_w.clone();
                        std::thread::spawn(move || {
                            let _ = sock.set_read_timeout(Some(Duration::from_secs(2)));
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
                                                if !k.trim().eq_ignore_ascii_case("content-length") { return None; }
                                                v.trim().parse::<usize>().ok()
                                            }).unwrap_or(0);
                                            if got.len() >= end + len { break; }
                                        }
                                    }
                                }
                            }
                            let body = String::from_utf8_lossy(&got);
                            let content = if body.contains("You plan research") {
                                "- What colour does the note give the wash?\n- How would a cooler mix read at night?\n- Which hex is still missing from the note?\n"
                            } else if body.contains("Helper notes:") {
                                "The wash is amber and daylight is the assumed light."
                            } else {
                                let deadline = Instant::now() + Duration::from_secs(5);
                                while !release_c.load(Ordering::SeqCst) && Instant::now() < deadline {
                                    std::thread::sleep(Duration::from_millis(10));
                                }
                                if body.contains("cooler mix") {
                                    "Inferred\n- A cooler mix would read as night.\n"
                                } else if body.contains("still missing") {
                                    "Unknown\n- The hex is not in the note.\n"
                                } else {
                                    "Found\n- The wash is amber. [1]\n[1] wash-note.txt\n[9] Invented Journal 2024\n"
                                }
                            };
                            let payload = format!(
                                "{{\"choices\":[{{\"message\":{{\"content\":{}}}}}]}}",
                                galion_cascade::hermes::json_string(content)
                            );
                            let resp = format!(
                                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                                payload.len()
                            );
                            let _ = sock.write_all(resp.as_bytes());
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(5)),
                    Err(_) => break,
                }
            }
        });
        let mut llm = galion_cascade::HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            "hermes-agent",
            key,
            Duration::from_millis(4000),
        );
        llm.set_research_budget(galion_cascade::hermes::ResearchBudget {
            helpers: 3,
            concurrency: 2,
            budget: Duration::from_secs(12),
            calls: 5,
        });
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.set_research_hermes(llm);
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let file = post_json(a, "/research/sources", r#"{"name":"wash-note.txt","kind":"file","text":"The wash is amber."}"#);
        assert!(file.contains("wash-note.txt") && file.contains("\"id\":1"), "{file}");
        let sid_a = research_cookie_of(&file);
        let other = get(a, "/research/hermes");
        let sid_b = research_cookie_of(&other);
        assert_ne!(sid_a, sid_b, "two browsers are two sessions");
        let ask_a = a;
        let cookie_a = cookie_header(&sid_a);
        let ask = std::thread::spawn(move || {
            call(
                ask_a,
                "POST",
                "/research/ask",
                &cookie_a,
                Some(r#"{"question":"Compare the daylight wash with the tungsten wash and say which hex is still unknown?"}"#),
            )
        });
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut saw = String::new();
        while Instant::now() < deadline {
            let prog = call(a, "GET", "/research/progress", &cookie_header(&sid_a), None);
            if prog.contains("\"running\"") {
                saw = prog;
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        let other_prog = call(a, "GET", "/research/progress", &cookie_header(&sid_b), None);
        release.store(true, Ordering::SeqCst);
        assert!(saw.contains("\"phase\":\"helpers\"") && saw.contains("\"running\""), "{saw}");
        assert!(saw.contains("What colour does the note"), "{saw}");
        assert!(!saw.contains(key), "{saw}");
        assert!(!other_prog.contains("What colour does the note"), "another browser does not see these helpers: {other_prog}");
        assert!(!other_prog.contains("still missing"), "{other_prog}");
        let asked = ask.join().unwrap();
        assert!(asked.contains("HTTP/1.1 200") && asked.contains("\"split\":true"), "{asked}");
        assert!(asked.contains("\"calls\":5"), "{asked}");
        assert!(asked.contains("wash-note.txt") && asked.contains("\"n\":1"), "{asked}");
        assert!(asked.contains("amber"), "{asked}");
        assert!(!asked.contains("Invented Journal"), "{asked}");
        assert!(!asked.contains(key) && !asked.contains("Bearer"), "{asked}");
        let hist_b = call(a, "GET", "/research/history", &cookie_header(&sid_b), None);
        assert!(!hist_b.contains("What colour does the note"), "{hist_b}");
        assert!(!hist_b.contains("Compare the daylight wash"), "{hist_b}");
        let hist_a = call(a, "GET", "/research/history", &cookie_header(&sid_a), None);
        assert!(hist_a.contains("\"calls\":5") && hist_a.contains("\"split\":true"), "{hist_a}");
        assert!(!hist_a.contains("Invented Journal") && !hist_a.contains(key), "{hist_a}");
        w.close().unwrap();
    }

    #[test]
    fn research_reply_speaks_the_stub_hermes_answer() {
        let key = "test-gateway-key-not-for-git";
        let payload = "{\"choices\":[{\"message\":{\"content\":\"The wash stays near-white.\\nUnknown\\n- The wash stays near-white.\"}}]}";
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else { return };
            let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = [0u8; 8192];
            loop {
                match sock.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") { break; }
                    }
                }
            }
            let resp = format!(
                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        let w = WebSurface::mailbox().with_page("<!doctype html><canvas></canvas>");
        w.set_research_hermes(galion_cascade::HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            "hermes-agent",
            key,
            Duration::from_millis(1000),
        ));
        let spoken = (w.research_ask_fn())("what colour is the wash");
        assert!(spoken.contains("near-white"), "{spoken}");
        assert!(!spoken.contains(key) && !spoken.contains("Bearer"), "{spoken}");
        let hist = {
            let desk = w.sh.research.lock().unwrap();
            let mut notes = VecDeque::new();
            for session in &desk.sessions {
                for note in &session.notes {
                    notes.push_back(note.clone());
                }
            }
            research_history_json(&notes)
        };
        assert!(hist.contains("what colour is the wash") && hist.contains("near-white"), "{hist}");
        assert!(!hist.contains(key), "{hist}");
    }

    #[test]
    fn research_ask_continues_offline_when_hermes_is_down() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap()).with_page("<!doctype html><canvas></canvas>");
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let file = post_json(
            a,
            "/research/sources",
            r#"{"name":"notes/wash-note.txt","kind":"file","text":"The wash is amber. The hull is grey."}"#,
        );
        assert!(file.contains("wash-note.txt") && file.contains("\"id\":1"), "{file}");
        assert_research_cookie(&file);
        let sid = research_cookie_of(&file);
        let asked = call(a, "POST", "/research/ask", &cookie_header(&sid), Some(r#"{"question":"What colour is the wash?"}"#));
        assert!(asked.contains("HTTP/1.1 200") && asked.contains("\"status\":\"Offline\""), "{asked}");
        assert!(asked.contains("\"model\":\"local\""), "{asked}");
        assert!(asked.contains("Offline.") && asked.contains("The wash is amber."), "{asked}");
        assert!(asked.contains("wash-note.txt") && asked.contains("\"n\":1"), "{asked}");
        assert!(!asked.contains("grey"), "{asked}");
        let miss = call(a, "POST", "/research/ask", &cookie_header(&sid), Some(r#"{"question":"What is the capital of France?"}"#));
        assert!(miss.contains("\"status\":\"Offline\"") && miss.contains("I don't have a source for that."), "{miss}");
        assert!(miss.contains("\"citations\":[]") && miss.contains("\"found\":[]"), "{miss}");
        assert!(!miss.contains("Paris") && !miss.contains("amber") && !miss.contains("wash-note"), "{miss}");
        let other = post_json(a, "/research/ask", r#"{"question":"What colour is the wash?"}"#);
        assert!(other.contains("\"status\":\"Offline\"") && other.contains("I don't have a source for that."), "{other}");
        assert!(!other.contains("The wash is amber.") && !other.contains("wash-note"), "another browser does not search this session: {other}");
        let hist = call(a, "GET", "/research/history", &cookie_header(&sid), None);
        assert!(hist.contains("The wash is amber.") && hist.contains("\"status\":\"Offline\""), "{hist}");
        let cleared = call(a, "DELETE", "/research/sources", &cookie_header(&sid), None);
        assert!(cleared.contains("\"sources\":[]"), "{cleared}");
        let again = call(a, "POST", "/research/ask", &cookie_header(&sid), Some(r#"{"question":"What colour is the wash?"}"#));
        assert!(again.contains("\"status\":\"Offline\"") && again.contains("The wash is amber."), "{again}");
        assert!(again.contains("\"citations\":[]") && again.contains("\"found\":[]"), "a removed file is not cited: {again}");
        assert!(again.contains("\"inferred\":[\"The wash is amber.\"]"), "{again}");
        w.close().unwrap();

        let key = "test-gateway-key-not-for-git";
        let poison = "The wash is violet according to the Invented Journal.";
        let payload = format!("{{\"choices\":[{{\"message\":{{\"content\":\"{poison}\"}}}}]}}");
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
                "HTTP/1.0 500 Down\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
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
        assert!(ready.contains("\"key\":\"set\"") && ready.contains("\"status\":\"Idle\""), "{ready}");
        let attached = post_json(a, "/research/sources", r#"{"name":"wash-note.txt","text":"The wash is amber."}"#);
        let sid = research_cookie_of(&attached);
        let asked = call(a, "POST", "/research/ask", &cookie_header(&sid), Some(r#"{"question":"What colour is the wash?"}"#));
        assert!(asked.contains("HTTP/1.1 200") && asked.contains("\"status\":\"Offline\""), "{asked}");
        assert!(asked.contains("The wash is amber.") && asked.contains("wash-note.txt"), "{asked}");
        assert!(!asked.contains("violet") && !asked.contains("Invented"), "{asked}");
        assert!(!asked.contains(key) && !asked.contains("Bearer"), "{asked}");
        let stranger = post_json(a, "/research/ask", r#"{"question":"What colour is the wash?"}"#);
        assert!(!stranger.contains("The wash is amber.") && !stranger.contains("wash-note"), "{stranger}");
        let upstream = seen.lock().unwrap().clone();
        assert!(upstream.contains("What colour is the wash?"), "the stub was reached: {upstream}");
        assert!(upstream.contains("Authorization: Bearer "), "{upstream}");
        assert!(!upstream.contains("violet"), "{upstream}");
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
        let unnamed = post_json(a, "/research/sources", r#"{"text":"no name"}"#);
        assert!(unnamed.starts_with("HTTP/1.1 400"), "{unnamed}");
        let image = post_json(a, "/research/sources", r#"{"name":"../shot.png","kind":"image","text":"pixels"}"#);
        assert!(image.contains("HTTP/1.1 200") && image.contains("shot.png") && image.contains("\"kind\":\"image\"") && image.contains("\"chars\":0"), "{image}");
        assert!(!image.contains("../"), "{image}");
        assert_research_cookie(&image);
        let sid = research_cookie_of(&image);
        let file = call(
            a,
            "POST",
            "/research/sources",
            &cookie_header(&sid),
            Some(r#"{"name":"notes/wash-note.txt","kind":"file","text":"The wash is amber."}"#),
        );
        assert!(file.contains("wash-note.txt") && file.contains("\"kind\":\"file\"") && file.contains("\"chars\":18") && file.contains("\"id\":2"), "{file}");
        assert!(!file.contains("notes/"), "{file}");
        let asked = call(a, "POST", "/research/ask", &cookie_header(&sid), Some(r#"{"question":"What colour is the wash?"}"#));
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
        let hist = call(a, "GET", "/research/history", &cookie_header(&sid), None);
        assert!(hist.contains("\"status\":\"Found\"") && !hist.contains("Invented Journal"), "{hist}");
        w.close().unwrap();
    }

    #[test]
    fn research_desk_keeps_the_latest_eight_sources() {
        let desk = Arc::new(Mutex::new(ResearchDesk::new()));
        let (other_json, other_cookie) = research_attach(&desk, None, None, r#"{"name":"kept.txt","text":"stay"}"#).unwrap();
        let other = session_id_from_cookie(&other_cookie);
        assert!(other_json.contains("kept.txt") && !other_json.contains("n1.txt"), "{other_json}");
        let mut sid: Option<String> = None;
        for i in 1..=9 {
            let body = format!(r#"{{"name":"n{i}.txt","text":"t{i}"}}"#);
            let (json, cookie) = research_attach(&desk, sid.as_deref(), None, &body).unwrap();
            if sid.is_none() {
                sid = Some(session_id_from_cookie(&cookie));
            }
            if i == 9 {
                assert!(json.contains("n9.txt") && json.contains("n2.txt") && !json.contains("n1.txt"), "{json}");
                assert!(!json.contains("kept.txt"), "{json}");
            }
        }
        let id = sid.unwrap();
        let (kept, _) = research_list(&desk, Some(&other), None).unwrap();
        assert!(kept.contains("kept.txt") && !kept.contains("n9.txt") && !kept.contains("n2.txt"), "{kept}");
        let d = desk.lock().unwrap();
        let s = d.sessions.iter().find(|s| s.id == id).unwrap();
        assert_eq!(s.sources.len(), 8);
        assert_eq!(s.sources[0].name, "n2.txt");
        assert_eq!(s.sources[7].id, 9);
        assert_eq!(s.sources[7].name, "n9.txt");
    }

    #[test]
    fn research_cookie_wins_over_the_header_and_a_forged_id_is_not_adopted() {
        let desk = Arc::new(Mutex::new(ResearchDesk::new()));
        let (_, cookie_a) = research_attach(&desk, None, None, r#"{"name":"alpha.txt","text":"alpha-secret"}"#).unwrap();
        let id_a = session_id_from_cookie(&cookie_a);
        let (_, cookie_b) = research_attach(&desk, None, None, r#"{"name":"beta.txt","text":"beta-secret"}"#).unwrap();
        let id_b = session_id_from_cookie(&cookie_b);
        assert_ne!(id_a, id_b);
        let (listed, _) = research_list(&desk, Some(&id_b), Some(&id_a)).unwrap();
        assert!(listed.contains("beta.txt") && !listed.contains("alpha.txt"), "{listed}");
        let (listed, _) = research_list(&desk, None, Some(&id_a)).unwrap();
        assert!(listed.contains("alpha.txt") && !listed.contains("beta-secret"), "{listed}");
        let forged = "ab".repeat(32);
        let (listed, cookie) = research_list(&desk, Some(&forged), None).unwrap();
        assert!(listed.contains("\"sources\":[]") && !listed.contains("alpha.txt"), "{listed}");
        assert_ne!(session_id_from_cookie(&cookie), forged);
        let removed = research_remove(&desk, Some(&id_b), None, SourceDelete::One(1));
        assert!(removed.is_ok(), "beta's own id 1 is removed");
        let (alpha, _) = research_list(&desk, Some(&id_a), None).unwrap();
        assert!(alpha.contains("alpha.txt"), "deleting beta's id leaves alpha: {alpha}");
    }

    #[test]
    fn research_files_and_idle_sessions_expire() {
        let desk = Arc::new(Mutex::new(ResearchDesk::new()));
        desk.lock().unwrap().ttl = Duration::from_millis(30);
        let (_, cookie) = research_attach(&desk, None, None, r#"{"name":"fade.txt","text":"fades"}"#).unwrap();
        let id = session_id_from_cookie(&cookie);
        std::thread::sleep(Duration::from_millis(70));
        desk.lock().unwrap().sessions[0].touched = Instant::now();
        let (listed, cookie2) = research_list(&desk, Some(&id), None).unwrap();
        assert!(listed.contains("\"sources\":[]") && !listed.contains("fade.txt") && !listed.contains("fades"), "{listed}");
        assert_eq!(session_id_from_cookie(&cookie2), id, "a live session keeps its id when only the file expired");

        let (_, cookie) = research_attach(&desk, None, None, r#"{"name":"idle.txt","text":"goes"}"#).unwrap();
        let idle = session_id_from_cookie(&cookie);
        std::thread::sleep(Duration::from_millis(70));
        let (listed, cookie3) = research_list(&desk, Some(&idle), None).unwrap();
        assert!(listed.contains("\"sources\":[]") && !listed.contains("idle.txt"), "{listed}");
        assert_ne!(session_id_from_cookie(&cookie3), idle, "an idle session is not resumed");
    }

    #[test]
    fn research_session_cap_drops_the_quiet_browser() {
        let desk = Arc::new(Mutex::new(ResearchDesk::new()));
        let (json, cookie) = research_attach(&desk, None, None, r#"{"name":"first.txt","text":"quiet-browser"}"#).unwrap();
        let first = session_id_from_cookie(&cookie);
        assert!(json.contains("first.txt"), "{json}");
        for _ in 1..RESEARCH_SESSION_CAP {
            research_touch(&desk, None, None).unwrap();
        }
        assert_eq!(desk.lock().unwrap().sessions.len(), RESEARCH_SESSION_CAP);
        research_touch(&desk, None, None).unwrap();
        let d = desk.lock().unwrap();
        assert!(d.sessions.len() <= RESEARCH_SESSION_CAP);
        assert!(d.sessions.iter().all(|s| s.id != first));
        assert!(d.sessions.iter().all(|s| s.sources.iter().all(|src| src.name != "first.txt")));
    }

    #[test]
    fn two_browsers_cannot_see_each_others_research_files() {
        let key = "test-gateway-key-not-for-git";
        let payload = "{\"choices\":[{\"message\":{\"content\":\"Unknown\\n- The file is the limit.\\n\"}}]}";
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_w = seen.clone();
        std::thread::spawn(move || {
            for _ in 0..2 {
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
        let page = get(a, "/research");
        assert_research_cookie(&page);
        assert!(page.contains("data-ui=\"minimal\"") && page.contains("--ink:#E6EDF3") && page.contains("--sky:#58C4FF"), "{page}");
        let blocked = get(a, "/research/sources?galion_research=not-a-cookie");
        assert!(blocked.starts_with("HTTP/1.1 400"), "{blocked}");

        let alpha = call(a, "POST", "/research/sources", "", Some(r#"{"name":"alpha.txt","text":"alpha-secret-ink"}"#));
        assert_research_cookie(&alpha);
        let sid_a = research_cookie_of(&alpha);
        assert!(alpha.contains("alpha.txt") && !alpha.contains("beta.txt"), "{alpha}");
        let beta = call(a, "POST", "/research/sources", "", Some(r#"{"name":"beta.txt","text":"beta-secret-sky"}"#));
        let sid_b = research_cookie_of(&beta);
        assert_ne!(sid_a, sid_b);
        assert!(beta.contains("beta.txt") && !beta.contains("alpha.txt") && !beta.contains("alpha-secret-ink"), "{beta}");
        let second = call(a, "POST", "/research/sources", &cookie_header(&sid_a), Some(r#"{"name":"alpha-2.txt","text":"alpha-second"}"#));
        assert!(second.contains("alpha.txt") && second.contains("alpha-2.txt") && !second.contains("beta.txt"), "{second}");
        assert_eq!(research_cookie_of(&second), sid_a);

        let via_header = call(a, "GET", "/research/sources", &format!("X-Galion-Research: {sid_a}\r\n"), None);
        assert!(via_header.contains("alpha.txt") && via_header.contains("alpha-2.txt") && !via_header.contains("beta"), "{via_header}");
        let forged = "ab".repeat(32);
        let forged_list = call(a, "GET", "/research/sources", &format!("X-Galion-Research: {forged}\r\n"), None);
        assert!(forged_list.contains("\"sources\":[]"), "{forged_list}");
        assert_ne!(research_cookie_of(&forged_list), forged);
        assert!(!forged_list.contains("alpha") && !forged_list.contains("beta"), "{forged_list}");
        let list_b = call(a, "GET", "/research/sources", &cookie_header(&sid_b), None);
        assert!(list_b.contains("beta.txt") && !list_b.contains("alpha"), "{list_b}");

        let ask_a = call(a, "POST", "/research/ask", &cookie_header(&sid_a), Some(r#"{"question":"What is in alpha?"}"#));
        let ask_b = call(a, "POST", "/research/ask", &cookie_header(&sid_b), Some(r#"{"question":"What is in beta?"}"#));
        assert!(!ask_a.contains("beta-secret-sky") && !ask_a.contains("beta.txt"), "{ask_a}");
        assert!(!ask_b.contains("alpha-secret-ink") && !ask_b.contains("alpha.txt"), "{ask_b}");
        let upstream = seen.lock().unwrap().clone();
        assert_eq!(upstream.len(), 2, "{upstream:?}");
        assert!(upstream[0].contains("alpha-secret-ink") && upstream[0].contains("alpha-second") && !upstream[0].contains("beta-secret-sky"), "{}", upstream[0]);
        assert!(upstream[1].contains("beta-secret-sky") && !upstream[1].contains("alpha-secret-ink") && !upstream[1].contains("alpha-second"), "{}", upstream[1]);

        let hist_a = call(a, "GET", "/research/history", &cookie_header(&sid_a), None);
        let hist_b = call(a, "GET", "/research/history", &cookie_header(&sid_b), None);
        assert!(hist_a.contains("What is in alpha?") && !hist_a.contains("What is in beta?"), "{hist_a}");
        assert!(hist_b.contains("What is in beta?") && !hist_b.contains("What is in alpha?"), "{hist_b}");

        let cross = call(a, "DELETE", "/research/sources/2", &cookie_header(&sid_b), None);
        assert!(cross.starts_with("HTTP/1.1 404"), "{cross}");
        let still_a = call(a, "GET", "/research/sources", &cookie_header(&sid_a), None);
        assert!(still_a.contains("alpha-2.txt") && still_a.contains("alpha.txt"), "{still_a}");
        let removed = call(a, "DELETE", "/research/sources/1", &cookie_header(&sid_a), None);
        assert!(removed.starts_with("HTTP/1.1 200") && removed.contains("alpha-2.txt") && !removed.contains("alpha.txt"), "{removed}");
        let still_b = call(a, "GET", "/research/sources", &cookie_header(&sid_b), None);
        assert!(still_b.contains("beta.txt"), "{still_b}");
        let cleared = call(a, "DELETE", "/research/sources", &cookie_header(&sid_a), None);
        assert!(cleared.starts_with("HTTP/1.1 200") && cleared.contains("\"sources\":[]"), "{cleared}");
        let still_b = call(a, "GET", "/research/sources", &cookie_header(&sid_b), None);
        assert!(still_b.contains("beta.txt") && !still_b.contains("alpha"), "{still_b}");
        let clear_b = call(a, "DELETE", "/research/sources", &cookie_header(&sid_b), None);
        assert!(clear_b.contains("\"sources\":[]") && !clear_b.contains("beta.txt"), "{clear_b}");
        let bad = call(a, "DELETE", "/research/sources/nope", &cookie_header(&sid_b), None);
        assert!(bad.starts_with("HTTP/1.1 400"), "{bad}");
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
        assert!(line.mouth.is_none(), "sense off does not steer the mouth");
        assert!(!dot.contains("\"wash_amber\""), "sense off does not publish the feeling wash: {dot}");
        w.close().unwrap();
    }

    #[test]
    fn sensed_say_moves_wash_and_queues_mouth_prosody() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let post = |body: &str| {
            let mut s = std::net::TcpStream::connect(a).unwrap();
            s.write_all(format!(
                "POST /v2/say HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ).as_bytes()).unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        };
        let sam = post("{\"text\":\"My mother died this morning.\",\"e\":1,\"sense\":true}");
        assert!(sam.contains("HTTP/1.1 200"), "{sam}");
        assert!(sam.contains("\"persona\":\"samantha\""), "{sam}");
        assert!(sam.contains("\"sense\":true"), "{sam}");
        assert!(sam.contains("\"feeling_e\":1"), "{sam}");
        assert!(sam.contains("\"wash_amber\":0.5"), "{sam}");
        assert!(sam.contains("\"pre_pause_ms\":250"), "{sam}");
        assert!(sam.contains("\"loudness\":0.8"), "{sam}");
        assert!(sam.contains("\"pitch_st\":-0.5"), "{sam}");
        assert!(sam.contains("\"l\":0.72") && sam.contains("\"h\":310"), "{sam}");
        assert!(sam.contains("\"mouth\":\"piper\""), "{sam}");
        assert!(!sam.to_ascii_lowercase().contains("i hear you"), "{sam}");
        assert!(!sam.contains("Your mother died this morning."), "{sam}");
        let line = w.say_inbox().take_latest().expect("samantha line");
        let mouth = line.mouth.expect("sense on queues mouth prosody");
        assert!((mouth.pitch_st - -0.5).abs() < 1e-3);
        assert!((mouth.loudness - 0.8).abs() < 1e-3);
        assert_eq!(mouth.pre_pause_ms, 250);
        assert!((mouth.rate - 0.92).abs() < 1e-3);
        let dot = get(a, "/dot");
        assert!(dot.contains("\"wash_amber\":0.5"), "{dot}");
        assert!(dot.contains("\"feeling_e\":1"), "{dot}");
        assert!(dot.contains("\"oklch\""), "{dot}");
        let jar = post("{\"text\":\"be Jarvis\",\"e\":0,\"sense\":true}");
        assert!(jar.contains("\"persona\":\"jarvis\""), "{jar}");
        let _ = w.say_inbox().take_latest();
        let high = post("{\"text\":\"My mother died this morning.\",\"e\":1,\"sense\":true}");
        assert!(high.contains("\"persona\":\"jarvis\""), "{high}");
        assert!(high.contains("\"wash_amber\":0.5"), "{high}");
        assert!(high.contains("\"pre_pause_ms\":120"), "{high}");
        assert!(high.contains("\"loudness\":0.88"), "{high}");
        assert!(high.contains("\"pitch_st\":-0.4"), "{high}");
        let rest = post("{\"text\":\"My mother died this morning.\",\"e\":0,\"sense\":true}");
        assert!(rest.contains("\"persona\":\"jarvis\""), "{rest}");
        assert!(rest.contains("\"wash_amber\":0.28"), "{rest}");
        assert!(rest.contains("\"pre_pause_ms\":80"), "{rest}");
        assert!(rest.contains("\"loudness\":0.92"), "{rest}");
        assert!(rest.contains("\"pitch_st\":-0.3"), "{rest}");
        assert!(rest.contains("\"l\":0.971"), "{rest}");
        let denied = post("{\"text\":\"My mother died this morning.\",\"e\":1,\"sense\":false}");
        assert!(denied.contains("\"sense\":false"), "{denied}");
        let denied_line = w.say_inbox().take_latest().expect("denied still speaks");
        assert!(denied_line.mouth.is_none(), "sense off does not move the mouth");
        let dot_denied = get(a, "/dot");
        assert!(!dot_denied.contains("\"wash_amber\""), "{dot_denied}");
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
        assert!(!t.voice_stack, "Host x is not the voice stack");
        w.close().unwrap();
    }

    #[test]
    fn voice_stack_host_marks_the_turn_and_the_one_host_does_not() {
        let mut w = WebSurface::serve("127.0.0.1:0".parse().unwrap());
        let inbox = w.turn_inbox();
        w.open().unwrap();
        let a = w.local_addr().unwrap();
        let wav = encode_wav_pcm16(16_000, 1, &[0, 1, 0]);
        let post = |host: &str| {
            let mut s = std::net::TcpStream::connect(a).unwrap();
            s.write_all(
                format!(
                    "POST /turn HTTP/1.1\r\nHost: {host}\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\n\r\n",
                    wav.len()
                )
                .as_bytes(),
            )
            .unwrap();
            s.write_all(&wav).unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            assert!(out.starts_with("HTTP/1.1 202"), "{host} {out}");
        };
        post("voice.galion.app");
        let voice = inbox.take_latest().unwrap();
        assert!(voice.voice_stack, "voice.galion.app is the voice stack");
        post("galion.app");
        let one = inbox.take_latest().unwrap();
        assert!(!one.voice_stack, "galion.app/one is not handed to research");
        post("galion.app:443");
        assert!(!inbox.take_latest().unwrap().voice_stack);
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
            voice_stack: false,
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
