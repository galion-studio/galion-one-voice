// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let landing = manifest_dir.join("../../horbm/crates/horbm/Cargo.toml");
    println!("cargo:rerun-if-changed={}", landing.display());
    if landing.is_file() {
        println!(
            "cargo:warning=HORBM is on disk at {}. Feature `horbm` still links seam/horbm. Point that path dependency at \"../../horbm/crates/horbm\".",
            landing.display()
        );
    }
}
