// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Turn on in-crate stages when their source is on this tree.
//!
//! The files are added by open pull requests. Cargo path dependencies cannot
//! point at a crate that is not on disk yet, so those crates are probed at
//! runtime. Modules that land inside crates this package already links are
//! selected here, and the smoke calls them only under the matching cfg.

use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_dialogue)");
    println!("cargo::rustc-check-cfg=cfg(has_memory)");
    println!("cargo::rustc-check-cfg=cfg(has_moment_voice)");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let root = Path::new(&manifest);
    flag(root, "has_dialogue", "../horbm/src/dialogue/mod.rs");
    flag(root, "has_memory", "../horbm/src/memory.rs");
    flag(
        root,
        "has_moment_voice",
        "../horbm-voice/src/moment_voice.rs",
    );
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=probes");
}

fn flag(root: &Path, cfg: &str, rel: &str) {
    let path = root.join(rel);
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_file() {
        println!("cargo:rustc-cfg={cfg}");
    }
}
