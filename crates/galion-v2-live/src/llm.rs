// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Streaming chat completions.
//!
//! A loopback `GALION_LLM_URL` (the local llama-server with Qwen2.5-1.5B) goes
//! through the in-process keep-alive client in [`crate::local`]. It uses no
//! key, no curl, and no provider field. Any other URL goes through a `curl`
//! child (no TLS stack in tree). The key is read from OPENROUTER_API_KEY and
//! handed to curl on stdin as a config line, so it never appears in argv,
//! logs, or output.
use crate::now_ms;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

#[derive(Debug)]
pub enum LlmEv { Thought(f64, String), Token(f64, String), Done(f64, Option<f64>, String) }

#[derive(Clone, Debug)]
pub struct LlmCfg { pub url: String, pub model: String, pub max_tokens: u32, pub reasoning: Option<&'static str>, pub provider_order: Vec<String>, pub temperature: f64 }
impl LlmCfg {
    pub fn from_env() -> Self {
        let url = std::env::var("GALION_LLM_URL").unwrap_or_else(|_| "https://openrouter.ai/api/v1/chat/completions".into());
        let local = crate::local::split_local(&url).is_some();
        Self {
            model: std::env::var("GALION_LLM_MODEL").unwrap_or_else(|_| if local { "qwen2.5-1.5b-instruct".into() } else { "meta-llama/llama-3.1-8b-instruct".into() }),
            max_tokens: std::env::var("GALION_LLM_MAX_TOKENS").ok().and_then(|v| v.parse().ok()).unwrap_or(60),
            reasoning: None,
            provider_order: provider_order(std::env::var("GALION_LLM_PROVIDER").ok().as_deref(), local),
            temperature: std::env::var("GALION_LLM_TEMPERATURE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.7),
            url,
        }
    }
    /// True when the URL is the loopback llama-server (in-process client).
    pub fn is_local(&self) -> bool { crate::local::split_local(&self.url).is_some() }
}

/// `GALION_LLM_PROVIDER`: a comma list. Empty entries are dropped, so `""`
/// (or `" , "`) means **no provider field at all**. The old code sent
/// `provider:{order:[""]}`. Unset means `Groq` for OpenRouter and nothing for a
/// local server.
pub fn provider_order(env: Option<&str>, local: bool) -> Vec<String> {
    match env {
        Some(s) => s.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect(),
        None if local => Vec::new(),
        None => vec!["Groq".into()],
    }
}

/// Start a streaming completion; events arrive on the returned channel.
pub fn stream(cfg: &LlmCfg, messages: serde_json::Value) -> Result<Receiver<LlmEv>, String> {
    // GALION_LLM_CLIENT=curl forces the old per-turn curl path (used for A/B timing against the same local server).
    let force_curl = std::env::var("GALION_LLM_CLIENT").as_deref() == Ok("curl");
    if cfg.is_local() && !force_curl { return crate::local::stream(cfg, messages); }
    let key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(k) => Some(k),
        Err(_) if cfg.is_local() => None,
        Err(_) => return Err("OPENROUTER_API_KEY not set; no LLM call made".to_string()),
    };
    let mut body = serde_json::json!({"model": cfg.model, "messages": messages, "stream": true,
        "max_tokens": cfg.max_tokens, "temperature": cfg.temperature, "usage": {"include": true}});
    if !cfg.provider_order.is_empty() { body["provider"] = serde_json::json!({"order": cfg.provider_order, "allow_fallbacks": true}); }
    if let Some(r) = cfg.reasoning { body["reasoning"] = serde_json::json!({"effort": r}); }
    let tmp = std::env::temp_dir().join(format!("galion-llm-{}-{}.json", std::process::id(), (now_ms() * 1000.0) as u64));
    std::fs::write(&tmp, body.to_string()).map_err(|e| e.to_string())?;
    let mut child = Command::new("curl")
        .args(["-sS", "-N", "--http1.1", "-K", "-", "-H", "Content-Type: application/json", "--data-binary"])
        .arg(format!("@{}", tmp.display())).arg(&cfg.url)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|e| format!("spawn curl: {e}"))?;
    {
        let mut si = child.stdin.take().unwrap();
        if let Some(key) = key { si.write_all(format!("header = \"Authorization: Bearer {key}\"\n").as_bytes()).map_err(|e| e.to_string())?; }
    }
    let out = child.stdout.take().unwrap();
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
        let _ = child.wait();
        let _ = std::fs::remove_file(&tmp);
        let _ = tx.send(LlmEv::Done(now_ms(), cost, provider));
    });
    Ok(rx)
}

/// Clause chunker for sentence-piece streaming into Piper.
pub struct Clauser { buf: String, pub emitted: usize, pub first_words: usize, pub hard_cap: bool }
impl Clauser {
    /// The first clause is cut after at most `first_words` words, as soon as
    /// the token that starts word N+1 arrives (or at punctuation, earlier).
    /// latency-240 fix: the old cap only fired when the buffer ended in a
    /// space, which Llama/BPE tokens (" word") almost never do, so first
    /// clauses ran 8-12 words waiting for punctuation.
    pub fn new(first_words: usize) -> Self { Self { buf: String::new(), emitted: 0, first_words, hard_cap: true } }
    /// Alias of `new` (explicit name for the 2-4 word first-clause cap).
    pub fn capped(cap: usize) -> Self { Self::new(cap) }
    /// Pre-fix behaviour, kept only so A/B benches can reproduce the baseline.
    pub fn legacy(first_words: usize) -> Self { Self { buf: String::new(), emitted: 0, first_words, hard_cap: false } }
    /// Byte offset where word `n+1` (1-based) starts, if it has started.
    fn word_start(&self, n: usize) -> Option<usize> {
        let mut words = 0; let mut prev_ws = true;
        for (i, ch) in self.buf.char_indices() {
            let ws = ch.is_whitespace();
            if prev_ws && !ws { words += 1; if words == n + 1 { return Some(i); } }
            prev_ws = ws;
        }
        None
    }
    pub fn push(&mut self, tok: &str) -> Vec<String> {
        self.buf.push_str(tok);
        let mut out = Vec::new();
        loop {
            let words = self.buf.split_whitespace().count();
            let cut = self.buf.char_indices().find(|&(i, ch)| {
                let hard = matches!(ch, '.' | '!' | '?' | ';' | ':' | '\u{2014}');
                let soft = ch == ',';
                let w = self.buf[..i].split_whitespace().count();
                (hard && w >= 1) || (soft && w >= if self.emitted == 0 { 2 } else { 5 })
            }).map(|(i, ch)| i + ch.len_utf8());
            let cut = cut.or_else(|| if self.hard_cap && self.emitted == 0 { self.word_start(self.first_words) } else { None });
            let cut = cut.or_else(|| {
                // first clause: don't wait for punctuation past N words (cut at the last space)
                if self.emitted == 0 && words > self.first_words && self.buf.ends_with(' ') { Some(self.buf.len()) } else { None }
            });
            match cut {
                Some(c) => {
                    let s: String = self.buf.drain(..c).collect();
                    let s = s.trim().to_string();
                    if !s.is_empty() { out.push(s); self.emitted += 1; }
                }
                None => break,
            }
        }
        out
    }
    pub fn flush(&mut self) -> Option<String> {
        let s = std::mem::take(&mut self.buf).trim().to_string();
        if s.is_empty() { None } else { self.emitted += 1; Some(s) }
    }
}

#[cfg(test)]
mod t {
    use super::{provider_order, Clauser};
    #[test]
    fn empty_provider_env_sends_no_provider() {
        assert!(provider_order(Some(""), false).is_empty(), "GALION_LLM_PROVIDER=\"\" must not send order:[\"\"]");
        assert!(provider_order(Some(" , "), false).is_empty());
        assert_eq!(provider_order(Some("Groq, Cerebras"), false), vec!["Groq".to_string(), "Cerebras".to_string()]);
        assert_eq!(provider_order(None, false), vec!["Groq".to_string()]);
        assert!(provider_order(None, true).is_empty(), "local server: no provider by default");
        let cfg = super::LlmCfg { url: "https://x/v1".into(), model: "m".into(), max_tokens: 4, reasoning: None, provider_order: provider_order(Some(""), false), temperature: 0.7 };
        let p = crate::warm::Prompt::new(&cfg, "s", 0.7);
        let v: serde_json::Value = serde_json::from_str(&p.body("u")).unwrap();
        assert!(v.get("provider").is_none(), "{v}");
    }
    #[test]
    fn capped_first_clause() {
        let mut c = Clauser::capped(3);
        let mut out = vec![];
        for t in ["What", " a", " fascinating", " tale", " you're", " sharing", ",", " I'm", " intrigued", "."] { out.extend(c.push(t)); }
        assert_eq!(out[0], "What a fascinating");
        assert_eq!(out[1], "tale you're sharing, I'm intrigued."); // later clauses: old rules
        let mut c = Clauser::capped(3);
        assert_eq!(c.push("That's"), Vec::<String>::new());
        assert_eq!(c.push(" great,"), vec!["That's great,".to_string()]);
        // Llama-style leading-space tokens: new(5) now fires at 5 words; legacy never did
        let toks = ["What", " a", " fascinating", " tale", " you're", " sharing", " with", " me", ","];
        let mut n = Clauser::new(5); let mut v = vec![];
        for t in toks { v.extend(n.push(t)); }
        assert_eq!(v[0], "What a fascinating tale you're");
        let mut c = Clauser::new(2); let mut v = vec![];
        for t in [" Oh", " wow", " that", " is", " great", "."] { v.extend(c.push(t)); }
        assert_eq!(v[0], "Oh wow");
        let mut o = Clauser::legacy(5);
        let mut v = vec![];
        for t in ["What", " a", " fascinating", " tale", " you're", " sharing", ","] { v.extend(o.push(t)); }
        assert_eq!(v, vec!["What a fascinating tale you're sharing,".to_string()]);
    }
}
