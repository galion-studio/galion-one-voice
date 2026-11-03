// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Loopback phone turn. Binds `127.0.0.1` only. The virtual user posts text or a WAV
//! and reads the JSON reply. When Piper or flite can speak and the request asks for
//! audio, the server writes the reply WAV before it answers.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use galion_cascade::phone::wav_to_pcm16k;
use galion_cascade::{AsrMode, AsrSource, LlmMode, PhoneConfig, PhoneLane};
use galion_presence::json::{self, Value};

use crate::listen::{self, peak};
use crate::score::claims_recognition;

pub fn require_loopback(addr: SocketAddr) -> Result<(), String> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err(format!("refusing non-loopback bind {addr}"))
    }
}

struct RawHttp {
    method: String,
    path: String,
    status_code: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl RawHttp {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

fn read_http(stream: &mut TcpStream) -> Result<RawHttp, String> {
    stream.set_read_timeout(Some(Duration::from_secs(60))).ok();
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let header_end = loop {
        if buf.len() > 65_536 {
            return Err("headers too large".into());
        }
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("connection closed before headers".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = std::str::from_utf8(&buf[..header_end]).map_err(|_| "headers are not utf-8")?;
    let mut lines = head.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let second = parts.next().unwrap_or("").to_string();
    // A response line is `HTTP/1.1 200 OK`. A request line is `POST /turn HTTP/1.1`.
    let (path, status_code) = if method.starts_with("HTTP/") {
        (String::new(), second)
    } else {
        (second, String::new())
    };
    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();
            if key == "content-length" {
                content_length = val.parse().map_err(|_| "bad Content-Length")?;
            }
            headers.push((key, val));
        }
    }
    if content_length > 4 * 1024 * 1024 {
        return Err("body too large".into());
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    if body.len() < content_length {
        return Err("short body".into());
    }
    body.truncate(content_length);
    Ok(RawHttp {
        method,
        path,
        status_code,
        headers,
        body,
    })
}

fn write_json(stream: &mut TcpStream, status: &str, body: &str) {
    let msg = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(msg.as_bytes());
    let _ = stream.flush();
}

fn json_str(s: &str) -> String {
    Value::Str(s.to_string()).to_json()
}

fn outcome(
    ok: bool,
    error: &str,
    transcript: &str,
    reply: &str,
    asr: &str,
    claims: bool,
    fixture: bool,
    backend: &str,
    speech_label: &str,
    ttfa_ms: Option<u32>,
    wav: Option<&str>,
    wav_peak: Option<u32>,
    wav_rate: Option<u32>,
) -> String {
    let num = |n: Option<u32>| match n {
        Some(n) => n.to_string(),
        None => "null".into(),
    };
    let wav_json = match wav {
        Some(w) => json_str(w),
        None => "null".into(),
    };
    format!(
        "{{\"ok\":{ok},\"error\":{err},\"transcript\":{tr},\"reply\":{reply},\"asr\":{asr},\"claims_recognition\":{claims},\"fixture_decoded\":{fixture},\"backend\":{backend},\"speech_label\":{label},\"ttfa_ms\":{ttfa},\"wav\":{wav},\"wav_peak\":{peak},\"wav_rate\":{rate}}}",
        ok = if ok { "true" } else { "false" },
        err = json_str(error),
        tr = json_str(transcript),
        reply = json_str(reply),
        asr = json_str(asr),
        claims = if claims { "true" } else { "false" },
        fixture = if fixture { "true" } else { "false" },
        backend = json_str(backend),
        label = json_str(speech_label),
        ttfa = num(ttfa_ms),
        wav = wav_json,
        peak = num(wav_peak),
        rate = num(wav_rate),
    )
}

fn safe_case(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn handle_turn(lane: &mut PhoneLane, req: &RawHttp, wav_dir: &Path) -> String {
    let (backend, speech_label) = {
        let (id, label) = lane.speech_backend();
        (id.to_string(), label)
    };
    let fail = |msg: &str| {
        outcome(
            false,
            msg,
            "",
            "",
            "none",
            false,
            false,
            &backend,
            &speech_label,
            None,
            None,
            None,
            None,
        )
    };
    let case_id = req.header("x-galion-case").unwrap_or("turn");
    if !safe_case(case_id) {
        return fail("case id must be ascii letters, digits, or hyphen");
    }
    let speak = req.header("x-galion-speak").is_some_and(|v| v == "1");
    let ctype = req.header("content-type").unwrap_or("");
    let text = if ctype.starts_with("text/plain") {
        match String::from_utf8(req.body.clone()) {
            Ok(t) => Some(t),
            Err(_) => return fail("text body is not utf-8"),
        }
    } else if ctype.starts_with("audio/wav") || req.body.starts_with(b"RIFF") {
        None
    } else {
        return fail("content-type must be text/plain or audio/wav");
    };

    if speak && lane.can_speak() {
        let t0 = Instant::now();
        let turned = if let Some(text) = text.as_deref() {
            lane.turn_text(text)
        } else {
            lane.turn_wav(&req.body)
        };
        let turn = match turned {
            Ok(t) => t,
            Err(e) => return fail(&e.to_string()),
        };
        let wall = t0.elapsed().as_millis().min(u32::MAX as u128) as u32;
        let parsed = match listen::parse_pcm16(&turn.wav_bytes) {
            Ok(p) => p,
            Err(e) => return fail(&e),
        };
        let lead = listen::first_voiced_ms(&parsed.samples, parsed.rate);
        let ttfa = lead.map(|ms| wall.saturating_add(ms));
        let pk = peak(&parsed.samples);
        let name = format!("{case_id}.wav");
        if let Err(e) = std::fs::write(wav_dir.join(&name), &turn.wav_bytes) {
            return fail(&format!("could not write reply wav: {e}"));
        }
        return outcome(
            true,
            "",
            &turn.transcript,
            &turn.reply_text,
            turn.asr.id(),
            claims_recognition(turn.asr),
            turn.asr == AsrSource::StubFixture,
            turn.speech_backend,
            &turn.speech_label,
            ttfa,
            Some(&name),
            Some(pk),
            Some(parsed.rate),
        );
    }

    let prepared = if let Some(text) = text.as_deref() {
        lane.inject_text(text)
    } else {
        match wav_to_pcm16k(&req.body) {
            Ok(pcm) => lane.reply_text(&pcm),
            Err(e) => return fail(&e.to_string()),
        }
    };
    match prepared {
        Ok((transcript, asr, reply)) => outcome(
            true,
            "",
            &transcript,
            &reply,
            asr.id(),
            claims_recognition(asr),
            asr == AsrSource::StubFixture,
            &backend,
            &speech_label,
            None,
            None,
            None,
            None,
        ),
        Err(e) => fail(&e.to_string()),
    }
}

pub struct Loopback {
    port: u16,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Loopback {
    pub fn bind(wav_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&wav_dir).map_err(|e| e.to_string())?;
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .map_err(|e| e.to_string())?;
        let addr = listener.local_addr().map_err(|e| e.to_string())?;
        require_loopback(addr)?;
        let port = addr.port();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = shutdown.clone();
        let join = thread::Builder::new()
            .name("galion-eval-loopback".into())
            .spawn(move || {
                let mut lane = PhoneLane::new(PhoneConfig {
                    asr: AsrMode::Interim,
                    llm: LlmMode::Canned,
                });
                while !flag.load(Ordering::Relaxed) {
                    let Ok((mut sock, peer)) = listener.accept() else {
                        break;
                    };
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    if !peer.ip().is_loopback() {
                        write_json(
                            &mut sock,
                            "403 Forbidden",
                            &outcome(
                                false,
                                "non-loopback peer",
                                "",
                                "",
                                "none",
                                false,
                                false,
                                "none",
                                "",
                                None,
                                None,
                                None,
                                None,
                            ),
                        );
                        continue;
                    }
                    match read_http(&mut sock) {
                        Ok(req) if req.path == "/shutdown" => {
                            flag.store(true, Ordering::Relaxed);
                            write_json(&mut sock, "200 OK", "{\"ok\":true}");
                            break;
                        }
                        Ok(req) if req.method == "POST" && req.path == "/turn" => {
                            let body = handle_turn(&mut lane, &req, &wav_dir);
                            write_json(&mut sock, "200 OK", &body);
                        }
                        Ok(_) => write_json(
                            &mut sock,
                            "404 Not Found",
                            &outcome(
                                false,
                                "not found",
                                "",
                                "",
                                "none",
                                false,
                                false,
                                "none",
                                "",
                                None,
                                None,
                                None,
                                None,
                            ),
                        ),
                        Err(e) => write_json(
                            &mut sock,
                            "400 Bad Request",
                            &outcome(
                                false, &e, "", "", "none", false, false, "none", "", None, None,
                                None, None,
                            ),
                        ),
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            port,
            shutdown,
            join: Some(join),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn close(&mut self) {
        if self.join.is_none() {
            return;
        }
        let _ = post(self.port, "/shutdown", "text/plain", "close", false, b"");
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for Loopback {
    fn drop(&mut self) {
        if self.join.is_none() {
            return;
        }
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnView {
    pub ok: bool,
    pub error: String,
    pub transcript: String,
    pub reply: String,
    pub asr: String,
    pub claims_recognition: bool,
    pub fixture_decoded: bool,
    pub backend: String,
    pub speech_label: String,
    pub ttfa_ms: Option<u32>,
    pub wav: Option<String>,
    pub wav_peak: Option<u32>,
    pub wav_rate: Option<u32>,
}

pub fn post_turn(
    port: u16,
    content_type: &str,
    case_id: &str,
    speak: bool,
    body: &[u8],
) -> Result<TurnView, String> {
    let value = post(port, "/turn", content_type, case_id, speak, body)?;
    let req_str = |k: &str| {
        value
            .get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("missing {k}"))
    };
    let req_bool = |k: &str| match value.get(k) {
        Some(Value::Bool(b)) => Ok(*b),
        _ => Err(format!("missing {k}")),
    };
    let opt_u32 = |k: &str| match value.get(k) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::Num(n)) if n.is_finite() && *n >= 0.0 && *n <= u32::MAX as f64 => {
            Ok(Some(*n as u32))
        }
        _ => Err(format!("bad {k}")),
    };
    let opt_str = |k: &str| match value.get(k) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::Str(s)) => Ok(Some(s.clone())),
        _ => Err(format!("bad {k}")),
    };
    Ok(TurnView {
        ok: req_bool("ok")?,
        error: req_str("error")?,
        transcript: req_str("transcript")?,
        reply: req_str("reply")?,
        asr: req_str("asr")?,
        claims_recognition: req_bool("claims_recognition")?,
        fixture_decoded: req_bool("fixture_decoded")?,
        backend: req_str("backend")?,
        speech_label: req_str("speech_label")?,
        ttfa_ms: opt_u32("ttfa_ms")?,
        wav: opt_str("wav")?,
        wav_peak: opt_u32("wav_peak")?,
        wav_rate: opt_u32("wav_rate")?,
    })
}

fn post(
    port: u16,
    path: &str,
    content_type: &str,
    case_id: &str,
    speak: bool,
    body: &[u8],
) -> Result<Value, String> {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    require_loopback(addr)?;
    let mut stream = TcpStream::connect(addr).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(60))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(30))).ok();
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nX-Galion-Case: {case_id}\r\nX-Galion-Speak: {}\r\nConnection: close\r\n\r\n",
        body.len(),
        if speak { "1" } else { "0" }
    );
    stream
        .write_all(head.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(body).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    let resp = read_http(&mut stream)?;
    if resp.status_code != "200" {
        return Err(format!(
            "HTTP {} {}",
            resp.status_code,
            String::from_utf8_lossy(&resp.body)
        ));
    }
    let text = String::from_utf8(resp.body).map_err(|e| e.to_string())?;
    json::parse(&text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_guard_rejects_wildcard() {
        assert!(require_loopback(SocketAddr::from((Ipv4Addr::LOCALHOST, 9))).is_ok());
        assert!(require_loopback(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 9))).is_err());
    }
}
