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

`web.rs` does not call `std::fs`. `dot_console_demo` writes `summary.txt` only
when PNG output is on. `scripts/run-v1-screen.sh` passes `--no-png`.

There is no account, no cookie, no raw-audio folder. A debug dump of audio
would be a new flag and an owner decision. It does not exist.

## Headers

Every response carries:

- `Cache-Control: no-store`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: no-referrer`
- `Permissions-Policy: microphone=(self)`
- a Content-Security-Policy that allows the single inline page and the blob
  AudioWorklet, and nothing else (`frame-ancestors 'none'`, `base-uri 'none'`)

CORS: `GALION_CORS_ORIGINS` unset or `*` sends `Access-Control-Allow-Origin: *`
(the V1 phone default, because an ngrok name is ephemeral). Set it to a comma
separated list and the server echoes `Origin` only on an exact match. That is
the allowlist. It is not a substitute for TLS.

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

## Edge

ngrok or Caddy is the only public listener. `WebSurface::open` refuses a
non-loopback bind (`refuses_public_bind`). Port 7878 stays off the firewall.
See `docs/ops/PLAN-V5.md`.

— Maciej Grajczyk
