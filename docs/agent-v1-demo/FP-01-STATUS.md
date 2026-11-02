# FP-01 · First-principles pass: evaluate + delete (one-dot)
Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk. · 2026-11-02 (CET)

## Result
- I deleted or disabled the extras. The list is in `FP-01-DELETED.md`. There is now one port (`:7878`) and one page.
- `scripts/run-v1-screen.sh --check` passes. I ran it on a side port (`:17901`) so it would not collide with other lanes. Output: `/ : one canvas page OK (3802 bytes)` and `/dot modes over ~12 s: idle listening rika speaking`.
- The page is valid HTML. Its elements are `html head meta×3 title style body canvas script`, with no text nodes in the body.
- `bash -n` passes on all three edited scripts, and `run-v1-demo.sh --help` still prints.

## Language inventory (V1 surfaces)
| V1 piece | Language | Where |
|---|---|---|
| Orchestrator `run-v1-demo.sh` (162 lines), `run-v1-screen.sh` (57), `run-en-cascade-demo.sh`, `run-live-demo.sh`, `run-dot-console-demo.sh`, `check-locks.sh`, `v1/soft-checks.sh`, gates `galion-gates/*.sh` | **Shell** (bash) | `scripts/`, `galion-gates/` |
| WAV padding | **Python** (stdlib, 10 lines) | `scripts/v1/pad-wav.py` |
| One-dot screen | **HTML/JS** (65 lines, one `<canvas>`, polls `/dot` every 80 ms, local idle pulse if the feed is gone for more than 1 s) | `crates/galion-console/assets/v1-screen.html`, embedded via `include_str!` in `galion-console::dotviz::SCREEN_HTML` |
| HTTP server for `/`, `/dot`, `/frame`, `/health` (std-only, loopback only) | **Rust** | `galion-render/src/backends/web.rs` |
| Heartbeat 80 ms `step()` + Rika on the feeling band | **Rust** | `crates/galion-heartbeat` (+ `galion-emotion`, `galion-presence`) |
| Live loop WAV → heartbeat → stereo WAV + dot feed | **Rust** | `crates/galion-live-demo` (`--features dot-viz-web`) → `galion-console` (dotviz) → `galion-render` |
| Synthetic-pump screen (`run-v1-screen.sh`) | **Rust** example | `crates/galion-console/examples/dot_console_demo.rs` |
| EN cascade (StubAsr → MockLlm → TTS) + A21 slot (weights absent) | **Rust** | `crates/galion-cascade` (`en_lane.rs`), `crates/galion-ml-voice` (`speech/{mod,weights,flite,sha256}.rs`, feature `speech`) |
| flite interim TTS | **C** (distro `libflite1`, CMU Flite 2.2). Nothing is vendored or linked; it is loaded at runtime with `dlopen` through Rust FFI. | system `/usr/lib/x86_64-linux-gnu/libflite.so.1` |
| Mimi / student / tensor (stub lane on the me band) | **Rust** | `galion-mimi`, `galion-student-backend`, `galion-tensor` (only rayon is an external dep) |
| Runbook | Markdown | `docs/V1-DEMO.md` |

Cargo.toml files (13 + 1 build scratch): root workspace · `galion-render` · `galion-probe` · `galion-emotion` · `crates/{galion-cascade, galion-student-backend, galion-tensor, galion-presence, galion-console, galion-mimi, galion-sideband, galion-ml-voice, galion-heartbeat, galion-live-demo}` · `galion-gates/.build/g4` (gate scratch).
Rust LOC: tensor 4875 · emotion 3745 · console 3307 · mimi 2218 · presence 2103 · render 1697 · live-demo 1160 · ml-voice 978 · heartbeat 831 · student 652 · cascade 516 · sideband 392 · probe 139.

## Remaining complexity (flagged, not deleted, because it is outside this lane)
1. **`/frame` endpoint** (`galion-render` web.rs). This is the full VizFrame JSON for a "galion-console web UI" that does not exist. The page reads only `/dot`. → FP-02: drop the route and its test.
2. **`galion-console` TUI** (`main.rs`, `ui.rs`, `feeds.rs`, `live.rs`, `mic.rs`; about 2.9k lines) and the **dot_console_demo panel/bars/glyph row** (`dotviz.rs::panel`, `summary_line`). These are developer tools, and LOCKS says `console_is_user_ui = false`, so they are harmless for the user. They are still the largest part of the screen-adjacent code. → Keep them out of every V1 path, or move them behind a dev feature.
3. **`--window` / feature `dot-viz-window`**. It is a stub (`Unsupported`), a dead second surface. → Delete it (Rust).
4. **`galion-live-demo` DotSink always writes a PNG sequence** under `<out>/dot/`, even in `--web` mode. That is per-frame disk I/O the screen does not need. → FP-02: skip PNGs when `--web` is set.
5. **`run-v1-demo.sh` is still 162 lines**, with three speech-labelling branches (V1_SPEECH_CMD hook → cascade JSON via python → sed). `scripts/v1/speech.sh` does not exist, so the hook branch is dead code. → FP-03: one source of truth, the cascade JSON `reply_kind` + `speech_backend`.
6. **Background wash** (amber→violet vignette at 38%). I kept it because LOCKS `[ui]` locks `gradient_scope = "background"`. It reads as "near-black with a glow". Removing it needs an owner decision.
7. Headless gate path = 3 binaries + a retry. It is fine for CI, but → FP-04.

## Recommendations for other lanes
- **FP-02 (heartbeat/wiring):** remove `/frame`, stop the PNG writes in web mode, and drop the `dot-viz-window` stub. Leave `/dot` + `/health` + `/` as the whole HTTP surface.
- **FP-03 (TTS/A21):** build the A21 slot (CPU-only, MIT-licensed, e.g. Piper) to replace flite. Also delete the dead `V1_SPEECH_CMD`/`speech.sh` branch, or ship `speech.sh`. **UX bug:** the live `run-v1-demo.sh` loop never *plays* the reply. Only `run-en-cascade-demo.sh --play` calls paplay/aplay, so a person watching the dot hears nothing. The simplest fix that serves the one dot is to play the me-band/reply WAV locally (paplay/aplay) in sync with the turn. It should not be an HTML `<audio>` element, because that would need a user click and controls.
- **FP-04 (E2E):** use the side-port trick (`V1_SCREEN_ADDR=127.0.0.1:<free>`) so parallel lanes don't fight over `:7878`. The `--check` above takes about 15 s.
- Runbook: `docs/V1-DEMO.md` needs only the one URL. Deployment to 51.38.137.193 is NOT done (SSH tunnel later).
