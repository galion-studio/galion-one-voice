// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! Resolved tool + data paths for the Cloning V2 tree.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub cloning_v2: PathBuf,
    pub autocheck: PathBuf,
    pub speak: PathBuf,
    pub ecapa: PathBuf,
    pub emotion: PathBuf,
    pub wer: PathBuf,
    pub text12: PathBuf,
    pub render_lock: PathBuf,
    pub reports: PathBuf,
    pub work: PathBuf,
    pub voices_dir: PathBuf,
    pub fixes_dir: PathBuf,
}

impl Paths {
    pub fn discover(autocheck: &Path) -> Result<Self> {
        let autocheck = autocheck
            .canonicalize()
            .unwrap_or_else(|_| autocheck.to_path_buf());
        let blend = autocheck.parent().context("autocheck parent")?.to_path_buf();
        let cloning_v2 = blend.parent().context("emotion-blend parent")?.to_path_buf();
        let scorers = cloning_v2.join("scorers");
        let pipeline = cloning_v2.join("pipeline");
        let speak = pipeline.join("bin/cbx-speak-voice");
        let ecapa = scorers.join("ecapa/target/release/galion-ecapa");
        let emotion = scorers.join("emotion/target/release/galion-emotion");
        let wer = scorers.join("quality/bin/galion-wer");
        let text12 = {
            let local = autocheck.join("fixtures/text12");
            if local.join("t01.txt").is_file() {
                local
            } else {
                let alt = PathBuf::from("/workspace/swarm/voice-v2-sprint/own-clone-analysis/text");
                if alt.join("t01.txt").is_file() {
                    alt
                } else {
                    local
                }
            }
        };
        let voices_dir = PathBuf::from(
            std::env::var("CBX_VOICES_DIR").unwrap_or_else(|_| {
                format!(
                    "{}/.cache/huggingface/cbx/voices",
                    std::env::var("HOME").unwrap_or_else(|_| "/home/box".into())
                )
            }),
        );
        Ok(Self {
            cloning_v2,
            render_lock: blend.join("render.lock"),
            reports: autocheck.join("reports"),
            work: autocheck.join("work"),
            fixes_dir: autocheck.join("fixes"),
            autocheck,
            speak,
            ecapa,
            emotion,
            wer,
            text12,
            voices_dir,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.reports)?;
        std::fs::create_dir_all(&self.work)?;
        Ok(())
    }

    pub fn require_tools(&self) -> Result<()> {
        for (label, p) in [
            ("cbx-speak-voice", &self.speak),
            ("galion-ecapa", &self.ecapa),
            ("galion-emotion", &self.emotion),
            ("galion-wer", &self.wer),
        ] {
            if !p.is_file() {
                bail!("missing {label} at {}", p.display());
            }
        }
        if !self.text12.join("t01.txt").is_file() {
            bail!("missing 12-line texts under {}", self.text12.display());
        }
        Ok(())
    }

    pub fn clip_store_original(persona: &str) -> PathBuf {
        PathBuf::from(format!(
            "/workspace/swarm/voice-v2-sprint/clip-store-own/{persona}/original.wav"
        ))
    }

    pub fn clip_store_reference(persona: &str) -> PathBuf {
        PathBuf::from(format!(
            "/workspace/swarm/voice-v2-sprint/clip-store-own/{persona}/reference.wav"
        ))
    }

    pub fn merge_v2_wavs(persona: &str) -> PathBuf {
        PathBuf::from(format!(
            "/workspace/swarm/voice-v2-sprint/cloning-v2/merge-v2/wav/{persona}"
        ))
    }

    pub fn line_ids() -> Vec<String> {
        (1..=12).map(|i| format!("t{i:02}")).collect()
    }

    pub fn line_text(&self, line_id: &str) -> Result<String> {
        let p = self.text12.join(format!("{line_id}.txt"));
        let s = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
        Ok(s.trim().to_string())
    }
}

/// Refuse to write over protected production voices.
pub fn assert_safe_voice_name(name: &str) -> Result<()> {
    let protected = ["own-samantha", "own-jarvis", "piper-samantha", "piper-jarvis"];
    if protected.contains(&name) {
        bail!("refusing to modify protected voice '{name}' — use own-<persona>-v2-check-* only");
    }
    if name.starts_with("own-") && name.contains("-v2-check-") {
        return Ok(());
    }
    // Allow other experimental names that are clearly not production.
    if name.starts_with("own-") && (name.contains("-v2-") || name.contains("-check-")) {
        return Ok(());
    }
    bail!(
        "voice name '{name}' must be own-<persona>-v2-check-* (never overwrite own-samantha/own-jarvis)"
    );
}
