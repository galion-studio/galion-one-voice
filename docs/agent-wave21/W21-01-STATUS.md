# W21-01 · Live local demo — STATUS: GREEN
*2026-11-02 (Europe/Zagreb, UTC+1). Local only: no push, no weights, no training.*

## What landed
- **`galion-live-demo`**, a new crate with one binary and a library. It reads WAV input offline (16-bit PCM, any rate, linear resample to 24 kHz and noted in the capture header) or a seeded synthetic "you" script (202 frames, 16.16 s). It cuts the input into 80 ms × 1920-sample frames and runs each frame in a real-time-shaped loop through the **real `Heartbeat::step` + `RikaReplayBackend`** path, using the new `step_live`. It writes a stereo WAV (L = you, R = me), `console_capture.txt` and `SHA256SUMS` (std-only SHA-256).
- **`RikaReplayBackend::step_live(f, pcm, consent_bits, purpose)`** in `galion-heartbeat/src/presence.rs` is additive. It copies PCM into the fixed buffer and is zero-alloc (covered by a test). The old `step` is unchanged and shares the new private `finish` tail.
- **Flags:** `--in`, `--out` (default `out/live`), `--rika-opener` (default **off**), `--consent none|sense,adapt`, `--purpose personal|workplace|education|…`, `--realtime` (sleeps *between* frames, never inside `step`; frames that run late become `LATE` + pause), `--mimi off|bypass|codec|nokernels`, `--dot`, `--quiet`.
- **W21-02 Mimi:** `MimiVoice` wraps the tone voice through `StudentBackend::with_mimi(MimiPassthrough::new(MimiLaneConfig::demo().with_monitor(DryBypass)))`. Frames are labelled `MIMI synthetic (IS_STUB) BYPASS`. It is on by default in release builds only. It never uses the full config. `nokernels` falls back to `no_kernels()`. If the lane errors, the voice drops back to dry with the tag `MIMI ERR→dry`.
- **W21-03 EN cascade:** `LIVE_EN_CASCADE=1` uses the WAV from `scripts/run-en-cascade-demo.sh` as `--in`. It is optional and never blocks.
- **W21-04 dot-viz:** with the feature `dot-viz` and `--dot`, `DotSink` calls `DotConsole::frame(...)` after each step. It writes a PNG sequence plus `latest.png` to `<out>/dot/`. The window surface is unsupported upstream, so it is not used.

## How to run
```bash
scripts/run-live-demo.sh                                   # synthetic you → galion-demo/out/live/
scripts/run-live-demo.sh --showcase                        # default / opener / workplace(hard-OFF) + PASS checks
scripts/run-live-demo.sh --consent sense,adapt --rika-opener
scripts/run-live-demo.sh --in some.wav --out /tmp/live      # your own 16-bit PCM WAV
LIVE_REALTIME=1 scripts/run-live-demo.sh                   # paced to the wall clock, 80 ms/frame
LIVE_EN_CASCADE=1 scripts/run-live-demo.sh                 # W21-03 EN cascade WAV as the you band
LIVE_DOT=1 scripts/run-live-demo.sh --consent sense,adapt --rika-opener   # + W21-04 dot PNGs in <out>/dot/
```
Every run builds with `cargo build --offline --release -p galion-live-demo`, and the last line printed is `GALION_LIVE_DEMO_OUT=<dir>`.

**Artifacts** (in `galion-demo/out/live/`; `golden.sh` keeps using `out/canned`, so the two don't collide):
`live_input_24k.wav`, `live_output_stereo.wav`, `console_capture.txt`, `SHA256SUMS`, plus the showcase subdirectories `default/`, `opener/` and `workplace/`.

**Opener summary (verified):** frames=202, late=0, yields=2, lean_in_frames=21, soft_hum_frames=1, rika=ON(v0+soft_ack), lean_in_latency=**160 ms**. The explain line reads: "I noticed you paused and took a breath, so I waited for you." The workplace run is hard-OFF: `OFF(Gate(DenyProhibitedPurpose))`, and its output is byte-identical to the default run. In release builds the Mimi lane takes about 18–30 ms per frame, inside the 80 ms budget.

## Tests (all PASS)
- `cargo test --offline -p galion-live-demo` (with and without `--features dot-viz`): sha256 vector + 7 live tests (N frames / stereo length / no panic, artifacts + determinism, 160 ms lean-in + single soft hum, Rika off by default / without consent, workplace & education hard-OFF == default me band, 16 kHz→24 kHz resample, Mimi bypass lane labels) + zero-alloc `step_live`.
- `cargo test --offline -p galion-heartbeat --all-features`: PASS.
- `galion-demo/golden.sh`: `DEMO GOLDEN PASS 63133d73…` (unchanged).
- `scripts/check-no-forbidden.sh`: PASS.

## Blockers / caveats
- Mimi runs with ZeroWeights in DryBypass, so it is **IS_STUB**. The audio you hear is the tone voice, not codec output. Real codec audio needs weights, which are out of scope.
- Turn-taking comes from `galion-presence` MockBackend, and the "me" voice is a synthetic harmonic tone. There is no ASR/LLM/TTS in the loop (the W21-03 cascade only feeds the you band).
- There is no live mic or speaker I/O. Input is a WAV read offline and paced in real time.
- `cargo clippy` is not installed on this box, so it was not run. `cargo metadata --offline` fails on a foreign-platform dependency (crossterm_winapi); build and test are unaffected.

## Hooks for later waves
- `trait MeVoice` (lib.rs) is the "me" renderer (W21-02/03); `tag()` labels capture lines.
- `trait FrameSink` has `on_frame(&FrameRecord)`, `on_heartbeat(&FrameOut, you, me, me_voiced, step_us)` and `finish(&Summary)`. These cover renderers and loggers (W21-04 `DotSink` in `src/dot.rs`); see `W21-04-ATTACH.md`.

## Files touched
- `Cargo.toml` (workspace member)
- `crates/galion-heartbeat/src/presence.rs` (additive: `step_live`, `heartbeat_mut`, `finish`)
- `crates/galion-live-demo/{Cargo.toml, src/lib.rs, src/main.rs, src/sha256.rs, src/mimi.rs, src/dot.rs, tests/live.rs, tests/zero_alloc_live.rs}` (new)
- `scripts/run-live-demo.sh` (new)
- `docs/agent-wave21/W21-01-STATUS.md` (this file)

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
