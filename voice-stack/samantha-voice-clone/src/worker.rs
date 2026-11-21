// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Process adapter. `s711_worker.py` is the Samantha child.
//! `run_repo_stub_probe` drives `scripts/clone/chatterbox_worker.py` itself
//! so the s711 exaggeration and cfg are shown to reach that process.
//! The repo child refuses the s711 reference prefix and refuses CUDA, so
//! expressive generate does not go through that entry point.

use crate::cache::{decode_s16le, repo_root};
use crate::device::{python_bin, s711_script};
use crate::error::TtsError;
use crate::recipe::{SentenceJob, CFG, EXAGGERATION, MODE, REF_SHA_PREFIX};
use crate::tts::{SentenceSpeaker, Spoken};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

const MAX_PCM_BYTES: usize = 24_000 * 120 * 2;

pub fn s711_worker_script() -> PathBuf {
    s711_script()
}

pub fn repo_chatterbox_worker() -> PathBuf {
    repo_root().join("scripts/clone/chatterbox_worker.py")
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerLog {
    pub mode: String,
    pub exaggeration: f64,
    pub cfg: f64,
    pub seed: u32,
    pub index: usize,
    pub chain: bool,
    pub raw: bool,
    pub rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub ref_prefix: String,
    pub device: String,
    pub ttfa_us: u128,
    pub pcm_bytes: usize,
}

pub fn parse_worker_log(log: &str) -> Result<WorkerLog, TtsError> {
    let mut mode = String::new();
    let mut exaggeration = None;
    let mut cfg = None;
    let mut seed = None;
    let mut index = None;
    let mut chain = None;
    let mut raw = false;
    let mut rate = None;
    let mut channels = None;
    let mut bits = None;
    let mut ref_prefix = String::new();
    let mut device = String::new();
    let mut ttfa_us = None;
    let mut pcm_bytes = None;
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix("galion-s711-recipe:") {
            for field in rest.split_whitespace() {
                if let Some((key, value)) = field.split_once('=') {
                    match key {
                        "mode" => mode = value.to_string(),
                        "exaggeration" => exaggeration = value.parse().ok(),
                        "cfg" => cfg = value.parse().ok(),
                        "seed" => seed = value.parse().ok(),
                        "index" => index = value.parse().ok(),
                        "chain" => chain = Some(value == "true"),
                        "ref_prefix" => ref_prefix = value.to_string(),
                        _ => {}
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("galion-s711-device:") {
            device = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("galion-s711-format:") {
            for field in rest.split_whitespace() {
                if let Some((key, value)) = field.split_once('=') {
                    match key {
                        "rate" => rate = value.parse().ok(),
                        "channels" => channels = value.parse().ok(),
                        "bits" => bits = value.parse().ok(),
                        "raw" => raw = value == "true",
                        _ => {}
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("galion-s711-ttfa-us:") {
            ttfa_us = rest.trim().parse().ok();
        } else if let Some(rest) = line.strip_prefix("galion-s711-utt:") {
            pcm_bytes = rest.trim().parse().ok();
        }
    }
    let missing = mode.is_empty()
        || exaggeration.is_none()
        || cfg.is_none()
        || seed.is_none()
        || index.is_none()
        || chain.is_none()
        || rate.is_none()
        || device.is_empty()
        || ttfa_us.is_none()
        || pcm_bytes.is_none()
        || ref_prefix.len() != REF_SHA_PREFIX.len();
    if missing {
        return Err(TtsError::new(
            "worker",
            format!("s711 worker log is incomplete: {log}"),
        ));
    }
    Ok(WorkerLog {
        mode,
        exaggeration: exaggeration.unwrap(),
        cfg: cfg.unwrap(),
        seed: seed.unwrap(),
        index: index.unwrap(),
        chain: chain.unwrap(),
        raw,
        rate: rate.unwrap(),
        channels: channels.unwrap_or(0),
        bits: bits.unwrap_or(0),
        ref_prefix,
        device,
        ttfa_us: ttfa_us.unwrap(),
        pcm_bytes: pcm_bytes.unwrap(),
    })
}

pub struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: BufReader<ChildStderr>,
    stub: bool,
    device: String,
}

impl WorkerProcess {
    pub fn spawn(device: &str, stub: bool) -> Result<Self, TtsError> {
        let script = s711_worker_script();
        if !script.is_file() {
            return Err(TtsError::new(
                "worker",
                format!("s711 worker missing at {}", script.display()),
            ));
        }
        let mut cmd = Command::new(python_bin());
        cmd.arg(&script)
            .current_dir(repo_root())
            .env("GALION_SAMANTHA_DEVICE", device)
            .env("GALION_SAMANTHA_STUB", if stub { "1" } else { "0" })
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if device == "cpu" || stub {
            cmd.env("CUDA_VISIBLE_DEVICES", "");
        }
        let mut child = cmd
            .spawn()
            .map_err(|err| TtsError::new("worker", format!("spawn {}: {err}", python_bin())))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TtsError::new("worker", "worker stdin missing"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TtsError::new("worker", "worker stdout missing"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| TtsError::new("worker", "worker stderr missing"))?;
        Ok(Self {
            child,
            stdin,
            stdout,
            stderr: BufReader::new(stderr),
            stub,
            device: device.to_string(),
        })
    }

    fn ensure(&mut self) -> Result<(), TtsError> {
        match self.child.try_wait() {
            Ok(None) => Ok(()),
            _ => {
                let device = self.device.clone();
                let stub = self.stub;
                *self = Self::spawn(&device, stub)?;
                Ok(())
            }
        }
    }

    pub fn exchange(&mut self, line: &str) -> Result<(Vec<u8>, String), TtsError> {
        self.ensure()?;
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|err| TtsError::new("worker", format!("stdin: {err}")))?;
        let mut log = String::new();
        for _ in 0..100_000 {
            let mut err_line = String::new();
            let n = self
                .stderr
                .read_line(&mut err_line)
                .map_err(|err| TtsError::new("worker", format!("stderr: {err}")))?;
            if n == 0 {
                return Err(TtsError::new(
                    "worker",
                    format!("worker closed before an utterance: {log}"),
                ));
            }
            log.push_str(&err_line);
            if err_line.starts_with("galion-s711-refuse:")
                || err_line.starts_with("galion-s711-absent:")
            {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(TtsError::new("worker", log));
            }
            if let Some(rest) = err_line.strip_prefix("galion-s711-utt:") {
                let nbytes: usize = rest.trim().parse().map_err(|_| {
                    TtsError::new("worker", format!("bad utterance length in {err_line}"))
                })?;
                if nbytes > MAX_PCM_BYTES {
                    let _ = self.child.kill();
                    return Err(TtsError::new(
                        "worker",
                        format!("utterance is {nbytes} bytes"),
                    ));
                }
                let mut pcm = vec![0u8; nbytes];
                if nbytes > 0 {
                    self.stdout
                        .read_exact(&mut pcm)
                        .map_err(|err| TtsError::new("worker", format!("stdout: {err}")))?;
                }
                return Ok((pcm, log));
            }
        }
        let _ = self.child.kill();
        Err(TtsError::new("worker", "worker log exceeded 100000 lines"))
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct WorkerSpeaker {
    process: WorkerProcess,
}

impl WorkerSpeaker {
    pub fn spawn(device: &str, stub: bool) -> Result<Self, TtsError> {
        Ok(Self {
            process: WorkerProcess::spawn(device, stub)?,
        })
    }
}

impl SentenceSpeaker for WorkerSpeaker {
    fn speak(&mut self, job: &SentenceJob) -> Result<Spoken, TtsError> {
        let line = job.to_json_line()?;
        let (pcm, log) = self.process.exchange(&line)?;
        let parsed = parse_worker_log(&log)?;
        if parsed.mode != MODE {
            return Err(TtsError::new("worker", format!("mode {}", parsed.mode)));
        }
        if (parsed.exaggeration - EXAGGERATION).abs() > 1e-6 || (parsed.cfg - CFG).abs() > 1e-6 {
            return Err(TtsError::new(
                "worker",
                "worker did not echo the s711 knobs",
            ));
        }
        if parsed.seed != job.seed || parsed.index != job.index || parsed.chain || !parsed.raw {
            return Err(TtsError::new(
                "worker",
                format!("recipe echo mismatch: {log}"),
            ));
        }
        if parsed.rate != 24_000 || parsed.channels != 1 || parsed.bits != 16 {
            return Err(TtsError::new(
                "worker",
                format!("format echo mismatch: {log}"),
            ));
        }
        let expect = &job.reference_sha256[..REF_SHA_PREFIX.len()];
        if !parsed.ref_prefix.eq_ignore_ascii_case(expect) {
            return Err(TtsError::new(
                "worker",
                format!(
                    "reference prefix {} does not match {expect}",
                    parsed.ref_prefix
                ),
            ));
        }
        if parsed.pcm_bytes != pcm.len() {
            return Err(TtsError::new(
                "worker",
                "utterance byte count does not match pcm",
            ));
        }
        let samples = decode_s16le(&pcm)?;
        if samples.is_empty() {
            return Err(TtsError::new("worker", "worker returned no samples"));
        }
        Ok(Spoken {
            samples,
            device: parsed.device,
            worker_ttfa_us: Some(parsed.ttfa_us),
            from_cache: false,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfCheck {
    pub mode: String,
    pub exaggeration: String,
    pub cfg: String,
    pub seed0: u32,
    pub seed1: u32,
    pub prefix_ok: bool,
    pub prefix_bad: bool,
    pub chain: String,
    pub train: String,
    pub wrong_seed: String,
    pub reuse: String,
    pub stub_samples: usize,
    pub stub_ttfa_us: u128,
    pub resample_out: usize,
    pub device_probe: String,
    pub device_note: String,
    pub consent: String,
    pub wall_us: u128,
}

pub fn parse_self_check(text: &str, wall_us: u128) -> Result<SelfCheck, TtsError> {
    let mut map = std::collections::BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    let need = |key: &str| -> Result<String, TtsError> {
        map.get(key)
            .cloned()
            .ok_or_else(|| TtsError::new("worker", format!("self-check missing {key}: {text}")))
    };
    let parse_u32 = |key: &str| -> Result<u32, TtsError> {
        need(key)?
            .parse()
            .map_err(|_| TtsError::new("worker", format!("self-check bad {key}")))
    };
    let parse_u128 = |key: &str| -> Result<u128, TtsError> {
        need(key)?
            .parse()
            .map_err(|_| TtsError::new("worker", format!("self-check bad {key}")))
    };
    let parse_usize = |key: &str| -> Result<usize, TtsError> {
        need(key)?
            .parse()
            .map_err(|_| TtsError::new("worker", format!("self-check bad {key}")))
    };
    Ok(SelfCheck {
        mode: need("mode")?,
        exaggeration: need("exaggeration")?,
        cfg: need("cfg")?,
        seed0: parse_u32("seed0")?,
        seed1: parse_u32("seed1")?,
        prefix_ok: need("prefix_ok")? == "true",
        prefix_bad: need("prefix_bad")? == "true",
        chain: need("chain")?,
        train: need("train")?,
        wrong_seed: need("wrong_seed")?,
        reuse: need("reuse")?,
        stub_samples: parse_usize("stub_samples")?,
        stub_ttfa_us: parse_u128("stub_ttfa_us")?,
        resample_out: parse_usize("resample_out")?,
        device_probe: need("device_probe")?,
        device_note: need("device_note")?,
        consent: need("consent")?,
        wall_us,
    })
}

pub fn run_self_check() -> Result<SelfCheck, TtsError> {
    let script = s711_worker_script();
    let started = Instant::now();
    let output = Command::new(python_bin())
        .arg(&script)
        .arg("--self-check")
        .current_dir(repo_root())
        .env("PYTHONUTF8", "1")
        .output()
        .map_err(|err| TtsError::new("worker", format!("self-check spawn: {err}")))?;
    let wall_us = started.elapsed().as_micros();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(TtsError::new(
            "worker",
            format!("self-check exited {}: {stdout}{stderr}", output.status),
        ));
    }
    parse_self_check(&stdout, wall_us)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoStubProbe {
    pub pcm_bytes: usize,
    pub first_sample: i16,
    pub controls_line: String,
    pub wall_us: u128,
    pub exit_code: i32,
}

/// One protocol utterance on the repo Chatterbox worker, stub PCM, no reference.
/// The s711 knobs are on the JSON line. This does not load a model.
pub fn run_repo_stub_probe() -> Result<RepoStubProbe, TtsError> {
    let script = repo_chatterbox_worker();
    if !script.is_file() {
        return Err(TtsError::new(
            "worker",
            format!("chatterbox worker missing at {}", script.display()),
        ));
    }
    let line = serde_json::json!({
        "text": "Hello.",
        "model": "nano",
        "persona": "samantha",
        "tags": true,
        "device": "cpu",
        "denoise": false,
        "exaggeration": EXAGGERATION,
        "cfg_weight": CFG,
        "temperature": 0.8,
        "prosody": {
            "pitch_st": 0.0,
            "range": 1.0,
            "rate": 1.0,
            "energy": 1.0,
            "warmth": 0.0,
            "pre_pause_ms": 0
        }
    });
    let started = Instant::now();
    let mut child = Command::new(python_bin())
        .arg(&script)
        .current_dir(repo_root())
        .env("GALION_CLONE_STUB_PCM", "1")
        .env("GALION_CLONE_STUB_FRAMES", "1")
        .env("GALION_CLONE_DEVICE", "cpu")
        .env("CUDA_VISIBLE_DEVICES", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| TtsError::new("worker", format!("spawn chatterbox worker: {err}")))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| TtsError::new("worker", "chatterbox stdin missing"))?;
        stdin
            .write_all(line.to_string().as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .map_err(|err| TtsError::new("worker", format!("chatterbox stdin: {err}")))?;
    }
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| TtsError::new("worker", "chatterbox stdout missing"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| TtsError::new("worker", "chatterbox stderr missing"))?;
    let out_handle = std::thread::spawn(move || {
        let mut pcm = Vec::new();
        let _ = stdout.read_to_end(&mut pcm);
        pcm
    });
    let err_handle = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let status = child
        .wait_timeout_like()
        .map_err(|err| TtsError::new("worker", format!("chatterbox wait: {err}")))?;
    let pcm = out_handle
        .join()
        .map_err(|_| TtsError::new("worker", "chatterbox stdout thread"))?;
    let err = err_handle
        .join()
        .map_err(|_| TtsError::new("worker", "chatterbox stderr thread"))?;
    let wall_us = started.elapsed().as_micros();
    if !status.success() {
        return Err(TtsError::new(
            "worker",
            format!("chatterbox stub exited {status}: {err}"),
        ));
    }
    let controls_line = err
        .lines()
        .find(|line| line.starts_with("galion-clone-controls:"))
        .unwrap_or("")
        .to_string();
    if pcm.len() < 2 {
        return Err(TtsError::new(
            "worker",
            format!("chatterbox stub returned no pcm: {err}"),
        ));
    }
    let first_sample = i16::from_le_bytes([pcm[0], pcm[1]]);
    Ok(RepoStubProbe {
        pcm_bytes: pcm.len(),
        first_sample,
        controls_line,
        wall_us,
        exit_code: status.code().unwrap_or(-1),
    })
}

trait WaitStatus {
    fn wait_timeout_like(&mut self) -> std::io::Result<std::process::ExitStatus>;
}

impl WaitStatus for Child {
    fn wait_timeout_like(&mut self) -> std::io::Result<std::process::ExitStatus> {
        let started = Instant::now();
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            if started.elapsed() > Duration::from_secs(30) {
                let _ = self.kill();
                return self.wait();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_log_parser_reads_the_recipe_echo() {
        let log = "\
galion-s711-recipe: mode=expr exaggeration=0.5000 cfg=0.5000 seed=711 index=0 chain=false ref_prefix=10ac44dc
galion-s711-device: stub
galion-s711-format: rate=24000 channels=1 bits=16 encoding=s16le raw=true
galion-s711-ttfa-us: 40
galion-s711-utt: 3840
";
        let parsed = parse_worker_log(log).unwrap();
        assert_eq!(parsed.mode, "expr");
        assert!((parsed.exaggeration - 0.5).abs() < 1e-9);
        assert!((parsed.cfg - 0.5).abs() < 1e-9);
        assert_eq!(parsed.seed, 711);
        assert!(!parsed.chain);
        assert!(parsed.raw);
        assert_eq!(parsed.rate, 24_000);
        assert_eq!(parsed.ref_prefix, "10ac44dc");
        assert_eq!(parsed.ttfa_us, 40);
        assert_eq!(parsed.pcm_bytes, 3840);
    }
}
