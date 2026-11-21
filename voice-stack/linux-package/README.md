<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# Samantha Linux package

User install for the local Samantha companion on Ubuntu, WSL, and a Galion OS image that runs systemd. This folder is its own Cargo workspace. It is not a member of the repository workspace.

The process is a loopback supervisor. It serves the one-dot page, ticks every 80 ms, and reports whether the public Vosk and Piper files are on disk. HORBM is not linked. `POST /turn` returns 501 and does not invent a reply. Mood stays at the resting pad (arousal 0.3, valence 0.25, dominance 0.5) and the dot stays `idle`.

## What it reuses

- Sandbox keys from `deploy/systemd/templates/galion-voice-stage.service` and `deploy/systemd/galion-clone-cbx.service`: `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome`, `PrivateTmp`, `RestrictAddressFamilies`, `IPAddressDeny=any`, `IPAddressAllow=localhost`.
- Model pins from `scripts/asr/fetch-vosk.sh` and `scripts/a21/fetch-a21.sh`. The bytes are checked again here. They are not vendored.
- One-dot colours from `LOCKS.toml`: fill `#F5F5F7`, background `#0A0A0C`.
- The 80 ms clock number from `galion-heartbeat` (`FRAME_MS`). This crate does not link that crate. A late tick does not catch up.

Piper remains the public CPU voice (`en_US-ljspeech-medium`). Vosk small-en remains the public recognizer. The Chatterbox clone worker stays the existing system unit `galion-clone-cbx.service`. This package does not install it and does not fetch clone weights.

## What it does not do

- It does not download or store Samantha, Jarvis, s711, or any reference clip. Set `GALION_SAMANTHA_REF` to a local file. `install.sh` writes that path into a user drop-in and does not copy the file.
- It does not call a paid model or an HTTP API.
- It does not need sudo. It refuses to run as root unless you pass `--prefix` for a throwaway home.
- It does not install a system unit and it does not deploy.

## Install

```bash
make -C voice-stack/linux-package release
voice-stack/linux-package/install.sh --dry-run
voice-stack/linux-package/install.sh
systemctl --user enable --now galion-samantha.service
```

`--dry-run` prints the plan, writes nothing, and fetches nothing. A second install leaves matching files unchanged. `--check` re-hashes what is on disk and does not use the network. `--skip-models` installs the binary, the desktop file, and the unit only. `--prefix DIR` installs under `DIR` as if it were `$HOME` and does not call `systemctl`.

WSL needs systemd enabled in `/etc/wsl.conf` before `systemctl --user` works. This script does not edit that file.

### Layout

| Path | What |
|---|---|
| `~/.local/bin/galion-samantha` | supervisor |
| `~/.local/share/galion/samantha/models/` | public Vosk and Piper packs |
| `~/.config/systemd/user/galion-samantha.service` | user unit |
| `~/.local/share/applications/galion-samantha.desktop` | launcher |

The desktop entry runs `galion-samantha open`, which starts the user service and opens `http://127.0.0.1:7881/`. That is the companion socket (`GALION_SAMANTHA_WS`). Port 7879 stays with the hard loop (`GALION_HARD_ADDR`). The bind is numeric loopback. `0.0.0.0` and public addresses are refused.

Public models, when the download finishes on x86_64:

- Vosk `vosk-model-small-en-us-0.15` (Apache-2.0) and libvosk 0.3.45
- Piper 2023.11.14-2 and `en_US-ljspeech-medium` (MIT runtime, public-domain LJSpeech voice)

Those binaries are pinned for linux x86_64, the same limit as the existing fetch scripts. On another architecture the unit still installs, and the script says the binaries were skipped. The Piper smoke test writes raw PCM in a temporary file, prints the byte count, and deletes the file.

## WSL (Ubuntu 24.04)

Galion One Voice runs inside the `Galion-Ubuntu` WSL2 distro. Store that distro on `G:\WSL\Galion-Ubuntu`. Galion OS desktop and the Obsidian desktop are separate installs beside it. This script installs the voice service.

From Windows, import Ubuntu 24.04 onto that path:

```powershell
wsl --install -d Ubuntu-24.04
wsl --export Ubuntu-24.04 G:\WSL\ubuntu-24.04.tar
wsl --unregister Ubuntu-24.04
wsl --import Galion-Ubuntu G:\WSL\Galion-Ubuntu G:\WSL\ubuntu-24.04.tar --version 2
```

`systemctl --user` needs systemd. Add this to `/etc/wsl.conf` inside the distro, then run `wsl --shutdown` from Windows and open `Galion-Ubuntu` again. The installer does not edit that file.

```ini
[boot]
systemd=true
```

Windows opens the service at `http://127.0.0.1:7881/`. WSL2 localhost forwarding carries that loopback port across. Keep it in `%UserProfile%\.wslconfig`:

```ini
[wsl2]
localhostForwarding=true
```

The process binds numeric loopback. `0.0.0.0` is refused.

### Install

CPU packages, once:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libasound2-dev libasound2-plugins curl unzip tar ca-certificates
```

Rust 1.85 or newer (`rustup toolchain install 1.85.0` then `rustup default 1.85.0`). From the repository:

```bash
voice-stack/linux-package/wsl/install.sh --dry-run
voice-stack/linux-package/wsl/install.sh
systemctl --user enable --now galion-one-voice.service
```

A second run leaves matching files unchanged. `--check` re-hashes files and does not use the network. `--prefix DIR` writes under that directory and does not call `systemctl`. `--skip-build` and `--skip-models` skip those steps. `--skip-audio` skips PulseAudio setup.

The workspace build is:

```bash
cargo build --release --locked --manifest-path voice-stack/Cargo.toml
```

`CARGO_TARGET_DIR` is `voice-stack/target`. The command passes no CUDA feature. The workspace excludes the `vad` crate, so this build does not download `ort`. Silero's ONNX file is still fetched for the server.

### Models

Public files land in `voice-stack/linux-package/wsl/var/`, which is gitignored:

| Path | What |
|---|---|
| `var/models/vad/silero_vad.onnx` | Silero VAD v5.1.2 (MIT) |
| `var/models/asr/vosk-model-small-en-us-0.15/` | Vosk small English (Apache-2.0) |
| `var/models/tts/en_US-ljspeech-medium.onnx` | Piper LJ Speech (public domain) |

libvosk and the Piper runtime for linux x86_64 are fetched beside them. The pins match `scripts/asr/fetch-vosk.sh` and `scripts/a21/fetch-a21.sh`. Speech clips and `GALION_SAMANTHA_REF` stay off this path.

### Audio

WSLg publishes its PulseAudio server at `/mnt/wslg/PulseServer`. The installer sets `PULSE_SERVER` and writes `~/.config/galion/one-voice.asoundrc` so ALSA's `default` device is that server. `~/.asoundrc` is left alone. `audio-io` reaches Pulse through the ALSA plugin in `libasound2-plugins`.

A machine with no WSLg socket, including the `ubuntu:24.04` CI container, prints `WSL_AUDIO=SKIPPED` and the install continues. `--require-audio` fails that case.

Interactive shells:

```bash
. ~/.config/galion/wsl-audio.sh
```

The installer adds that line to `~/.bashrc` between `# BEGIN galion-one-voice` and `# END galion-one-voice`.

### Service

`~/.config/systemd/user/galion-one-voice.service` runs `~/.local/bin/galion-one-voice` on `127.0.0.1:7881`. The launcher execs `voice-stack/target/release/samantha-local` when that binary exists, and otherwise `galion-samantha serve`. `GALION_ONE_VOICE_BIN` selects another executable. It is exec'd with no extra arguments and inherits `GALION_SAMANTHA_WS`. The one-dot page and the VAD, ASR, brain, and Piper wiring stay in the voice-stack crates.

`galion-samantha.service` binds the same port. Enable one of the two units.

The sandbox matches the Samantha unit: `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome`, `IPAddressDeny=any`, `IPAddressAllow=localhost`. A drop-in adds read-only paths for `var/` and the WSLg socket directory.

### CI

`.github/workflows/voice-stack-wsl.yml` runs `voice-stack/linux-package/wsl/ci/container.sh` in an `ubuntu:24.04` container. That image has no sound device and no `/mnt/wslg/PulseServer`. The audio check skips. The job does not download models and does not deploy.

## Checks

```bash
make -C voice-stack/linux-package check
```

That runs shellcheck, `systemd-analyze verify`, the exposure ceiling, `cargo test`, and the offline install tests. CI is `.github/workflows/voice-stack-linux-package.yml` because GitHub only loads workflows from the repository root. The workflow runs this folder's scripts and does not download models.

## Licence

Code in this folder is the repository's Perpetual Open MIT licence (`LICENSE`). Downloaded models keep their own upstream licences.
