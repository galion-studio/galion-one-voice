# Staging

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The app binds `127.0.0.1` only. Port 7878 is not public. A phone microphone needs HTTPS, so staging is a tunnel in front of that loopback, with auth on the tunnel and an exact CORS origin.

```text
phone  -- HTTPS -->  ngrok now, Caddy later
                         |
                         v
                    127.0.0.1:7878
```

## Bring a staging name up

```bash
scripts/run-v1-screen.sh
# second terminal. Password from the environment.
ngrok http 127.0.0.1:7878 --basic-auth "$GALION_NGROK_USER:$GALION_NGROK_PASS"
```

Set `GALION_CORS_ORIGINS` to the exact `https://` origin the tunnel printed. A foreign `Origin` is then 403. `/turn` requires the session token from `POST /session`. A literal `*` in the variable is not a wildcard. OAuth on the tunnel is the other acceptable auth.

The page sends `ngrok-skip-browser-warning: 1` on its fetches. Open the `https://` name on the phone, then:

```bash
scripts/check-phone-pipeline.sh --no-start
NGROK_URL=https://<the-name> scripts/check-phone-pipeline.sh --no-start
```

Local `http://127.0.0.1:7878/` stays the desk environment. It does not need the tunnel. See [RUNBOOKS.md](RUNBOOKS.md) and [ONE-DOT.md](ONE-DOT.md).

## What is not a deploy

These print steps and open no connection. `--execute` exits 2.

```bash
scripts/deploy/harden-dryrun.sh
scripts/deploy/release-dryrun.sh
```

The host named in the plan, when a human has finished access setup, is the Titanaxe VPS `51.38.137.193`. This page does not open SSH and does not ship a key. Access is per-user codes, installed by a human. Notes: [PLAN-V5.md](../../ops/PLAN-V5.md), [SECURITY-EDGE.md](../SECURITY-EDGE.md), [EDGE-NGROK.md](../../agent-v1-demo/EDGE-NGROK.md), [TITANAXE-VPS.md](../../deploy/TITANAXE-VPS.md).

Routes the edge forwards: `GET /`, `GET /screen`, `GET /dot`, `POST /mic`, `POST /turn`, `GET /reply/<id>.wav`, `GET /health`, and CORS preflight. `/turn` needs about 4 MiB and a read timeout of at least 10 seconds. Anything else is 404.

## Gate

**PASS** when the process listens on `127.0.0.1` only, the public name requires auth, and `GALION_CORS_ORIGINS` is that exact origin. Evidence is the check command and the bind address.

**FAIL** when the app binds a public interface, when the tunnel has no auth, or when CORS is left as `*` on a shared name.

**BLOCKED** when no tunnel is up. Local loopback can still be scored on its own row. Staging stays **BLOCKED** until the `https://` name exists.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
