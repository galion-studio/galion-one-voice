# V1-02 · Wiring · STATUS
*2026-11-02, 14:50 CET (Europe/Zagreb). LOCAL ONLY: no push, no gh, no HF, no he-IL, no VPS deploy.*

**Status: DONE (wiring).** One command (`scripts/run-v1-demo.sh`) runs the pipeline and feeds the one
V1-01 screen from the **real** 80 ms Heartbeat + Rika loop. Nothing else opens: no second window, no extra page.
V1-04 now owns hardening of `run-v1-demo.sh` (exit codes, timeouts, `--headless`). The V1-02 parts are marked in it.

## Wiring diagram

```text
 scripts/run-v1-demo.sh  (the one command)
 │
 ├─[0] scripts/check-locks.sh ─────────── V4 Eureka locks: 80 ms clock, you/me/feeling, one-dot, consent deny
 │
 ├─[1] speech turn ──► TURN WAV (24 kHz mono)
 │      V1_SPEECH_CMD / scripts/v1/speech.sh (hook, prints GALION_SPEECH_WAV=)   else
 │      scripts/run-en-cascade-demo.sh  (W21-03 + V1-03: ASR-stub → route → MockLlm → TTS → WAV)
 │        └─ speech kind read from en-cascade.json  →  "speech / interim libflite (NOT A21)"
 │      scripts/v1/pad-wav.py  → turn-input.wav (+1 s room before, +3 s after, so "me" can take its turn)
 │
 ├─[2] cargo build -p galion-live-demo --features dot-viz-web
 │      galion-live-demo --in turn-input.wav --consent sense,adapt --purpose personal --rika-opener
 │                       --realtime --web 127.0.0.1:7878 --hold 1          (one process per turn, looped)
 │        per 80 ms frame:  you band ─► Heartbeat::step (Rika/D20 inside step, feeling band only)
 │                          ─► me band (ToneVoice → W21-02 Mimi lane, IS_STUB) ─► stereo WAV + capture
 │                          ─► DotSink (after step, never inside) ─► DotConsole ─┬► PngSeqSurface  <out>/turn-N/dot/latest.png
 │                                                                                └► WebSurface 127.0.0.1:7878
 │                                                                                     /  /screen  V1-01 page (SCREEN_HTML)
 │                                                                                     /dot /frame /health
 │
 ├─[3] screen ═══ http://127.0.0.1:7878/  ◄── the ONE screen (V1-01 crates/galion-console/assets/v1-screen.html,
 │                                             modes idle / rika / listening / speaking from /dot)
 │      optional V1_SCREEN_HOST=1: scripts/v1/screen-host.py on :7870 = cached mirror (same page + /dot,
 │      keeps the last frame between turns; adds /v1/state and /audio.wav). Off by default.
 │
 └─ fallback (live build fails): scripts/run-v1-screen.sh (V1-01's own: same page, synthetic 80 ms pump, :7878)

 state: <V1_OUT>/state.json {"turn","stage","audio","speech"} — written per turn (read by screen-host /v1/state)
```

## How the pieces connect
| Piece | Lane | Role in V1 | Hook used |
|---|---|---|---|
| `scripts/run-live-demo.sh` / `galion-live-demo` | W21-01 | real 80 ms Heartbeat + RikaReplayBackend | new feature `dot-viz-web` + `--web ADDR`, `--hold SECS` (additive) |
| D20 Rika moment | heartbeat (D20) | already inside `Heartbeat::step`; `--rika-opener` + `sense,adapt` turn on the lean-in; workplace/education = hard-OFF (showcase) | none needed |
| `scripts/run-en-cascade-demo.sh` | W21-03 + V1-03 | the turn audio (now spoken words via interim flite) | `GALION_EN_CASCADE_WAV=` last line + `en-cascade.json` `reply_kind` |
| `scripts/run-dot-console-demo.sh --web` / `run-v1-screen.sh` | W21-04 / V1-01 | same WebSurface + page; synthetic pump | fallback only |
| V1-01 page | V1-01 | the one screen | `galion_console::dotviz::SCREEN_HTML` via `WebSurface::with_page` |
| V4 branding/locks | V4 | dot #F5F5F7 solid, bg #0A0A0C, amber→violet gradient in the background only, no chrome | `check-locks.sh` step [0]; refs: `/workspace/galion-voice-v4/BRANDING.md`, `FINAL-BUILD-SPEC.md`, `/workspace/galion-voice-20/D20-RIKA-MOMENT.md`, `LOCKS.toml` |

## Files (V1-02)
- `crates/galion-live-demo/Cargo.toml`: feature `dot-viz-web = ["dot-viz", "galion-console/dot-viz-web", "galion-render/web"]`
- `crates/galion-live-demo/src/dot.rs`: `DotSink::attach_web(addr)` (WebSurface + V1-01 page)
- `crates/galion-live-demo/src/main.rs`: `--web 127.0.0.1:PORT` (implies `--dot`; PNG goes latest-only), `--hold SECS`
- `scripts/run-v1-demo.sh`: the skeleton (now hardened by V1-04)
- `scripts/v1/screen-host.py`: optional cached mirror, loopback only
- `scripts/v1/pad-wav.py`: pads the turn WAV with silence

Env: `V1_TURN_INPUT=speech|alternate|synthetic` (default `speech`; `alternate` makes even turns synthetic so the
D20 lean-in and barge-in yield show), `FEED_PORT=7878`, `SCREEN_PORT=7870`, `V1_OUT`, `V1_SPEECH_CMD`, `V1_SCREEN_HOST`.

## Verified (local, 2026-11-02 CET)
- `scripts/run-v1-demo.sh --check` → `PASS check (screen + live /dot fed by Heartbeat+Rika)`. Passes with the screen host on and off.
- The live binary serves the V1-01 page (`<canvas id="s">`) at `:7878/`. `/dot` cycles listening → idle → speaking → idle and carries `mode` and `lean`.
- Speech WAV turn: `frames=74 me_onsets=1`. Synthetic turn: `frames=202 me_onsets=3 yields=2 lean_in_frames=21`.
- `cargo test -p galion-live-demo --features dot-viz-web` passes. Locks pass. No forbidden brand string appears in V1-02 files.

## Open / for other lanes
- **Rika lean-in on the speech turn:** a turn that is only a flite WAV gives `lean_in_frames=0`. The lean-in needs the in-breath/room cue that the synthetic you band has. To show the D20 moment, use `V1_TURN_INPUT=alternate`, or prepend a breath cue in `pad-wav.py` (V1-03/V1-04 decision).
- **The me band is still ToneVoice through the Mimi stub.** The spoken words come from the cascade WAV, which feeds the *you* band, and from `/audio.wav`. Real me-band speech is blocked on the A21 weights (see V1-03).
- **No live audio playback on the page:** the screen shows one dot. If FRXDEL wants the words to be audible in the browser, V1-01 can play `/audio.wav` (screen-host) without adding anything visible.
- **Server (51.38.137.193, FRXDEL Q4):** not deployed (this lane is LOCAL ONLY). The WebSurface and screen-host refuse public binds by design. On the server later: run the same command, then `ssh -L 7878:127.0.0.1:7878 <host>`.
- **Concurrent edits:** V1-04 edited `run-v1-demo.sh` while I was working in it. One of my line rewrites briefly broke the `screen-host` launch line in the `V1_SCREEN_HOST` block. I restored it and both `--check` modes pass again. V1-04 should re-read the file before its next edit.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
