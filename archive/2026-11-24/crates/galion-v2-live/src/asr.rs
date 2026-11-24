// Galion Voice — Perpetual Open MIT License
//! Client for the galion-v2-asr streaming sidecar.
use crate::now_ms;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

pub struct AsrEvent { pub t_ms: f64, pub fin: bool, pub text: String, /// true for the `S` sync ack (lockstep scans)
    pub sync: bool }
pub struct Asr { child: Child, stdin: ChildStdin, pub rx: Receiver<AsrEvent> }
impl Asr {
    pub fn spawn() -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let bin = exe.with_file_name("galion-v2-asr");
        let mut child = Command::new(&bin).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
            .spawn().map_err(|e| format!("spawn {}: {e}", bin.display()))?;
        let stdin = child.stdin.take().unwrap();
        let out = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        let (rtx, rrx) = channel();
        std::thread::spawn(move || {
            let mut ready = Some(rtx);
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let t = now_ms();
                let v: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
                match v["k"].as_str() {
                    Some("ready") => { if let Some(r) = ready.take() { let _ = r.send(()); } }
                    Some(k) => { let _ = tx.send(AsrEvent { t_ms: t, fin: k == "f", sync: k == "s", text: v["text"].as_str().unwrap_or("").to_string() }); }
                    None => {}
                }
            }
        });
        rrx.recv_timeout(std::time::Duration::from_secs(30)).map_err(|_| "vosk sidecar not ready (model/lib missing?)".to_string())?;
        Ok(Self { child, stdin, rx })
    }
    pub fn feed(&mut self, pcm16k: &[i16]) -> Result<(), String> {
        let mut b = Vec::with_capacity(3 + pcm16k.len() * 2);
        b.push(b'A'); b.extend_from_slice(&(pcm16k.len() as u16).to_le_bytes());
        for s in pcm16k { b.extend_from_slice(&s.to_le_bytes()); }
        self.stdin.write_all(&b).and_then(|_| self.stdin.flush()).map_err(|e| e.to_string())
    }
    /// Lockstep: ask the sidecar to ack after every frame fed so far (`{"k":"s"}`).
    pub fn sync(&mut self) -> Result<(), String> {
        self.stdin.write_all(b"S").and_then(|_| self.stdin.flush()).map_err(|e| e.to_string())
    }
    pub fn finalize(&mut self) -> Result<(), String> {
        self.stdin.write_all(b"F").and_then(|_| self.stdin.flush()).map_err(|e| e.to_string())
    }
}
impl Drop for Asr { fn drop(&mut self) { let _ = self.stdin.write_all(b"Q"); let _ = self.child.kill(); let _ = self.child.wait(); } }
