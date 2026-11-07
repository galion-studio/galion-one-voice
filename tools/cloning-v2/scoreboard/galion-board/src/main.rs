//! Artifact-first Cloning V2 scoreboard entrypoint.
//! All acoustic/inference metrics are computed by the existing Rust scorers.
//! Python performs only filesystem discovery, JSON aggregation and Markdown.
//! Scorer CLIs are deliberately distinct: ECAPA takes explicit WAV paths;
//! emotion requires `score`; articulation uses READY bin/galion-artic profile;
//! quality uses bin/galion-{wer,mos,hygiene}. Never render from this program.
use std::{path::Path, process::Command};
fn main() {
    let driver = Path::new(env!("CARGO_MANIFEST_DIR")).join("aggregate.py");
    let status = Command::new("python3")
        .arg(driver)
        .env("OMP_NUM_THREADS", "2").env("OPENBLAS_NUM_THREADS", "2")
        .env("RAYON_NUM_THREADS", "2").env("CUDA_VISIBLE_DEVICES", "")
        .status().expect("artifact aggregation driver failed to start");
    std::process::exit(status.code().unwrap_or(1));
}
