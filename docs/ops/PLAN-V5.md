# Plan V5 — deploy track

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

Plan V4 is the dry-run that already exists (`scripts/deploy/harden-dryrun.sh`,
`scripts/deploy/release-dryrun.sh`, `docs/agent-wave20/R0-R3-DRY-RUN.md`,
`docs/deploy/TITANAXE-VPS.md`). Plan V5 is the same discipline applied to the
stack after the phone path: `/turn`, `/reply`, the turn worker, the orb page.
It does not open a connection.

## 1. Target

```text
phone  -- HTTPS :443 -->  edge (ngrok today, Caddy later)
                              |
                              |  plain HTTP
                              v
                         127.0.0.1:7878
                         dot_console_demo --web 127.0.0.1:7878
                         WebSurface refuses any other bind
```

Host, when a human has finished the console work: Titanaxe VPS `51.38.137.193`.
SSH user in the V4 draft: `galion`, key only. This plan does not ship a key,
does not read one out of the environment, and does not try port 22.

Port 7878 is never public. 22, 80, and 443 are the only ports the V4 notes
open, and only after the human steps below.

## 2. What is already on disk

| Piece | Where | Connects? |
|---|---|---|
| Loopback screen | `scripts/run-v1-screen.sh` | no |
| Phone smoke | `scripts/check-phone-pipeline.sh` | loopback, or an `NGROK_URL` you already have |
| Hardening dry-run | `scripts/deploy/harden-dryrun.sh` | no; `--execute` exits 2 |
| Release dry-run | `scripts/deploy/release-dryrun.sh` | no; `--execute` exits 2 |
| Gate runner | `galion-gates/run-gates.sh` | remote half only if `GALION_VPS_HOST` is set |
| Caddy + unit draft | described in `docs/deploy/TITANAXE-VPS.md` | not applied |
| systemd unit | drafted there: loopback, restart, sandbox, memory cap | not installed |

The V4 probe of 51.38.137.193 was inconclusive (TCP accept from this network
is not proof the VPS answered). Plan V5 does not repeat that probe.

## 3. Public routes

Caddy (later) and ngrok (now) must forward:

| Method | Path | Body |
|---|---|---|
| GET | `/`, `/screen` | the one page |
| GET | `/dot` | JSON, includes `reply` when a turn finished |
| POST | `/mic` | small JSON, RMS only |
| POST | `/turn` | WAV or PCM16, cap 4 MiB (`MAX_BODY_BYTES`) |
| GET | `/reply/<id>.wav` | the spoken reply, RAM |
| GET | `/health` | `ok` |
| OPTIONS | those paths | CORS preflight |

Anything else stays 404 at the edge. The app also 404s `/frame` (removed on purpose).

Edge body limits from the V1 deploy note still apply, with one correction the
phone path forces: `/turn` needs about 4 MiB and a read timeout of at least
10 seconds (`REQ_DEADLINE`), not the 1 KB page limit. `/mic` stays small.
`/reply/*` is a download of a short WAV; do not cap it at 1 KB or the phone
gets a truncated voice.

## 4. Today, without a key

Public HTTPS is ngrok, started by a human on the machine that runs the screen:

```bash
scripts/run-v1-screen.sh
# other terminal, staging only — OAuth or basic auth is required.
# Put the password in the environment. Do not write it into the repo.
ngrok http 127.0.0.1:7878 --basic-auth "$GALION_NGROK_USER:$GALION_NGROK_PASS"
```

The page sends `ngrok-skip-browser-warning: 1` on its fetches. The phone must
use the `https://` name or the browser will refuse the microphone. OAuth on
the tunnel is the other acceptable auth. Details: `docs/galion-voice-v2/SECURITY-EDGE.md`.

Check:

```bash
scripts/check-phone-pipeline.sh --no-start
NGROK_URL=https://<the-name-ngrok-printed> scripts/check-phone-pipeline.sh --no-start
```

`GALION_ASR` defaults to `interim` inside `run-v1-screen.sh` only. That is a
labelled placeholder (`asr=interim-placeholder`). It is not recognition.
Unsetting it restores refusal (`AsrUnratified`) and the two-tone "turn arrived"
signal. `GALION_LLM` defaults to `canned` in that script. `GALION_SPEECH`
defaults to `auto` (Piper if `scripts/a21/fetch-a21.sh` has been run, else flite).

CORS sends `Access-Control-Allow-Origin: *` only while `GALION_CORS_ORIGINS`
is unset (the local V1 demo). Staging must set that variable to the exact
`https://` origin. A foreign `Origin` is then 403 with no allow-origin header,
and `/turn` requires the session token from `POST /session` (body, not URL).
A literal `*` in the variable is not the wildcard.

Every attempt, including a dry-run and a failed check, gets a LEDGER line:
date (Europe/Zagreb), what was run, result, and the fact that SSH was not used.

## 5. Human steps before any VPS deploy

These are copied forward from Plan V4 because they are still undone.

- **H1.** **Approved-pending-setup** (2026-11-03, corrected by t198u).
  A human installs **per-user anonymized access codes** on Titanaxe
  `51.38.137.193`. Every user gets their own code. There is no shared
  `galion` pubkey. This plan does not SSH, does not generate a code, and
  does not store one. Confirm a second session before closing the one that
  installed access.
- **H2.** Public name: a DNS A record to `51.38.137.193`, or the sslip.io
  fallback written in the Titanaxe note. Set `GALION_ACME_EMAIL` to a real address.
- **H3.** **Decided.** Staging and any public demo are behind login
  (ngrok OAuth or basic auth). CORS is the exact `https://` origin only.
  No wildcard. The local loopback demo may still omit `GALION_CORS_ORIGINS`.
  The Caddy auth block stays the human's to enable. Do not commit a password.
- **H4.** Owner approval to run a release. The dry-run script is the checklist,
  not the approval.

Until the human finishes H1, "deploy" means section 4. An agent must not
invent a code or a key. Approval is not a connection.

## 6. What "auto merge + deploy" means

**Merge, when checks are green.**

A pull request may merge when all of these are true:

1. `scripts/check-locks.sh` passes.
2. `scripts/check-no-forbidden.sh` passes.
3. `cargo test --workspace` passes on Rust 1.85.
4. Phone surface tests pass: `galion-render` with feature `web`,
   `galion-cascade` with feature `cascade`, `galion-console` with feature `phone-turn`.
5. The diff does not change `LOCKS.toml` values. A lock edit is an owner commit,
   not an auto-merge.
6. The diff does not add a secret, a weight file, or a GGML-family runtime.

`.github/workflows/gates.yml` runs that set on pull requests (Rust 1.85).
Auto-merge is the owner's policy for that green set. It is not a robot with
deploy rights. A red check does not merge. A lock edit does not auto-merge
even if tests are green; an owner reads it.

**Deploy, today (no key).**

Auto-merge does not deploy. The follow-up is:

1. LEDGER line for the merge.
2. `scripts/deploy/release-dryrun.sh` printed and saved. `--execute` must keep
   exiting 2.
3. ngrok, if a human still wants a phone URL, pointed at the new binary on
   loopback. That is a restart of `run-v1-screen.sh` on the demo machine, not
   an SSH session.

**Deploy, when a key exists.**

The same dry-run script becomes the recipe a human runs, in order:

1. Build the release package locally (the V4 notes name `package-release.sh`;
   that script is not in this tree — do not invent a second packer; the
   release dry-run's own steps are the checklist until the owner adds the packer back).
2. `sha256sum` recorded in LEDGER before copy.
3. Copy to `/opt/galion/<ver>/` over the key the human installed. Verify the sum on the VPS.
4. Symlink `current`. Restart the systemd unit. The unit binds `127.0.0.1:7878` only.
5. Caddy reloads the route list in section 3. Port 7878 stays closed on the firewall.
6. From the VPS itself, `scripts/check-phone-pipeline.sh --no-start` against
   loopback. From a phone, the same script with the public HTTPS name.
7. R0–R3 via `galion-gates/run-gates.sh` only after H4, with `GALION_VPS_HOST`
   set by the human in that session. Results file committed or attached, LEDGER updated.
8. Rollback is the previous symlink. The dry-run already lists it. Practice it
   once before calling the host production.

If the key is present but H2 or H4 is not, stop after the local package and
the LEDGER line. A key is not approval to publish.

## 7. systemd and Caddy, when they are installed

The unit drafted in the Titanaxe note is the one to install: `Restart=always`,
sandboxing, a memory cap, `User=galion`, bind `127.0.0.1:7878`,
`Environment=GALION_ASR=interim` only while the placeholder is the demo,
`GALION_LLM=canned`, `GALION_SPEECH=auto`, and the A21 pack root if the weights
were fetched on that machine. No raw-audio path in the unit. `NoNewPrivileges`
and a read-only root are preferred; the unit's own draft wins if it already
says so — do not fork a second unit file in this plan.

Caddy terminates TLS, sets HSTS, forwards the routes in section 3, and sends
the same security headers the app already sends (duplicating `nosniff` and
`Referrer-Policy` at the edge is fine). Microphone permission stays
`microphone=(self)`.

## 8. Recording

Append to `LEDGER.md`, newest at the bottom of the deploy notes, one row or
one short paragraph:

- when (Europe/Zagreb)
- command (no secrets, no key material)
- result (pass, fail, refused, dry-run)
- whether any socket to `51.38.137.193` was opened (for this program: no)

A failed check is still recorded. Silence is how a later reader invents a deploy.

— Maciej Grajczyk
