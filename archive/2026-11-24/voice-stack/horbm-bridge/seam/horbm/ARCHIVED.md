# Archived 2026-11-24

Former path: `voice-stack/horbm-bridge/seam/horbm`

This package was also named `horbm`. It was a compile seam: `turn` returned an empty line and the source `seam:horbm-not-landed`. `voice-stack/horbm-bridge/Cargo.toml` links `horbm/crates/horbm` instead. Nothing in the workspace or in CI named this path as a dependency.

The linked bridge and the real `horbm` crate stay where they are.
