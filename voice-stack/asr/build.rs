// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".into());
    let version = version.trim().replace('\n', " ");
    println!("cargo:rustc-env=S03_RUSTC={version}");
    println!("cargo:rerun-if-changed=build.rs");
}
