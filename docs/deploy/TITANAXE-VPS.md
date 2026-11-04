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

## Planned public path through Caddy (audio routes)

- Public routes that must proxy via Caddy to `127.0.0.1:7878`: `/`, `/dot`, `/mic`, `/turn`, `/reply/*`. Everything else stays 404 at the edge.
- Body size limits: `/`, `/dot`, `/reply/*` keep `max_size 1KB`. Audio uploads (`/mic`, `/turn`) need a separate matcher with `request_body { max_size 2MB }` and a longer `read_body` timeout (~15s); tune once real clip sizes are measured.
- Port 7878 stays loopback-only (WebSurface refuses public binds); do not open it in the firewall. Caddy :80/:443 are the only public listeners.
- Status: documented in `deploy/Caddyfile` comments only; the live config is unchanged.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
