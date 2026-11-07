// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Client for the persistent out-of-process Chatterbox Nano clone worker
//! (LOCKS v2s14 exception: PyTorch CPU, out of process). The worker loads
//! once and caches speaker conditionals per persona. Protocol: one JSON
//! line in, one JSON line out (`{"ok":true,"pcm":path,...}`); 24 kHz s16 WAV.
use crate::now_ms;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct CloneWorker { child: Child, stdin: ChildStdin, out: BufReader<ChildStdout> }
impl CloneWorker {
    pub fn spawn() -> Result<Self, String> { Self::spawn_pinned(None, None) }
    /// `cpus` (taskset list, e.g. "0-3") and `threads` keep two voices from
    /// fighting over the same cores.
    pub fn spawn_pinned(cpus: Option<&str>, threads: Option<usize>) -> Result<Self, String> {
        let py = std::env::var("GALION_CLONE_PY").map_err(|_| "GALION_CLONE_PY unset (venv python for the Nano worker)".to_string())?;
        let script = std::env::var("GALION_CLONE_SCRIPT").map_err(|_| "GALION_CLONE_SCRIPT unset".to_string())?;
        let mut cmd = match cpus { Some(c) => { let mut k = Command::new("taskset"); k.arg("-c").arg(c).arg(&py); k } None => Command::new(&py) };
        cmd.arg(script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
        if let Some(t) = threads { let t = t.to_string(); for k in ["GALION_CLONE_THREADS", "OMP_NUM_THREADS", "MKL_NUM_THREADS"] { cmd.env(k, &t); } }
        let mut child = cmd.spawn().map_err(|e| format!("spawn clone worker: {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());
        let mut w = Self { child, stdin, out };
        let r = w.read()?;
        if r["ok"] != true { return Err(format!("clone worker failed to load: {r}")); }
        Ok(w)
    }
    fn read(&mut self) -> Result<serde_json::Value, String> {
        let mut l = String::new();
        self.out.read_line(&mut l).map_err(|e| e.to_string())?;
        if l.is_empty() { return Err("clone worker exited".into()); }
        serde_json::from_str(&l).map_err(|e| format!("bad worker line {l:?}: {e}"))
    }
    pub fn call(&mut self, req: serde_json::Value) -> Result<(serde_json::Value, f64), String> {
        let t = now_ms();
        self.stdin.write_all(format!("{req}\n").as_bytes()).and_then(|_| self.stdin.flush()).map_err(|e| e.to_string())?;
        let r = self.read()?;
        if r["ok"] != true { return Err(format!("clone worker: {r}")); }
        Ok((r, now_ms() - t))
    }
    pub fn condition(&mut self, voice: &str, reference: &str) -> Result<f64, String> {
        self.call(serde_json::json!({"op":"cond","voice":voice,"ref":reference})).map(|(_, ms)| ms)
    }
    /// Synthesize `text` in `voice`; PCM comes back on the pipe (no temp WAV).
    pub fn say_pcm(&mut self, voice: &str, text: &str) -> Result<(Vec<i16>, f64), String> {
        let (r, ms) = self.call(serde_json::json!({"op":"say","voice":voice,"text":text}))?;
        let n = r["pcm_bytes"].as_u64().ok_or("no pcm_bytes")? as usize;
        let mut b = vec![0u8; n];
        self.out.read_exact(&mut b).map_err(|e| e.to_string())?;
        Ok((b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect(), ms))
    }
    /// Synthesize `text` in `voice` via a WAV file; returns (24 kHz pcm, wall ms).
    pub fn say(&mut self, voice: &str, text: &str, out: &str) -> Result<(Vec<i16>, f64), String> {
        let (_, ms) = self.call(serde_json::json!({"op":"say","voice":voice,"text":text,"out":out}))?;
        let w = crate::audio::read_wav(out)?;
        Ok((crate::audio::to_mono(&w), ms))
    }
}
impl Drop for CloneWorker { fn drop(&mut self) { let _ = self.stdin.write_all(b"{\"op\":\"quit\"}\n"); let _ = self.child.wait(); } }
