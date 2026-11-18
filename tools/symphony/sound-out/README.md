# galion-sound-out

Perpetual Open MIT License. (c) 2026 Galion Labs.

Sound-out layer. One `ClonerBackend` trait, adapters for the Chatterbox worker, Piper, and a Moshi stub, then the lungs breath track, nose bursts, and the moment's room.

```bash
cargo test --manifest-path tools/symphony/sound-out/Cargo.toml
cargo test --manifest-path tools/symphony/sound-out/Cargo.toml --features real-backends
python3 scripts/clone/test_controls.py
```

The feature-flag tests do not download weights and do not synthesize. CI does not pass `--features real-backends`.

```bash
cargo run --manifest-path tools/symphony/sound-out/Cargo.toml -- speak --text "Hello." --backend piper
```

A WAV is written only when `--out` is set. The page that explains the stubs is `docs/galion-voice-v2/symphony/SOUND-OUT.md`.
