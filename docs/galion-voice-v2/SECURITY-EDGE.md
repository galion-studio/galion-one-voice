# V2-S3 · Secure edge and retention

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
TLS ends at the edge. The app speaks HTTP on loopback. This file is the
retention note the phone path owes.

## Where bytes live

| Data | Where | How long | On disk? |
|---|---|---|---|
| Mic RMS JSON | `MicTap` + one string | until the next POST, and `latest()` ignores it after the caller's max age | no |
| Utterance body | `TurnInbox` (`MAX_PENDING_TURNS` = 8) | until `take` / `take_latest`, or dropped when the cap hits | no |
| Reply WAV | `ReplyBox` (`MAX_STORED_REPLIES` = 8) | until evicted by a newer reply | no |
| Reply PCM in the demo | `PhoneHost` deque | one 80 ms frame at a time, then dropped | no |
| Transcript | inside `TurnReply` in the worker channel | until the heartbeat takes it | no, and the demo log does not print it |

`web.rs` does not call `std::fs` except to read `/dev/urandom` when minting a
session token. `dot_console_demo` writes `summary.txt` only when PNG output is
on. `scripts/run-v1-screen.sh` passes `--no-png`.

There is no account, no cookie, no raw-audio folder. A debug dump of audio
would be a new flag and an owner decision. It does not exist.

Turn bodies are wiped when the `Turn` is dropped (overflow, take, end of the
host's use). Reply WAVs are wiped when evicted if this process is the last
holder. The phone host zeroes its playback queue on barge-in and as each
sample is consumed. The turn worker zeroes the input WAV after the lane
returns. The demo log line is `phone_reply_log_line`: id, ASR label, backend,
durations, and a short error. It has no parameter for PCM or transcript. An
error string that contains `RIFF`, `pcm`, or `transcript` is logged as
`redacted`.

## Headers

Every response carries:

- `Cache-Control: no-store`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: no-referrer`
- `Permissions-Policy: microphone=(self)`
- a Content-Security-Policy that allows the single inline page and the blob
  AudioWorklet, and nothing else (`frame-ancestors 'none'`, `base-uri 'none'`)

CORS: `GALION_CORS_ORIGINS` **unset** sends `Access-Control-Allow-Origin: *`.
That wildcard is only the local V1 default, so a laptop demo still works
before anyone has a stable name. **Staging must set exact origins**
(`https://<the-ngrok-or-caddy-host>`, and `http://127.0.0.1:7878` if a local
page is used too). A literal `*` in the variable is **not** the wildcard: it
matches nothing. The server echoes `Origin` only on an exact string match
(no suffix match, no credentials header). A foreign `Origin` gets **403** and
no `Access-Control-Allow-Origin`. A sensitive route (`POST /turn`, `POST /mic`,
`POST /session`, `GET /reply/…`) with the list set and no `Origin` is also 403.
`GET /health` without an `Origin` still answers, so a loopback probe works.
The allow-list is not a substitute for TLS or for edge auth.

## GDPR, stated narrowly

The phone path processes a voice recording of a person long enough to answer,
then drops it. That recording is personal data while it exists.

- Purpose: one spoken reply in the demo.
- Lawful basis is not chosen here. A public demo needs the owner's instruction
  before it is offered to people who did not build it.
- Retention: RAM only, bounded, process lifetime at worst, usually under a minute.
- No transcript log by default. `PhoneHost` prints id, ASR label, backend, and
  durations. It does not print the words.
- Withdrawal: closing the tab stops capture. Restarting the process drops every
  buffer. There is nothing to delete on disk because nothing was written.
- Processors: the machine running the demo, plus whoever terminates TLS
  (ngrok today, Caddy on the VPS later). ngrok's own retention is theirs, not ours.
- No special-category inference on the caller. `ProsodyEmotion` exists and is
  not wired. LOCKS `[consent] sense` stays deny.

Counsel still owns the A21 espeak-ng question and any public-demo notice.
This note does not replace that.

## Origin and session (no WebSocket)

Galion Voice V2 has no WebSocket. The phone path is HTTP. The cross-site
check that a WebSocket `Origin` test would do is applied to the sensitive
routes above.

The session token is the HTTP stand-in for "put the token in the first
message body, not the URL":

1. The page `POST`s `/session` with a JSON body (`{"op":"open"}`).
2. The response **body** is `{"token","ttl_s","transport":"body"}`. Default
   life is 120 seconds (`GALION_SESSION_TTL_SECS`, clamped to 15…900).
3. A token in the query string (`token`, `session`, `access_token`,
   `galion_session`) is **400**, and the allow-origin header is omitted.
4. Later calls send `X-Galion-Session` (required for binary `POST /turn` and
   `GET /reply/…wav`) or `"session"` inside the `POST /mic` JSON body.
   `GET /mic` does not repeat that field.

The token is required when `GALION_REQUIRE_SESSION=1`, and also whenever an
exact allow-list is set (staging). The local demo leaves both unset, so
`scripts/check-phone-pipeline.sh` still posts without a token. Tokens live in
RAM, are compared in constant time, and are wiped when they expire or fall
off the cap (32).

## AI Act Art. 50

The demo page shows, and speaks on the first tap, both of:

- HR: `Razgovarate s umjetnom inteligencijom. Galion je AI asistent, a ne osoba.`
- EN: `You are talking to an AI. Galion is an AI assistant, not a person.`

The first accepted `POST /turn` repeats those strings in `ai_disclosure`
(`article` 50). Later turns do not. This is on for the demo.
`GALION_AI_DISCLOSURE=0` strips the HTML block and omits the JSON.
That flag is for benches. A public demo leaves it on.

## Bind

`WebSurface::open` refuses `0.0.0.0` and `::`. Loopback (`127.0.0.1`, `::1`)
always binds. A public bind is possible only when **both** are set:

- `GALION_ALLOW_PUBLIC_BIND=1`
- auth: `GALION_REQUIRE_SESSION=1` **and** an exact `GALION_CORS_ORIGINS` list

This repository does not set the public-bind flag. Port 7878 stays off the
firewall. Do not widen the bind in a unit file or a tunnel command.

## Staging tunnel auth (ngrok)

ngrok's HTTPS certificate authenticates the server, not the caller. Anyone
who can open the URL can open the microphone. Staging must put auth on the
tunnel **before** the URL is shared. Pick OAuth or basic auth. Do not commit
the password, an OAuth client secret, an authtoken, or an allow-listed email.

OAuth (preferred), restricted to the owner in the ngrok dashboard or Traffic
Policy. The policy shape is the documented `oauth` action (Google or GitHub).
The owner writes the email allow-list outside this repo.

Basic auth, also outside the repo, password from the operator's environment:

```bash
ngrok http 127.0.0.1:7878 --basic-auth "$GALION_NGROK_USER:$GALION_NGROK_PASS"
```

IP restriction may sit beside basic auth. Turn the request inspector off for
a demo that carries voice, or clear it when the tunnel stops. Kill the tunnel
when it is idle. Set `GALION_CORS_ORIGINS` to the exact `https://` origin
ngrok prints. An unauthenticated `curl` of that URL must fail at ngrok
(401 or an OAuth redirect) before the bytes reach loopback. This repository
does not start ngrok and does not store those credentials.

Caddy on the VPS, later, is the same rule: TLS and auth at the edge, app
still bound to loopback. HSTS belongs there. No SSH from this change.
See `docs/ops/PLAN-V5.md`.

— Maciej Grajczyk
