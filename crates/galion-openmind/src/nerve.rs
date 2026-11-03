// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Nerve notes from the voice engine.
//!
//! The wire shape matches `openmind_nerve::evt::NerveEvent` schema version 1
//! (NDJSON, one object per line) and the older B33 body that `POST /event`
//! accepts: `{"source","kind","severity","data"}`. Field names were read from
//! `galion-studio/obsidian-open-mind` main `7ba85b60` on 2026-11-03. This
//! file does not depend on that crate.
//!
//! [`NerveEmitter::note`] is the only call a frame host may make, and only
//! **after** `Heartbeat::step` returns. It copies a [`VoiceNote`] into a
//! bounded queue. It does not format JSON, touch the clock, open a socket,
//! or take a lock. The worker thread does those.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// `openmind_nerve::evt::SCHEMA_VERSION`.
pub const NERVE_SCHEMA_VERSION: u32 = 1;

/// `source` on every voice note. The nerve's HTTP ingest rewrites the stream
/// source to `http`; this name is what `POST /event` records and what the
/// local NDJSON line carries.
pub const SOURCE: &str = "galion-voice";

/// The three kinds in `galion-student/configs/openmind-slot.toml` `[hooks]`.
pub const VOICE_KINDS: [&str; 3] = ["voice.frame_miss", "voice.fallback", "voice.backend_up"];

/// Fourth Open Mind voice backend, plus the three that sit in front of it.
/// Numbers are the voice crate's own codes in `data.code`, not an OOM enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VoiceBackendSlot {
    Mock = 1,
    CpuPipeline = 2,
    /// PersonaPlex / relay.
    Relay = 3,
    /// `StudentBackend`. The slot contract's fourth backend.
    Student = 4,
}

impl VoiceBackendSlot {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::CpuPipeline => "cpu_pipeline",
            Self::Relay => "relay",
            Self::Student => "student",
        }
    }
}

/// Inventory group on the nerve event. These two names are the voice seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NerveGroup {
    /// The voice engine's own clock and backends.
    GalionOs,
    /// A fallback whose analysis belongs to Hermes, not to the audio tick.
    Hermes,
}

impl NerveGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GalionOs => "galion-os",
            Self::Hermes => "hermes",
        }
    }

    /// Unit string `openmind_nerve::evt::classify_unit` maps back onto
    /// [`Self::as_str`]. `from_http_event` copies `data.unit` through that
    /// function, so a pushed event lands in the same group as the local line.
    pub fn unit(self) -> &'static str {
        match self {
            Self::GalionOs => "galion-os-voice",
            Self::Hermes => "hermes-sideband",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceKind {
    FrameMiss,
    Fallback,
    BackendUp,
}

impl VoiceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FrameMiss => "voice.frame_miss",
            Self::Fallback => "voice.fallback",
            Self::BackendUp => "voice.backend_up",
        }
    }

    fn msg(self) -> &'static str {
        match self {
            Self::FrameMiss => "voice frame missed",
            Self::Fallback => "voice backend fell back",
            Self::BackendUp => "voice backend up",
        }
    }
}

/// Nerve severity spellings (`info|low|medium|high|critical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Copy-sized observation. Safe to build on the frame host after `step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceNote {
    pub kind: VoiceKind,
    pub group: NerveGroup,
    pub severity: Severity,
    pub frame: u32,
    pub turn: u64,
    pub code: u32,
    pub backend: Option<VoiceBackendSlot>,
    pub from: Option<VoiceBackendSlot>,
    pub to: Option<VoiceBackendSlot>,
}

impl VoiceNote {
    /// A paused heartbeat frame. Group `galion-os`, severity medium.
    pub fn frame_miss(frame: u32) -> Self {
        Self {
            kind: VoiceKind::FrameMiss,
            group: NerveGroup::GalionOs,
            severity: Severity::Medium,
            frame,
            turn: 0,
            code: 0,
            backend: None,
            from: None,
            to: None,
        }
    }

    /// The supervisor gave up on `from` and continued on `to`.
    /// Group `hermes`: the audio path already fell back; thinking about why
    /// is the side band's job.
    pub fn fallback(frame: u32, turn: u64, from: VoiceBackendSlot, to: VoiceBackendSlot) -> Self {
        Self {
            kind: VoiceKind::Fallback,
            group: NerveGroup::Hermes,
            severity: Severity::High,
            frame,
            turn,
            code: ((from as u32) << 8) | (to as u32),
            backend: None,
            from: Some(from),
            to: Some(to),
        }
    }

    /// A backend became available. Group `galion-os`, severity info.
    pub fn backend_up(frame: u32, slot: VoiceBackendSlot) -> Self {
        Self {
            kind: VoiceKind::BackendUp,
            group: NerveGroup::GalionOs,
            severity: Severity::Info,
            frame,
            turn: 0,
            code: slot as u32,
            backend: Some(slot),
            from: None,
            to: None,
        }
    }
}

/// One rendered note: the stream line, and the `POST /event` body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedEvent {
    /// `evt::NerveEvent` v1, one JSON object, no trailing newline.
    pub line: String,
    /// B33 body `event_from` accepts.
    pub post_body: String,
}

/// Format one note. `seq` is the nerve monotonic id (workers start at 1).
/// `unix_s` is seconds since the Unix epoch; the timestamp is UTC so this
/// crate does not depend on `zagreb-time`. The nerve stores `ts` as a string.
pub fn render_event(note: VoiceNote, seq: u64, unix_s: u64) -> RenderedEvent {
    let data = data_json(&note);
    let kind = note.kind.as_str();
    let key = format!(
        "{kind}:frame:{}:turn:{}:code:{}",
        note.frame, note.turn, note.code
    );
    let line = format!(
        concat!(
            "{{\"v\":{v},\"seq\":{seq},\"ts\":\"{ts}\",\"t\":{t},\"source\":\"{source}\",",
            "\"kind\":\"{kind}\",\"severity\":\"{sev}\",\"unit\":\"{unit}\",\"group\":\"{group}\",",
            "\"key\":\"{key}\",\"msg\":\"{msg}\",\"data\":{data},\"count\":1}}"
        ),
        v = NERVE_SCHEMA_VERSION,
        seq = seq,
        ts = rfc3339_utc(unix_s),
        t = unix_s,
        source = SOURCE,
        kind = kind,
        sev = note.severity.as_str(),
        unit = note.group.unit(),
        group = note.group.as_str(),
        key = key,
        msg = note.kind.msg(),
        data = data,
    );
    let post_body = format!(
        "{{\"source\":\"{source}\",\"kind\":\"{kind}\",\"severity\":\"{sev}\",\"data\":{data}}}",
        source = SOURCE,
        kind = kind,
        sev = note.severity.as_str(),
        data = data,
    );
    RenderedEvent { line, post_body }
}

fn data_json(note: &VoiceNote) -> String {
    let mut s = format!(
        "{{\"frame\":{},\"turn\":{},\"code\":{},",
        note.frame, note.turn, note.code
    );
    if let (Some(from), Some(to)) = (note.from, note.to) {
        s.push_str(&format!(
            "\"from\":\"{}\",\"to\":\"{}\",",
            from.as_str(),
            to.as_str()
        ));
    }
    if let Some(backend) = note.backend {
        s.push_str(&format!("\"backend\":\"{}\",", backend.as_str()));
    }
    s.push_str(&format!(
        "\"unit\":\"{}\",\"group\":\"{}\",\"msg\":\"{}\"}}",
        note.group.unit(),
        note.group.as_str(),
        note.kind.msg(),
    ));
    s
}

/// `http://127.0.0.1:8670` or `http://[::1]:8670`. Numeric loopback only.
/// Hostnames are refused so this function never blocks in DNS. Any other
/// scheme, userinfo, or non-loopback address is `None`. The path is ignored;
/// the worker always posts to `/event`.
pub fn parse_loopback_http(url: &str) -> Option<SocketAddr> {
    let rest = url.trim().strip_prefix("http://")?;
    if rest.contains('@') {
        return None;
    }
    let hostport = rest.split(['/', '?', '#']).next()?.trim();
    let (host, port) = if let Some(bracketed) = hostport.strip_prefix('[') {
        bracketed.split_once("]:")?
    } else {
        let (host, port) = hostport.rsplit_once(':')?;
        if host.is_empty() || host.contains(':') {
            return None;
        }
        (host, port)
    };
    if host.is_empty() || port.is_empty() {
        return None;
    }
    let port: u16 = port.parse().ok()?;
    let ip: IpAddr = host.parse().ok()?;
    ip.is_loopback().then_some(SocketAddr::new(ip, port))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteDrop {
    /// Bounded queue is full. The frame host keeps going.
    QueueFull,
    /// The worker has gone.
    Closed,
}

/// Where a formatted line goes after the worker has stored it in the RAM ring.
pub trait NerveSink: Send + 'static {
    fn accept(&mut self, line: &str, post_body: &str);
}

/// Counts. All monotonic except where a constructor sets `forward_refused`
/// or `log_skipped` once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NerveStats {
    pub queued: u64,
    pub dropped: u64,
    pub published: u64,
    pub forwarded: u64,
    pub forward_failed: u64,
    /// A URL was set and it was not numeric loopback HTTP. Nothing was contacted.
    pub forward_refused: u64,
    /// `GALION_NERVE_LOG` was set and the file could not be opened.
    pub log_skipped: u64,
}

fn fresh_counters() -> Arc<Counters> {
    Arc::new(Counters {
        queued: AtomicU64::new(0),
        dropped: AtomicU64::new(0),
        published: AtomicU64::new(0),
        forwarded: AtomicU64::new(0),
        forward_failed: AtomicU64::new(0),
        forward_refused: AtomicU64::new(0),
        log_skipped: AtomicU64::new(0),
    })
}

struct Counters {
    queued: AtomicU64,
    dropped: AtomicU64,
    published: AtomicU64,
    forwarded: AtomicU64,
    forward_failed: AtomicU64,
    forward_refused: AtomicU64,
    log_skipped: AtomicU64,
}

impl Counters {
    fn snapshot(&self) -> NerveStats {
        NerveStats {
            queued: self.queued.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            published: self.published.load(Ordering::Relaxed),
            forwarded: self.forwarded.load(Ordering::Relaxed),
            forward_failed: self.forward_failed.load(Ordering::Relaxed),
            forward_refused: self.forward_refused.load(Ordering::Relaxed),
            log_skipped: self.log_skipped.load(Ordering::Relaxed),
        }
    }
}

/// How [`NerveEmitter::spawn`] delivers lines. `note` does not read this.
#[derive(Debug, Clone)]
pub struct NerveConfig {
    pub queue: usize,
    pub line_cap: usize,
    pub forward: Option<SocketAddr>,
    pub log_path: Option<PathBuf>,
}

impl Default for NerveConfig {
    fn default() -> Self {
        Self {
            queue: 32,
            line_cap: 64,
            forward: None,
            log_path: None,
        }
    }
}

struct Delivery {
    forward: Option<SocketAddr>,
    log: Option<std::fs::File>,
    counters: Arc<Counters>,
    down_until: Option<Instant>,
}

impl NerveSink for Delivery {
    fn accept(&mut self, line: &str, post_body: &str) {
        if let Some(f) = self.log.as_mut() {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
        if let Some(addr) = self.forward {
            self.post(addr, post_body);
        }
    }
}

impl Delivery {
    fn post(&mut self, addr: SocketAddr, body: &str) {
        if self.down_until.is_some_and(|t| Instant::now() < t) {
            self.counters.forward_failed.fetch_add(1, Ordering::Relaxed);
            return;
        }
        match post_event(addr, body) {
            Ok(code) if (200..300).contains(&code) => {
                self.down_until = None;
                self.counters.forwarded.fetch_add(1, Ordering::Relaxed);
            }
            _ => {
                self.down_until = Some(Instant::now() + Duration::from_secs(2));
                self.counters.forward_failed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Local NDJSON ring plus an optional loopback forward and an optional file.
///
/// Drop joins the worker. That can wait out a slow post, so drop it when the
/// session ends, not from the frame tick.
pub struct NerveEmitter {
    tx: Option<SyncSender<VoiceNote>>,
    counters: Arc<Counters>,
    lines: Arc<Mutex<VecDeque<String>>>,
    worker: Option<JoinHandle<()>>,
}

impl NerveEmitter {
    /// RAM ring only. No socket, no file, no environment.
    pub fn local(queue: usize) -> Self {
        Self::spawn(NerveConfig {
            queue,
            ..NerveConfig::default()
        })
    }

    /// Build from the environment. Unset URL means local only.
    ///
    /// * `GALION_NERVE_URL`, else `NERVE_URL`: numeric loopback `http://ip:port`.
    ///   Anything else is ignored (`forward_refused`) and is not contacted.
    /// * `GALION_NERVE_LOG`: append NDJSON here. A path that cannot be opened
    ///   leaves the lines in RAM (`log_skipped`).
    pub fn from_env() -> Self {
        let raw = std::env::var("GALION_NERVE_URL")
            .or_else(|_| std::env::var("NERVE_URL"))
            .ok();
        let forward = raw.as_deref().and_then(parse_loopback_http);
        let refused = raw.is_some() && forward.is_none();
        let log_path = std::env::var("GALION_NERVE_LOG")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let em = Self::spawn(NerveConfig {
            forward,
            log_path,
            ..NerveConfig::default()
        });
        if refused {
            em.counters.forward_refused.store(1, Ordering::Relaxed);
        }
        em
    }

    pub fn spawn(cfg: NerveConfig) -> Self {
        let counters = fresh_counters();
        let log = cfg.log_path.as_deref().and_then(|p| open_log(p, &counters));
        let sink = Delivery {
            forward: cfg.forward,
            log,
            counters: counters.clone(),
            down_until: None,
        };
        Self::start(cfg.queue, cfg.line_cap, counters, sink)
    }

    /// Same queue as [`Self::spawn`], with a caller-supplied sink. The RAM
    /// ring is still filled before `sink.accept` runs.
    pub fn spawn_sink<S: NerveSink>(queue: usize, line_cap: usize, sink: S) -> Self {
        Self::start(queue, line_cap, fresh_counters(), sink)
    }

    fn start<S: NerveSink>(
        queue: usize,
        line_cap: usize,
        counters: Arc<Counters>,
        sink: S,
    ) -> Self {
        let line_cap = line_cap.max(1);
        let (tx, rx) = sync_channel(queue.max(1));
        let lines = Arc::new(Mutex::new(VecDeque::new()));
        let lines_w = lines.clone();
        let published = counters.clone();
        let worker = std::thread::Builder::new()
            .name("voice-nerve".into())
            .spawn(move || worker(rx, lines_w, line_cap, published, sink))
            .expect("spawn voice-nerve");
        Self {
            tx: Some(tx),
            counters,
            lines,
            worker: Some(worker),
        }
    }

    /// Never blocks. Does not allocate the JSON line (the worker does).
    pub fn note(&self, note: VoiceNote) -> Result<(), NoteDrop> {
        let tx = self.tx.as_ref().ok_or(NoteDrop::Closed)?;
        match tx.try_send(note) {
            Ok(()) => {
                self.counters.queued.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(TrySendError::Full(_)) => {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                Err(NoteDrop::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => Err(NoteDrop::Closed),
        }
    }

    pub fn stats(&self) -> NerveStats {
        self.counters.snapshot()
    }

    /// Oldest first. Allocates. Not for the frame tick.
    pub fn lines(&self) -> Vec<String> {
        self.lines
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }
}

impl Drop for NerveEmitter {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

fn worker<S: NerveSink>(
    rx: Receiver<VoiceNote>,
    lines: Arc<Mutex<VecDeque<String>>>,
    line_cap: usize,
    counters: Arc<Counters>,
    mut sink: S,
) {
    let mut seq = 0u64;
    for note in rx {
        seq = seq.saturating_add(1);
        let unix_s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let rendered = render_event(note, seq, unix_s);
        {
            let mut g = lines.lock().unwrap_or_else(|e| e.into_inner());
            if g.len() >= line_cap {
                g.pop_front();
            }
            g.push_back(rendered.line.clone());
        }
        counters.published.fetch_add(1, Ordering::Relaxed);
        sink.accept(&rendered.line, &rendered.post_body);
    }
}

fn open_log(path: &Path, counters: &Counters) -> Option<std::fs::File> {
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(f) => Some(f),
        Err(_) => {
            counters.log_skipped.store(1, Ordering::Relaxed);
            None
        }
    }
}

fn post_event(addr: SocketAddr, body: &str) -> Result<u16, ()> {
    let mut sock = TcpStream::connect_timeout(&addr, Duration::from_millis(300)).map_err(|_| ())?;
    sock.set_read_timeout(Some(Duration::from_millis(300)))
        .map_err(|_| ())?;
    sock.set_write_timeout(Some(Duration::from_millis(300)))
        .map_err(|_| ())?;
    let req = format!(
        "POST /event HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    sock.write_all(req.as_bytes()).map_err(|_| ())?;
    let mut buf = [0u8; 256];
    let n = sock.read(&mut buf).unwrap_or(0);
    Ok(status_of(&buf[..n]))
}

fn status_of(head: &[u8]) -> u16 {
    let s = String::from_utf8_lossy(head);
    let line = s.lines().next().unwrap_or("");
    if !line.starts_with("HTTP/") {
        return 0;
    }
    line.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}

fn rfc3339_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    let h = tod / 3600;
    let min = (tod % 3600) / 60;
    let s = tod % 60;
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}Z")
}

/// Howard Hinnant, `civil_from_days`, public domain algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_epoch_and_a_known_day() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(86_400), "1970-01-02T00:00:00Z");
        assert_eq!(rfc3339_utc(1_609_459_200), "2021-01-01T00:00:00Z");
    }

    #[test]
    fn loopback_parser_refuses_the_world() {
        assert_eq!(
            parse_loopback_http("http://127.0.0.1:8670"),
            Some(SocketAddr::from(([127, 0, 0, 1], 8670)))
        );
        assert_eq!(
            parse_loopback_http("  http://[::1]:8670/ignored "),
            Some(SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, 8670)))
        );
        assert!(parse_loopback_http("http://10.1.2.3:8670").is_none());
        assert!(parse_loopback_http("http://0.0.0.0:8670").is_none());
        assert!(parse_loopback_http("https://127.0.0.1:8670").is_none());
        assert!(parse_loopback_http("http://localhost:8670").is_none());
        assert!(parse_loopback_http("http://user@127.0.0.1:8670").is_none());
        assert!(parse_loopback_http("http://127.0.0.1").is_none());
        assert!(parse_loopback_http("").is_none());
    }

    #[test]
    fn kinds_match_the_slot_contract() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../galion-student/configs/openmind-slot.toml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert!(
            text.contains("schema = \"galion.slot.v2\""),
            "slot schema moved"
        );
        for k in VOICE_KINDS {
            assert!(text.contains(&format!("\"{k}\"")), "{k} missing from slot");
        }
        assert_eq!(VoiceKind::FrameMiss.as_str(), VOICE_KINDS[0]);
        assert_eq!(VoiceKind::Fallback.as_str(), VOICE_KINDS[1]);
        assert_eq!(VoiceKind::BackendUp.as_str(), VOICE_KINDS[2]);
    }

    #[test]
    fn frame_miss_line_is_schema_v1() {
        let ev = render_event(VoiceNote::frame_miss(7), 1, 0);
        assert_eq!(
            ev.line,
            concat!(
                r#"{"v":1,"seq":1,"ts":"1970-01-01T00:00:00Z","t":0,"source":"galion-voice","#,
                r#""kind":"voice.frame_miss","severity":"medium","unit":"galion-os-voice","group":"galion-os","#,
                r#""key":"voice.frame_miss:frame:7:turn:0:code:0","msg":"voice frame missed","#,
                r#""data":{"frame":7,"turn":0,"code":0,"unit":"galion-os-voice","group":"galion-os","msg":"voice frame missed"},"count":1}"#
            )
        );
        assert_eq!(
            ev.post_body,
            concat!(
                r#"{"source":"galion-voice","kind":"voice.frame_miss","severity":"medium","#,
                r#""data":{"frame":7,"turn":0,"code":0,"unit":"galion-os-voice","group":"galion-os","msg":"voice frame missed"}}"#
            )
        );
        assert!(!ev.post_body.contains("\"seq\""));
    }

    #[test]
    fn fallback_is_hermes_and_backend_up_is_galion_os() {
        let fb = render_event(
            VoiceNote::fallback(3, 9, VoiceBackendSlot::Relay, VoiceBackendSlot::Student),
            2,
            1,
        );
        assert!(fb.line.contains("\"kind\":\"voice.fallback\""));
        assert!(fb.line.contains("\"group\":\"hermes\""));
        assert!(fb.line.contains("\"unit\":\"hermes-sideband\""));
        assert!(fb.line.contains("\"from\":\"relay\""));
        assert!(fb.line.contains("\"to\":\"student\""));
        assert!(fb.line.contains("\"code\":772"));
        assert!(fb.line.contains("\"severity\":\"high\""));
        let up = render_event(VoiceNote::backend_up(1, VoiceBackendSlot::Student), 3, 2);
        assert!(up.line.contains("\"kind\":\"voice.backend_up\""));
        assert!(up.line.contains("\"group\":\"galion-os\""));
        assert!(up.line.contains("\"backend\":\"student\""));
        assert!(up.line.contains("\"severity\":\"info\""));
        assert!(up.line.contains("\"code\":4"));
    }
}
