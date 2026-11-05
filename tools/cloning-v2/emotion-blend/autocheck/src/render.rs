// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! Serialised TTS via flock on emotion-blend/render.lock + cbx-speak-voice.

use anyhow::{bail, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::Paths;

pub struct RenderRequest<'a> {
    pub voice: &'a str,
    pub line_id: &'a str,
    pub text: &'a str,
    pub out_wav: &'a Path,
    pub temperature: f64,
    pub seed: u64,
}

fn flock_exclusive(lock_path: &Path) -> Result<File> {
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .with_context(|| format!("open lock {}", lock_path.display()))?;
    // libc flock via flock(1) helper to avoid extra crate deps under OOM.
    // We hold the file open; acquisition uses `flock` binary.
    let status = Command::new("flock")
        .args(["--exclusive", "--timeout", "3600"])
        .arg(lock_path)
        .arg("-c")
        .arg("true")
        .status()
        .context("flock binary")?;
    if !status.success() {
        bail!("failed to acquire render.lock at {}", lock_path.display());
    }
    // Re-acquire for the lifetime of this File using Python/fcntl fallback if flock -c doesn't hold.
    // Prefer holding via a child that we keep? Simpler: use flock wrapping the speak command itself.
    let _ = f;
    Ok(OpenOptions::new()
        .create(true)
        .append(true)
        .open(lock_path)?)
}

/// Render one line under the box-wide render lock. Uses `flock` around the speak invocation.
pub fn render_line(paths: &Paths, req: &RenderRequest<'_>) -> Result<()> {
    if let Some(parent) = req.out_wav.parent() {
        fs::create_dir_all(parent)?;
    }
    let speak = paths.speak.display().to_string();
    let out = req.out_wav.display().to_string();
    let lock = paths.render_lock.display().to_string();
    // flock holds the lock for the duration of the inner shell command.
    let inner = format!(
        "nice -n 10 env CBX_INTRA_THREADS=2 {speak} --voice {voice} --text {text} --temperature {temp} --seed {seed} --dtype fp32 --out-wav {out}",
        speak = shell_quote(&speak),
        voice = shell_quote(req.voice),
        text = shell_quote(req.text),
        temp = req.temperature,
        seed = req.seed,
        out = shell_quote(&out),
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let log_line = format!(
        "{stamp} RENDER start voice={} line={} out={}\n",
        req.voice,
        req.line_id,
        req.out_wav.display()
    );
    let _ = flock_exclusive(&paths.render_lock); // touch lock file
    {
        let mut lf = OpenOptions::new()
            .create(true)
            .append(true)
            .open(paths.work.join("render.log"))
            .unwrap_or_else(|_| File::create("/dev/null").unwrap());
        let _ = lf.write_all(log_line.as_bytes());
    }
    let status = Command::new("flock")
        .args(["--exclusive", "--timeout", "7200"])
        .arg(&lock)
        .arg("bash")
        .arg("-lc")
        .arg(&inner)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .context("spawn flock+speak")?;
    if !status.success() {
        bail!(
            "render failed for {} {}: exit {:?}",
            req.voice,
            req.line_id,
            status.code()
        );
    }
    if !req.out_wav.is_file() {
        bail!("render claimed success but missing {}", req.out_wav.display());
    }
    Ok(())
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn render_set(
    paths: &Paths,
    voice: &str,
    out_dir: &Path,
    line_ids: &[String],
    temperature: f64,
) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(out_dir)?;
    let mut wavs = Vec::new();
    for (i, id) in line_ids.iter().enumerate() {
        let text = paths.line_text(id)?;
        let out = out_dir.join(format!("{id}.wav"));
        render_line(
            paths,
            &RenderRequest {
                voice,
                line_id: id,
                text: &text,
                out_wav: &out,
                temperature,
                seed: i as u64,
            },
        )?;
        wavs.push(out);
    }
    Ok(wavs)
}
