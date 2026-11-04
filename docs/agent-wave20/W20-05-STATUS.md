# W20-05 — Sector 06 Render (one dot) — status

*2026-11-02 ~12:40 CET (Europe/Zagreb, UTC+1). LOCAL ONLY — no push/gh/VPS/weights.*

## Done this wave
- `galion-render/src/backends/window.rs`: added std-only `FrameMailbox` (single-slot, latest-wins, `Arc<Mutex<..>>`, poison-tolerant, overwrite counter) — the handoff between `present()` (voice thread) and a future window / `/dev/fb0` thread. Re-exported from `lib.rs` (window feature). Test `mailbox_latest_wins`.
- No new dependencies. `cargo test -p galion-render --offline`: 17 pass (default); `--all-features`: 22 pass.

## Optional window backend — design (proposal, not implemented)
- Feature `window` stays zero-dep; real backend goes behind a new `window-native = ["window", "dep:softbuffer", "dep:winit"]` so `--all-features` / `full` policy decision (G3) is explicit.
- Thread model: `WindowSurface::open()` spawns event-loop thread holding a `FrameMailbox` clone; `present()` = `mailbox.put(frame)` (O(1), < 1 ms G4 unchanged); redraw tick at `max_fps` calls `take()` → `render_offscreen` → softbuffer present. Window close ⇒ thread exit ⇒ next `present` returns error ⇒ `Fanout` detaches (voice never stalls).
- winit on Linux requires main-thread event loop on some platforms → for macOS, keep `open()` returning `Unsupported` unless caller runs loop on main thread; document.
- `/dev/fb0` (Galion OS Voice): same mailbox + `rasterize` into mmap'd buffer; needs `libc` mmap or plain `File::write_at` per frame (std-only, 0x00RRGGBB for 32bpp; convert for 16bpp RGB565). Spec only.

## NullSurface
- Already records `last_dot` + frame count; unchanged (avoid breaking struct-literal users). Possible follow-up: state-transition trace for goldens.

## Open / next
- Wire `galion-heartbeat` `FrameOut` → `VizFrame`/`DotState` (not touched; cross-crate).
- Policy call on window dep (G3).

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
