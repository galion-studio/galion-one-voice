// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! Binary entry for the Cloning V2 preflight check.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = galion_cloning_preflight::run(&args);
    std::process::exit(code);
}
