# Deploy path · Titanaxe VPS 51.38.137.193 · DEPLOY-01 · 2026-11-02 (Europe/Zagreb)

**Status: PLAN + LOCAL ARTIFACTS ONLY. Nothing has connected to the VPS.**
Access design (2026-11-03, t198u): **per-user anonymized access codes**, not
one shared `galion` pubkey. Status **approved-pending-setup**. A human
installs the codes. This tree does not hold a code or a private key.

## Shape
```
phone/browser ──https:443──▶ Caddy (TLS, ACME) ──http──▶ 127.0.0.1:7878 dot_console_demo (WebSurface)
                 http:80 ──▶ Caddy (ACME HTTP-01 + redirect)
operator ──ssh:22 (key only, user galion)──▶ VPS
```
- WebSurface keeps binding loopback only (`refuses_public_bind` test). Caddy is the only public listener.
- Mic capture needs HTTPS, so the public name must have a real certificate:
  - preferred: a domain with an A record → 51.38.137.193 (human DNS step), or
  - zero-DNS default: `51.38.137.193.sslip.io` (Let's Encrypt HTTP-01 works with it).
- Edge allows only `/ /dot /mic /health /favicon.ico`, 1 KB request bodies, HSTS, `Permissions-Policy: microphone=(self)`.
- The page has no auth. While the demo is private, turn on the commented `basic_auth` block in `deploy/Caddyfile`.

## Public port plan
| Port | Who | Why |
|---|---|---|
| 22/tcp | operator | SSH, key-only, `AllowUsers galion`, `ufw limit` |
| 80/tcp | Caddy | ACME + redirect |
| 443/tcp | Caddy | TLS → 127.0.0.1:7878 |
| 7878 | — | **never public** |

## Files
| Path | What |
|---|---|
| `deploy/Caddyfile` | reverse proxy + TLS + headers + route allow-list |
| `deploy/galion-voice.service` | systemd unit: loopback bind, `Restart=always`, sandboxing, 256 MB cap |
| `scripts/deploy/harden-dryrun.sh [host] [--preflight] [--probe]` | 14-step hardening plan; local file checks; read-only TCP/HTTP probe. `--execute` is refused |
| `scripts/deploy/package-release.sh [ver]` | builds `target/release-pkg/galion-voice-<ver>-<arch>.tar.gz` + `.sha256` locally |
| `scripts/deploy/release-dryrun.sh [ver] [host]` | gate + scp/verify/symlink/restart/rollback plan. `--execute` is refused |

## Hermes gateway key

The voice brain (`HermesLlm::from_env` in `crates/galion-cascade`) reads the gateway bearer from a dotenv file when both of these are set:

- `GALION_LLM_KEY_FILE` — path of the file
- `GALION_LLM_KEY_NAME` — the `NAME=value` line. Same shape as Research `GALION_RESEARCH_KEY_FILE` / `GALION_RESEARCH_KEY_NAME`. On the Hermes gateway file that name is `API_SERVER_KEY`.

`GALION_LLM_KEY` is the fallback when the file is unset, unreadable, or has no matching line. The key is sent as `Authorization` and is never logged. `Debug` prints `set` or `none`.

`deploy/systemd/templates/galion-voice-stage.service` leaves those lines commented. The key is not written in the unit, the override, or this note. A human sets the path in a drop-in and adds that path to `ReadOnlyPaths`, because `ProtectSystem=strict` otherwise hides it. Mode `0600`, owner `galion`. This note does not install the unit and does not read a key.

## Dry-run results (2026-11-02, CET)
- `harden-dryrun.sh --preflight`: all files present; Caddy upstream is loopback 7878; the unit binds loopback. `caddy validate` was skipped because caddy is not installed on the box. `systemd-analyze verify` cannot run unprivileged here.
- `package-release.sh v1-20261102`: built `galion-voice-v1-20261102-x86_64.tar.gz`, sha256 `d128efcd2b12df18ac25c345f301c4cfd12598e5792c77c6311395ed3a9a3196`.
- `harden-dryrun.sh --probe`: TCP connects to 22/80/443 succeeded, but HTTP on 80/443 timed out. This box's egress proxy probably accepts the connect itself, so the result is **inconclusive**. It does not show that anything is running on the VPS.

## Human steps
- **H1** Approved-pending-setup. A human installs per-user anonymized access codes through the Titanaxe console or rescue mode. Every user gets their own code. No shared key. Nothing in this repo connects, and no code is generated here.
- **H2** Choose the public name: a domain (set its DNS A record) or the sslip.io default. Set `GALION_ACME_EMAIL` to a real address.
- **H3** Decided: the public demo is behind login (OAuth or basic auth), with exact https CORS origins. The human turns the edge block on. No password in git.
- **H4** Approve the release (gate in `release-dryrun.sh`).

— Maciej Grajczyk

## Voice preview (`GALION_PREVIEW_AUDIO_DIR`)

The live page plays a pre-rendered clip when a tap on the orb has no live session: `POST /session` fails, or it does not return within 1.5 seconds, or the microphone cannot open. The browser speech engine is not used. A second tap stops the clip.

The process reads `GALION_PREVIEW_AUDIO_DIR` when the web surface is created. The directory is outside this repo. Do not commit the audio. A missing directory or a missing file is HTTP 404 with `Cache-Control: no-store`, and the page shows the caption text instead.

```
$GALION_PREVIEW_AUDIO_DIR/
  voice-preview.opus    Ogg Opus. Content-Type: audio/ogg; codecs=opus
  voice-preview.mp3     Content-Type: audio/mpeg
  voice-preview.wav     Content-Type: audio/wav
  voice-preview.json    {"text":"...","segments":[{"start":0.0,"end":1.2,"text":"..."}]}
```

`start` and `end` are seconds. The page tries opus, then mp3, then wav, skipping a format the browser cannot decode. The JSON is the synced caption script. If every audio file is missing and the JSON is present, the page shows `text` (or the segments joined). If the JSON is missing too, it shows the Art. 50 lines as text.

The service answers only those four names under `/preview/`. `GET` and `HEAD` send `Accept-Ranges: bytes`, an `ETag` of `"<bytes>-<mtime>"`, and `Cache-Control: public, max-age=3600`. One `Range: bytes=` interval returns `206` with `Content-Range`. A range that starts past the end is `416`. `If-None-Match` with the current ETag returns `304`. Anything larger than 16 MiB is `413`. A symlink that resolves outside the directory is `404`.

On the stage unit, a commented `Environment=GALION_PREVIEW_AUDIO_DIR=/opt/galion/voice/preview` sits next to the other environment lines. That directory is already under `ReadWritePaths=/opt/galion/voice`. A path outside that tree needs its own `ReadOnlyPaths` entry because `ProtectSystem=strict` hides it.

To hear the installed file without opening the microphone, open `/?preview=1` and tap the orb. The same player runs for the session fallback.

Caddy must proxy these in addition to the routes below. Do not apply the 1 KB body cap, and do forward `Range`:

- `/preview/voice-preview.opus`
- `/preview/voice-preview.mp3`
- `/preview/voice-preview.wav`
- `/preview/voice-preview.json`

## Planned public path through Caddy (audio routes)

- Public routes that must proxy via Caddy to `127.0.0.1:7878`: `/`, `/dot`, `/mic`, `/turn`, `/reply/*`, and the four `/preview/voice-preview.*` files above. Everything else stays 404 at the edge.
- Body size limits: `/`, `/dot`, `/reply/*` keep `max_size 1KB`. Audio uploads (`/mic`, `/turn`) need a separate matcher with `request_body { max_size 2MB }` and a longer `read_body` timeout (~15s); tune once real clip sizes are measured.
- Port 7878 stays loopback-only (WebSurface refuses public binds); do not open it in the firewall. Caddy :80/:443 are the only public listeners.
- Status: documented in `deploy/Caddyfile` comments only; the live config is unchanged.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
