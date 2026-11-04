# V1 Phone · Reworked Sector Plan · 2026-11-02 (Europe/Zagreb)

Goal: **Galion Voice English phone spoken demo.** You speak English into the phone browser and hear a spoken reply from the phone speaker.
Status: docs only. This file supersedes the S1–S6 list in FP-05-STATUS.md §6 for the phone lane. PROGRESS.md "English phone silence" stays the root-cause record.

## Rule: reuse, don't reinvent
- Use the edge we already have: the **ngrok** HTTPS tunnel to loopback `127.0.0.1:7878` is running. Do **not** open new tunnels, write a custom TLS terminator, or set up any other relay while ngrok works. For the VPS (51.38.137.193, still pending SSH), use the **Caddy** config already drafted in `deploy/`.
- Use the components that already work: `web.rs` Turn API, Piper A21 (`en_US-ljspeech-medium`), CannedLlm, MockLlm fixtures, `/mic` RMS, `run-v1-screen.sh`. Fix the gaps. Do not replace working parts.
- No new dependency may break LOCKS.toml. Whisper.cpp / GGML stays out.

## Sectors

| ID | Sector | Already landed | Gap / work |
|---|---|---|---|
| S1 | Edge/TLS | ngrok HTTPS → loopback :7878 is running. `web.rs` refuses non-loopback binds. | Harden at the edge: send the `ngrok-skip-browser-warning` header (and set it on page fetches), add security headers (HSTS at edge, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, a tight CSP, `Permissions-Policy: microphone=(self)`), and match the edge body cap to `MAX_BODY_BYTES` (4 MiB). Later: Caddy on the VPS replaces ngrok, with the same headers. |
| S2 | Phone capture/play | `/mic` RMS (LevelMic) and the one-dot page | `v1-screen.html`: an AudioWorklet VAD cuts each utterance and sends it as `POST /turn` (WAV, Content-Length). Unlock an AudioContext on the first tap, then fetch and play `/reply/<id>.wav` when `/dot` shows a new `reply.id`. Keep the `/mic` RMS feed for the "you" band. |
| S3 | Turn API | `web.rs`: full Content-Length body read with looped reads and a deadline; 411/413/400 handling; `Expect: 100-continue`; `/turn` inbox; `/reply` store; `/dot` `reply` field `{id,url,ms}`; tests (WEB-BODY-READ-PATCH.md). | **Largely landed.** Left: one end-to-end test over the tunnel (S7 acceptance). |
| S4 | EN ASR | StubAsr (STUB-ASR-LIMIT.md) | Must stay LOCKS-safe. Interim: **PhoneStub** (accepts the turn and returns a fixed or keyword-matched English transcript). Then ratify one of: **A** Vosk (EN small model) or **C** native Whisper from safetensors in-tree. **No whisper.cpp / GGML.** |
| S5 | Reply LLM | CannedLlm (opt-in) and MockLlm fixtures | The demo uses CannedLlm with English replies. MockLlm fixtures stay unchanged for the gates. |
| S6 | TTS | Piper A21 en_US-ljspeech-medium, CPU, offline, sha256-pinned (FP-03, A21-EN-TTS-READY.md) | Reuse as is. Write the reply WAV into the `/reply` store and gate the server-speaker player (paplay/aplay) off for phone mode. Open: the LOCKS onnx exception needs ratifying; espeak-ng GPL-3 needs counsel (A21-BLOCKERS.md). |
| S7 | Demo glue | TURN-WORKER.md design and the `run-v1-screen.sh` / `run-v1-demo.sh` scripts | Move the turn worker off the 80 ms heartbeat thread onto its own worker. Chain /turn → ASR → LLM → TTS, then publish the reply (id, url, ms) to `/dot`. Barge-in: a new VAD onset stops client playback and cancels the reply in flight. Rebuild `run-v1-screen.sh` around phone mode. |
| S8 | Architecture harden | Loopback-only bind is enforced in `web.rs` | Keep the app loopback-only; TLS terminates only at the edge (ngrok now, Caddy later). Turn buffers are ephemeral (in memory, dropped after the reply, bounded queue). **No raw audio on disk by default** (opt-in debug flag only). Write a GDPR retention doc (what is held, for how long, purge on reply). Replace `Access-Control-Allow-Origin: *` with an allowlist (ngrok host, the VPS domain, localhost); allow headers `Content-Type, ngrok-skip-browser-warning`; answer OPTIONS preflight. |

## Order
S3 (done) → S6 reply store → S2 page → S1 headers → S5/S4 PhoneStub → S7 glue → S8 → swap ASR after ratification.

## Acceptance
1. Open the ngrok HTTPS URL on an Android phone. There is no ngrok warning page, and the mic permission prompt appears.
2. Tap once to unlock audio, then speak an English sentence.
3. `/turn` returns 2xx and the dot shows listening, then speaking.
4. The phone speaker plays an English reply in the Piper ljspeech voice, within a few seconds.
5. Speaking over the reply stops it (barge-in).
6. Nothing under the data dirs holds raw audio. The server log shows a loopback bind, and CORS rejects origins outside the allowlist.
7. `galion-gates/run-gates.sh` passes with fails=0.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
