# Archived 2026-11-24

Former path: `crates/galion-v2-live`

This was the V2 duplex lane: warm Piper, a Vosk sidecar, a clone worker, and an optional cloud reply stream. It had its own Cargo workspace, so `cargo test --workspace` never built it. `samantha-local` does not depend on it. The loopback voice server answers with `horbm::Brain::turn`.

Voice-stack comments that used this tree as the source of the energy-VAD and Piper-child protocol now point here. The formulas already copied into `voice-stack` stay there.

Do not put this lane back on the HORBM path. Retrieval answers first. Galion Research stays off the tick.
