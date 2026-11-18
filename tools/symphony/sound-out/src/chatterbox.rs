// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Adapter for `scripts/clone/chatterbox_worker.py`.
//!
//! The default process is `GALION_CLONE_STUB_PCM=1`. It does not load torch
//! and it does not download weights. The request line carries exaggeration,
//! cfg_weight, temperature, and prosody. The worker applies them.
//!
//! `ChatterboxBackend::real` refuses to start when the pinned safetensors
//! are not already on disk. It never fetches them.

use crate::backend::{emit_frames, Capabilities, ClonerBackend, PcmChunk, SpeakMeta};
use crate::request::CloneRequest;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

pub struct ChatterboxBackend {
    real: bool,
    script: PathBuf,
    python: String,
}

impl ChatterboxBackend {
    /// Real Nano when the pinned pack is already on disk, otherwise the protocol stub.
    /// A missing pack is not downloaded.
    pub fn present() -> Self {
        Self::real().unwrap_or_else(|_| Self::stub())
    }

    pub fn stub() -> Self {
        Self {
            real: false,
            script: worker_script(),
            python: "python3".into(),
        }
    }

    /// Real Nano, only when the pinned files are already on disk.
    /// A missing pack is an error. Nothing is downloaded.
    pub fn real() -> Result<Self, String> {
        let root = weights_root();
        if !weights_present(&root) {
            return Err(format!(
                "chatterbox weights are not on disk at {} (not downloaded)",
                root.display()
            ));
        }
        Ok(Self {
            real: true,
            script: worker_script(),
            python: "python3".into(),
        })
    }
}

impl ClonerBackend for ChatterboxBackend {
    fn id(&self) -> &'static str {
        if self.real {
            "chatterbox-nano"
        } else {
            "chatterbox-stub"
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            // Nano generate on CPU is not the realtime path. Piper is.
            realtime_on_cpu: false,
            streaming: true,
            weights_used: self.real,
            stub: !self.real,
            note: "The worker process stays on CPU. GPU auto-detect does not cross that landing."
                .into(),
        }
    }

    fn speak_stream(
        &mut self,
        request: &CloneRequest,
        on_chunk: &mut dyn FnMut(PcmChunk) -> Result<(), String>,
    ) -> Result<SpeakMeta, String> {
        if !self.script.is_file() {
            return Err(format!(
                "chatterbox worker missing at {}",
                self.script.display()
            ));
        }
        let line = request.clone().normalized().worker_line();
        let mut cmd = Command::new(&self.python);
        cmd.arg(&self.script)
            .env("GALION_CLONE_DEVICE", "cpu")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if self.real {
            cmd.env("GALION_CLONE_STUB_PCM", "0");
        } else {
            cmd.env("GALION_CLONE_STUB_PCM", "1");
            cmd.env("GALION_CLONE_STUB_FRAMES", "4");
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("spawn {}: {e}", self.python))?;
        {
            let mut stdin = child.stdin.take().ok_or("worker stdin missing")?;
            stdin
                .write_all(line.as_bytes())
                .and_then(|_| stdin.write_all(b"\n"))
                .map_err(|e| format!("worker stdin: {e}"))?;
        }
        let mut stderr = child.stderr.take().ok_or("worker stderr missing")?;
        let err_handle = thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s);
            s
        });
        let mut stdout = child.stdout.take().ok_or("worker stdout missing")?;
        let mut pcm = Vec::new();
        stdout
            .read_to_end(&mut pcm)
            .map_err(|e| format!("worker stdout: {e}"))?;
        let status = child.wait().map_err(|e| format!("worker wait: {e}"))?;
        let err = err_handle.join().unwrap_or_default();
        if !status.success() {
            return Err(format!("chatterbox worker exited {status}: {}", err.trim()));
        }
        if err.contains("galion-clone-refuse") || err.contains("galion-clone-absent") {
            return Err(err.trim().to_string());
        }
        let samples = s16le_to_f64(&pcm);
        emit_frames(&samples, on_chunk)?;
        let echo = err
            .lines()
            .find(|l| l.starts_with("galion-clone-controls:"))
            .map(|s| s.to_string());
        Ok(SpeakMeta {
            backend: self.id().into(),
            weights_used: self.real,
            stub: !self.real,
            realtime_on_cpu: false,
            controls_echo: echo,
            note: if self.real {
                "chatterbox nano, weights already on disk".into()
            } else {
                "GALION_CLONE_STUB_PCM=1, no weights".into()
            },
        })
    }
}

fn worker_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/clone/chatterbox_worker.py")
}

fn weights_root() -> PathBuf {
    if let Ok(root) = std::env::var("GALION_CLONE_WEIGHTS") {
        let root = root.trim();
        if !root.is_empty() {
            return PathBuf::from(root);
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../weights/clone")
}

fn weights_present(root: &Path) -> bool {
    const RELS: &[&str] = &[
        "shared/s3gen.safetensors",
        "shared/s3gen_meanflow.safetensors",
        "shared/ve.safetensors",
        "nano/t3_nano_v1.safetensors",
    ];
    RELS.iter().all(|rel| root.join(rel).is_file())
}

fn s16le_to_f64(pcm: &[u8]) -> Vec<f64> {
    let n = pcm.len() / 2;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let sample = i16::from_le_bytes([pcm[i * 2], pcm[i * 2 + 1]]);
        out.push(sample as f64 / 32768.0);
    }
    out
}
