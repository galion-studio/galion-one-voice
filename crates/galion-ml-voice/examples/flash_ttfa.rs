// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2-S14 Flash eval stub. One JSON object on stdout.
//!
//! Without `GALION_FLASH_EVAL=1` the object has `"measured": false` and no
//! `ttfa_ms`. A protocol-stub run (both that flag and `GALION_CLONE_STUB_PCM=1`)
//! may print a clock of the first chunk, plus `utt_ms` for the closing
//! marker. `product_claim` stays false unless `engine` is `model` and
//! `path` is `stream`. This is not a Flash result.
//! Nothing here downloads weights or sends a reference clip.
//!
//! ```text
//! cargo run -q -p galion-ml-voice --features speech --example flash_ttfa
//! ```

fn main() {
    println!(
        "{}",
        galion_ml_voice::speech::clone_worker::flash_from_env().json()
    );
}
