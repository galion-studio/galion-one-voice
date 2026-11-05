// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Reply stream. Default: the local canned reply (no network). Opt-in only
//! (GALION_LLM=openrouter): streaming chat completions via a `curl` child.
//! The key is read from OPENROUTER_API_KEY and handed to curl on stdin as
//! a config line, so it never appears in argv, logs, or output.
use crate::now_ms;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

#[derive(Debug)]
pub enum LlmEv { Thought(f64, String), Token(f64, String), Done(f64, Option<f64>, String) }

pub struct LlmCfg { pub url: String, pub model: String, pub max_tokens: u32, pub reasoning: Option<&'static str>, pub provider_order: Vec<String> }
impl LlmCfg {
    pub fn from_env() -> Self {
        Self {
            url: std::env::var("GALION_LLM_URL").unwrap_or_else(|_| "https://openrouter.ai/api/v1/chat/completions".into()),
            model: if canned() { "canned".into() } else { std::env::var("GALION_LLM_MODEL").unwrap_or_else(|_| "meta-llama/llama-3.1-8b-instruct".into()) },
            max_tokens: std::env::var("GALION_LLM_MAX_TOKENS").ok().and_then(|v| v.parse().ok()).unwrap_or(60),
            reasoning: None,
            provider_order: std::env::var("GALION_LLM_PROVIDER").map(|s| s.split(',').map(str::to_string).collect()).unwrap_or_default(),
        }
    }
}


/// Default reply path (PR #65 fix): canned. No network, no third-party LLM.
/// The cloud OpenRouter path runs only when GALION_LLM=openrouter is set
/// explicitly; unset, empty, `canned` or anything else stays canned. A local `sh` child
/// reads the request body to EOF, waits GALION_CANNED_TTFT_MS (default 0),
/// then streams a fixed OpenAI-style SSE reply with Llama-style " word"
/// tokens, so the rest of the loop (clauser, Piper, timing) is unchanged.
pub fn canned() -> bool { !matches!(std::env::var("GALION_LLM").as_deref().map(str::trim), Ok("openrouter")) }
pub const CANNED_REPLY: &str = "That sounds like a lovely idea, and I would be glad to help you plan it out step by step.";
pub fn canned_cmd() -> Command {
    let ttft: f64 = std::env::var("GALION_CANNED_TTFT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let mut script = String::from("cat >/dev/null; ");
    if ttft > 0.0 { script.push_str(&format!("sleep {:.3}; ", ttft / 1000.0)); }
    let mut first = true;
    for w in CANNED_REPLY.split(' ') {
        let tok = if first { w.to_string() } else { format!(" {w}") }; first = false;
        let line = serde_json::json!({"provider": "canned", "choices": [{"delta": {"content": tok}}]}).to_string();
        script.push_str(&format!("printf 'data: %s\\n' '{}'; ", line.replace('\'', "'\\''")));
    }
    script.push_str("printf 'data: [DONE]\\n'");
    let mut c = Command::new("sh"); c.arg("-c").arg(script); c
}

/// Start a streaming completion; events arrive on the returned channel.
pub fn stream(cfg: &LlmCfg, messages: serde_json::Value) -> Result<Receiver<LlmEv>, String> {
    if canned() {
        let mut child = canned_cmd().stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| format!("spawn canned: {e}"))?;
        let out = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let t = now_ms();
                let Some(d) = line.strip_prefix("data: ") else { continue };
                if d.trim() == "[DONE]" { break; }
                let Ok(v) = serde_json::from_str::<serde_json::Value>(d) else { continue };
                if let Some(c) = v["choices"][0]["delta"]["content"].as_str() { let _ = tx.send(LlmEv::Token(t, c.to_string())); }
            }
            let _ = child.wait();
            let _ = tx.send(LlmEv::Done(now_ms(), Some(0.0), "canned".into()));
        });
        let _ = messages; return Ok(rx);
    }
    let key = std::env::var("OPENROUTER_API_KEY").map_err(|_| "OPENROUTER_API_KEY not set; no LLM call made".to_string())?;
    let mut body = serde_json::json!({"model": cfg.model, "messages": messages, "stream": true,
        "max_tokens": cfg.max_tokens, "temperature": 0.7, "usage": {"include": true}});
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
        si.write_all(format!("header = \"Authorization: Bearer {key}\"\n").as_bytes()).map_err(|e| e.to_string())?;
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
    use super::Clauser;
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
