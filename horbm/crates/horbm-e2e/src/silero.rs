// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Silero VAD v5 through `voice_activity_detector` (MIT).
//!
//! The crate can build a session from bytes it embeds. This module does not
//! use those bytes. It downloads the official ONNX named in `silero.lock`,
//! checks the sha256, and passes that file to the crate as the session.
//! The weights are not committed.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpu_time::ThreadTime;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use sha2::{Digest, Sha256};
use voice_activity_detector::VoiceActivityDetector;

use crate::percentile_nearest;
use crate::vad::{FrameCounts, FRAME_SAMPLES};

const WINDOW: usize = 512;
const RATE: i64 = 16_000;
/// Silero's published operating point. Same number as `voice-stack/vad`.
const VOICED: f32 = 0.5;
const DOWNLOAD_CAP: u64 = 20 * 1024 * 1024;

struct ModelLock {
    url: String,
    sha256: String,
    license: String,
}

pub struct ModelTimes {
    pub counts: FrameCounts,
    pub frame_cpu_p50_ns: Option<u64>,
    pub frame_cpu_p95_ns: Option<u64>,
    pub model_cpu_p50_ns: Option<u64>,
    pub model_cpu_p95_ns: Option<u64>,
    pub warm_cpu_ns: Option<u64>,
    pub sha256: String,
}

pub fn ensure_model() -> Result<PathBuf, String> {
    let lock = read_lock()?;
    if lock.license != "MIT" && lock.license != "Apache-2.0" {
        return Err(format!(
            "silero model license is {}, want MIT or Apache-2.0",
            lock.license
        ));
    }
    let dest = cache_file();
    if file_matches(&dest, &lock.sha256)? {
        return Ok(dest);
    }
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("silero cache: {err}"))?;
    }
    let partial = dest.with_extension("partial");
    download(&lock.url, &partial)?;
    let got = sha256_file(&partial)?;
    if got != lock.sha256 {
        let _ = fs::remove_file(&partial);
        return Err(format!("silero sha256 mismatch: got {got}"));
    }
    fs::rename(&partial, &dest).map_err(|err| format!("silero cache rename: {err}"))?;
    Ok(dest)
}

pub fn score_file(pcm: &[i16], model: &Path) -> Result<ModelTimes, String> {
    let lock = read_lock()?;
    if !file_matches(model, &lock.sha256)? {
        return Err("silero model sha256 does not match silero.lock".into());
    }
    let mut vad = load(model)?;
    let warm_cpu_ns = warmup(&mut vad);
    vad.reset();
    let mut pending = Vec::with_capacity(WINDOW);
    let mut last = 0.0f32;
    let mut counts = FrameCounts {
        voiced: 0,
        quiet: 0,
    };
    let mut frame_cpu = Vec::new();
    let mut model_cpu = Vec::new();
    for frame in pcm.chunks_exact(FRAME_SAMPLES) {
        let started = ThreadTime::try_now().ok();
        pending.extend_from_slice(frame);
        if pending.len() >= WINDOW {
            let window: Vec<i16> = pending.drain(..WINDOW).collect();
            let model_started = ThreadTime::try_now().ok();
            last = vad.predict(window);
            if let Some(mark) = model_started {
                model_cpu.push(nanos(mark.elapsed()));
            }
        }
        if let Some(mark) = started {
            frame_cpu.push(nanos(mark.elapsed()));
        }
        if last > VOICED {
            counts.voiced = counts.voiced.saturating_add(1);
        } else {
            counts.quiet = counts.quiet.saturating_add(1);
        }
    }
    Ok(ModelTimes {
        counts,
        frame_cpu_p50_ns: pct(&frame_cpu, 0.50),
        frame_cpu_p95_ns: pct(&frame_cpu, 0.95),
        model_cpu_p50_ns: pct(&model_cpu, 0.50),
        model_cpu_p95_ns: pct(&model_cpu, 0.95),
        warm_cpu_ns,
        sha256: lock.sha256,
    })
}

fn load(path: &Path) -> Result<VoiceActivityDetector, String> {
    let session = Session::builder()
        .map_err(|err| err.to_string())?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|err| err.to_string())?
        .with_intra_threads(1)
        .map_err(|err| err.to_string())?
        .with_inter_threads(1)
        .map_err(|err| err.to_string())?
        .commit_from_file(path)
        .map_err(|err| err.to_string())?;
    VoiceActivityDetector::builder()
        .sample_rate(RATE)
        .chunk_size(WINDOW)
        .session(Arc::new(Mutex::new(session)))
        .build()
        .map_err(|err| err.to_string())
}

fn warmup(vad: &mut VoiceActivityDetector) -> Option<u64> {
    let started = ThreadTime::try_now().ok()?;
    let _ = vad.predict(std::iter::repeat(0i16).take(WINDOW));
    Some(nanos(started.elapsed()))
}

fn pct(samples: &[u64], quantile: f64) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut copy = samples.to_vec();
    copy.sort_unstable();
    percentile_nearest(&copy, quantile)
}

fn nanos(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
}

fn read_lock() -> Result<ModelLock, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("silero.lock");
    let text = fs::read_to_string(&path).map_err(|err| format!("silero.lock: {err}"))?;
    let mut url = None;
    let mut sha256 = None;
    let mut license = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("").trim();
        if value.is_empty() {
            return Err(format!("silero.lock has an empty {key}"));
        }
        match key {
            "url" => url = Some(value.to_string()),
            "sha256" => sha256 = Some(value.to_ascii_lowercase()),
            "license" => license = Some(value.to_string()),
            other => return Err(format!("silero.lock has an unknown key {other}")),
        }
    }
    let sha256 = sha256.ok_or("silero.lock has no sha256")?;
    if sha256.len() != 64 || !sha256.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("silero.lock sha256 is not 64 hex digits".into());
    }
    Ok(ModelLock {
        url: url.ok_or("silero.lock has no url")?,
        sha256,
        license: license.ok_or("silero.lock has no license")?,
    })
}

fn cache_file() -> PathBuf {
    if let Ok(path) = std::env::var("GALION_SILERO_VAD") {
        let path = path.trim();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    cache_dir().join("silero_vad.onnx")
}

fn cache_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(xdg).join("horbm-e2e");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(home).join(".cache").join("horbm-e2e");
    }
    if let Some(home) = std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
        return PathBuf::from(home).join(".cache").join("horbm-e2e");
    }
    std::env::temp_dir().join("horbm-e2e-cache")
}

fn file_matches(path: &Path, expect: &str) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }
    Ok(sha256_file(path)? == expect)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|err| format!("read {}: {err}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    out
}

fn download(url: &str, dest: &Path) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(60))
        .build();
    let response = agent
        .get(url)
        .call()
        .map_err(|err| format!("silero download: {err}"))?;
    let mut reader = response.into_reader().take(DOWNLOAD_CAP);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|err| format!("silero download read: {err}"))?;
    if bytes.len() < 1024 {
        return Err("silero download was too small to be the model".into());
    }
    let mut file = File::create(dest).map_err(|err| format!("silero partial: {err}"))?;
    file.write_all(&bytes)
        .map_err(|err| format!("silero partial: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_names_the_official_v5_model() {
        let lock = read_lock().unwrap();
        assert_eq!(
            lock.sha256,
            "2623a2953f6ff3d2c1e61740c6cdb7168133479b267dfef114a4a3cc5bdd788f"
        );
        assert_eq!(lock.license, "MIT");
        assert!(lock.url.contains("silero-vad/v5.1.2/"));
        assert!(lock.url.ends_with("silero_vad.onnx"));
    }

    #[test]
    fn silence_is_quiet_and_frame_cpu_is_measured() {
        let path = ensure_model().expect("silero model");
        let silence = vec![0i16; FRAME_SAMPLES * 50];
        let scored = score_file(&silence, &path).expect("score");
        assert!(
            scored.counts.quiet > scored.counts.voiced,
            "silence voiced {} quiet {}",
            scored.counts.voiced,
            scored.counts.quiet
        );
        let p50 = scored.frame_cpu_p50_ns.expect("p50");
        let p95 = scored.frame_cpu_p95_ns.expect("p95");
        assert!(p50 <= p95, "frame cpu p50 {p50} p95 {p95}");
        let model_p50 = scored.model_cpu_p50_ns.expect("model p50");
        let model_p95 = scored.model_cpu_p95_ns.expect("model p95");
        assert!(
            model_p50 <= model_p95,
            "model cpu p50 {model_p50} p95 {model_p95}"
        );
        assert!(scored.warm_cpu_ns.is_some());
    }
}
