// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Turn stage detection on when the duplex sources grow the matching API.
//!
//! End-of-turn, listener backchannels, and echo cancellation are open pull
//! requests. This build does not call those methods. When a merge adds them
//! under `horbm-duplex`, the cfg flips and the eval starts scoring that stage.

use std::fs;
use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let duplex = Path::new(&manifest).join("../horbm-duplex/src");
    println!("cargo:rerun-if-changed={}", duplex.display());
    println!("cargo:rustc-check-cfg=cfg(duplex_aec)");
    println!("cargo:rustc-check-cfg=cfg(duplex_eot)");
    println!("cargo:rustc-check-cfg=cfg(duplex_backchannel)");
    let mut aec = false;
    let mut eot = false;
    let mut backchannel = false;
    visit(&duplex, &mut aec, &mut eot, &mut backchannel);
    if aec {
        println!("cargo:rustc-cfg=duplex_aec");
    }
    if eot {
        println!("cargo:rustc-cfg=duplex_eot");
    }
    if backchannel {
        println!("cargo:rustc-cfg=duplex_backchannel");
    }
}

fn visit(dir: &Path, aec: &mut bool, eot: &mut bool, backchannel: &mut bool) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            visit(&path, aec, eot, backchannel);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if text.contains("fn set_aec") && text.contains("fn aec_enabled") {
            *aec = true;
        }
        if text.contains("tick_turn")
            || text.contains("TurnMsg::Hold")
            || text.contains("TurnMsg::Yield")
        {
            *eot = true;
        }
        if text.contains("set_backchannel_config") || text.contains("ListenCue") {
            *backchannel = true;
        }
    }
}
