// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Print the S0 student probe. Does not download, write, or speak a weight.
//!
//! `cargo run -p galion-cascade --example student_probe`

fn main() {
    let probe = galion_cascade::student::probe_env();
    println!(
        "ready={} label={} source={} reason={} tensors={} path={} line={}",
        probe.ready, probe.label, probe.source, probe.reason, probe.tensors, probe.path, probe.line
    );
}
