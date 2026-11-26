<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# Linux package (`galion-samantha`)

User install for the configured persona's local companion on Ubuntu, WSL, and a Galion OS image that runs systemd. The package, binary, and unit names stay `galion-samantha`. This folder is its own Cargo workspace. It is not a member of the repository workspace.

The process is a loopback supervisor. It serves the one-dot page, ticks every 80 ms, and reports whether the public Vosk and Piper files are on disk. HORBM is not linked. `POST /turn` returns 501 and does not invent a reply. Mood stays at the resting pad (arousal 0.3, valence 0.25, dominance 0.5) and the dot stays `idle`.

## What it reuses

- Sandbox keys from `deploy/systemd/templates/galion-voice-stage.service` and `deploy/systemd/galion-clone-cbx.service`: `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome`, `PrivateTmp`, `RestrictAddressFamilies`, `IPAddressDeny=any`, `IPAddressAllow=localhost`.
- Model pins from `scripts/asr/fetch-vosk.sh` and `scripts/a21/fetch-a21.sh`. The bytes are checked again here. They are not vendored.
- One-dot colours from `LOCKS.toml`: fill `#F5F5F7`, background `#0A0A0C`.
- The 80 ms clock number from `galion-heartbeat` (`FRAME_MS`). This crate does not link that crate. A late tick does not catch up.

Piper remains the public CPU voice (`en_US-ljspeech-medium`). Vosk small-en remains the public recognizer. The Chatterbox clone worker stays the existing system unit `galion-clone-cbx.service`. This package does not install it and does not fetch clone weights.

## What it does not do

- It does not download or store a reference clip. Set `GALION_VOICE_REF` to a local file. `GALION_SAMANTHA_REF` is a deprecated fallback and logs one warning. `install.sh` writes that path into a user drop-in and does not copy the file.
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

The desktop entry runs `galion-samantha open`, which starts the user service and opens `http://127.0.0.1:7881/`. That is the companion socket (`GALION_VOICE_WS`). `GALION_SAMANTHA_WS` is a deprecated alias, and `GALION_SAMANTHA_BIND` is the oldest name. Port 7879 stays with the hard loop (`GALION_HARD_ADDR`). The bind is numeric loopback. `0.0.0.0` and public addresses are refused.

Public models, when the download finishes on x86_64:

- Vosk `vosk-model-small-en-us-0.15` (Apache-2.0) and libvosk 0.3.45
- Piper 2023.11.14-2 and `en_US-ljspeech-medium` (MIT runtime, public-domain LJSpeech voice)

Those binaries are pinned for linux x86_64, the same limit as the existing fetch scripts. On another architecture the unit still installs, and the script says the binaries were skipped. The Piper smoke test writes raw PCM in a temporary file, prints the byte count, and deletes the file.

## WSL (Ubuntu 24.04)

### Entry point

The Galion OS installer calls one script. The checkout is `G:\galion\src\galion-one-voice`. Inside the guest that path is `/mnt/g/galion/src/galion-one-voice`.

```bash
voice-stack/linux-package/wsl/install.sh
```

From Windows, after the `Galion-Ubuntu` distro is registered:

```powershell
wsl -d Galion-Ubuntu -- bash /mnt/g/galion/src/galion-one-voice/voice-stack/linux-package/wsl/install.sh
```

The script prints these lines when the paths are on G::

```text
GALION_WSL_ENTRY=voice-stack/linux-package/wsl/install.sh
GALION_WSL_BIND=127.0.0.1:7881
GALION_WSL_DISTRO=Galion-Ubuntu
GALION_WSL_DISTRO_PATH=G:\galion\wsl\Galion-Ubuntu
GALION_WSL_CHECKOUT=G:\galion\src\galion-one-voice
```

Exit 0 means the user unit for `127.0.0.1:7881` is in place. A second run leaves matching files unchanged. The distro disk is `G:\galion\wsl\Galion-Ubuntu\ext4.vhdx`. The checkout and the gitignored model directory stay on `G:`. A model directory or prefix on another Windows drive is refused.

The script builds the nested `voice-stack` workspace with `cargo build --manifest-path voice-stack/Cargo.toml`. Root `cargo test` does not build that workspace. The `vad` crate stays out of the build because it depends on `ort`.

The unit listens on `127.0.0.1:7881` only. It does not bind one-dot `127.0.0.1:7878`, the hard loop `127.0.0.1:7879`, the clone worker `127.0.0.1:7880`, Hermes `127.0.0.1:8642`, or nerve `127.0.0.1:8670`. A nerve address is that numeric loopback. The hostname `localhost` is refused. This step does not start Ollama, an LLM, Hermes, nerve, or the Research gateway. The reply stays with HORBM. Until that brain is linked, the server does not invent an answer.

Galion OS desktop and the Obsidian desktop are separate installs. This script is the voice step.

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

libvosk and the Piper runtime for linux x86_64 are fetched beside them. The pins match `scripts/asr/fetch-vosk.sh` and `scripts/a21/fetch-a21.sh`. Speech clips and `GALION_VOICE_REF` stay off this path. `GALION_SAMANTHA_REF` is only the deprecated fallback.

### Audio

WSLg publishes its PulseAudio server at `/mnt/wslg/PulseServer`. The installer sets `PULSE_SERVER` and writes `~/.config/galion/one-voice.asoundrc` so ALSA's `default` device is that server. `~/.asoundrc` is left alone. `audio-io` reaches Pulse through the ALSA plugin in `libasound2-plugins`.

A machine with no WSLg socket, including the `ubuntu:24.04` CI container, prints `WSL_AUDIO=SKIPPED` and the install continues. `--require-audio` fails that case.

Interactive shells:

```bash
. ~/.config/galion/wsl-audio.sh
```

The installer adds that line to `~/.bashrc` between `# BEGIN galion-one-voice` and `# END galion-one-voice`.

### Service

`~/.config/systemd/user/galion-one-voice.service` runs `~/.local/bin/galion-one-voice` on `127.0.0.1:7881`. The launcher execs `voice-stack/target/release/samantha-local` when that binary exists, and otherwise `galion-samantha serve`. `GALION_ONE_VOICE_BIN` selects another executable. It is exec'd with no extra arguments and inherits `GALION_VOICE_WS` (`GALION_SAMANTHA_WS` is the deprecated alias). The unit sets no `GALION_LLM` and no nerve URL.

`galion-samantha.service` binds the same port. [`deploy/linux/`](../../deploy/linux/README.md) is a third listener for that socket, with a health restart. Enable one listener.

### GPU mouth (optional)

The WSL unit above stays on CPU (`GALION_CPU=1`, no `GALION_TTS_DEVICE`). A separate Piper mouth can use the laptop GPU. It binds `127.0.0.1:8002` only, and it falls back to CPU when CUDA is not actually there. Caches and the bench log stay on `G:`. The steps are in [`voice-stack/mouth/README.md`](../mouth/README.md). This install does not start that process and does not download a CUDA library.

The sandbox matches the `galion-samantha` unit: `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome`, `IPAddressDeny=any`, `IPAddressAllow=localhost`. A drop-in adds read-only paths for `var/` and the WSLg socket directory.

### CI

`.github/workflows/voice-stack-wsl.yml` runs `voice-stack/linux-package/wsl/ci/container.sh` in an `ubuntu:24.04` container. That image has no sound device and no `/mnt/wslg/PulseServer`. The audio check skips. The job does not download models and does not deploy.

## Checks

```bash
make -C voice-stack/linux-package check
```

That runs shellcheck, `systemd-analyze verify`, the exposure ceiling, `cargo test`, and the offline install tests. CI is `.github/workflows/voice-stack-linux-package.yml` because GitHub only loads workflows from the repository root. The workflow runs this folder's scripts and does not download models.

## Licence

Code in this folder is the repository's Perpetual Open MIT licence (`LICENSE`). Downloaded models keep their own upstream licences.
