// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Warm LLM requests (latency-240 lane 04).
//!
//! `llm::stream` pays, on every turn and inside the measured TTFT: JSON
//! build, a temp file write, a curl fork/exec, DNS, TCP, TLS (~30-40 ms to
//! OpenRouter from this box) and HTTP/1.1. Here a curl child is started
//! *ahead of the turn*: it connects, finishes TLS, negotiates HTTP/2 and
//! sends the request headers, then blocks reading the body from stdin
//! (`-T -`). At turn time we write one prebuilt JSON body (only the user
//! text is escaped) and close stdin. An unused spare never sends a body, so
//! it never reaches the model and costs nothing; spares are recycled before
//! idle timeouts. The key stays in the environment: curl expands it from
//! `--variable %OPENROUTER_API_KEY`, so it is never in argv, a file, or logs.
use crate::llm::{LlmCfg, LlmEv};
use crate::now_ms;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

const USER_MARK: &str = "@@GALION_USER_TEXT@@";

/// Request body serialised once per persona; per turn only the user text is escaped in.
#[derive(Clone)]
pub struct Prompt { prefix: String, suffix: String }
impl Prompt {
    pub fn new(cfg: &LlmCfg, system: &str, temperature: f64) -> Self {
        let mut body = serde_json::json!({"model": cfg.model, "messages": [{"role":"system","content": system}, {"role":"user","content": USER_MARK}],
            "stream": true, "max_tokens": cfg.max_tokens, "temperature": temperature, "usage": {"include": true}});
        if !cfg.provider_order.is_empty() { body["provider"] = serde_json::json!({"order": cfg.provider_order, "allow_fallbacks": true}); }
        if let Some(r) = cfg.reasoning { body["reasoning"] = serde_json::json!({"effort": r}); }
        let s = body.to_string();
        let m = format!("\"{USER_MARK}\"");
        let i = s.find(&m).expect("marker");
        Self { prefix: s[..i].to_string(), suffix: s[i + m.len()..].to_string() }
    }
    pub fn body(&self, user: &str) -> String {
        let mut b = String::with_capacity(self.prefix.len() + self.suffix.len() + user.len() + 8);
        b.push_str(&self.prefix); b.push_str(&serde_json::to_string(user).unwrap()); b.push_str(&self.suffix); b
    }
}

/// A pre-connected curl child waiting for its body.
pub struct Warm { child: Child, stdin: ChildStdin, stdout: ChildStdout, pub born_ms: f64 }
impl Warm {
    pub fn spawn(url: &str) -> Result<Self, String> {
        if std::env::var_os("OPENROUTER_API_KEY").is_none() { return Err("OPENROUTER_API_KEY not set; no LLM call made".into()); }
        let born_ms = now_ms();
        let mut child = Command::new("curl")
            .args(["-sS", "-N", "--http2", "-X", "POST", "-T", "-", "-H", "Expect:", "-H", "Content-Type: application/json",
                   "--variable", "%OPENROUTER_API_KEY", "--expand-header", "Authorization: Bearer {{OPENROUTER_API_KEY}}", url])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn().map_err(|e| format!("spawn curl: {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        Ok(Self { child, stdin, stdout, born_ms })
    }
    /// Send the body; returns the event stream and a cancel handle.
    pub fn fire(mut self, body: &str) -> Result<Inflight, String> {
        let t_sent = now_ms();
        self.stdin.write_all(body.as_bytes()).map_err(|e| format!("warm curl stdin: {e}"))?;
        drop(self.stdin); // EOF: curl finishes the upload
        let child = Arc::new(Mutex::new(self.child));
        let c2 = child.clone();
        let out = self.stdout;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut cost = None; let mut provider = String::new();
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let t = now_ms();
                let Some(d) = line.strip_prefix("data: ") else { continue };
                if d.trim() == "[DONE]" { break; }
                let Ok(v) = serde_json::from_str::<serde_json::Value>(d) else { continue };
                if let Some(p) = v["provider"].as_str() { provider = p.to_string(); }
                if let Some(c) = v["usage"]["cost"].as_f64() { cost = Some(c); }
                let delta = &v["choices"][0]["delta"];
                if let Some(r) = delta["reasoning"].as_str() { if !r.is_empty() { let _ = tx.send(LlmEv::Thought(t, r.to_string())); } }
                if let Some(c) = delta["content"].as_str() { if !c.is_empty() { let _ = tx.send(LlmEv::Token(t, c.to_string())); } }
            }
            let _ = c2.lock().unwrap().wait();
            let _ = tx.send(LlmEv::Done(now_ms(), cost, provider));
        });
        Ok(Inflight { rx, child, t_sent })
    }
}
impl Warm { fn kill(mut self) { let _ = self.child.kill(); let _ = self.child.wait(); } }

pub struct Inflight { pub rx: Receiver<LlmEv>, child: Arc<Mutex<Child>>, pub t_sent: f64 }
impl Inflight {
    /// Abort: the curl child dies, the stream ends with `Done`.
    pub fn cancel(&self) { let _ = self.child.lock().map(|mut c| c.kill()); }
    pub fn canceller(&self) -> impl Fn() + Send + 'static { let c = self.child.clone(); move || { let _ = c.lock().map(|mut c| c.kill()); } }
}

/// Keeps `n` pre-connected spares; recycles any older than `max_age_ms`.
pub struct WarmPool { url: String, n: usize, pub max_age_ms: f64, spares: VecDeque<Warm>, pub cold_takes: usize }
impl WarmPool {
    pub fn new(cfg: &LlmCfg, n: usize) -> Result<Self, String> {
        let mut p = Self { url: cfg.url.clone(), n: n.max(1), max_age_ms: 20_000.0, spares: VecDeque::new(), cold_takes: 0 };
        p.refill()?; Ok(p)
    }
    pub fn refill(&mut self) -> Result<(), String> {
        let now = now_ms();
        while let Some(w) = self.spares.front() { if now - w.born_ms > self.max_age_ms { self.spares.pop_front().unwrap().kill(); } else { break; } }
        while self.spares.len() < self.n { self.spares.push_back(Warm::spawn(&self.url)?); }
        Ok(())
    }
    /// Oldest live spare (most likely through TLS already).
    pub fn take(&mut self) -> Result<Warm, String> {
        match self.spares.pop_front() { Some(w) => Ok(w), None => { self.cold_takes += 1; Warm::spawn(&self.url) } }
    }
}
impl Drop for WarmPool { fn drop(&mut self) { for w in self.spares.drain(..) { w.kill(); } } }

#[cfg(test)]
mod t {
    #[test]
    fn prompt_body_is_valid_json_with_escaped_user() {
        let cfg = crate::llm::LlmCfg { url: "x".into(), model: "m".into(), max_tokens: 40, reasoning: None, provider_order: vec!["Groq".into()], temperature: 0.7 };
        let p = super::Prompt::new(&cfg, "sys \"q\"", 0.7);
        let v: serde_json::Value = serde_json::from_str(&p.body("he said \"hi\"\n")).unwrap();
        assert_eq!(v["messages"][1]["content"], "he said \"hi\"\n");
        assert_eq!(v["messages"][0]["content"], "sys \"q\"");
        assert_eq!(v["max_tokens"], 40);
    }
}
