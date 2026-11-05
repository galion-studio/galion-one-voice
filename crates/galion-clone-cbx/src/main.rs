// Galion Voice — Perpetual Open MIT License
//! `galion-clone-cbx`: the separate cloning worker (owner t652u item 6).
//!
//! Chatterbox Turbo ONNX, **fp32** (the t648 bake-off winner; the quantized
//! graph fails to load on cbx's bundled ONNX Runtime), driven through the
//! `cbx` Rust CLI. Its own process and systemd unit, never inside the live
//! voice service. Piper stays the live TTS.
//!
//! * Loopback only: a non-loopback `GALION_CBX_ADDR` exits 2.
//! * Consent-gated at registration: `POST /register?persona=` runs
//!   `galion_consent::clone_gate` once and builds the cbx voice profile from
//!   the gated reference. `POST /speak` needs a registered persona and does no
//!   consent work per synthesis (same rule as #70).
//! * Off during live calls: any request is refused (409) while a call is live,
//!   and a running synthesis is killed when a call starts. Live means the flag
//!   file exists, or an ESTABLISHED TCP connection sits on the live voice port
//!   (`GALION_CBX_LIVE_PORT`, 7878), or the live log tail shows a non-idle dot.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DTYPE: &str = "fp32";
const DEFAULT_REVISION: &str = "d21799bd0354adb85e348b8a0442a8405110a2cf";
const MAX_TEXT: usize = 600;
const MAX_HEADER: usize = 16 * 1024;

#[derive(Clone, Debug)]
struct Config {
    addr: SocketAddr,
    cbx: PathBuf,
    revision: String,
    threads: usize,
    state: PathBuf,
    live_port: u16,
    live_log: Option<PathBuf>,
    live_flag: PathBuf,
    repo: PathBuf,
    accept_cc0: bool,
    max_synth: Duration,
}

fn env_or(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => default.to_string(),
    }
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let addr: SocketAddr = env_or("GALION_CBX_ADDR", "127.0.0.1:7880")
            .parse()
            .map_err(|e| format!("GALION_CBX_ADDR: {e}"))?;
        check_loopback(&addr)?;
        let state = PathBuf::from(env_or("GALION_CBX_STATE", "/opt/galion/voice/cbx/state"));
        let log = env_or("GALION_CBX_LIVE_LOG", "");
        Ok(Self {
            addr,
            cbx: PathBuf::from(env_or("GALION_CBX_BIN", "/opt/galion/voice/cbx/bin/cbx")),
            revision: env_or("GALION_CBX_REVISION", DEFAULT_REVISION),
            threads: env_or("GALION_CBX_THREADS", "4").parse().unwrap_or(4).clamp(1, 8),
            live_flag: PathBuf::from(env_or("GALION_CBX_LIVE_FLAG", &state.join("live-call").to_string_lossy())),
            state,
            live_port: env_or("GALION_CBX_LIVE_PORT", "7878").parse().unwrap_or(7878),
            live_log: (!log.is_empty()).then(|| PathBuf::from(log)),
            repo: PathBuf::from(env_or("GALION_REPO", &galion_consent::repo_root().to_string_lossy())),
            accept_cc0: env_or("GALION_CBX_ACCEPT_CC0", "0") == "1",
            max_synth: Duration::from_secs(env_or("GALION_CBX_MAX_SECONDS", "180").parse().unwrap_or(180)),
        })
    }
}

fn check_loopback(addr: &SocketAddr) -> Result<(), String> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err(format!("refusing non-loopback bind {}", addr.ip()))
    }
}

/// ESTABLISHED (`01`) sockets whose local port is `port`, from /proc/net/tcp text.
fn established_on(proc_tcp: &str, port: u16) -> usize {
    proc_tcp
        .lines()
        .skip(1)
        .filter(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 4 || f[3] != "01" {
                return false;
            }
            f[1].rsplit(':').next().and_then(|p| u16::from_str_radix(p, 16).ok()) == Some(port)
        })
        .count()
}

/// The live log's last `lines` summary lines show a dot that is not idle.
fn log_tail_busy(tail: &str, lines: usize) -> bool {
    tail.lines()
        .rev()
        .filter_map(|l| l.split(" dot=").nth(1))
        .take(lines)
        .any(|rest| rest.split_whitespace().next().is_some_and(|s| s != "idle"))
}

fn read_tail(path: &Path, bytes: u64) -> Option<String> {
    use std::io::{Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(bytes))).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Why cloning is off right now, or `None` when no call is live.
fn live_call(cfg: &Config) -> Option<&'static str> {
    if cfg.live_flag.exists() {
        return Some("live_call_flag");
    }
    let tcp: usize = ["/proc/net/tcp", "/proc/net/tcp6"]
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|t| established_on(&t, cfg.live_port))
        .sum();
    if tcp > 0 {
        return Some("live_call_connection");
    }
    if let Some(log) = &cfg.live_log {
        if read_tail(log, 32 * 1024).is_some_and(|t| log_tail_busy(&t, 120)) {
            return Some("live_call_dot");
        }
    }
    None
}

fn registered_path(cfg: &Config) -> PathBuf {
    cfg.state.join("registered")
}

/// `persona sha256` lines written at registration.
fn registered(cfg: &Config) -> Vec<(String, String)> {
    std::fs::read_to_string(registered_path(cfg))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?.to_string(), it.next().unwrap_or("").to_string()))
        })
        .collect()
}

fn write_registered(cfg: &Config, list: &[(String, String)]) -> std::io::Result<()> {
    let body: String = list.iter().map(|(p, s)| format!("{p} {s}\n")).collect();
    let tmp = cfg.state.join("registered.new");
    std::fs::write(&tmp, body)?;
    std::fs::rename(tmp, registered_path(cfg))
}

fn voice_name(persona: &str) -> String {
    format!("galion-{persona}")
}

struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    body: Vec<u8>,
}

fn parse_head(head: &str) -> Option<(String, String, Vec<(String, String)>, usize)> {
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let target = first.next()?;
    let (path, q) = target.split_once('?').unwrap_or((target, ""));
    let query = q
        .split('&')
        .filter(|kv| !kv.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (k.to_string(), v.to_string())
        })
        .collect();
    let len = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0);
    Some((method, path.to_string(), query, len))
}

fn read_request(s: &mut TcpStream) -> Result<Request, &'static str> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let end = loop {
        let n = s.read(&mut chunk).map_err(|_| "read")?;
        if n == 0 {
            return Err("eof");
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEADER {
            return Err("header");
        }
    };
    let head = String::from_utf8_lossy(&buf[..end]).into_owned();
    let (method, path, query, len) = parse_head(&head).ok_or("head")?;
    if len > MAX_TEXT * 4 {
        return Err("body");
    }
    let mut body = buf[end + 4..].to_vec();
    while body.len() < len {
        let n = s.read(&mut chunk).map_err(|_| "read")?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(len);
    Ok(Request { method, path, query, body })
}

fn respond(s: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(body);
}

fn json_err(s: &mut TcpStream, status: &str, reason: &str) {
    respond(s, status, "application/json", format!("{{\"ok\":false,\"reason\":\"{reason}\"}}").as_bytes());
}

/// Run cbx; kill it when a live call starts or it overruns. `Err(reason)`.
fn run_cbx(cfg: &Config, args: &[String]) -> Result<(), String> {
    let mut child = Command::new(&cfg.cbx)
        .arg("--intra-threads")
        .arg(cfg.threads.to_string())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let t0 = Instant::now();
    loop {
        if let Some(st) = child.try_wait().map_err(|e| format!("wait: {e}"))? {
            if st.success() {
                return Ok(());
            }
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            let last = err.lines().last().unwrap_or("").chars().take(200).collect::<String>();
            return Err(format!("cbx exit {:?}: {last}", st.code()));
        }
        if let Some(why) = live_call(cfg) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(why.to_string());
        }
        if t0.elapsed() > cfg.max_synth {
            let _ = child.kill();
            let _ = child.wait();
            return Err("timeout".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn persona_param(req: &Request) -> Option<&'static str> {
    req.query.iter().find(|(k, _)| k == "persona").and_then(|(_, v)| galion_consent::persona_of(v))
}

fn handle(cfg: &Config, s: &mut TcpStream) {
    let req = match read_request(s) {
        Ok(r) => r,
        Err(_) => return json_err(s, "400 Bad Request", "request"),
    };
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/health") => {
            let live = live_call(cfg);
            let reg: Vec<String> = registered(cfg).into_iter().map(|(p, _)| format!("\"{p}\"")).collect();
            let body = format!(
                "{{\"ok\":true,\"engine\":\"chatterbox-turbo-onnx\",\"runtime\":\"cbx\",\"dtype\":\"{DTYPE}\",\"revision\":\"{}\",\"threads\":{},\"cloning\":\"{}\",\"registered\":[{}]}}",
                cfg.revision,
                cfg.threads,
                live.unwrap_or("available"),
                reg.join(",")
            );
            respond(s, "200 OK", "application/json", body.as_bytes());
        }
        ("POST", "/register") => {
            let Some(persona) = persona_param(&req) else { return json_err(s, "400 Bad Request", "persona") };
            if let Some(why) = live_call(cfg) {
                return json_err(s, "409 Conflict", why);
            }
            let env = galion_consent::Env::from_process(&cfg.repo);
            let approval = match galion_consent::clone_gate(persona, &env) {
                Ok(a) => a,
                Err(e) => return json_err(s, "403 Forbidden", e.reason),
            };
            if approval.cc0 && !cfg.accept_cc0 {
                return json_err(s, "403 Forbidden", "cc0_licence");
            }
            let args = vec![
                "voice".into(), "add".into(),
                "--name".into(), voice_name(persona),
                "--voice-wav".into(), approval.wav.to_string_lossy().into_owned(),
                "--dtype".into(), DTYPE.into(),
                "--revision".into(), cfg.revision.clone(),
            ];
            if let Err(e) = run_cbx(cfg, &args) {
                return json_err(s, "503 Service Unavailable", &e.replace('"', "'"));
            }
            let mut list: Vec<_> = registered(cfg).into_iter().filter(|(p, _)| p != persona).collect();
            list.push((persona.to_string(), approval.sha256.clone()));
            if write_registered(cfg, &list).is_err() {
                return json_err(s, "500 Internal Server Error", "state");
            }
            respond(s, "200 OK", "application/json", format!("{{\"ok\":true,\"persona\":\"{persona}\"}}").as_bytes());
        }
        ("POST", "/unregister") => {
            let Some(persona) = persona_param(&req) else { return json_err(s, "400 Bad Request", "persona") };
            let list: Vec<_> = registered(cfg).into_iter().filter(|(p, _)| p != persona).collect();
            let _ = write_registered(cfg, &list);
            respond(s, "200 OK", "application/json", b"{\"ok\":true}");
        }
        ("POST", "/speak") => {
            let Some(persona) = persona_param(&req) else { return json_err(s, "400 Bad Request", "persona") };
            if let Some(why) = live_call(cfg) {
                return json_err(s, "409 Conflict", why);
            }
            if !registered(cfg).iter().any(|(p, _)| p == persona) {
                return json_err(s, "403 Forbidden", "not_registered");
            }
            let text = String::from_utf8_lossy(&req.body).trim().to_string();
            if text.is_empty() || text.len() > MAX_TEXT {
                return json_err(s, "400 Bad Request", "text");
            }
            let out = cfg.state.join("last.wav");
            let args = vec![
                "speak".into(),
                "--dtype".into(), DTYPE.into(),
                "--revision".into(), cfg.revision.clone(),
                "--voice".into(), voice_name(persona),
                "--text".into(), text,
                "--out-wav".into(), out.to_string_lossy().into_owned(),
            ];
            match run_cbx(cfg, &args).and_then(|_| std::fs::read(&out).map_err(|e| e.to_string())) {
                Ok(wav) => respond(s, "200 OK", "audio/wav", &wav),
                Err(e) if e.starts_with("live_call") => json_err(s, "409 Conflict", &e),
                Err(e) => json_err(s, "503 Service Unavailable", &e.replace('"', "'")),
            }
        }
        _ => json_err(s, "404 Not Found", "route"),
    }
}

fn main() {
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("galion-clone-cbx: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = std::fs::create_dir_all(&cfg.state) {
        eprintln!("galion-clone-cbx: state dir: {e}");
        std::process::exit(2);
    }
    let listener = match TcpListener::bind(cfg.addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("galion-clone-cbx: bind {}: {e}", cfg.addr);
            std::process::exit(2);
        }
    };
    eprintln!(
        "galion-clone-cbx: listening {} · chatterbox-turbo-onnx {DTYPE}@{} · {} threads · live port {}",
        cfg.addr, &cfg.revision[..cfg.revision.len().min(8)], cfg.threads, cfg.live_port
    );
    // One job at a time: the clone path is offline work, never concurrent.
    for conn in listener.incoming() {
        let Ok(mut s) = conn else { continue };
        let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
        handle(&cfg, &mut s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_loopback_bind_is_refused() {
        assert!(check_loopback(&"127.0.0.1:7880".parse().unwrap()).is_ok());
        assert!(check_loopback(&"[::1]:7880".parse().unwrap()).is_ok());
        assert!(check_loopback(&"0.0.0.0:7880".parse().unwrap()).is_err());
    }

    #[test]
    fn established_connection_on_the_live_port_counts() {
        let t = "  sl  local_address rem_address   st tx_queue rx_queue\n   0: 0100007F:1EC6 00000000:0000 0A 0\n   1: 0100007F:1EC6 0100007F:D2A4 01 0\n   2: 0100007F:1EC8 0100007F:D2A6 01 0\n   3: 0100007F:1EC6 0100007F:D2A8 06 0\n";
        assert_eq!(established_on(t, 7878), 1, "listen (0A) and TIME_WAIT (06) do not count");
        assert_eq!(established_on(t, 7880), 1);
        assert_eq!(established_on(t, 9), 0);
    }

    #[test]
    fn non_idle_dot_in_the_log_tail_is_a_live_call() {
        let idle = "t=  1.00s f1 dot=idle      r=0.1\nt=  2.00s f2 dot=idle      r=0.1\n";
        assert!(!log_tail_busy(idle, 120));
        let live = format!("{idle}t=  3.00s f3 dot=listening r=0.3\n");
        assert!(log_tail_busy(&live, 120));
        assert!(!log_tail_busy(&live, 0));
    }

    #[test]
    fn head_parses_path_query_and_length() {
        let (m, p, q, n) = parse_head("POST /speak?persona=jarvis HTTP/1.1\r\nHost: x\r\nContent-Length: 12").unwrap();
        assert_eq!((m.as_str(), p.as_str(), n), ("POST", "/speak", 12));
        assert_eq!(q, vec![("persona".to_string(), "jarvis".to_string())]);
    }
}
