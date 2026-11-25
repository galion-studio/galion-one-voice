# HORBM portability

Perpetual Open MIT License. (c) 2026 Galion Labs.

The current runtime and CI are Linux. There is no `windows-latest` job. A Windows build is a later step, not a gate on this tree.

The code stays portable so that step can compile. Unix-only calls sit behind `#[cfg(unix)]`:

- `crates/galion-cores` pins a child with `sched_setaffinity` after fork. On other targets `apply_affinity` does not change the command, and `pin_current_thread` uses `SetThreadAffinityMask` where that call exists.
- `crates/galion-asr-vosk` waits on the worker pipe with `poll`. On Windows the same wait is `PeekNamedPipe`.
- The cascade speech workers (`galion-ml-voice` Piper, clone, and Flite) keep `pre_exec`, `poll`, and `dlopen` on Unix. On other targets those calls return an error instead of failing to compile.
- Helper scripts try `python3`, then `python` (the other order on Windows). `GALION_SAMANTHA_PYTHON` still wins for the s711 clone.
- Tests that spawn a binary of the same package use `env!("CARGO_BIN_EXE_<name>")`.

`cargo check` does not link. The `x86_64-pc-windows-gnu` standard library from rustup is enough. A MinGW cross linker is not required. Run this by hand when you want the cross-check. It does not run the binaries, and it is not part of CI.

```text
rustup target add x86_64-pc-windows-gnu
cargo check -p galion-cores --target x86_64-pc-windows-gnu --all-targets
cargo check --manifest-path horbm/Cargo.toml --workspace --target x86_64-pc-windows-gnu --all-targets
cargo check --manifest-path voice-stack/Cargo.toml --workspace --target x86_64-pc-windows-gnu --all-targets
```

Later Windows check, from a checkout, Rust 1.85+, no weight download:

```text
cargo test -p galion-cores
cargo test --manifest-path voice-stack/Cargo.toml --workspace
cargo test --workspace
cargo test -p galion-cascade --features cascade --lib
```

`voice-stack/vad` stays out of the voice-stack workspace because it links `ort`. Do not add a GitHub `windows-latest` workflow until the owner asks for a Windows runtime.
