//! Hermes reply client. Std only: one `TcpStream` to a loopback OpenAI-compatible
//! chat-completions endpoint. No TLS, no new crate, no public host.
//!
//! ```text
//! GALION_LLM=hermes
//! GALION_LLM_URL=http://127.0.0.1:8642/v1/chat/completions
//! GALION_LLM_MODEL=hermes-agent
//! GALION_LLM_KEY=...            # optional; sent as Authorization, never logged
//! GALION_LLM_TIMEOUT_MS=3000
//! ```
//!
//! A timeout, a refused connect, a non-loopback URL, or a bad response is the
//! plain line [`UNREACHABLE`]. This client does not call [`crate::CannedLlm`].

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

use crate::{CascadeError, Emotion, Llm, Transcript};

/// Spoken when the gateway does not answer. Not a canned intent line.
pub const UNREACHABLE: &str = "model can't be reached";
/// Galion Research desk. A voice of Galion One, with Hermes as the agent behind it.
pub const RESEARCH_PROMPT: &str = "You are Galion Research, the research app on Hermes, a voice of Galion One. \
Answer the question first, then the limit. Separate found, inferred, and unknown. \
Cite only a source that was handed to you, one per line, starting with [1]. \
If you have no source, say \"I don't have a source for that.\" \
Do not invent a paper, a URL, a date, or a measurement. English only.";
/// Spoken when the model returns a forbidden empathy script.
pub const REFUSED: &str = "I'll stay on what you said.";
/// Stable client id when no model name is configured.
pub const LLM_NAME: &str = "hermes-agent";
/// Approved gateway when `GALION_LLM_URL` is unset and the mode is hermes.
pub const DEFAULT_URL: &str = "http://127.0.0.1:8642/v1/chat/completions";
pub const DEFAULT_TIMEOUT_MS: u64 = 3_000;

/// Empathy scripts the model must not speak. Same list the `/v2` floor uses.
/// This is the one list: `scripts/eval/v2_hard_loop.py` parses it from this
/// file, and `forbidden_list_is_mirrored_by_the_hard_harness` keeps them in sync.
pub const FORBIDDEN: &[&str] = &[
    "i hear you",
    "that sounds so hard",
    "that sounds hard",
    "i'm here for you",
    "i feel that in my soul",
    "i know this is difficult",
    "i'm worried about you",
    "take a deep breath",
    "so what i'm hearing",
    "noted, that is hard",
    "i'm sorry for your loss",
    "let's slow down",
    "i'm sorry",
    "i am sorry",
    "i'm so sorry",
    "i am so sorry",
    "so sorry to hear",
    "sorry to hear",
    "sorry for your loss",
    "my condolences",
    "my deepest condolences",
    "you are not alone",
    "you're not alone",
    "allow yourself to grieve",
    "losing a loved one",
    "i can only imagine",
    "i can't imagine how",
    "that must be so hard",
    "that must be hard",
    "that must be difficult",
];

pub fn contains_forbidden(reply: &str) -> bool {
    // Curly apostrophes and runs of whitespace must not slip a phrase past the list.
    let low: String = reply
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' | '`' => '\'',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    let low = low.split_whitespace().collect::<Vec<_>>().join(" ");
    FORBIDDEN.iter().any(|p| low.contains(p))
}

/// Loopback chat endpoint. Host is numeric; the path is the POST target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HermesTarget {
    pub addr: SocketAddr,
    pub path: String,
}

/// `http://127.0.0.1:8642/v1/chat/completions` or `http://[::1]:8642/...`.
/// HTTPS, DNS names, userinfo, and non-loopback addresses are `None`.
pub fn parse_loopback_chat_url(url: &str) -> Option<HermesTarget> {
    let rest = url.trim().strip_prefix("http://")?;
    if rest.contains('@') || rest.contains(' ') {
        return None;
    }
    let (hostport, raw_path) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let path = raw_path.split(['?', '#']).next().unwrap_or("");
    if path.contains(['\r', '\n']) {
        return None;
    }
    let path = if path.is_empty() {
        "/v1/chat/completions".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        return None;
    };
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
    if !ip.is_loopback() {
        return None;
    }
    Some(HermesTarget {
        addr: SocketAddr::new(ip, port),
        path,
    })
}

/// Short register prompt. The words stay on the matter. Empathy is prosody, not a script.
/// No product framing: the persona is a voice on a call, not a support desk.
/// The ban list in the prompt is [`FORBIDDEN`] itself, so the prompt and the
/// guard cannot drift.
pub fn system_prompt(persona: &str) -> &'static str {
    use std::sync::OnceLock;
    static SAMANTHA: OnceLock<String> = OnceLock::new();
    static JARVIS: OnceLock<String> = OnceLock::new();
    if persona == "jarvis" {
        JARVIS.get_or_init(|| {
            prompt_with_bans(
                "You are Jarvis, a calm and exact voice on a call. You are not a film voice and you have no clip. \
Say the fact they named back in your own words, then stay on it. One or two short spoken sentences.",
            )
        })
    } else {
        SAMANTHA.get_or_init(|| {
            prompt_with_bans(
                "You are Samantha, a warm voice on a call. You are not a film voice and you have no clip. \
Say the fact they named back in your own words, then stay on it. One or two short spoken sentences.",
            )
        })
    }
}

fn prompt_with_bans(head: &str) -> String {
    let mut s = String::from(head);
    s.push_str(" Do not use a canned empathy script. Never say any of these: ");
    for (i, p) in FORBIDDEN.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push('"');
        s.push_str(p);
        s.push('"');
    }
    s.push_str(". Do not claim you feel their pain. No film quote. English only.");
    s
}

/// One Hermes research reply. `citations` are the `[n]` lines from the model.
/// `text` is the guarded answer. Neither field carries the gateway key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchAnswer {
    pub text: String,
    pub citations: Vec<(u32, String)>,
}

/// OpenAI-compatible chat client. `Send`. Connects only when [`HermesLlm::complete_as`] runs.
#[derive(Clone)]
pub struct HermesLlm {
    target: Option<HermesTarget>,
    model: String,
    key: String,
    timeout: Duration,
    persona: String,
}

impl std::fmt::Debug for HermesLlm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HermesLlm")
            .field("target", &self.target)
            .field("model", &self.model)
            .field("key", &if self.key.is_empty() { "none" } else { "set" })
            .field("timeout", &self.timeout)
            .field("persona", &self.persona)
            .finish()
    }
}

impl HermesLlm {
    /// Build a client. A URL that is not numeric loopback `http://` leaves the
    /// client unable to connect; replies are then [`UNREACHABLE`].
    pub fn new(url: &str, model: &str, key: &str, timeout: Duration) -> Self {
        let timeout = if timeout.is_zero() {
            Duration::from_millis(DEFAULT_TIMEOUT_MS)
        } else {
            timeout
        };
        let model = {
            let m = model.trim();
            if m.is_empty() {
                LLM_NAME.to_string()
            } else {
                m.chars().take(64).collect()
            }
        };
        Self {
            target: parse_loopback_chat_url(url),
            model,
            key: key.trim().to_string(),
            timeout,
            persona: "samantha".into(),
        }
    }

    /// `GALION_LLM_URL` (default [`DEFAULT_URL`]), `GALION_LLM_MODEL`, `GALION_LLM_KEY`,
    /// `GALION_LLM_TIMEOUT_MS` (default 3000, clamped 50..=30000).
    pub fn from_env() -> Self {
        let url = std::env::var("GALION_LLM_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
        let model = std::env::var("GALION_LLM_MODEL").unwrap_or_else(|_| LLM_NAME.to_string());
        let key = std::env::var("GALION_LLM_KEY").unwrap_or_default();
        let timeout_ms: u64 = std::env::var("GALION_LLM_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        let timeout_ms = timeout_ms.clamp(50, 30_000);
        Self::new(
            url.trim(),
            model.trim(),
            key.trim(),
            Duration::from_millis(timeout_ms),
        )
    }

    /// Attach only when the process asked for Hermes.
    ///
    /// `GALION_LLM=hermes`, or a set `GALION_LLM_URL` with no explicit mode.
    /// `canned`, `echo`, and `mock` stay off this client. An unknown mode does
    /// not attach and does not select canned; the phone lane reports that as an error.
    pub fn attached_from_env() -> Option<Self> {
        let mode = std::env::var("GALION_LLM")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase());
        let url_set = std::env::var("GALION_LLM_URL")
            .ok()
            .is_some_and(|s| !s.trim().is_empty());
        let attach = match mode.as_deref() {
            Some("canned") | Some("echo") | Some("mock") => false,
            Some("hermes") => true,
            None => url_set,
            Some(_) => false,
        };
        if attach {
            Some(Self::from_env())
        } else {
            None
        }
    }

    /// Galion Research client, separate from the voice LLM (`GALION_LLM` stays as it is).
    ///
    /// `GALION_RESEARCH_URL` (loopback chat URL) turns it on; unset or not loopback is `None`,
    /// and the desk keeps answering Unreachable. `GALION_RESEARCH_MODEL` (default
    /// `hermes-agent`), `GALION_RESEARCH_TIMEOUT_MS` (default 60000, clamped 1000..=90000).
    /// Key: `GALION_RESEARCH_KEY`, or one `NAME=value` line named by `GALION_RESEARCH_KEY_NAME`
    /// in the dotenv file `GALION_RESEARCH_KEY_FILE`. The key is never logged or returned.
    pub fn research_from_env() -> Option<Self> {
        Self::research_from_vars(|k| std::env::var(k).ok(), |p| std::fs::read_to_string(p).ok())
    }

    /// [`HermesLlm::research_from_env`] with injected lookups (tests).
    pub fn research_from_vars(
        var: impl Fn(&str) -> Option<String>,
        read: impl Fn(&str) -> Option<String>,
    ) -> Option<Self> {
        let get = |k: &str| var(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let url = get("GALION_RESEARCH_URL")?;
        parse_loopback_chat_url(&url)?;
        let model = get("GALION_RESEARCH_MODEL").unwrap_or_else(|| LLM_NAME.to_string());
        let key = match get("GALION_RESEARCH_KEY") {
            Some(k) => k,
            None => match (get("GALION_RESEARCH_KEY_FILE"), get("GALION_RESEARCH_KEY_NAME")) {
                (Some(file), Some(name)) => read(&file).and_then(|t| dotenv_value(&t, &name)).unwrap_or_default(),
                _ => String::new(),
            },
        };
        let timeout_ms: u64 = get("GALION_RESEARCH_TIMEOUT_MS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(60_000)
            .clamp(1_000, 90_000);
        Some(Self::new(&url, &model, &key, Duration::from_millis(timeout_ms)))
    }

    /// `hermes-agent`, or `GALION_LLM_MODEL` when that name is set.
    pub fn label(&self) -> &str {
        if self.model.is_empty() {
            LLM_NAME
        } else {
            &self.model
        }
    }

    pub fn set_persona(&mut self, id: &str) {
        self.persona = if id == "jarvis" {
            "jarvis".into()
        } else {
            "samantha".into()
        };
    }

    /// One completion. Network failure is [`UNREACHABLE`], never a canned line.
    pub fn complete(&self, user: &str) -> String {
        self.complete_as(&self.persona, user)
    }

    pub fn complete_as(&self, persona: &str, user: &str) -> String {
        self.complete_prompt(system_prompt(persona), &user, 480)
    }

    /// Research turn for the Galion Research desk. The gateway key stays on this
    /// client. It is not copied into the reply. Citation lines are read before
    /// the bracket strip, so `[1]` still names a source.
    pub fn research_turn(&self, user: &str) -> ResearchAnswer {
        let empty = || ResearchAnswer { text: UNREACHABLE.to_string(), citations: Vec::new() };
        let Some(target) = self.target.clone() else {
            return empty();
        };
        if !self.key.is_empty() && !key_ok(&self.key) {
            return empty();
        }
        let user = clip_chars(user.trim(), 2_000);
        if user.is_empty() {
            return empty();
        }
        match post_chat(&target, &self.model, &self.key, RESEARCH_PROMPT, &user, self.timeout) {
            Ok(text) => ResearchAnswer {
                citations: cite_lines(&text),
                text: guard_model_text_cap(&text, 4_000),
            },
            Err(()) => empty(),
        }
    }

    /// `set` or `none`. Never the key bytes.
    pub fn key_state(&self) -> &'static str {
        if self.key.is_empty() { "none" } else { "set" }
    }

    fn complete_prompt(&self, system: &str, user: &str, cap: usize) -> String {
        let Some(target) = self.target.clone() else {
            return UNREACHABLE.to_string();
        };
        if !self.key.is_empty() && !key_ok(&self.key) {
            return UNREACHABLE.to_string();
        }
        let user = clip_chars(user.trim(), 2_000);
        if user.is_empty() {
            return UNREACHABLE.to_string();
        }
        match post_chat(&target, &self.model, &self.key, system, &user, self.timeout) {
            Ok(text) if cap <= 480 => guard_model_text(&text),
            Ok(text) => guard_model_text_cap(&text, cap),
            Err(()) => UNREACHABLE.to_string(),
        }
    }
}

impl Llm for HermesLlm {
    fn name(&self) -> &'static str {
        LLM_NAME
    }

    fn reply(&mut self, t: &Transcript, _e: Emotion) -> Result<String, CascadeError> {
        if t.text.trim().is_empty() {
            return Err(CascadeError::Empty);
        }
        Ok(self.complete(&t.text))
    }

    fn set_persona(&mut self, id: &str) {
        HermesLlm::set_persona(self, id);
    }
}

fn key_ok(key: &str) -> bool {
    !key.is_empty() && key.len() <= 512 && key.bytes().all(|b| b.is_ascii_graphic())
}

fn clip_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// `NAME=value` from dotenv text. `export`, quotes, and `#` comment lines are handled.
fn dotenv_value(text: &str, name: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let t = t.strip_prefix("export ").unwrap_or(t);
        let Some((k, v)) = t.split_once('=') else { continue };
        if k.trim() != name {
            continue;
        }
        let v = v.trim();
        let v = v
            .strip_prefix('"').and_then(|x| x.strip_suffix('"'))
            .or_else(|| v.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')))
            .unwrap_or(v);
        let v = v.trim();
        return if v.is_empty() { None } else { Some(v.to_string()) };
    }
    None
}

/// `[1] Author, title` lines. Read before brackets are stripped from the spoken text.
fn cite_lines(raw: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix('[') else { continue };
        let Some((n, after)) = rest.split_once(']') else { continue };
        let Ok(n) = n.trim().parse::<u32>() else { continue };
        let text = after.trim().trim_start_matches([':', '-', ' ']).trim();
        if text.is_empty() || out.len() >= 12 {
            continue;
        }
        out.push((n, clip_chars(text, 240)));
    }
    out
}

fn guard_model_text(raw: &str) -> String {
    guard_model_text_cap(raw, 480)
}

fn guard_model_text_cap(raw: &str, cap: usize) -> String {
    let text = strip_tags(raw);
    let text = text.trim();
    if text.is_empty() {
        return UNREACHABLE.to_string();
    }
    if contains_forbidden(text) {
        return REFUSED.to_string();
    }
    let cap = cap.clamp(80, 4_000);
    let capped = clip_chars(text, cap);
    if capped.chars().count() < text.chars().count() {
        if let Some(i) = capped.rfind(' ') {
            if i > 80 {
                return capped[..i].to_string();
            }
        }
    }
    capped
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0i32;
    for c in s.chars() {
        if c == '[' {
            depth += 1;
            continue;
        }
        if c == ']' && depth > 0 {
            depth -= 1;
            continue;
        }
        if depth == 0 {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn post_chat(
    target: &HermesTarget,
    model: &str,
    key: &str,
    system: &str,
    user: &str,
    timeout: Duration,
) -> Result<String, ()> {
    let body = format!(
        "{{\"model\":{model},\"messages\":[{{\"role\":\"system\",\"content\":{system}}},{{\"role\":\"user\",\"content\":{user}}}]}}",
        model = json_string(model),
        system = json_string(system),
        user = json_string(user),
    );
    let mut sock = TcpStream::connect_timeout(&target.addr, timeout).map_err(|_| ())?;
    sock.set_read_timeout(Some(timeout)).map_err(|_| ())?;
    sock.set_write_timeout(Some(timeout)).map_err(|_| ())?;
    let auth = if key.is_empty() {
        String::new()
    } else {
        format!("Authorization: Bearer {key}\r\n")
    };
    let req = format!(
        "POST {path} HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n{auth}\r\n{body}",
        path = target.path,
        addr = target.addr,
        len = body.len(),
        auth = auth,
        body = body,
    );
    sock.write_all(req.as_bytes()).map_err(|_| ())?;
    let raw = read_limited(&mut sock, 64 * 1024)?;
    let (status, payload) = split_http(&raw)?;
    if !(200..300).contains(&status) {
        return Err(());
    }
    let text = std::str::from_utf8(&payload).map_err(|_| ())?;
    assistant_content(text).ok_or(())
}

fn read_limited(sock: &mut TcpStream, cap: usize) -> Result<Vec<u8>, ()> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if buf.len() >= cap {
            return Err(());
        }
        let n = match sock.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                if buf.is_empty() {
                    return Err(());
                }
                break;
            }
        };
        let room = cap - buf.len();
        buf.extend_from_slice(&tmp[..n.min(room)]);
        if let Some(header_end) = find_header_end(&buf) {
            let head = &buf[..header_end];
            if is_chunked(head) {
                continue;
            }
            if let Some(len) = content_length(head) {
                let total = header_end + 4 + len;
                if buf.len() >= total {
                    buf.truncate(total.min(cap));
                    break;
                }
            }
        }
    }
    if buf.is_empty() {
        Err(())
    } else {
        Ok(buf)
    }
}

fn split_http(raw: &[u8]) -> Result<(u16, Vec<u8>), ()> {
    let sep = find_header_end(raw).ok_or(())?;
    let head = std::str::from_utf8(&raw[..sep]).map_err(|_| ())?;
    let status = status_of(head);
    let mut body = raw[sep + 4..].to_vec();
    if is_chunked(head.as_bytes()) {
        body = decode_chunked(&body).ok_or(())?;
    } else if let Some(len) = content_length(head.as_bytes()) {
        if body.len() > len {
            body.truncate(len);
        }
    }
    if status == 0 {
        return Err(());
    }
    Ok((status, body))
}

fn status_of(head: &str) -> u16 {
    let line = head.lines().next().unwrap_or("");
    if !line.starts_with("HTTP/") {
        return 0;
    }
    line.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn header_value<'a>(head: &'a [u8], name: &str) -> Option<&'a str> {
    let head = std::str::from_utf8(head).ok()?;
    for line in head.split("\r\n") {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case(name) {
                return Some(v.trim());
            }
        }
    }
    None
}

fn content_length(head: &[u8]) -> Option<usize> {
    header_value(head, "content-length")?.parse().ok()
}

fn is_chunked(head: &[u8]) -> bool {
    header_value(head, "transfer-encoding")
        .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
}

fn decode_chunked(body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let rel = body[i..].windows(2).position(|w| w == b"\r\n")?;
        let hex = std::str::from_utf8(&body[i..i + rel]).ok()?.trim();
        let hex = hex.split(';').next()?.trim();
        let n = usize::from_str_radix(hex, 16).ok()?;
        i += rel + 2;
        if n == 0 {
            break;
        }
        if i + n > body.len() {
            return None;
        }
        out.extend_from_slice(&body[i..i + n]);
        i += n;
        if body.get(i..i + 2) != Some(b"\r\n") {
            return None;
        }
        i += 2;
        if out.len() > 64 * 1024 {
            return None;
        }
    }
    Some(out)
}

fn assistant_content(body: &str) -> Option<String> {
    let rest = match body.find("\"choices\"") {
        Some(i) => &body[i..],
        None => body,
    };
    let rest = match rest.find("\"message\"") {
        Some(i) => &rest[i..],
        None => rest,
    };
    let idx = rest.find("\"content\"")?;
    let after = rest[idx + "\"content\"".len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    if after.starts_with("null") {
        return None;
    }
    parse_json_string(after)
}

pub fn json_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c.is_control() => {
                let u = c as u32;
                o.push_str(&format!("\\u{u:04x}"));
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn parse_json_string(s: &str) -> Option<String> {
    let s = s.trim_start();
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
        if out.len() > 4_000 {
            return Some(out);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Lang;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Instant;

    fn tr(s: &str) -> Transcript {
        Transcript {
            text: s.into(),
            lang: Lang::En,
        }
    }

    struct Serve {
        addr: SocketAddr,
        seen: Arc<Mutex<String>>,
    }

    fn serve_body(status: &str, payload: &str, stall: Option<Duration>) -> Serve {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        let seen_w = seen.clone();
        let status = status.to_string();
        let payload = payload.to_string();
        thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = vec![0u8; 16_384];
            let mut got = Vec::new();
            loop {
                match sock.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        got.extend_from_slice(&buf[..n]);
                        if got.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            if let Ok(s) = String::from_utf8(got) {
                *seen_w.lock().unwrap() = s;
            }
            if let Some(d) = stall {
                thread::sleep(d);
            }
            let resp = format!(
                "HTTP/1.0 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        Serve { addr, seen }
    }

    fn client(addr: SocketAddr, timeout_ms: u64) -> HermesLlm {
        HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            "hermes-agent",
            "",
            Duration::from_millis(timeout_ms),
        )
    }

    #[test]
    fn loopback_only() {
        assert!(parse_loopback_chat_url(DEFAULT_URL).is_some());
        assert!(parse_loopback_chat_url("http://[::1]:8642/v1/chat/completions").is_some());
        assert!(
            parse_loopback_chat_url("http://127.0.0.1:8642")
                .unwrap()
                .path
                == "/v1/chat/completions"
        );
        assert!(parse_loopback_chat_url("https://127.0.0.1:8642/v1/chat/completions").is_none());
        assert!(parse_loopback_chat_url("http://8.8.8.8:8642/v1/chat/completions").is_none());
        assert!(parse_loopback_chat_url("http://localhost:8642/v1/chat/completions").is_none());
        assert!(
            parse_loopback_chat_url("http://user@127.0.0.1:8642/v1/chat/completions").is_none()
        );
        let down = HermesLlm::new(
            "https://127.0.0.1:8642/v1/chat/completions",
            "",
            "",
            Duration::from_millis(50),
        );
        let t = Instant::now();
        assert_eq!(down.complete("hello"), UNREACHABLE);
        assert!(t.elapsed() < Duration::from_millis(200));
        assert!(down.label() == LLM_NAME);
    }

    #[test]
    fn mock_completion_is_the_reply_and_not_canned() {
        let payload = "{\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"She died this morning.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let mut llm = client(srv.addr, 1000);
        llm.set_persona("samantha");
        let reply = llm
            .reply(&tr("My mother died this morning."), Emotion::Neutral)
            .unwrap();
        assert_eq!(reply, "She died this morning.");
        assert!(!reply.contains("doing well"));
        assert!(!contains_forbidden(&reply));
        assert_eq!(llm.name(), LLM_NAME);
        let seen = srv.seen.lock().unwrap().clone();
        assert!(seen.contains("POST /v1/chat/completions"));
        assert!(seen.contains("Samantha"));
        assert!(seen.contains("empathy"));
        assert!(seen.contains("My mother died this morning."));
        assert!(!seen.contains("Hi. I'm here"));
    }

    #[test]
    fn jarvis_prompt_and_bearer_and_escaped_user() {
        let payload = "{\"choices\":[{\"message\":{\"content\":\"The job is on the table.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let mut llm = HermesLlm::new(
            &format!("http://{}/v1/chat/completions", srv.addr),
            "demo-model",
            "test-key",
            Duration::from_millis(1000),
        );
        llm.set_persona("jarvis");
        let reply = llm.complete("say \"hello\", not a script");
        assert_eq!(reply, "The job is on the table.");
        assert_eq!(llm.label(), "demo-model");
        let seen = srv.seen.lock().unwrap().clone();
        assert!(seen.contains("Authorization: Bearer test-key"));
        assert!(seen.contains("Jarvis"));
        assert!(seen.contains("say \\\"hello\\\""));
        assert!(!reply.contains("test-key"));
    }

    #[test]
    fn forbidden_model_line_is_refused() {
        let payload =
            "{\"choices\":[{\"message\":{\"content\":\"I hear you. That sounds so hard.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let llm = client(srv.addr, 1000);
        let reply = llm.complete("My mother died this morning.");
        assert_eq!(reply, REFUSED);
        assert!(!contains_forbidden(&reply));
    }

    #[test]
    fn tags_are_stripped_before_the_check() {
        let payload =
            "{\"choices\":[{\"message\":{\"content\":\"[laugh] The room went quiet.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let llm = client(srv.addr, 1000);
        assert_eq!(llm.complete("hello"), "The room went quiet.");
    }

    #[test]
    fn unreachable_on_refuse_timeout_and_http_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let llm = client(addr, 200);
        let t = Instant::now();
        assert_eq!(llm.complete("hello"), UNREACHABLE);
        assert!(
            t.elapsed() < Duration::from_millis(800),
            "{:?}",
            t.elapsed()
        );

        let stall = serve_body("200 OK", "{\"choices\":[]}", Some(Duration::from_secs(2)));
        let llm = client(stall.addr, 150);
        let t = Instant::now();
        assert_eq!(llm.complete("hello"), UNREACHABLE);
        assert!(
            t.elapsed() < Duration::from_millis(1000),
            "{:?}",
            t.elapsed()
        );
        assert!(!llm.complete("hi").contains("I'm here"));

        let bad = serve_body("500 Nope", "{\"error\":\"x\"}", None);
        let llm = client(bad.addr, 1000);
        assert_eq!(llm.complete("hello"), UNREACHABLE);
    }

    #[test]
    fn chunked_body_decodes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf);
            let chunk = "{\"choices\":[{\"message\":{\"content\":\"Chunked answer.\"}}]}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{chunk}\r\n0\r\n\r\n",
                chunk.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        let llm = client(addr, 1000);
        assert_eq!(llm.complete("hello"), "Chunked answer.");
    }

    #[test]
    fn prompt_has_no_product_framing_and_carries_the_ban_list() {
        for persona in ["samantha", "jarvis"] {
            let p = system_prompt(persona);
            assert!(!p.contains("Galion"), "{p}");
            assert!(!p.to_ascii_lowercase().contains("product"), "{p}");
            assert!(p.contains("fact they named"), "{p}");
            for phrase in FORBIDDEN {
                assert!(p.contains(phrase), "{phrase} missing from the prompt");
            }
        }
        assert!(system_prompt("jarvis").starts_with("You are Jarvis"));
        assert!(system_prompt("samantha").starts_with("You are Samantha"));
    }

    #[test]
    fn research_turn_keeps_the_key_off_the_reply() {
        let key = "test-gateway-key-not-for-git";
        let payload = "{\"choices\":[{\"message\":{\"content\":\"The wash is amber to violet.\\n[1] Galion brand note\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let llm = HermesLlm::new(
            &format!("http://{}/v1/chat/completions", srv.addr),
            "hermes-agent",
            key,
            Duration::from_millis(1000),
        );
        let shown = format!("{llm:?}");
        assert_eq!(llm.key_state(), "set");
        assert!(!shown.contains(key), "{shown}");
        let turn = llm.research_turn("What colour is the wash?");
        assert!(turn.text.contains("amber to violet"), "{}", turn.text);
        assert_eq!(turn.citations.len(), 1);
        assert_eq!(turn.citations[0].0, 1);
        assert!(turn.citations[0].1.contains("Galion brand note"), "{}", turn.citations[0].1);
        assert!(!turn.text.contains(key));
        let seen = srv.seen.lock().unwrap().clone();
        assert!(seen.contains("Galion Research"), "{seen}");
        assert!(seen.contains("Authorization: Bearer "));
        assert!(!format!("{llm:?}").contains(key));
    }

    #[test]
    fn research_from_vars_reads_url_model_and_key_file() {
        use std::collections::HashMap;
        let env: HashMap<&str, &str> = [
            ("GALION_RESEARCH_URL", "http://127.0.0.1:8642/v1/chat/completions"),
            ("GALION_RESEARCH_MODEL", "river-flash"),
            ("GALION_RESEARCH_KEY_FILE", "/x/.env"),
            ("GALION_RESEARCH_KEY_NAME", "API_SERVER_KEY"),
        ]
        .into_iter()
        .collect();
        let file = "# c\nOTHER=1\nexport API_SERVER_KEY=\"test-key-not-for-git\"\n";
        let llm = HermesLlm::research_from_vars(
            |k| env.get(k).map(|v| v.to_string()),
            |p| (p == "/x/.env").then(|| file.to_string()),
        )
        .expect("configured");
        assert_eq!(llm.label(), "river-flash");
        assert_eq!(llm.key_state(), "set");
        assert!(!format!("{llm:?}").contains("test-key-not-for-git"));
        assert_eq!(dotenv_value(file, "API_SERVER_KEY").as_deref(), Some("test-key-not-for-git"));
        assert_eq!(dotenv_value(file, "MISSING"), None);
    }

    #[test]
    fn research_from_vars_is_none_without_a_loopback_url() {
        let none = |_: &str| None::<String>;
        assert!(HermesLlm::research_from_vars(none, none).is_none());
        let remote = |k: &str| (k == "GALION_RESEARCH_URL").then(|| "http://example.com/v1/chat/completions".to_string());
        assert!(HermesLlm::research_from_vars(remote, none).is_none());
        let bare = |k: &str| (k == "GALION_RESEARCH_URL").then(|| "http://127.0.0.1:8642".to_string());
        let llm = HermesLlm::research_from_vars(bare, none).expect("loopback");
        assert_eq!(llm.key_state(), "none");
        assert_eq!(llm.label(), LLM_NAME);
    }

    #[test]
    fn sorry_script_is_refused() {
        assert!(contains_forbidden("I'm sorry to hear that."));
        assert!(contains_forbidden("I am sorry about your mother."));
        assert!(!contains_forbidden("Your mother died this morning."));
    }

    /// The live persona-http reply that slipped past the old list.
    #[test]
    fn so_sorry_and_condolence_scripts_are_refused() {
        assert!(contains_forbidden(
            "I'm so sorry to hear that. Losing a loved one is difficult, but know that you are not alone in your pain."
        ));
        assert!(contains_forbidden("I\u{2019}m so sorry."));
        assert!(contains_forbidden("I am  so sorry."));
        assert!(contains_forbidden("So sorry to hear about your mother."));
        assert!(contains_forbidden("My condolences."));
        assert!(contains_forbidden("Please allow yourself to grieve."));
        assert!(contains_forbidden("You're not alone."));
        assert_eq!(guard_model_text("I'm so sorry to hear that."), REFUSED);
        assert!(!contains_forbidden("Your mother died this morning. Do you want to tell me about her?"));
    }

    /// The hard-test harness parses `FORBIDDEN` from this file and keeps a
    /// mirror for its self-check. Every phrase here must be in that mirror.
    #[test]
    fn forbidden_list_is_mirrored_by_the_hard_harness() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/eval/v2_hard_loop.py");
        let py = std::fs::read_to_string(&path).expect("hard harness present");
        let start = py.find("FORBIDDEN_MIRROR = (").expect("mirror tuple");
        let block = &py[start..];
        let block = &block[..block.find(')').unwrap()];
        let mirrored: Vec<&str> = block
            .lines()
            .filter_map(|l| {
                let l = l.trim().trim_end_matches(',');
                l.strip_prefix('"').and_then(|r| r.strip_suffix('"'))
            })
            .collect();
        assert_eq!(mirrored, FORBIDDEN.to_vec(), "harness mirror drifted from hermes.rs");
    }

    #[test]
    fn empty_transcript_is_an_error() {
        let mut llm = HermesLlm::new(DEFAULT_URL, LLM_NAME, "", Duration::from_millis(50));
        assert!(matches!(
            llm.reply(&tr("  "), Emotion::Neutral),
            Err(CascadeError::Empty)
        ));
    }
}
