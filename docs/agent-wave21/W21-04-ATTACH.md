# W21-04 → W21-01 · Attaching the one-dot visualizer + live console
*2026-11-02 (Europe/Zagreb, UTC+1). Local only.*

The visualizer is **pure presentation**: it reads a finished `FrameOut` plus the two band PCM slices and
never runs inside `Heartbeat::step`. A surface that errors is dropped by `Fanout`; the voice keeps going.

## 1. Dependency (crates/galion-live-demo/Cargo.toml)
```toml
galion-console = { path = "../galion-console", features = ["dot-viz"] }   # + "dot-viz-web" for http://127.0.0.1:PORT/dot
galion-render  = { path = "../../galion-render" }                          # only if you construct surfaces yourself
```
`dot-viz` pulls galion-render (std-only), galion-heartbeat, galion-emotion. No new crates.io deps.

## 2. Set up once
```rust
use galion_console::dotviz::{DemoPump, DotConsole, PanelOpts};
use galion_console::live::{StubAffect, StubTiming};          // or feeds::{TensorTimingFeed, EmotionAffectFeed}
use galion_render::{PngSeqConfig, PngSeqSurface, TerminalDotSurface};

let mut dc = DotConsole::new();
dc.attach(Box::new(PngSeqSurface::new(PngSeqConfig {
    dir: out_dir.join("dot"), sequence: true, ..Default::default()   // 320x180, ≤12.5 fps, frame_NNNNNN.png + latest.png
})))?;
// Optional, TTY only:  dc.attach(Box::new(TerminalDotSurface::stdout()))?;
// Optional, web:       dc.attach(Box::new(galion_render::WebSurface::serve("127.0.0.1:7878".parse()?)))?;
let mut pump = DemoPump::new(StubTiming::default(), StubAffect::new(sense_on));
```

## 3. Per 80 ms frame — after `hb.step`, never inside it
```rust
let t0 = std::time::Instant::now();
let out = hb.step(&frame_in);
let step_us = t0.elapsed().as_secs_f32() * 1e6;
let snap = pump.pump(i as u64, you_pcm, 24_000);                 // timing + affect from the you band
dc.frame(&out, you_pcm, me_pcm, me_voiced, &snap, step_us);      // → all attached surfaces
capture.push_str(&dc.summary_line(&out, &snap)); capture.push('\n');   // console capture .txt
// TTY: print!("\x1b[H{}", dc.panel(&out, &snap, step_us, PanelOpts::default()));
```
End of run: `dc.stats` (`DotStats`: frames, paused, lean_in_frames, yields, mean/max total ms, over_budget).

## 4. Or just shell out
`scripts/run-dot-console-demo.sh` honours `GALION_HEADLESS=1` and `GALION_DEMO_OUT` (writes to
`$GALION_DEMO_OUT/dot/`), and prints `GALION_DOT_DIR=<abs path>` as its last line:
```bash
DOT_DIR=$(GALION_HEADLESS=1 scripts/run-dot-console-demo.sh --seconds 8 | sed -n 's/^GALION_DOT_DIR=//p')
```

## Notes
- Window: `galion-render`'s `WindowSurface` is a stub (no softbuffer/winit vendored) → `open()` returns
  Unsupported; `--window` falls back to PNG/terminal. Use the web surface or `latest.png` for a "live" view.
- PNG files are uncompressed (stored deflate) ≈170 KB at 320x180; use `--png-latest-only` for long runs.
- Branding: dot is #F5F5F7, uniform brightness; amber→violet only in the background; the panel is developer-only.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
