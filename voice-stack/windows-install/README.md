<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# Samantha on the Windows laptop

Install and start scripts for the owner's machine. Large files go under `G:\galion\samantha`. The C: drive is not used for models, the Cargo registry, or build output.

This folder is standalone. It is not a member of the root Cargo workspace.

The scripts reuse the stack that is already in this repo:

| Piece | What the scripts use |
|---|---|
| One-dot UI | `crates/galion-console` example `dot_console_demo`, same flags as `scripts/run-v1-screen.sh`. Page at `http://127.0.0.1:7878/`. `galion-render` still refuses a public bind. |
| Companion socket | `galion-samantha-ws/1` on `127.0.0.1:7881` (`GALION_SAMANTHA_WS`). `GET /health` must answer 200 and `ok` (plain text or JSON `"ok": true`). Port 7878 is not this socket. |
| 80 ms heartbeat | `crates/galion-heartbeat` `FRAME_MS` is 80. These scripts do not change it. |
| Vosk ASR | Model pin from `scripts/asr/fetch-vosk.sh`, plus the Windows libvosk 0.3.45 zip. Worker binary `galion-asr-vosk`. |
| Piper TTS | Voice pin from `scripts/a21/fetch-a21.sh` (`en_US-ljspeech-medium`). Mouth is the Windows piper 2023.11.14-2 CLI, because `runtimes/galion-piper-rt` calls `libc::dlopen` and `sched_setaffinity`. |
| Silero VAD | ONNX file only, pointed at by `GALION_SILERO_VAD` (the name already used under `tools/symphony/articulation`). The current one-dot server does not load that ONNX. |
| Chatterbox | `scripts/clone/chatterbox_worker.py` is not started. Clone weights are not downloaded. |
| Persona | `GALION_CLONE_PERSONA=samantha`. The reference wav is not in git. |
| Linux sandbox | `deploy/systemd/templates/galion-voice-stage.service` stays the Linux unit. This folder does not install a Windows service. |

## What a run does not touch

Scheduled tasks named `GalionLaptopLink` and `GalionShellLink`. `sshd`. Drivers. GPU or CPU acceleration packages. Anything under the Windows system directories. The script does not call the task scheduler, does not change a service, and does not write a user or machine environment variable. `CUDA_VISIBLE_DEVICES` is set only on the Samantha process, from `G:\galion\samantha\env\samantha.env`.

## Dry-run

From this folder, in PowerShell:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\Install-Samantha.ps1 -DryRun
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\Start-Samantha.ps1 -DryRun
```

`-DryRun` prints the plan and exits. It does not download, build, or write. `-WhatIf` does the same for the install and start entry points.

`-Check` is for the laptop after an install. It hashes the Piper onnx, the Piper json, and the Silero onnx again. The Vosk and piper zip files are deleted after a good unpack, so `-Check` only reports that the unpacked files are on disk.

## Install

Rust 1.85 or newer, host `x86_64-pc-windows-msvc`, and the Visual Studio C++ tools. The script checks `rustc` and `vswhere` (or `cl.exe`). It does not run `rustup-init` and it does not download Visual Studio, because those installers write a large toolchain onto C:.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\Install-Samantha.ps1 -SamanthaRef D:\voices\owner-samantha.wav
```

`-SamanthaRef` is optional. When it is set, the path is written to `GALION_SAMANTHA_REF`. The wav is not copied, not hashed into git, and not downloaded. It must be a local `.wav` outside this checkout. A URL is refused. The existing Chatterbox worker is still not started; `GALION_CLONE_LOCAL_REFS` and `GALION_CLONE_FETCH` are left unset.

`CARGO_TARGET_DIR` and `CARGO_HOME` are placed under `G:\galion\samantha` so the registry download does not fill C:. Set `SAMANTHA_KEEP_CARGO_HOME=1` before the install if the existing Cargo home should be reused.

Release binaries copied to `G:\galion\samantha\bin`:

- `dot_console_demo.exe` (`galion-console`, feature `dot-viz-web`)
- `galion-asr-vosk.exe`

`GALION_LLM=canned` selects the offline rule table already in `galion-cascade`. That is not the HORBM brain. No API key and no public model URL are written. The `horbm` crate is not in this repo; this folder does not reimplement it.

## Start

`START-SAMANTHA.cmd` in this folder calls `scripts\Start-Samantha.ps1`. The install also writes `G:\galion\samantha\START-SAMANTHA.cmd` with the same job.

The one-dot command matches the existing screen script and stays on port 7878:

```text
dot_console_demo.exe --headless --realtime --no-png --web 127.0.0.1:7878 --seconds 3600
```

The companion is a separate process, `samantha-local.exe --addr 127.0.0.1:7881`. Readiness is `GET http://127.0.0.1:7881/health` (`galion-samantha-ws/1`): HTTP 200 and a body of `ok` or JSON with `"ok": true`. The browser then opens `http://127.0.0.1:7878/`. `GALION_SAMANTHA_WS` may be another `127.0.0.1` or `[::1]` port, and it may not be the one-dot address. `0.0.0.0` is refused.

`SAMANTHA_SECONDS` in the env file changes the one-dot lifetime (1 through 86400). Closing the window stops the screen process and the companion process this script started. It does not stop any other process.

`samantha-local.exe` is the S10 binary (`voice-stack/local-server`). This checkout does not contain that crate, so the install does not build it. Until that exe is in `G:\galion\samantha\bin`, start exits 1 and does not open the browser.

Logs: `G:\galion\samantha\logs\screen.out.log`, `screen.err.log`, `companion.out.log`, and `companion.err.log`.

If the Vosk model directory is missing, mic ASR stays `stub`, as in `scripts/run-v1-screen.sh`. Typed text still works. A mic turn does not invent a transcript.

## Models

| File | Pin |
|---|---|
| Vosk small English 0.15 | sha256 from `scripts/asr/fetch-vosk.sh` |
| libvosk Windows 0.3.45 | sha256 computed from the GitHub release zip on 2026-11-21 |
| Silero VAD 6.2.3 ONNX, commit `5cd7945676eb32225748052e2e6a0580e4686a08` | sha256 computed from that file on 2026-11-21. Upstream `LICENSE` at that commit is MIT. |
| `en_US-ljspeech-medium` onnx and json | sha256 from `scripts/a21/fetch-a21.sh`. The json was hashed again on 2026-11-21 and matched. |
| piper Windows amd64 2023.11.14-2 | sha256 computed from the GitHub release zip on 2026-11-21. Different file from the Linux piper archive. |

A mismatch deletes the partial download and stops. The zip is removed after a good unpack.

## CI

`.github/workflows/s14-windows-install.yml` runs PSScriptAnalyzer and Pester on `windows-latest`. GitHub Actions only loads workflows from `.github/workflows`, so that file sits outside this folder. The commands it runs live in `ci/`. The tests do not download models and do not build Rust.

## Licence

Perpetual Open MIT. The `LICENSE` file in this folder is a byte copy of the repository `LICENSE`.
