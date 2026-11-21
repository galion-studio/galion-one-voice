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

The desktop entry runs `galion-samantha open`, which starts the user service and opens `http://127.0.0.1:7879/`. The bind is numeric loopback. `0.0.0.0` and public addresses are refused.

Public models, when the download finishes on x86_64:

- Vosk `vosk-model-small-en-us-0.15` (Apache-2.0) and libvosk 0.3.45
- Piper 2023.11.14-2 and `en_US-ljspeech-medium` (MIT runtime, public-domain LJSpeech voice)

Those binaries are pinned for linux x86_64, the same limit as the existing fetch scripts. On another architecture the unit still installs, and the script says the binaries were skipped. The Piper smoke test writes raw PCM in a temporary file, prints the byte count, and deletes the file.

## Checks

```bash
make -C voice-stack/linux-package check
```

That runs shellcheck, `systemd-analyze verify`, the exposure ceiling, `cargo test`, and the offline install tests. CI is `.github/workflows/voice-stack-linux-package.yml` because GitHub only loads workflows from the repository root. The workflow runs this folder's scripts and does not download models.

## Licence

Code in this folder is the repository's Perpetual Open MIT licence (`LICENSE`). Downloaded models keep their own upstream licences.
