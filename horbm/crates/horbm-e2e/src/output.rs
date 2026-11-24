// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Where spoken samples go.
//!
//! The loopback buffer is the default and the fallback. On Linux, with the
//! `alsa` feature, a cpal stream on the ALSA host is tried first. A machine
//! with no card gets ALSA's null PCM through a temporary asoundrc, which is
//! the same plugin CI can use when `snd-dummy` is not loaded. If that open
//! fails, the loopback buffer is what the report names.

use std::env;
#[cfg(all(feature = "alsa", target_os = "linux"))]
use std::fs;
#[cfg(all(feature = "alsa", target_os = "linux"))]
use std::path::PathBuf;
#[cfg(all(feature = "alsa", target_os = "linux"))]
use std::sync::OnceLock;
#[cfg(all(feature = "alsa", target_os = "linux"))]
use std::time::Instant;

#[cfg(all(feature = "alsa", target_os = "linux"))]
use horbm_voice::SAMPLE_RATE;

use crate::buffer::Loopback;

pub struct Output {
    kind: Kind,
}

enum Kind {
    Loopback(Loopback),
    #[cfg(all(feature = "alsa", target_os = "linux"))]
    Alsa(Box<AlsaOut>),
}

#[derive(Debug)]
pub struct OpenError(pub String);

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Output {
    pub fn loopback() -> Self {
        Self {
            kind: Kind::Loopback(Loopback::default()),
        }
    }

    /// ALSA when this build and this machine can open it. Otherwise loopback.
    pub fn open() -> Self {
        if forced_loopback() {
            return Self::loopback();
        }
        #[cfg(all(feature = "alsa", target_os = "linux"))]
        if let Ok(output) = Self::open_alsa() {
            return output;
        }
        Self::loopback()
    }

    #[cfg(all(feature = "alsa", target_os = "linux"))]
    pub fn open_alsa() -> Result<Self, OpenError> {
        AlsaOut::open().map(|alsa| Self {
            kind: Kind::Alsa(Box::new(alsa)),
        })
    }

    pub fn name(&self) -> &'static str {
        match &self.kind {
            Kind::Loopback(_) => "loopback",
            #[cfg(all(feature = "alsa", target_os = "linux"))]
            Kind::Alsa(_) => "alsa",
        }
    }

    pub fn device(&self) -> Option<&str> {
        match &self.kind {
            Kind::Loopback(_) => None,
            #[cfg(all(feature = "alsa", target_os = "linux"))]
            Kind::Alsa(alsa) => Some(alsa.device.as_str()),
        }
    }

    pub fn push(&mut self, pcm: &[i16]) -> Result<(), String> {
        match &mut self.kind {
            Kind::Loopback(loopback) => {
                loopback.push(pcm);
                Ok(())
            }
            #[cfg(all(feature = "alsa", target_os = "linux"))]
            Kind::Alsa(alsa) => alsa.push(pcm),
        }
    }

    pub fn queued(&self) -> usize {
        match &self.kind {
            Kind::Loopback(loopback) => loopback.queued(),
            #[cfg(all(feature = "alsa", target_os = "linux"))]
            Kind::Alsa(alsa) => alsa.queued(),
        }
    }

    /// Drop whatever is still queued and time that drop through one silent
    /// render. `None` when nothing was queued.
    pub fn barge(&mut self) -> Option<u64> {
        match &mut self.kind {
            Kind::Loopback(loopback) => loopback.barge(),
            #[cfg(all(feature = "alsa", target_os = "linux"))]
            Kind::Alsa(alsa) => alsa.barge(),
        }
    }
}

fn forced_loopback() -> bool {
    env::var("GALION_E2E_OUTPUT")
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("loopback"))
        .unwrap_or(false)
}

#[cfg(all(feature = "alsa", target_os = "linux"))]
struct AlsaOut {
    session: galion_sam_audio_io::AudioSession,
    device: String,
    /// Samples not yet handed to the device. ALSA's null PCM ignores
    /// `snd_pcm_pause` and drains a playing ring immediately, so the queue
    /// lives here until barge-in writes it and flushes it in one step.
    held: Vec<i16>,
}

#[cfg(all(feature = "alsa", target_os = "linux"))]
impl AlsaOut {
    fn open() -> Result<Self, OpenError> {
        prepare_null_if_headless().map_err(OpenError)?;
        let session = galion_sam_audio_io::open(&galion_sam_audio_io::SessionConfig {
            input_name: None,
            output_name: env::var("GALION_AUDIO_OUT")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            direction: galion_sam_audio_io::Direction::Playback,
            // Eight seconds at 24 kHz. A short reply fits without a partial write.
            playback_samples: 24_000 * 8,
        })
        .map_err(|err| OpenError(err.to_string()))?;
        if !session.streams_open().1 {
            return Err(OpenError("alsa output stream did not open".into()));
        }
        if session.info().host != "alsa" {
            return Err(OpenError(format!(
                "audio host is {}, want alsa",
                session.info().host
            )));
        }
        let device = session
            .info()
            .output_name
            .clone()
            .unwrap_or_else(|| "default".into());
        Ok(Self {
            session,
            device,
            held: Vec::new(),
        })
    }

    fn push(&mut self, pcm: &[i16]) -> Result<(), String> {
        self.held.extend_from_slice(pcm);
        Ok(())
    }

    fn queued(&self) -> usize {
        self.held.len()
    }

    fn barge(&mut self) -> Option<u64> {
        if self.held.is_empty() {
            return None;
        }
        let pcm = std::mem::take(&mut self.held);
        // 100 ms at 16 kHz. The ring takes a block only when the resampled
        // block fits. The callback is already running, so this write and the
        // flush stay in one stretch.
        const CHUNK: usize = 1_600;
        for chunk in pcm.chunks(CHUNK) {
            let wrote = self.session.playback().push_pcm16(chunk, SAMPLE_RATE).ok()?;
            if wrote != chunk.len() {
                return None;
            }
        }
        if self.session.playback().queued() == 0 {
            return None;
        }
        let started = Instant::now();
        self.session.playback().barge_in();
        let deadline = started + std::time::Duration::from_millis(20);
        while self.session.playback().flush_honor_ns().is_none() {
            if Instant::now() >= deadline {
                break;
            }
            std::thread::yield_now();
        }
        self.session.playback().flush_honor_ns()?;
        Some(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX))
    }
}

/// Point ALSA at its null PCM when this process has no card and the caller
/// has not already chosen a config. The file stays for the life of the process
/// so a later open still has a config to read.
#[cfg(all(feature = "alsa", target_os = "linux"))]
fn prepare_null_if_headless() -> Result<(), String> {
    if env::var_os("ALSA_CONFIG_PATH").is_some() || linux_has_card() {
        return Ok(());
    }
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    if PATH.get().is_none() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = env::temp_dir().join(format!(
            "horbm-e2e-asound-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).map_err(|err| format!("asound dir: {err}"))?;
        let path = dir.join("asoundrc");
        fs::write(
            &path,
            "pcm.!default {\n    type null\n}\nctl.!default {\n    type null\n}\n",
        )
        .map_err(|err| format!("asoundrc: {err}"))?;
        let _ = PATH.set(path);
    }
    let Some(path) = PATH.get() else {
        return Err("asoundrc was not written".into());
    };
    // The 1.85 toolchain still has a safe set_var. This runs before any other
    // thread opens the host.
    env::set_var("ALSA_CONFIG_PATH", path);
    Ok(())
}

#[cfg(all(feature = "alsa", target_os = "linux"))]
fn linux_has_card() -> bool {
    let Ok(text) = fs::read_to_string("/proc/asound/cards") else {
        return false;
    };
    text.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && trimmed.starts_with(|ch: char| ch.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BARGE_BUDGET_NS: u64 = 1_000_000;

    #[test]
    fn loopback_flush_is_under_a_millisecond() {
        let mut output = Output::loopback();
        output.push(&vec![1_000; 4_800]).unwrap();
        assert!(output.queued() > 0);
        let ns = output.barge().expect("queued audio flushes");
        assert!(
            ns < BARGE_BUDGET_NS,
            "loopback barge_in_stop_ns {ns} is not under 1 ms"
        );
        assert_eq!(output.name(), "loopback");
        assert!(output.device().is_none());
    }

    #[cfg(all(feature = "alsa", target_os = "linux"))]
    #[test]
    fn alsa_flush_is_under_a_millisecond() {
        let mut output = Output::open_alsa().expect("alsa output");
        assert_eq!(output.name(), "alsa");
        output.push(&vec![1_000; 4_800]).expect("push");
        let queued_now = output.queued();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(
            output.queued(),
            queued_now,
            "the queue must still be there when barge-in arrives"
        );
        let ns = output.barge().expect("alsa flush was not honored");
        assert!(
            ns < BARGE_BUDGET_NS,
            "alsa barge_in_stop_ns {ns} is not under 1 ms"
        );
    }
}
