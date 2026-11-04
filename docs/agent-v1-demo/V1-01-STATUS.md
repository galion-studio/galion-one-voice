# V1-01 · Single screen · STATUS — DONE (local)

*Galion Voice V1 demo · 2026-11-02 ~14:45 (Europe/Zagreb, UTC+1) · signed Maciej Grajczyk*

**Decision (Q5 = YES):** the single-screen one-dot is the final V1 UI. One screen, one thing: the dot.

## Run it

```bash
scripts/run-v1-screen.sh            # → http://127.0.0.1:7878/   (Ctrl-C stops; 1 h default)
scripts/run-v1-screen.sh --open     # + opens it fullscreen/kiosk if chromium/chrome/firefox exists
scripts/run-v1-screen.sh --check    # ~15 s self-test: curls / and /dot, lists modes seen, exits
```

| Item | Value |
|---|---|
| Screen URL | `http://127.0.0.1:7878/` (alias `/screen`) |
| Data | `GET /dot` (dot JSON, 80 ms), `GET /frame` (dev frame), `GET /health` |
| Page source | `crates/galion-console/assets/v1-screen.html` (≈3.8 KB, no deps), embedded as `galion_console::dotviz::SCREEN_HTML` |
| Server | `galion-render` `WebSurface::serve(addr).with_page(SCREEN_HTML)` — std only, **loopback only** (non-loopback bind is refused by design) |
| Driver | `dot_console_demo` (feature `dot-viz-web`), headless + realtime: real `galion-heartbeat` (`HeartbeatConfig::rika_v0`) at 80 ms, real tensor timing + emotion SENSE feeds, synthetic you/me PCM (no mic, no weights) |
| Env | `V1_SCREEN_ADDR` (default `127.0.0.1:7878`), `V1_SCREEN_SECONDS` (3600), `OUT` (`target/v1-screen`: `heartbeat.log`, `latest.png`), `RELEASE=1` |
| Snapshot | `docs/agent-v1-demo/img/v1-01-screen.png` (headless Chrome, 1280×720) |

## Exactly what the screen shows

- **One solid neutral dot** (`#F5F5F7` scaled uniformly by brightness), centred, radius = `radius × min(width, height)`.
- **Background only:** obsidian `#0A0A0C` with the slow amber `#F5A524` → violet `#7C3AED` wash (24 s drift, vignette to obsidian at the edges) — a JS port of `palette::background_at`. The gradient never touches the dot.
- **Nothing else:** no text, captions, buttons, menus, logos, waveforms, glow, cursor (hidden), scrollbars. The tab title is "Galion Voice" (browser chrome, gone in kiosk/fullscreen).
- Fullscreen: click, double-click, `F` or `Enter` (no visible control). `--open` launches kiosk mode directly.
- Pacing: polls `/dot` every 80 ms (one heartbeat frame); radius/brightness change only when a new frame arrives (speaking stays quantised to the 80 ms grid).
- Heartbeat unreachable for >1 s → the page falls back to the plain idle pulse locally (still no text, no error UI).
- The terminal running the script is not the screen: it prints one URL line and logs status to `target/v1-screen/heartbeat.log`.

## State mapping (heartbeat → dot)

Locked `DotState` set is unchanged (`idle` / `listening` / `speaking`, LOCKS `[ui].dot_states`). Rika lean-in is an **idle sub-mode** exposed as `"mode":"rika"` on `/dot`.

| Mode (`/dot` `mode`) | Heartbeat condition | Radius (× short side) | Brightness | Motion |
|---|---|---|---|---|
| `idle` | neither band active | 0.100 ± 0.006 | 0.84 ± 0.03 | slow pulse, 1.6 s loop |
| `rika` (lean-in) | idle **and** `FrameOut.applied.lean_in > 0` (Rika ARMED; SENSE+ADAPT consented) | 0.100 + 0.012 × lean (0.112 at full lean) | 0.86 + 0.08 × lean (0.94) | **held breath**: pulse frozen, dot leans toward listening size |
| `listening` | you band active (RMS > 0.01) | 0.120 ± 0.002 + 0.004 × level | 0.96 | almost still, 3.2 s breathe |
| `speaking` | me band voiced (wins over listening; barge-in → `yield_now` flips to listening) | 0.115 + 0.030 × drive | 0.97–1.00 | one step per 80 ms (4-step rhythm × speak level) |

Observed on the wire (14 s live run): `idle … → rika r=0.1120 b=0.9400 (3 frames) → listening r≈0.120 b=0.96 → idle → speaking r 0.123–0.135 b≈0.98–0.99 → idle`; `rika_lean_in_ms=160` (R1 budget), `lean_in_frames=10`, `yields=2`, 0 over-budget. `--check` sees all four modes: `idle listening rika speaking`.

Precedence: speaking > listening > rika > idle. Lean-in never tints the dot and never adds a 4th locked state; with `--sense-off` (or workplace/education purpose) Rika is OFF and `rika` never appears.

## Code changes (additive)

| File | Change |
|---|---|
| `galion-render/src/dot.rs` | `DotParams::compute_lean`, `lean_in_of(&VizFrame)`, `screen_mode(&VizFrame)`; `from_frame` applies lean-in to idle only; +1 test |
| `galion-render/src/backends/web.rs` | `WebSurface::with_page(&'static str)` → serves `/` + `/screen` (404 if unset, as before); `/dot` gains `"mode"` + `"lean"`; query strings ignored; +1 test |
| `crates/galion-console/assets/v1-screen.html` | the single screen page (new) |
| `crates/galion-console/src/dotviz.rs` | `pub const SCREEN_HTML` |
| `crates/galion-console/examples/dot_console_demo.rs` | `--web` now attaches the page (`with_page(SCREEN_HTML)`) |
| `scripts/run-v1-screen.sh` | new launcher (`--open`, `--check`, `--seconds`, pass-through flags) |

## Verification (2026-11-02, local)

- `cargo test --offline -p galion-render --features full` → 28 pass · `-p galion-console --features dot-viz-web` → 39 pass · workspace `--all-targets` builds.
- `galion-gates/g4-render.sh` → **G4 PASS**, snapshot == golden `618a6b26…` (unchanged: `DotParams::to_json` untouched; G4 frames carry no lean-in).
- `scripts/check-locks.sh` → OK (dot_states still idle/listening/speaking).
- `scripts/run-v1-screen.sh --check` → page OK, all four modes seen.

## For V1-02 / V1-04 / deploy later

- Integrate: `run-v1-demo.sh` can call `scripts/run-v1-screen.sh` (or attach `WebSurface::serve(addr).with_page(galion_console::dotviz::SCREEN_HTML)` to the real live loop — any `Fanout` that gets `viz_from_heartbeat` frames drives the same screen, Rika included).
- Real speech (V1-03) needs no screen change: the dot is driven by the me-band PCM RMS, whatever produces it.
- Server 51.38.137.193 (Q4, **not deployed** — SSH denied in this lane): same command on the host, view via `ssh -L 7878:127.0.0.1:7878 <user>@51.38.137.193` → `http://127.0.0.1:7878/`. Public exposure would need a reverse proxy in front of the loopback bind (deliberately not in `galion-render`).
- Not done: no git commit/push, no VPS, no weights.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
