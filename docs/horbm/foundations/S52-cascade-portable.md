# S52 — cascade speech workers type-check off Unix

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S52, galion-cascade portability |
| Written | 2026-11-25. No voice audio committed, no weights, no paid API, no merge. |
| Base | `main` `5d14d7b6c589884691f1b9e001f00f7664a219ea` |
| Gap | `docs/horbm/foundations/F-integration-deep-dive.md`, "Gaps left open": the cascade feature's Unix speech workers still do not compile on Windows. |
| This change | `crates/galion-cascade` and this note. No new dependency. `Cargo.lock` is unchanged. |
| Not edited | `crates/galion-ml-voice`, `crates/galion-asr-vosk`, `crates/galion-cores`, `LEDGER.md`, root workspace members, `.github/workflows/*`, `LOCKS.toml`. `SOUL.md` is not in this tree. |
| CI | No Windows job. Mate: the cross-check stays a local `cargo check`. |

## What it decides

The phone lane and the English demo open their speech workers through `galion_cascade::speech_worker::speech_host` (`SpeechHost`). The host is chosen with `cfg(unix)`.

On Unix, `Host::open_stack` is `galion_ml_voice::speech::SpeechStack::detect` and `Host::open_vosk` is `galion_asr_vosk::VoskClient::open`. `PhoneLane::new` still pre-warms that stack. `PhoneLane::with_speech` still accepts the `SpeechStack` the caller already built, including `SpeechStack::silent`.

On every other target the host is `SpeechHostKind::Stub`. `open_stack` returns `SpeechStack::silent` with `STUB_REASON` in `crates/galion-cascade/src/speech_worker.rs`. `open_vosk` returns that same string as `Err`. Nothing is spawned, no pipe is polled, and no child signal is sent. `can_speak()` is false, so the lane returns `PhoneError::NoSpeech` instead of PCM.

`unix_host_matches_detect_bytes` speaks `Hi.` through the host and through `SpeechStack::detect` and compares `sample_rate` and the `f32` samples. On this machine both stacks are libflite. The samples matched.

The reply-latency example spawns the Piper child and `uname` only inside `cfg(unix)`. On another target `measure_piper` returns `None`, which is the example's existing "A21 Piper pack is not on disk" JSON, and the host line is `non-unix nproc=<n>`.

## Where the Unix calls live

`galion-cascade` has no `std::os::unix`, no `UnixListener`, and no `dlopen`. The workers it calls already gate those operations. This PR does not edit them. `docs/horbm/PORTABILITY.md` already records that layout. The deep-dive gap was written against an older tree; `cargo check` of this crate with `--features cascade` now finishes on `x86_64-pc-windows-gnu`.

| Worker | Unix | Other targets |
|---|---|---|
| Piper (`piper_worker.rs`) | `Command` spawn, optional `pre_exec` + `sched_setaffinity` when `GALION_PIPER_CPUS` is set, `poll` on the stdout and stderr pipe fds | `poll_pipes` returns `A21 piper pipe wait is not available on this platform` |
| Clone (`clone_worker.rs`) | `python3` child, `poll` on the pipe fds, `Child::kill` on drop | pipe wait returns an error; the spawn call is still the portable `Command` API |
| Flite (`flite.rs`) | `dlopen` of `libflite.so.1` and the voice libraries | `FliteTts::open` returns `libflite is only opened on Unix` |
| Vosk parent (`galion-asr-vosk` `client.rs`) | `poll` on the worker stdout pipe | `PeekNamedPipe` on Windows; other targets return `vosk worker wait is not available on this platform` |
| Vosk child (`galion-asr-vosk` `src/main.rs`) | `dlopen` of libvosk (`RTLD_NOW`) | The `extern "C"` block type-checks under `cargo check` because check does not link. The process is the Linux worker. |

There is no `UnixListener` or `UnixStream` on this path. The Unix wait is `poll` on the pipes the child inherits. `Child::kill` is `std::process` and, on Unix, delivers the kill signal. The cascade stub never constructs the child.

`galion-asr-vosk` already selects the file name `galion-asr-vosk.exe` under `cfg(windows)` and `galion-asr-vosk` otherwise. This PR does not add a hard-coded `.exe` or a hard-coded absolute path.

`cargo check` of `galion-ml-voice` on the Windows target warns about twelve unused imports and dead fields. Those items are the Unix `poll` / `dlopen` bodies, compiled out, while the struct fields stay. They are not errors. This PR leaves that crate alone.

## Cross type-check

Rustc 1.85.0. Target `x86_64-pc-windows-gnu` from rustup (`rust-std` only). No MinGW linker. Not a CI job.

```text
rustup target add x86_64-pc-windows-gnu
cargo check -p galion-cascade --features cascade --all-targets --target x86_64-pc-windows-gnu
```

The command finished. `galion-cascade` reported no warnings after `say_until_pcm` was placed under `cfg(unix)`. The twelve `galion-ml-voice` warnings above are the only diagnostics.

## Linux-only

The product runtime stays Galion-Ubuntu.

- Spoken audio is the Unix workers: Piper when the A21 pack is on disk, otherwise libflite, and the clone child only when that slot opens. This sandbox has libflite and no A21 pack. The byte test spoke through Flite.
- `GALION_PIPER_CPUS` affinity is `pre_exec` in the Piper child. It is off unless that variable is set.
- The Vosk recogniser process dlopens libvosk. The phone lane does not link libvosk.
- `examples/reply_latency.rs` measures a Piper child and prints `uname -sr` on Unix only.
- Ear prosody stays on (`GALION_EAR_SENSE` remains the off switch). Consent is unchanged: `remember=deny`, `learn=deny`, `learn_in_band=false`. Samantha is the persona. Aria stays parked.

## Tests

`cargo test -p galion-cascade --features cascade --lib` — 130 passed, 0 failed. That includes `host_kind_matches_the_target`, `unix_host_matches_detect_bytes`, the existing phone turns, and `reply_is_speech_not_tones`.

`cargo test --workspace` — 1072 passed, 5 ignored, 0 failed.

`cargo test --manifest-path horbm/Cargo.toml --workspace` — 248 passed, 1 ignored, 0 failed.

`cargo clippy -p galion-cascade --features cascade --all-targets -- -D warnings`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` are clean. Rustc 1.85.0 on Linux.
