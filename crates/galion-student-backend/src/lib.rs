// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Student slot, adapt-not-train (sector 09). NO weights are vendored or invented.
//!
//! * [`SlotManifest`] lists the published artifacts the slot expects (see
//!   `galion-student/configs/adapt-manifest.toml`).
//! * [`load_slot`] is the load path: it only checks that the owner placed files on
//!   disk. It never downloads, generates, or fabricates weights. Until A21 counsel
//!   sign-off and owner placement it returns [`LoadError::NotPlaced`].
//! * [`StudentBackend`] implements `ReplayBackend`; with no model loaded it is a
//!   silent, neutral-warm stub so P1–P8 / R1–R9 failures become recorded targets.
//! * W21-02: [`mimi_lane::MimiPassthrough`] (PCM → Mimi encode → codes → decode → PCM, one
//!   80 ms frame per call, ZeroWeights / synthetic only, `IS_STUB` stays true) and
//!   [`StudentBackend::step_frame`] for the live demo audio path.
pub mod mimi_lane;
pub use mimi_lane::{KernelMode, MimiLaneConfig, MimiLaneError, MimiPassthrough, Monitor, StepReport, FRAME_SAMPLES};

use galion_presence::backend::{AffectOut, FrameIn, FrameOut, ReplayBackend};
use std::path::{Path, PathBuf};

/// One published artifact the slot may adapt (loaded as published; int8 PTQ only).
#[derive(Debug, Clone, PartialEq)]
pub struct SlotArtifact {
    pub role: &'static str,
    pub upstream: &'static str,
    pub file: &'static str,
    pub licence: &'static str,
    /// Pinned sha256; `None` until the owner records it. Never guessed.
    pub sha256: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlotManifest {
    pub artifacts: &'static [SlotArtifact],
}

/// Default adapt manifest. Licences marked UNKNOWN pending A21.
pub const ADAPT_MANIFEST: SlotManifest = SlotManifest {
    artifacts: &[
        SlotArtifact { role: "codec", upstream: "kyutai/mimi", file: "mimi.safetensors", licence: "UNKNOWN (A21)", sha256: None },
        SlotArtifact { role: "dialogue", upstream: "kyutai/moshiko", file: "model.gq", licence: "CC-BY-4.0 (A21 to confirm)", sha256: None },
        SlotArtifact { role: "tokenizer", upstream: "kyutai/moshiko", file: "tokenizer.model", licence: "UNKNOWN (A21)", sha256: None },
        SlotArtifact { role: "ser_inbound", upstream: "FunAudioLLM/SenseVoiceSmall", file: "sensevoice-small.safetensors", licence: "UNKNOWN (A21)", sha256: None },
    ],
};

#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    /// Owner has not placed this file (expected state today).
    NotPlaced { file: &'static str },
    /// sha256 not pinned in the manifest; refuse to load unpinned weights.
    Unpinned { file: &'static str },
    /// Counsel gate not cleared.
    CounselPending,
}

#[derive(Debug)]
pub struct LoadedSlot {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
}

/// Load path. Order: counsel gate -> presence -> pin. Hash verification and
/// `.gq` mmap via galion-tensor are TODO once real files exist.
pub fn load_slot(dir: &Path, manifest: &SlotManifest, counsel_cleared: bool) -> Result<LoadedSlot, LoadError> {
    if !counsel_cleared {
        return Err(LoadError::CounselPending);
    }
    let mut files = Vec::new();
    for a in manifest.artifacts {
        let p = dir.join(a.file);
        if !p.is_file() {
            return Err(LoadError::NotPlaced { file: a.file });
        }
        if a.sha256.is_none() {
            return Err(LoadError::Unpinned { file: a.file });
        }
        files.push(p);
    }
    Ok(LoadedSlot { dir: dir.to_path_buf(), files })
}

/// Student backend. Stub mode = no model: me-band silent, neutral-warm affect.
#[derive(Debug, Default)]
pub struct StudentBackend {
    loaded: bool,
    /// W21-02: optional Mimi audio lane (synthetic). `None` → `step_frame` writes silence.
    mimi: Option<MimiPassthrough>,
}

impl StudentBackend {
    pub fn stub() -> Self {
        StudentBackend { loaded: false, mimi: None }
    }
    /// Stub backend with a Mimi passthrough lane attached (W21-02 live demo).
    pub fn with_mimi(lane: MimiPassthrough) -> Self {
        StudentBackend { loaded: false, mimi: Some(lane) }
    }
    /// Stub backend + [`MimiLaneConfig::demo`] lane (reduced widths, ZeroWeights).
    pub fn demo() -> Result<Self, MimiLaneError> {
        Ok(Self::with_mimi(MimiPassthrough::demo()?))
    }
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
    pub fn mimi(&self) -> Option<&MimiPassthrough> {
        self.mimi.as_ref()
    }
    pub fn mimi_mut(&mut self) -> Option<&mut MimiPassthrough> {
        self.mimi.as_mut()
    }
    /// One 80 ms audio frame through the student's codec lane. Zero allocation on success.
    /// Without a lane: `pcm_out` = silence, `codec_ran = false`. Output is synthetic (IS_STUB).
    pub fn step_frame(&mut self, pcm_in: &[f32], pcm_out: &mut [f32]) -> Result<StepReport, MimiLaneError> {
        match self.mimi.as_mut() {
            Some(lane) => lane.step_frame(pcm_in, pcm_out),
            None => {
                if pcm_out.len() != FRAME_SAMPLES {
                    return Err(MimiLaneError::Shape { what: "pcm_out", got: pcm_out.len(), want: FRAME_SAMPLES });
                }
                pcm_out.iter_mut().for_each(|s| *s = 0.0);
                Ok(StepReport {
                    frame: 0,
                    paused: pcm_in.len() != FRAME_SAMPLES,
                    codec_ran: false,
                    bypassed: false,
                    codes_nonzero: false,
                    codec_peak: 0.0,
                    sanitized: 0,
                    is_stub: mimi_lane::IS_STUB,
                })
            }
        }
    }
}

impl ReplayBackend for StudentBackend {
    fn name(&self) -> &'static str {
        "student-stub"
    }
    fn reset(&mut self) {
        if let Some(l) = self.mimi.as_mut() {
            l.reset();
        }
    }
    fn step(&mut self, _input: &FrameIn) -> FrameOut {
        // No model: honest silence. Zero allocation.
        FrameOut { me_voiced: false, affect_out: AffectOut::NEUTRAL_WARM, rika_lean_in: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counsel_gate_first() {
        assert_eq!(load_slot(Path::new("/nonexistent"), &ADAPT_MANIFEST, false).unwrap_err(), LoadError::CounselPending);
    }
    #[test]
    fn not_placed_today() {
        assert_eq!(load_slot(Path::new("/nonexistent"), &ADAPT_MANIFEST, true).unwrap_err(), LoadError::NotPlaced { file: "mimi.safetensors" });
    }
    #[test]
    fn no_sha_invented() {
        assert!(ADAPT_MANIFEST.artifacts.iter().all(|a| a.sha256.is_none()));
    }
    #[test]
    fn stub_is_silent_neutral() {
        let b = StudentBackend::stub();
        assert!(!b.is_loaded() && !b.supports_rika());
        assert_eq!(b.name(), "student-stub");
    }
}
