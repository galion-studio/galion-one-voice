# Voice releases

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Repo tooling. The git tree has no filled release index.

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network.
Developed and distributed by Galion Labs.
Perpetual Open MIT License.
Copyright © 2026 Galion Labs. All rights reserved.

A voice release on disk is a directory named by the git commit:

```text
/opt/galion/voice/release/<sha>/MANIFEST.json
/opt/galion/voice/release/<sha>/<binary>
/opt/galion/voice/release/<sha>/helpers/<name>
/opt/galion/voice/release/current -> <sha>
/opt/galion/voice/release/INDEX.json
```

`current` is the symlink a human points at the release they want running. This repository does not move that symlink and does not contain a filled index. A manifest is written when someone runs the script against a local tree.

## Manifest

`scripts/deploy/write-release-manifest.sh` writes `release/<sha>/MANIFEST.json` and rebuilds `release/INDEX.json` from every manifest it finds under that root. It hashes the binary and each helper after the bytes are in the release directory. Build time is the clock in `Europe/Zagreb` (`+01:00` or `+02:00`). Pass `--built-at` only when you already have that stamp.

```bash
scripts/deploy/write-release-manifest.sh \
  --root "$VOICE_ROOT" \
  --sha "$SHA" \
  --binary "$BIN" \
  --feature dot-viz-web \
  --helper "$HELPER"
```

`$VOICE_ROOT` for the voice tree is `/opt/galion/voice`. `$SHA` is the git commit, 7 to 40 lowercase hex characters. `$BIN` is the release binary. For the screen, `scripts/run-v1-screen.sh` builds `dot_console_demo` with Cargo feature `dot-viz-web` and also builds `galion-asr-vosk`. Pass each helper you copied with `--helper`. The script copies a helper that is not already at `release/<sha>/helpers/<name>` and records the sha256 of the copy.

`MANIFEST.json` fields:

| Field | Meaning |
|---|---|
| `sha` | Release directory name |
| `built_at` | Local time in Europe/Zagreb |
| `timezone` | `Europe/Zagreb` |
| `binary` | File name of the release binary |
| `binary_sha256` | sha256 of that file |
| `features` | Cargo features passed to the script, sorted |
| `helpers` | Copied helper binaries: `name`, `path`, `sha256` |

`INDEX.json` lists those releases in `built_at` order and sets `current` from the `current` symlink when the link target is a sha. The script leaves the symlink alone.

`--execute` exits 2. The script does not open a socket.

The older checklist in `scripts/deploy/release-dryrun.sh` still refuses `--execute`. It is a separate dry-run.

## Stage unit

The stage unit template is `deploy/systemd/templates/galion-voice-stage.service`. Its `Description` is `Galion Voice stage` and does not carry a commit hash.

The SHA is read from the override file (`SHA=` in `deploy/systemd/templates/galion-voice-stage.override`, empty in git). `scripts/deploy/render-stage-unit.sh` writes a local drop-in whose `Description` is `Galion Voice stage (<sha>)` and whose `GALION_RELEASE_SHA` is that same value. Systemd uses the drop-in description. The renderer writes a directory you choose. It refuses `--execute` and refuses an output directory under `/etc`. A human installs the unit.

```bash
scripts/deploy/render-stage-unit.sh \
  --override "$OVERRIDE" \
  --out "$OUT"
```

The template `ExecStart` is the screen command from `scripts/run-v1-screen.sh`: `dot_console_demo` on `127.0.0.1:7878` for 3600 seconds, from `/opt/galion/voice/release/current`. `MemoryMax=256M` is the cap named in `docs/deploy/TITANAXE-VPS.md`. `docs/galion-voice-v2/symphony/STAGING-CPU.md` says the human raises that cap at install. The template keeps 256M.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
