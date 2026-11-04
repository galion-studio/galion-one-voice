# R0–R3 VPS — Dry-Run Checklist & Script Inventory (W20-13)

*2026-11-02, Europe/Zagreb (CET, UTC+1). Local only. No SSH, no VPS contact, GALION_VPS_HOST never set.*

## Script inventory
| Script | Purpose | Connects? | No-arg exit | `--execute` exit |
|---|---|---|---|---|
| `scripts/deploy/harden-dryrun.sh [host]` | Prints 8 hardening steps (root pw, galion user, key, sshd, ufw, tunnel) | No (heredoc print only) | 0 | 2 (refuses) |
| `scripts/deploy/release-dryrun.sh [sha] [ver]` | Prints release gate + install/rollback steps | No | 0 | 2 (refuses) |
| `scripts/deploy/remote-r0-dryrun.sh` | Listed as done in `w3-briefs/12-DEPLOY.md` | **MISSING on disk** | — | — |
| `galion-gates/run-gates.sh` (lines ~101–111) | Remote R0–R3 block: ssh BatchMode, tar-copy, run `--strict-cv` on VPS | **Yes, only if `GALION_VPS_HOST` set**; else R0–R3 marked PENDING | — | — |

Verified locally (exit codes above) with `GALION_VPS_HOST` unset.

**Safety note:** `run-gates.sh` is the only path that can touch the VPS; it is gated purely by an env var. Recommend (not applied) an extra explicit `--remote` flag + `GALION_VPS_ALLOW=1` double opt-in.

## Owner prerequisites (human-only, before any R0)
- [ ] **Root password rotation** via provider console; store in vault.
- [ ] **Non-root user**: `adduser --disabled-password galion`.
- [ ] **Key-only SSH**: operator ed25519 pubkey → `/home/galion/.ssh/authorized_keys` (700/600); verify in 2nd session.
- [ ] `/etc/ssh/sshd_config.d/10-galion.conf`: `PermitRootLogin no`, `PasswordAuthentication no`, `AllowUsers galion`; `sshd -t` then reload; re-verify.
- [ ] ufw default deny, allow 22/tcp; web bound to 127.0.0.1, accessed via `ssh -N -L 8080:127.0.0.1:8080`.
- [ ] **K4**: owner go-ahead for push + CI (release gate requires pushed SHA, CI green, artifact sha256 match, human approval).

## R0–R3 dry-run checklist (after prerequisites, owner-triggered)
- [ ] R0: probe v0.1.0 reachability (prebuilt binary, no build on VPS).
- [ ] R1–R3: `run-gates.sh --strict-cv` remote results recorded in `galion-gates/results/`.
- [ ] Release: copy artifact to `/opt/galion/<ver>/`, `sha256sum -c`, symlink `current`, scoped-sudo restart, rollback path tested, LEDGER entry.

## Status
R0–R3 **PENDING** — blocked on owner. Agents perform nothing on the VPS.

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
