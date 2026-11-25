# Archived 2026-11-25

Former path: `crates/galion-live-demo`

This was the W21 live demo. It cut a you-band into 80 ms frames at 24 kHz and called `Heartbeat::step` with `RikaReplayBackend`. It wrote a stereo WAV, `console_capture.txt`, and checksums. Optional features drew the one-dot page. It was a member of the root workspace, so `cargo test --workspace` built it. Nothing else named it as a path dependency.

The member line is gone. `scripts/run-live-demo.sh` and `galion-gates/lv-live-demo.sh` no longer build this package. The live loop on main is `horbm-duplex` (`Duplex::tick`, 80 ms, 16 kHz, Samantha) and `scripts/horbm-e2e-smoke.sh`. `galion-heartbeat` stays in the workspace. This tree is not that path.

The manifest still says `version.workspace = true` and its path dependencies still point at the old sibling crates. Cargo will not build it from here. Do not add it back to the root members, and do not retarget those paths to put Heartbeat on the HORBM loop.
