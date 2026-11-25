// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Portable face of the cascade speech workers.
//!
//! Unix process spawn, pipe wait (`poll` on the child fds), and child signals
//! stay in `galion-ml-voice` and `galion-asr-vosk`. This module is the gate the
//! phone lane and the EN demo use. On Unix it calls those workers. On every
//! other target it returns [`STUB_REASON`] and does not spawn.

use galion_asr_vosk::VoskClient;
use galion_ml_voice::speech::SpeechStack;

/// Why a non-Unix host refuses to start a speech worker.
pub const STUB_REASON: &str = "cascade speech workers do not run on this target. Process spawn, Unix pipe wait, and signals are Unix-only. No transcript was invented.";

/// Which host [`speech_host`] built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechHostKind {
    /// Real Piper, clone, Flite, and Vosk workers.
    Unix,
    /// No process, no pipe wait, no signal.
    Stub,
}

/// Opens the speech stack and the Vosk worker for this target.
pub trait SpeechHost {
    fn kind(&self) -> SpeechHostKind;
    /// Speech stack for one lane. On Unix this is [`SpeechStack::detect`].
    fn open_stack(&self) -> SpeechStack;
    /// Vosk parent. On Unix this is [`VoskClient::open`].
    fn open_vosk(&self) -> Result<VoskClient, String>;
}

/// The host for the target this crate was compiled for.
#[derive(Debug, Clone, Copy, Default)]
pub struct Host;

impl Host {
    pub fn kind(&self) -> SpeechHostKind {
        SpeechHost::kind(self)
    }

    pub fn open_stack(&self) -> SpeechStack {
        SpeechHost::open_stack(self)
    }

    pub fn open_vosk(&self) -> Result<VoskClient, String> {
        SpeechHost::open_vosk(self)
    }
}

impl SpeechHost for Host {
    fn kind(&self) -> SpeechHostKind {
        #[cfg(unix)]
        {
            let _ = self;
            SpeechHostKind::Unix
        }
        #[cfg(not(unix))]
        {
            let _ = self;
            SpeechHostKind::Stub
        }
    }

    fn open_stack(&self) -> SpeechStack {
        #[cfg(unix)]
        {
            let _ = self;
            SpeechStack::detect()
        }
        #[cfg(not(unix))]
        {
            let _ = self;
            SpeechStack::silent(STUB_REASON)
        }
    }

    fn open_vosk(&self) -> Result<VoskClient, String> {
        #[cfg(unix)]
        {
            let _ = self;
            VoskClient::open()
        }
        #[cfg(not(unix))]
        {
            let _ = self;
            Err(STUB_REASON.to_string())
        }
    }
}

/// Host for this compilation target.
pub fn speech_host() -> Host {
    Host
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_kind_matches_the_target() {
        #[cfg(unix)]
        assert_eq!(speech_host().kind(), SpeechHostKind::Unix);
        #[cfg(not(unix))]
        assert_eq!(speech_host().kind(), SpeechHostKind::Stub);
    }

    /// The Unix host is `SpeechStack::detect`, so a short utterance is the
    /// same samples either way. No pack on disk still matches: both refuse.
    #[cfg(unix)]
    #[test]
    fn unix_host_matches_detect_bytes() {
        let mut via = speech_host().open_stack();
        let mut direct = SpeechStack::detect();
        assert_eq!(via.backend.id(), direct.backend.id());
        assert_eq!(via.weights.id(), direct.weights.id());
        assert_eq!(via.can_speak(), direct.can_speak());
        if !via.can_speak() {
            return;
        }
        let a = via.speak("Hi.").expect("host speak");
        let b = direct.speak("Hi.").expect("detect speak");
        assert_eq!(a.sample_rate, b.sample_rate);
        assert_eq!(a.samples, b.samples);
    }

    #[cfg(not(unix))]
    #[test]
    fn stub_does_not_spawn() {
        let host = speech_host();
        let stack = host.open_stack();
        assert!(!stack.can_speak());
        assert!(stack.backend.label().contains("Unix-only"));
        let err = match host.open_vosk() {
            Ok(_) => panic!("stub host spawned a vosk worker"),
            Err(err) => err,
        };
        assert_eq!(err, STUB_REASON);
        assert!(err.contains("No transcript was invented"));
    }
}
