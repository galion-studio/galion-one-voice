// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Integrator for the mouth-and-expressions stack (Swarm 13, S8).
//!
//! Feeling, lungs, throat, mouth, nose, sound-out and listen-back are called.
//! They are not reimplemented here. S9 voice-guard is a slot between sound-out
//! and listen-back; audio passes through until that crate lands.

#![forbid(unsafe_code)]

mod stack;

use std::path::Path;

use serde::Serialize;

pub use stack::{fixture_dir, run_fixtures, FixtureScore};

pub const TICK_MS: u32 = 80;

pub const SLOTS: &[LayerSlot] = &[
    LayerSlot {
        owner: "S1",
        role: "contract and fixtures",
        crate_name: "galion-mne-contract",
        path: "tools/symphony/mne-contract",
    },
    LayerSlot {
        owner: "S2",
        role: "feeling",
        crate_name: "galion-feeling",
        path: "tools/symphony/feeling",
    },
    LayerSlot {
        owner: "S3",
        role: "throat",
        crate_name: "galion-throat",
        path: "tools/symphony/throat",
    },
    LayerSlot {
        owner: "S4",
        role: "mouth",
        crate_name: "galion-mouth",
        path: "tools/symphony/mouth",
    },
    LayerSlot {
        owner: "S5",
        role: "nose and wordless sounds",
        crate_name: "galion-nose",
        path: "tools/symphony/nose",
    },
    LayerSlot {
        owner: "S6",
        role: "sound out",
        crate_name: "galion-sound-out",
        path: "tools/symphony/sound-out",
    },
    LayerSlot {
        owner: "S7",
        role: "listening back",
        crate_name: "galion-listen-back",
        path: "tools/symphony/listen-back",
    },
    LayerSlot {
        owner: "S9",
        role: "voice guard",
        crate_name: "galion-voice-guard",
        path: "tools/symphony/voice-guard",
    },
];

#[derive(Clone, Copy, Debug, Serialize)]
pub struct LayerSlot {
    pub owner: &'static str,
    pub role: &'static str,
    pub crate_name: &'static str,
    pub path: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct LayerStatus {
    pub owner: &'static str,
    pub role: &'static str,
    pub crate_name: &'static str,
    pub path: &'static str,
    pub state: &'static str,
    pub wired_to: &'static str,
}

pub fn layer_status(repo: &Path) -> Vec<LayerStatus> {
    SLOTS
        .iter()
        .map(|slot| LayerStatus {
            owner: slot.owner,
            role: slot.role,
            crate_name: slot.crate_name,
            path: slot.path,
            state: if repo.join(slot.path).join("Cargo.toml").is_file() {
                "landed"
            } else {
                "waiting"
            },
            wired_to: match slot.owner {
                "S1" => "galion-mne-contract fixtures",
                "S2" => "galion-feeling, frames mapped from the contract",
                "S3" => "galion-throat drive_from_document",
                "S4" => "galion-mouth from_document, clone_mouth into sound-out",
                "S5" => "galion-nose request_from_document, bursts into sound-out",
                "S6" => "galion-sound-out Piper stand-in, no weights",
                "S7" => "galion-listen-back decode and score",
                "S9" => "passthrough until tools/symphony/voice-guard lands",
                _ => "none",
            },
        })
        .collect()
}

pub fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}
