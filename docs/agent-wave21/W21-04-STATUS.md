# W21-04 · Sector 06 Render (one-dot) + 08 Console · STATUS
*2026-11-02, 14:35 CET (Europe/Zagreb, UTC+1). LOCAL ONLY: no push, no `gh`, no VPS, no weights.*

**State: DONE — one-dot + live console driven by the real 80 ms heartbeat, headless-safe, tests green.**

## Run
```bash
scripts/run-dot-console-demo.sh                       # TTY: live dot + TIMING/EMOTION/BANDS/RIKA panel, 8 s
scripts/run-dot-console-demo.sh --headless --seconds 12   # PNG sequence + summary lines (~4 s wall, debug)
scripts/run-dot-console-demo.sh --web                 # + http://127.0.0.1:7878/dot and /frame, paced 80 ms
VIDEO=1 scripts/run-dot-console-demo.sh --headless    # + target/dot-demo/dot.mp4 (if ffmpeg present)
scripts/run-dot-console-demo.sh --feed stub --sense-off   # stub feeds; SENSE off → "no reading"
```
Direct: `cargo run --offline -p galion-console --features dot-viz --example dot_console_demo -- --help-ish flags:`
`--seconds N --headless --realtime --png-dir DIR --png-latest-only --no-png --size WxH --feed stub|real --sense-off --web ADDR --window --no-dot`.
Env: `GALION_HEADLESS=1` forces headless, `GALION_DEMO_OUT` → `$GALION_DEMO_OUT/dot/`, `OUT`, `RELEASE=1`, `NO_COLOR=1`.
Last stdout line: `GALION_DOT_DIR=<abs path>`.

Outputs (default `target/dot-demo/`): `frame_000000.png…` (320x180 RGB), `latest.png`, `summary.txt`, optional `dot.mp4`.

## What landed
| item | files |
|---|---|
| std-only PNG encoder (stored deflate, CRC table, Adler-32) | `galion-render/src/png.rs` |
| `PngSeqSurface` / `PngSeqConfig` (frame_NNNNNN.png + latest.png, ≤12.5 fps, amber→violet bg) | `galion-render/src/backends/png_seq.rs`, `backends/mod.rs`, `lib.rs` |
| `dotviz` module (feature `dot-viz`): `viz_from_heartbeat`, `DemoPump` (TimingFeed + AffectFeed pumped from the demo loop), `DotConsole` (Fanout + stats + panel + summary line) | `crates/galion-console/src/dotviz.rs`, `lib.rs`, `Cargo.toml` (features `dot-viz`, `dot-viz-web`, `dot-viz-window`) |
| example | `crates/galion-console/examples/dot_console_demo.rs` |
| script | `scripts/run-dot-console-demo.sh` |
| W21-01 attach guide | `docs/agent-wave21/W21-04-ATTACH.md` |

Synthetic 6 s scene (75 frames, cycles): room → breath → you speaking → room → me speaking → room; barge-in
at frames 58–63 on odd cycles (heartbeat yields). Rika runs on the feeling band (`rika_v0`, consent granted for en).

## Verified (this run)
- headless 12 s, real feeds: 150 frames, lean_in_frames=5, yields=1, rika_lean_in_ms=160, mean total 7.1 ms,
  max 9.2 ms, over_budget(60 ms)=0, max hb.step 143 µs, wall 3.8 s (debug).
- `--web --realtime`: `curl 127.0.0.1:PORT/dot` returns live JSON (`"state":"listening"…`).
- TTY panel rendered under `script` (dot + bars + Rika glyph row).
- Tests: galion-render `--all-features` 26 pass; galion-console `--features dot-viz` 39 pass; default 37 pass.
  Builds clean (no warnings) with `--no-default-features --features dot-viz` and with `dot-viz-web,dot-viz-window`.
- Gates: G4 render golden PASS (unchanged), LK locks OK, forbidden-format/runtime OK on galion-render + galion-console.

## Limits
- No real window: `WindowSurface` is a stub (no softbuffer/winit vendored, none added) → `--window` reports
  Unsupported and falls back. Live view = TTY panel, web `/dot`, or `latest.png`.
- Audio is synthetic (tones/noise), not a WAV; W21-01 feeds real WAV frames via the attach guide.
- PNGs are uncompressed (~170 KB/frame); `--png-latest-only` for long runs. `PngSeqSurface` reports `SurfaceKind::Window`
  (no dedicated enum variant, to avoid touching the locked surface API).
- No push done.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
