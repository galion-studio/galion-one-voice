// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback supervisor for the configured persona's local companion.
//!
//! This crate does not implement HORBM. Mood stays at the resting pad
//! `{arousal: 0.3, valence: 0.25, dominance: 0.5}` until a reply engine is
//! linked. The 80 ms tick matches `galion-heartbeat`'s `FRAME_MS`. A late
//! tick does not catch up.

use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const TICK_MS: u64 = 80;
/// Companion socket. 7879 is `GALION_HARD_ADDR` and stays free.
pub const DEFAULT_BIND: &str = "127.0.0.1:7881";

/// Resting pad from the HORBM contract. Not a live mood.
pub const RESTING_AROUSAL: f32 = 0.3;
pub const RESTING_VALENCE: f32 = 0.25;
pub const RESTING_DOMINANCE: f32 = 0.5;

const MAX_REQUEST: usize = 4096;
const BREATH_FRAMES: u64 = 50;

/// One disc. Background and fill are the locked one-dot colours.
const PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title></title>
<style>
html,body{margin:0;height:100%;background:#0A0A0C;overflow:hidden}
.dot{position:absolute;left:50%;top:50%;width:18vh;height:18vh;margin:0;border-radius:50%;background:#F5F5F7;transform:translate(-50%,-50%)}
</style></head><body>
<div class="dot" id="d"></div>
<script>
const d=document.getElementById("d");
async function tick(){
  try{
    const r=await fetch("/dot",{cache:"no-store"});
    const j=await r.json();
    const level=typeof j.level==="number"?j.level:0.4;
    const size=16+level*8;
    d.style.width=size+"vh";
    d.style.height=size+"vh";
    d.style.opacity=String(0.7+level*0.3);
  }catch(e){
    d.style.opacity="0.35";
    d.style.width="12vh";
    d.style.height="12vh";
  }
}
setInterval(tick,80);
tick();
</script>
</body></html>
"#;

#[derive(Clone, Debug)]
pub struct Paths {
    pub home: PathBuf,
}

impl Paths {
    pub fn from_home(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn vosk_model(&self) -> PathBuf {
        self.home.join("models/asr/vosk-model-small-en-us-0.15")
    }

    pub fn libvosk(&self) -> PathBuf {
        self.home.join("models/asr/libvosk/libvosk.so")
    }

    pub fn piper_voice(&self) -> PathBuf {
        self.home.join("models/tts/en_US-ljspeech-medium.onnx")
    }

    pub fn piper_voice_json(&self) -> PathBuf {
        self.home.join("models/tts/en_US-ljspeech-medium.onnx.json")
    }

    pub fn piper_bin(&self) -> PathBuf {
        self.home.join("models/runtime/piper/piper")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Absent,
    Present,
}

impl Presence {
    fn as_str(self) -> &'static str {
        match self {
            Presence::Absent => "absent",
            Presence::Present => "present",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelReport {
    pub vosk: Presence,
    pub libvosk: Presence,
    pub piper_voice: Presence,
    pub piper_runtime: Presence,
}

impl ModelReport {
    pub fn scan(paths: &Paths) -> Self {
        let model = paths.vosk_model();
        let vosk =
            if model.join("am/final.mdl").is_file() && model.join("conf/model.conf").is_file() {
                Presence::Present
            } else {
                Presence::Absent
            };
        let libvosk = if paths.libvosk().is_file() {
            Presence::Present
        } else {
            Presence::Absent
        };
        let piper_voice = if paths.piper_voice().is_file() && paths.piper_voice_json().is_file() {
            Presence::Present
        } else {
            Presence::Absent
        };
        let piper_runtime = if is_executable_file(&paths.piper_bin()) {
            Presence::Present
        } else {
            Presence::Absent
        };
        Self {
            vosk,
            libvosk,
            piper_voice,
            piper_runtime,
        }
    }

    pub fn public_models_ready(&self) -> bool {
        self.vosk == Presence::Present
            && self.libvosk == Presence::Present
            && self.piper_voice == Presence::Present
            && self.piper_runtime == Presence::Present
    }
}

/// `unset`, `set`, `missing`, or `not_a_file`. The path is not returned.
pub fn reference_state(value: Option<&str>) -> &'static str {
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        None => "unset",
        Some(path) => match fs::metadata(path) {
            Ok(meta) if meta.is_file() => "set",
            Ok(_) => "not_a_file",
            Err(_) => "missing",
        },
    }
}

/// Why a deprecated name was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliasWarning {
    /// The new name is missing. The value came from an older name.
    Fallback,
    /// Both names are set and their values differ. The new value is kept.
    Conflict,
}

/// The value to use, and at most one warning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub value: Option<String>,
    pub warning: Option<AliasWarning>,
}

/// Pure alias. `new` wins. `old` is the fallback. Empty strings count as set.
pub fn resolve(new: Option<&str>, old: Option<&str>) -> Resolution {
    match (new, old) {
        (None, None) => Resolution {
            value: None,
            warning: None,
        },
        (Some(value), None) => Resolution {
            value: Some(value.to_string()),
            warning: None,
        },
        (None, Some(value)) => Resolution {
            value: Some(value.to_string()),
            warning: Some(AliasWarning::Fallback),
        },
        (Some(value), Some(previous)) if value == previous => Resolution {
            value: Some(value.to_string()),
            warning: None,
        },
        (Some(value), Some(_)) => Resolution {
            value: Some(value.to_string()),
            warning: Some(AliasWarning::Conflict),
        },
    }
}

/// One line. Names the new variable.
pub fn warning_text(new_name: &str, old_name: &str, warning: AliasWarning) -> String {
    match warning {
        AliasWarning::Fallback => format!("{old_name} is deprecated; set {new_name}"),
        AliasWarning::Conflict => {
            format!("{old_name} is deprecated; {new_name} and {old_name} differ; using {new_name}")
        }
    }
}

/// `GALION_VOICE_WS` wins, then `GALION_SAMANTHA_WS`, then `GALION_SAMANTHA_BIND`.
/// Blank values are ignored. The fallback is [`DEFAULT_BIND`].
pub fn resolve_bind(voice: Option<&str>, ws: Option<&str>, bind: Option<&str>) -> String {
    let (value, _) = resolve_bind_choice(voice, ws, bind);
    value
}

/// Same choice as [`resolve_bind`], plus at most one warning.
pub fn resolve_bind_choice(
    voice: Option<&str>,
    ws: Option<&str>,
    bind: Option<&str>,
) -> (String, Option<AliasWarning>) {
    let voice = blank_as_none(voice);
    let ws = blank_as_none(ws);
    let bind = blank_as_none(bind);
    let first = resolve(voice, ws);
    if let Some(value) = first.value {
        return (value, first.warning);
    }
    let oldest = resolve(None, bind);
    (
        oldest
            .value
            .unwrap_or_else(|| DEFAULT_BIND.to_string()),
        oldest.warning,
    )
}

fn blank_as_none(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

/// Print `line` once per `key`.
pub fn warn_once(key: &str, line: &str) {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = seen.lock().unwrap_or_else(|err| err.into_inner());
    if guard.insert(key.to_string()) {
        eprintln!("{line}");
    }
}

pub fn parse_bind(raw: &str) -> Result<SocketAddr, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("bind address is empty".into());
    }
    if raw.contains("://") {
        return Err("bind address must be host:port, not a URL".into());
    }
    let addr: SocketAddr = raw
        .parse()
        .map_err(|_| format!("bind address {raw} is not a numeric host:port"))?;
    if !is_loopback(addr.ip()) {
        return Err(format!(
            "bind address {raw} is not loopback; the configured persona stays on 127.0.0.1 or ::1"
        ));
    }
    Ok(addr)
}

fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Slow idle breath from the tick counter. Not an emotion and not a line.
pub fn idle_level(ticks: u64) -> f32 {
    let phase = (ticks % BREATH_FRAMES) as f32 / BREATH_FRAMES as f32;
    let wave = (phase * std::f32::consts::TAU).sin();
    0.35 + 0.15 * (wave * 0.5 + 0.5)
}

#[derive(Debug)]
pub struct TickClock {
    ticks: AtomicU64,
    late: AtomicU64,
    last_interval_us: AtomicU64,
}

impl TickClock {
    pub fn spawn() -> (Arc<Self>, JoinHandle<()>) {
        let clock = Arc::new(Self {
            ticks: AtomicU64::new(0),
            late: AtomicU64::new(0),
            last_interval_us: AtomicU64::new(0),
        });
        let worker = Arc::clone(&clock);
        let handle = thread::Builder::new()
            .name("samantha-tick".into())
            .spawn(move || worker.run())
            .expect("samantha tick thread");
        (clock, handle)
    }

    fn run(&self) {
        let step = Duration::from_millis(TICK_MS);
        let mut next = Instant::now() + step;
        let mut prev = Instant::now();
        loop {
            let now = Instant::now();
            if now < next {
                thread::sleep(next - now);
            }
            let woke = Instant::now();
            let interval = woke.saturating_duration_since(prev);
            prev = woke;
            self.last_interval_us
                .store(duration_us(interval), Ordering::Relaxed);
            if woke > next + Duration::from_millis(20) {
                self.late.fetch_add(1, Ordering::Relaxed);
                next = woke + step;
            } else {
                next += step;
            }
            self.ticks.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn ticks(&self) -> u64 {
        self.ticks.load(Ordering::Relaxed)
    }

    pub fn late(&self) -> u64 {
        self.late.load(Ordering::Relaxed)
    }

    pub fn last_interval_us(&self) -> u64 {
        self.last_interval_us.load(Ordering::Relaxed)
    }
}

fn duration_us(d: Duration) -> u64 {
    d.as_micros().min(u128::from(u64::MAX)) as u64
}

pub struct Running {
    pub addr: SocketAddr,
    pub clock: Arc<TickClock>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Running {
    pub fn spawn(bind: SocketAddr, home: PathBuf) -> std::io::Result<Self> {
        let listener = TcpListener::bind(bind)?;
        let addr = listener.local_addr()?;
        let (clock, tick_join) = TickClock::spawn();
        let clock_for_loop = Arc::clone(&clock);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_loop = Arc::clone(&stop);
        let join = thread::Builder::new()
            .name("samantha-http".into())
            .spawn(move || {
                let _ = tick_join;
                accept_loop(listener, home, clock_for_loop, Some(stop_for_loop));
            })
            .expect("samantha http thread");
        Ok(Self {
            addr,
            clock,
            stop,
            join: Some(join),
        })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

pub fn serve_blocking(bind: SocketAddr, home: PathBuf) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind)?;
    let addr = listener.local_addr()?;
    let (clock, tick_join) = TickClock::spawn();
    eprintln!("galion-samantha listening on {addr} tick_ms={TICK_MS} horbm=unlinked");
    let _ = tick_join;
    accept_loop(listener, home, clock, None);
    Ok(())
}

fn accept_loop(
    listener: TcpListener,
    home: PathBuf,
    clock: Arc<TickClock>,
    stop: Option<Arc<AtomicBool>>,
) {
    if stop.is_some() {
        let _ = listener.set_nonblocking(true);
    }
    loop {
        if stop
            .as_ref()
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            break;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let _ = handle_client(&mut stream, &home, &clock);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => {
                eprintln!("galion-samantha accept: {err}");
                break;
            }
        }
    }
}

fn is_executable_file(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => {
            #[cfg(unix)]
            {
                meta.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        }
        _ => false,
    }
}

fn handle_client(stream: &mut TcpStream, home: &Path, clock: &TickClock) -> std::io::Result<()> {
    let mut buf = [0_u8; MAX_REQUEST];
    let mut filled = 0_usize;
    let header_end = loop {
        if filled == buf.len() {
            return write_response(
                stream,
                400,
                "text/plain; charset=utf-8",
                b"request too large",
            );
        }
        match stream.read(&mut buf[filled..]) {
            Ok(0) => break None,
            Ok(n) => {
                filled += n;
                if let Some(pos) = find_header_end(&buf[..filled]) {
                    break Some(pos);
                }
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut =>
            {
                return write_response(stream, 408, "text/plain; charset=utf-8", b"timed out");
            }
            Err(err) => return Err(err),
        }
    };
    let Some(end) = header_end else {
        return write_response(stream, 400, "text/plain; charset=utf-8", b"empty request");
    };
    let head = match std::str::from_utf8(&buf[..end]) {
        Ok(text) => text,
        Err(_) => {
            return write_response(
                stream,
                400,
                "text/plain; charset=utf-8",
                b"request is not utf-8",
            );
        }
    };
    let mut lines = head.split("\r\n");
    let Some(request) = lines.next() else {
        return write_response(stream, 400, "text/plain; charset=utf-8", b"empty request");
    };
    let mut parts = request.split(' ');
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    if method != "GET" && !(method == "POST" && path == "/turn") {
        return write_response(
            stream,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed",
        );
    }
    match (method, path) {
        ("GET", "/") => write_response(stream, 200, "text/html; charset=utf-8", PAGE.as_bytes()),
        ("GET", "/health") => {
            let body = health_json(home, clock, reference_from_env());
            write_response(stream, 200, "application/json", body.as_bytes())
        }
        ("GET", "/dot") => {
            let body = dot_json(clock);
            write_response(stream, 200, "application/json", body.as_bytes())
        }
        ("POST", "/turn") => {
            let body = r#"{"error":"reply_engine_unlinked","horbm":"unlinked"}"#;
            write_response(stream, 501, "application/json", body.as_bytes())
        }
        _ => write_response(stream, 404, "text/plain; charset=utf-8", b"not found"),
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn reference_from_env() -> &'static str {
    let voice = std::env::var("GALION_VOICE_REF").ok();
    let old = std::env::var("GALION_SAMANTHA_REF").ok();
    let resolved = resolve(voice.as_deref(), old.as_deref());
    if let Some(warning) = resolved.warning {
        warn_once(
            "GALION_VOICE_REF",
            &warning_text("GALION_VOICE_REF", "GALION_SAMANTHA_REF", warning),
        );
    }
    match resolved.value.as_deref() {
        None => "unset",
        Some(value) => reference_state(Some(value)),
    }
}

pub fn health_json(home: &Path, clock: &TickClock, reference: &str) -> String {
    let report = ModelReport::scan(&Paths::from_home(home));
    format!(
        concat!(
            "{{\"service\":\"galion-samantha\",\"version\":\"{version}\",",
            "\"ok\":true,\"tick_ms\":{tick},\"ticks\":{ticks},\"late_ticks\":{late},",
            "\"last_interval_us\":{interval},\"cpu_only\":true,",
            "\"horbm\":\"unlinked\",\"reply_engine\":\"unlinked\",",
            "\"mood_source\":\"resting\",",
            "\"mood\":{{\"arousal\":0.3,\"valence\":0.25,\"dominance\":0.5}},",
            "\"reference\":\"{reference}\",",
            "\"public_models_ready\":{ready},",
            "\"models\":{{\"vosk\":\"{vosk}\",\"libvosk\":\"{lib}\",",
            "\"piper_voice\":\"{voice}\",\"piper_runtime\":\"{runtime}\"}}}}"
        ),
        version = VERSION,
        tick = TICK_MS,
        ticks = clock.ticks(),
        late = clock.late(),
        interval = clock.last_interval_us(),
        reference = json_escape_token(reference),
        ready = if report.public_models_ready() {
            "true"
        } else {
            "false"
        },
        vosk = report.vosk.as_str(),
        lib = report.libvosk.as_str(),
        voice = report.piper_voice.as_str(),
        runtime = report.piper_runtime.as_str(),
    )
}

pub fn dot_json(clock: &TickClock) -> String {
    let ticks = clock.ticks();
    format!(
        "{{\"mode\":\"idle\",\"level\":{level:.3},\"tick_ms\":{tick},\"ticks\":{ticks},\"mood_source\":\"resting\",\"mood\":{{\"arousal\":0.3,\"valence\":0.25,\"dominance\":0.5}},\"horbm\":\"unlinked\"}}",
        level = idle_level(ticks),
        tick = TICK_MS,
        ticks = ticks,
    )
}

fn json_escape_token(value: &str) -> String {
    // Reference state is one of a fixed set. Anything else is dropped.
    match value {
        "unset" | "set" | "missing" | "not_a_file" => value.to_string(),
        _ => "unset".to_string(),
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        501 => "Not Implemented",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

pub fn default_home() -> Result<PathBuf, String> {
    let voice = std::env::var("GALION_VOICE_HOME").ok();
    let old = std::env::var("GALION_SAMANTHA_HOME").ok();
    let resolved = resolve(voice.as_deref(), old.as_deref());
    if let Some(warning) = resolved.warning {
        warn_once(
            "GALION_VOICE_HOME",
            &warning_text("GALION_VOICE_HOME", "GALION_SAMANTHA_HOME", warning),
        );
    }
    if let Some(home) = resolved.value.filter(|home| !home.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    let home = std::env::var("HOME").map_err(|_| "HOME is unset".to_string())?;
    Ok(PathBuf::from(home).join(".local/share/galion/samantha"))
}

pub fn page_html() -> &'static str {
    PAGE
}

/// Used by tests to be sure the default bind stays numeric loopback.
pub fn default_bind_addr() -> SocketAddr {
    parse_bind(DEFAULT_BIND).expect("default bind")
}

pub fn ipv4_loopback(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::time::Duration;

    #[test]
    fn resting_pad_is_the_horbm_resting_value() {
        assert!((RESTING_AROUSAL - 0.3).abs() < f32::EPSILON);
        assert!((RESTING_VALENCE - 0.25).abs() < f32::EPSILON);
        assert!((RESTING_DOMINANCE - 0.5).abs() < f32::EPSILON);
        assert_eq!(TICK_MS, 80);
    }

    #[test]
    fn bind_rejects_public_and_unspecified() {
        assert!(parse_bind("127.0.0.1:7881").is_ok());
        assert!(parse_bind("[::1]:7881").is_ok());
        assert!(parse_bind("127.0.0.1:0").is_ok());
        assert!(parse_bind("0.0.0.0:7881").is_err());
        assert!(parse_bind("[::]:7881").is_err());
        assert!(parse_bind("8.8.8.8:80").is_err());
        assert!(parse_bind("192.168.1.10:7881").is_err());
        assert!(parse_bind("localhost:7881").is_err());
        assert!(parse_bind("http://127.0.0.1:7881").is_err());
    }

    #[test]
    fn default_bind_leaves_the_hard_loop_port_free() {
        assert_eq!(DEFAULT_BIND, "127.0.0.1:7881");
        assert!(!DEFAULT_BIND.contains("7879"));
        assert_eq!(
            resolve_bind(Some("127.0.0.1:7882"), Some("127.0.0.1:9"), None),
            "127.0.0.1:7882"
        );
        assert_eq!(
            resolve_bind(Some("  "), Some("127.0.0.1:9"), None),
            "127.0.0.1:9"
        );
        assert_eq!(
            resolve_bind(None, None, Some("127.0.0.1:9")),
            "127.0.0.1:9"
        );
        assert_eq!(resolve_bind(None, None, None), DEFAULT_BIND);
    }

    #[test]
    fn new_wins() {
        let only_new = resolve(Some("voice"), None);
        assert_eq!(only_new.value.as_deref(), Some("voice"));
        assert!(only_new.warning.is_none());
        let same = resolve(Some("voice"), Some("voice"));
        assert_eq!(same.value.as_deref(), Some("voice"));
        assert!(same.warning.is_none());
    }

    #[test]
    fn old_is_fallback_and_warns_once() {
        let choice = resolve(None, Some("old"));
        assert_eq!(choice.value.as_deref(), Some("old"));
        assert_eq!(choice.warning, Some(AliasWarning::Fallback));
        let line = warning_text("GALION_VOICE_WS", "GALION_SAMANTHA_WS", AliasWarning::Fallback);
        assert_eq!(line.lines().count(), 1);
        assert!(line.contains("GALION_VOICE_WS"));
    }

    #[test]
    fn none_is_none() {
        let choice = resolve(None, None);
        assert!(choice.value.is_none());
        assert!(choice.warning.is_none());
    }

    #[test]
    fn page_is_one_dot_without_chrome() {
        let page = page_html();
        assert!(page.contains("#F5F5F7"));
        assert!(page.contains("#0A0A0C"));
        assert!(!page.to_ascii_lowercase().contains("<button"));
        assert!(!page.contains("whisper"));
        assert_eq!(page.matches("class=\"dot\"").count(), 1);
    }

    #[test]
    fn health_reports_unlinked_brain_and_hides_reference_path() {
        let dir = std::env::temp_dir().join(format!("samantha-health-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (clock, handle) = TickClock::spawn();
        let body = health_json(&dir, &clock, "set");
        assert!(body.contains("\"horbm\":\"unlinked\""));
        assert!(body.contains("\"reply_engine\":\"unlinked\""));
        assert!(body.contains("\"arousal\":0.3"));
        assert!(body.contains("\"valence\":0.25"));
        assert!(body.contains("\"dominance\":0.5"));
        assert!(body.contains("\"reference\":\"set\""));
        assert!(body.contains("\"public_models_ready\":false"));
        assert!(!body.contains(&dir.display().to_string()));
        drop(handle);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn model_scan_sees_required_files_only() {
        let dir = std::env::temp_dir().join(format!("samantha-models-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let paths = Paths::from_home(&dir);
        fs::create_dir_all(paths.vosk_model().join("am")).unwrap();
        fs::create_dir_all(paths.vosk_model().join("conf")).unwrap();
        fs::write(paths.vosk_model().join("am/final.mdl"), b"x").unwrap();
        fs::write(paths.vosk_model().join("conf/model.conf"), b"x").unwrap();
        fs::create_dir_all(paths.libvosk().parent().unwrap()).unwrap();
        fs::write(paths.libvosk(), b"x").unwrap();
        fs::create_dir_all(paths.piper_voice().parent().unwrap()).unwrap();
        fs::write(paths.piper_voice(), b"x").unwrap();
        fs::write(paths.piper_voice_json(), b"{}").unwrap();
        fs::create_dir_all(paths.piper_bin().parent().unwrap()).unwrap();
        fs::write(paths.piper_bin(), b"x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(paths.piper_bin()).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(paths.piper_bin(), perms).unwrap();
        }
        let report = ModelReport::scan(&paths);
        assert!(report.public_models_ready());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reference_state_does_not_read_file_bytes() {
        let path = std::env::temp_dir().join(format!("samantha-ref-{}", std::process::id()));
        fs::write(&path, b"not-audio").unwrap();
        assert_eq!(reference_state(None), "unset");
        assert_eq!(reference_state(Some("")), "unset");
        assert_eq!(reference_state(Some(path.to_str().unwrap())), "set");
        assert_eq!(reference_state(Some("/no/such/samantha-ref")), "missing");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn http_serves_dot_health_and_refuses_a_reply() {
        let dir = std::env::temp_dir().join(format!("samantha-http-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let running = Running::spawn(ipv4_loopback(0), dir.clone()).unwrap();
        let health = http_get(&running.addr, "GET", "/health");
        assert!(health.status == 200, "{}", health.status);
        assert!(health.body.contains("\"tick_ms\":80"));
        assert!(health.body.contains("horbm\":\"unlinked"));
        thread::sleep(Duration::from_millis(450));
        let dot = http_get(&running.addr, "GET", "/dot");
        assert_eq!(dot.status, 200);
        assert!(dot.body.contains("\"mode\":\"idle\""));
        let ticks: u64 = json_number(&dot.body, "ticks");
        assert!(ticks >= 3, "ticks={ticks} body={}", dot.body);
        let page = http_get(&running.addr, "GET", "/");
        assert_eq!(page.status, 200);
        assert!(page.body.contains("#F5F5F7"));
        let turn = http_get(&running.addr, "POST", "/turn");
        assert_eq!(turn.status, 501);
        assert!(turn.body.contains("reply_engine_unlinked"));
        assert!(!turn.body.to_ascii_lowercase().contains("hello"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tick_interval_is_near_eighty_milliseconds() {
        let (clock, handle) = TickClock::spawn();
        thread::sleep(Duration::from_millis(500));
        let ticks = clock.ticks();
        let interval = clock.last_interval_us();
        assert!(ticks >= 4, "ticks={ticks}");
        assert!(
            (50_000..150_000).contains(&interval),
            "last_interval_us={interval} ticks={ticks} late={}",
            clock.late()
        );
        drop(handle);
    }

    struct Resp {
        status: u16,
        body: String,
    }

    fn http_get(addr: &SocketAddr, method: &str, path: &str) -> Resp {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let req =
            format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        stream.write_all(req.as_bytes()).unwrap();
        let mut raw = Vec::new();
        let _ = stream.read_to_end(&mut raw);
        let text = String::from_utf8_lossy(&raw);
        let status = text
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        Resp { status, body }
    }

    fn json_number(body: &str, key: &str) -> u64 {
        let pat = format!("\"{key}\":");
        let rest = body.split(&pat).nth(1).unwrap_or("0");
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().unwrap_or(0)
    }
}
