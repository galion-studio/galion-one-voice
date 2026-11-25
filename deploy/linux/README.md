<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# Linux supervision for samantha-local

Linux runtime supervision for the local voice preview, `samantha-local`, on `127.0.0.1:7881`. The folder is bash, curl, and a systemd user unit. The workflow runs on `ubuntu-24.04`.

A foreground start (`cargo run` in `voice-stack/local-server`, or the WSL launcher that execs the binary) ends when the process exits. This folder starts it again. Paths, the UI directory, the work directory, and the bind come from one env file.

`voice-stack/local-server/deploy/samantha-local.service` is a separate system-unit template with fixed `/opt/galion` paths. This folder is the user-session supervisor.

## What you get

- A crash (any exit other than 0) starts the process again.
- `GET /health` is checked on a timer. HTTP 200 and JSON `"ok": true` is healthy. Anything else, including a refused connection, counts as one failure. `SAMANTHA_HEALTH_FAILS` failures in a row restart the process. The default is 3. Failures during `SAMANTHA_HEALTH_GRACE` seconds after start are ignored so a slow boot can bind.
- The supervisor accepts `127.0.0.1:PORT` and refuses every other bind, including a wildcard, a hostname, and `[::1]`. The server binary still has its own loopback check.
- With a systemd user session, the unit's `Restart=on-failure` and `RestartSec=2` do the restart. After 8 starts in 120 seconds systemd stops trying. Exit 2 is a config error and is not restarted.
- With no systemd user session, `supervise.sh` is a foreground restart loop. The delay starts at `SAMANTHA_RESTART_BACKOFF_SEC` and doubles up to `SAMANTHA_RESTART_BACKOFF_MAX_SEC`. A generation that was healthy at least once resets the delay.

## Layout

| File | Role |
|---|---|
| `samantha-local.env.example` | placeholders only |
| `samantha-local.service` | systemd user unit |
| `supervise.sh` | health check and restart loop |
| `install.sh` | install, enable, start, stop, logs, status, rollback |
| `ci/smoke.sh` | shellcheck, unit check, and a bats-free smoke test |

Installed files, under the user home or `--prefix`:

| Path | Role |
|---|---|
| `~/.config/galion/samantha-local.env` | the env file, mode 600 |
| `~/.config/systemd/user/samantha-local.service` | the user unit |
| `~/.config/systemd/user/samantha-local.service.d/paths.conf` | read-only paths from the env file |
| `~/.local/bin/samantha-local-supervise` | the supervisor |

The unit reads `%h/.config/galion/samantha-local.env`. `%h` is the user home. The unit file uses that specifier and does not name a Windows binary.

## Env file

```bash
mkdir -p ~/.config/galion
cp deploy/linux/samantha-local.env.example ~/.config/galion/samantha-local.env
chmod 600 ~/.config/galion/samantha-local.env
```

Replace the three `/path/to/...` placeholders:

| Key | Meaning |
|---|---|
| `SAMANTHA_LOCAL_BIN` | the `samantha-local` binary from `voice-stack/local-server` |
| `GALION_SAMANTHA_UI` | directory that contains `index.html` |
| `SAMANTHA_LOCAL_WORKDIR` | working directory for the process |
| `GALION_SAMANTHA_WS` | `127.0.0.1:7881`, or another `127.0.0.1` port |

Each value is one word: letters, digits, and `/ . _ : + - @`. No spaces, no `~`, no substitution. Extra `KEY=VALUE` lines are passed through to the server (model directories, for example). Keep credentials and reference-clip paths out of a file you might commit. The example file has none.

`samantha-local` reads `GALION_SAMANTHA_WS` and `GALION_SAMANTHA_UI` itself. The supervisor exports the env file and does not add arguments, so a stand-in binary used in the smoke test can ignore them.

Create the work directory before enable. Re-run `install.sh install` after editing paths so `paths.conf` picks up the new directories. `ProtectHome=yes` hides the home directory except for the paths listed in the unit and that drop-in.

## Install

From the repository:

```bash
deploy/linux/install.sh --dry-run
deploy/linux/install.sh install
```

`--dry-run` writes nothing. A second install leaves matching files in place. The example env file is copied only when `~/.config/galion/samantha-local.env` is absent. An existing env file is left as it is.

The installer refuses to run as root unless you pass `--prefix DIR` for a throwaway directory. `--prefix` does not call `systemctl`.

## Enable (boot)

```bash
deploy/linux/install.sh enable
```

That runs `supervise.sh --check`, then:

```bash
systemctl --user daemon-reload
systemctl --user enable samantha-local.service
```

`WantedBy=default.target` attaches the unit to the user session. The session has to be up at boot:

```bash
loginctl enable-linger "$USER"
```

The installer does not run `loginctl`. On WSL, systemd has to be turned on inside the distro, then the distro started again:

```ini
[boot]
systemd=true
```

The installer does not edit `/etc/wsl.conf`. A real reboot is not part of CI. The smoke test checks `WantedBy=default.target`, the enable command, and `systemd-analyze verify`.

`enable` fails while the binary in the env file is missing or not executable. Fix the env file and run it again.

## Start

With a systemd user session:

```bash
deploy/linux/install.sh start
```

which is `systemctl --user start samantha-local.service`.

Open `http://127.0.0.1:7881/`. One check:

```bash
curl --fail --silent http://127.0.0.1:7881/health
```

Healthy output is JSON with `"ok": true`.

With no systemd user session, `start` runs the restart loop in the foreground:

```bash
~/.local/bin/samantha-local-supervise
```

Ctrl-C stops it. A stop signal exits 0 and does not schedule another start.

## Stop

```bash
deploy/linux/install.sh stop
```

With systemd that is `systemctl --user stop samantha-local.service`. The unit's `KillMode=control-group` stops the supervisor and the server. With no systemd it signals the pid in `$XDG_RUNTIME_DIR/samantha-local.pid` (or `~/.local/share/galion/samantha-local.pid`).

## Logs

```bash
deploy/linux/install.sh logs
journalctl --user -u samantha-local.service -f
```

`logs` prints the last 200 lines. `-f` follows. The restart loop, when it is in a terminal, writes to that terminal. Lines are prefixed `samantha-supervise:`.

`systemctl --user status samantha-local.service` is `install.sh status`.

## Rollback

```bash
deploy/linux/install.sh rollback
```

Rollback does four things:

1. `systemctl --user disable --now samantha-local.service` when a user session is up. Otherwise it signals the restart loop.
2. Restores `samantha-local.service.bak` when the first install had found a different unit and saved it. Otherwise it removes the unit.
3. Removes `samantha-local.service.d/` and `~/.local/bin/samantha-local-supervise`.
4. Leaves `~/.config/galion/samantha-local.env` in place.

`systemctl --user daemon-reload` runs after that when a user session is up. The server binary, UI directory, and work directory stay where the env file points.

The same steps by hand:

```bash
systemctl --user disable --now samantha-local.service
rm -f ~/.config/systemd/user/samantha-local.service
rm -rf ~/.config/systemd/user/samantha-local.service.d
rm -f ~/.local/bin/samantha-local-supervise
systemctl --user daemon-reload
```

Move `samantha-local.service.bak` back onto `samantha-local.service` first when that backup exists.

## One listener on 7881

These units also bind `127.0.0.1:7881`. Enable one of them:

- `galion-one-voice.service` from `voice-stack/linux-package/wsl/`
- `galion-samantha.service` from `voice-stack/linux-package/`
- `samantha-local.service` from this folder

## Sandbox

The user unit sets `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp`, `PrivateDevices`, `IPAddressDeny=any`, and `IPAddressAllow=localhost`. `localhost` here is the systemd loopback range. The process bind is still only `127.0.0.1`, checked by `supervise.sh` before exec.

`PrivateDevices=yes` matches the WSL user unit. Pulse through a unix socket still works. A direct ALSA device node needs a drop-in that sets `PrivateDevices=no`.

Memory cap is `MemoryMax=2G`, the same cap as `galion-one-voice.service`.

## Checks

```bash
deploy/linux/ci/smoke.sh
```

The script needs Bash 4.4 or newer, `shellcheck`, `systemd-analyze`, `curl`, and `python3`. It does not build the server, download models, or open a session. CI is `.github/workflows/deploy-linux.yml` on `ubuntu-24.04`.
