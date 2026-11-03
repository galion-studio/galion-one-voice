# EDGE-NGROK — public edge for the phone demo (S1 + S8)

Status checked 2026-11-02 21:50 CET. Docs + comments only, no new tunnel, no VPS access.

## Topology (today)
```
phone browser ──HTTPS (TLS 1.2/1.3, ngrok cert)──▶ ngrok edge
   https://unreeling-ventricle-deodorant.ngrok-free.dev
        │  agent tunnel (ngrok 3.39.13, /tmp/ngrok http 127.0.0.1:7878)
        ▼
 127.0.0.1:7878  dot_console_demo --web 127.0.0.1:7878  (galion-render WebSurface)
```
- **TLS terminates at ngrok.** The app speaks plain HTTP/1.1 on loopback only.
- **App stays loopback.** `WebSurface::open()` refuses any non-loopback bind
  (`galion-render/src/backends/web.rs`, `open()` → `RenderError::Unsupported`); `ss -ltnp`
  shows `127.0.0.1:7878` only. Nothing on the box listens publicly for this app.
- **Phone needs HTTPS** because `getUserMedia` (mic) is only exposed in a secure
  context; the ngrok URL gives that without us managing certs.
- Free-tier interstitial: non-browser clients / `fetch` send
  `ngrok-skip-browser-warning: 1` (allowed by CORS, see below).
- Free-tier limits (data cap, rate limits, latency): `docs/deploy/NGROK-AUDIO-UPLOAD.md`.
- Future VPS: same contract behind Caddy (`deploy/Caddyfile` comments mirror these
  routes). We reuse ngrok now and Caddy later; no custom proxy.

## Health (verified this run)
| Check | Result |
|---|---|
| ngrok agent API `127.0.0.1:4040/api/tunnels` | 1 tunnel `command_line`, https → `http://127.0.0.1:7878`, ~530 reqs served |
| `GET http://127.0.0.1:7878/health` | 200 `ok` |
| `GET https://…ngrok-free.dev/health` | 200 `ok` |
| `scripts/check-phone-pipeline.sh` local + `NGROK_URL` | PASS both (health, /mic, /dot) |
| `POST /turn`, `OPTIONS /turn` (local + public) | **404** — running binary (pid started 21:31) predates the `/turn` + CORS-headers build in web.rs; needs rebuild + restart by the web.rs owner |

Re-run: `NGROK_URL=https://unreeling-ventricle-deodorant.ngrok-free.dev scripts/check-phone-pipeline.sh`

Note: a `cloudflared tunnel --url http://127.0.0.1:7878` (quick tunnel) is also running
on the box since 20:09. One public edge is enough; owner should decide whether to stop
it (not touched here).

## Architecture hardening (cited)
All in `galion-render/src/backends/web.rs` unless noted.

1. **Ephemeral `/turn` buffers.** `POST /turn` body goes into `TurnInbox`, an in-memory
   bounded `VecDeque` (`MAX_PENDING_TURNS = 8`, oldest dropped; `push`/`take`/`take_latest`).
   Bytes live only until the host `take()`s them. Replies live in `ReplyBox`
   (`MAX_STORED_REPLIES = 8`, oldest evicted) and are served from memory as
   `GET /reply/<id>.wav`; ids are validated (`valid_reply_id`).
2. **No disk audio by default.** web.rs has no `std::fs` use. The live demo host
   (`crates/galion-console/examples/dot_console_demo.rs`) only writes `summary.txt` into
   the PNG dir, and the running instance uses `--no-png`. The only audio-to-disk writers
   are offline tools (`crates/galion-live-demo/src/lib.rs` → `out_dir`,
   `crates/galion-ml-voice/examples/roundtrip.rs`), never the phone path. `/mic` carries
   RMS only (≤512 B JSON kept); no mic audio leaves the page except an explicit `/turn`.
3. **CORS + phone headers.** Every response carries `Access-Control-Allow-Origin: *`,
   `Allow-Methods: GET, POST, OPTIONS`, `Allow-Headers: {CORS_ALLOW_HEADERS}` and
   `Cache-Control: no-store`; `OPTIONS *` → 204. `CORS_ALLOW_HEADERS` =
   `Content-Type, X-Turn-Id, X-Sample-Rate, X-Barge-In, ngrok-skip-browser-warning`
   (const near the top of web.rs). Already present in source — **no patch needed**.
4. **Body/head caps + timeouts.** `MAX_BODY_BYTES = 4 MiB` (413 above), `MAX_HEAD_BYTES =
   16 KiB`, whole-request `REQ_DEADLINE = 10 s`, chunked bodies rejected (411), missing
   Content-Length on `/turn` → 411, empty → 400. `MAX_CONN_THREADS = 32` so one stalled
   tunnel socket can't starve `/dot`.
5. **Edge.** Only ngrok (today) or Caddy :443 (future) is public; both terminate TLS and
   forward to loopback. Caddy's planned `/turn` cap = 4 MiB to match the app.

## Open items
- Rebuild + restart `dot_console_demo` so `/turn` + CORS preflight are live behind ngrok
  (owner of web.rs / demo process).
- Decide on the stray cloudflared quick tunnel.

## Staging auth (V2-S3)

The notes above describe the 2026-11-02 edge, when every response sent
`Access-Control-Allow-Origin: *`. That wildcard remains only while
`GALION_CORS_ORIGINS` is unset. A shared staging URL must:

1. Set `GALION_CORS_ORIGINS` to the exact `https://` origin (not `*`).
2. Put OAuth or basic auth on the tunnel. The password and any OAuth client
   secret stay in the operator's environment, not in this repo.
   `ngrok http 127.0.0.1:7878 --basic-auth "$GALION_NGROK_USER:$GALION_NGROK_PASS"`
   or the ngrok `oauth` Traffic Policy. Unauthenticated `curl` must fail at
   ngrok before loopback.
3. Leave the app on `127.0.0.1`. Do not pass `0.0.0.0`.

See `docs/galion-voice-v2/SECURITY-EDGE.md`.

— Maciej Grajczyk
