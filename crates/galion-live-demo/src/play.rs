// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! FP-03 · local playback of the spoken reply, in sync with the dot.
//!
//! A [`FrameSink`]: on each me-band **onset** (the frame the dot turns *speaking*) it starts the
//! system player on the reply WAV (A21 Piper words, or the flite fallback); on a **yield /
//! barge-in** it stops it. The player is an existing desktop tool (`paplay` / `pw-play` /
//! `aplay` / `ffplay`), spawned out of process — no new crates, no audio element on the page.
//! Spawning happens in `on_frame`, after `Heartbeat::step`, never inside it.
//! Env: `GALION_PLAYER="cmd args"` overrides detection; `GALION_PLAY=0` disables.
use crate::{FrameRecord, FrameSink};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

const CANDIDATES: &[&[&str]] = &[
    &["paplay"],
    &["pw-play"],
    &["aplay", "-q"],
    &["ffplay", "-nodisp", "-autoexit", "-loglevel", "quiet"],
];

fn on_path(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

/// The player command line (without the file), or `None` if nothing can play.
pub fn detect_player() -> Option<Vec<String>> {
    if std::env::var("GALION_PLAY").is_ok_and(|v| v == "0") {
        return None;
    }
    if let Ok(c) = std::env::var("GALION_PLAYER") {
        let v: Vec<String> = c.split_whitespace().map(str::to_string).collect();
        return (!v.is_empty() && on_path(&v[0])).then_some(v);
    }
    CANDIDATES.iter().find(|c| on_path(c[0])).map(|c| c.iter().map(|s| s.to_string()).collect())
}

/// Plays `wav` on every me onset; stops on yield.
pub struct PlaySink {
    wav: PathBuf,
    cmd: Vec<String>,
    child: Option<Child>,
    prev_voiced: bool,
    pub starts: u32,
    pub stops: u32,
    pub errors: u32,
}

impl PlaySink {
    pub fn new(wav: &Path, cmd: Vec<String>) -> Self {
        Self { wav: wav.to_path_buf(), cmd, child: None, prev_voiced: false, starts: 0, stops: 0, errors: 0 }
    }
    pub fn player(&self) -> &str {
        &self.cmd[0]
    }
    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            if c.try_wait().ok().flatten().is_none() {
                let _ = c.kill();
                self.stops += 1;
            }
            let _ = c.wait();
        }
    }
}

impl FrameSink for PlaySink {
    fn on_frame(&mut self, r: &FrameRecord) {
        let onset = r.me_voiced && !self.prev_voiced && !r.yield_now;
        self.prev_voiced = r.me_voiced;
        if r.yield_now {
            self.stop(); // barge-in: the words stop with the dot
        }
        if let Some(c) = self.child.as_mut() {
            if c.try_wait().ok().flatten().is_some() {
                self.child = None; // finished; reaped
            }
        }
        if onset {
            self.stop();
            match Command::new(&self.cmd[0]).args(&self.cmd[1..]).arg(&self.wav).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
                Ok(c) => {
                    self.child = Some(c);
                    self.starts += 1;
                }
                Err(_) => self.errors += 1,
            }
        }
    }
}

impl Drop for PlaySink {
    fn drop(&mut self) {
        // Let the last reply finish playing (bounded), then reap.
        if let Some(mut c) = self.child.take() {
            for _ in 0..100 {
                if c.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
