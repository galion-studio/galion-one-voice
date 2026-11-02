# FP-02 — Backend wiring: Heartbeat + Rika → ONE dot on :7878 (first-principles pass)

Perpetual Open MIT License · © 2026 Maciej Grajczyk · 2026-11-02 (Europe/Zagreb, UTC+1)
**LOCAL ONLY.** Nothing was deployed (51.38.137.193: SSH denied) and nothing was pushed.

## 1. Wiring (live path, `scripts/run-v1-demo.sh`)

```
[0] scripts/check-locks.sh                                   V4 locks (80 ms, one dot, consent deny)
[1] scripts/run-en-cascade-demo.sh --text "hello galion"
      galion-ml-voice  (example roundtrip)                   codec roundtrip fixture
      galion-cascade   (example en_cascade, --features cascade)
        StubAsr → MockLlm "you said: …" → TTS: A21 slot (Piper VITS, FP-03) | flite interim | exit 3
      → <V1_OUT>/cascade/en-cascade.wav (24 kHz mono) + en-cascade.json (reply_kind, speech_backend_label)
[2] cargo build --release -p galion-live-demo --features dot-viz-web
      → copied to <V1_OUT>/bin/galion-live-demo (private snapshot; rebuilds can't swap it mid-demo)
[3] ONE process, ONE bind, runs until Ctrl-C (or --loops N):
    galion-live-demo --reply en-cascade.wav --loops 0 --mimi off --realtime --rika-opener
                     --consent sense,adapt --purpose personal --web 127.0.0.1:7878
      you band: synth_you (16.16 s/loop): room → in-breath → voice → room → voice → barge-in
      me band : ReplyVoice = the cascade WORDS (onset → play to the end; barge-in yield → stop)
                (between words: SoftBreath hum, unvoiced)
      per 80 ms frame (1920 samples @ 24 kHz):
        galion-heartbeat  presence::RikaReplayBackend::step_live
            = Heartbeat::step (galion-emotion feel → Rika D20 lean-in)
            + galion-presence MockBackend turn-taking (me_planned = ReplyVoice::planned())
        → FrameSink DotSink (after step, never inside)
        → galion-console dotviz::viz_from_heartbeat → VizFrame
        → galion-render Fanout (12.5 fps decimation on a monotonic t_ms) → WebSurface
    WebSurface (loopback-only, std-only): GET /  /screen → v1-screen.html (FP-01)
                                          GET /dot      → {"state","mode","lean",…}
                                          GET /health
[4] v1-screen.html polls /dot every 80 ms → idle · rika (lean-in) · listening · speaking
    (feed gone > 1 s → local idle pulse)
fallback: only if the live BUILD fails → scripts/run-v1-screen.sh (same page, synthetic pump)
port busy → exit 69 (pre-check in the script AND in the binary)
```

Measured on the box (side port :17902, Piper words 1.36 s, flite 1.92 s earlier):
one loop = 202 frames, `me_onsets=3 yields=2 lean_in_frames=21 over_budget=0`, max step 14 µs.
/dot sequence per loop: `idle → rika → listening → speaking (≈1.4–1.9 s = the words) → idle → rika →
listening → speaking → listening (barge-in) → speaking (cut, then reply to the barge-in)`. Loop 2 identical (clock continues).

## 2. Me band vs you band

| band | what it is | drives the dot | V1 source |
|---|---|---|---|
| **you** | what the person says (input, mono 24 kHz) | `listen.active = you_rms > 0.01` → **listening**; room + in-breath → Rika **lean-in** (`mode:"rika"`) | `synth_you()` (no mic in V1) |
| **me** | what Galion says (output) | `speak.active = me_voiced && !paused` → **speaking** (beats listening) | `ReplyVoice` = the cascade words (`--reply`) |

`DotState::from_frame`: speaking > listening > idle; `screen_mode` = `rika` when idle with lean-in.

**The bug this fixes:** before FP-02 the cascade words were fed with `--in`, i.e. into the **you** band, so while the
words played the dot showed **listening**; "speaking" was only the 640 ms ToneVoice beeps, and `me_planned` was
hard-coded `false`. Simplest fix: put the words in the me band and let the voice say how long it plans to talk.
- `MeVoice::planned()` (new, default `false`) → `TurnInput::me_planned`. `ReplyVoice` returns true while words remain,
  so MockBackend holds `me_voiced` for the whole sentence (not its 8-frame mock reply). `MimiVoice` forwards it.
- Barge-in still works: the yield stops the words for good (test `reply_words_hold_me_voiced_and_yield_on_barge_in`).

## 3. Synthetic room/breath turns — kept, but only as the you band

The "alternate" mode (odd turns = speech WAV, even turns = synthetic) existed only because the speech WAV went into
the you band and had no room/breath, so Rika never leaned in (`lean_in_frames=0`). With the words in the me band,
the synthetic you band gives every loop the full story (lean-in, listening, speaking words, barge-in) in one process.
So: **synthetic you band kept** (the Rika lean-in demo needs it); `V1_TURN_INPUT`, alternate mode, `pad-wav.py` deleted.

## 4. Bugs fixed

1. **Mode mapping:** the words showed as *listening* → now *speaking* (§2).
2. **Mock reply truncation:** 640 ms mock reply would cut 1.9 s words → `me_planned` from `ReplyVoice::planned()`.
3. **Silent port-busy fallback:** a failed `--web` bind printed "PNG only" and kept running with no feed (the
   screen showed the idle fallback forever). The binary now **exits 69** with `port busy? (pkill -f galion-live-demo)`.
4. **Per-turn re-exec + rebind:** the script re-launched the binary every turn (~1 s gap with no feed, rebinding
   :7878 each time; risk of TIME_WAIT/bind races). Now one process, one bind, `--loops`.
5. **Hot spin:** `turn "$i" || true` in a forever loop spun on a failing binary. Now a crash ends the demo with its rc.
6. **Rebuild race:** the script ran `target/release/galion-live-demo` directly; a concurrent `cargo build` (other lanes)
   could swap it mid-run. Now it runs a private copy in `<V1_OUT>/bin/`.
7. **Dot clock across loops:** frame index restarted at 0 each loop → Fanout's decimator would freeze the feed.
   `DotSink` now offsets by a running base (monotonic `t_ms`).
8. **Memory growth:** `DotSink.lines` pushed a summary string per frame (unbounded in forever mode, never read) → deleted.
9. **PNG writes in web mode:** every 80 ms frame was written as PNG while serving the web dot → `--web` attaches no PNG surface.

## 5. Deleted / simplified (V1 path only; Wave-21 research crates untouched)

| what | where | why |
|---|---|---|
| `GET /frame` + per-frame `VizFrame::to_json` copy | `galion-render/src/backends/web.rs` | no consumer (the web console never existed); test now asserts 404 |
| `latest_json()` | same | only served `/frame` |
| `dot-viz-window` feature + `--window` branch | `crates/galion-console/Cargo.toml`, `examples/dot_console_demo.rs` | stub (`open()` → Unsupported); galion-render's `window` backend itself kept for research |
| PNG surface under `--web` | `crates/galion-live-demo/src/{main,dot}.rs` | not a dot state |
| `DotSink.lines` | `dot.rs` | unbounded, unread |
| per-turn `turn()` re-exec, `V1_TURN_INPUT`, alternate mode | `scripts/run-v1-demo.sh` | one process `--loops` |
| `scripts/v1/pad-wav.py` | deleted | padding only existed for the you-band speech hack |
| `turn-N/` output dirs | script | single `<V1_OUT>/live-web/` + `live-web.log` |

Developer console/panels (`DotConsole::panel`, `summary_line`) are not on the V1 path (`--quiet`; web surface only).
The :7870 mirror / screen-host.py stay deleted (FP-01).

## 6. Depends on FP-03 (TTS / A21)

- **Contract (unchanged call sites):** A21/Piper (or flite) writes `<V1_OUT>/cascade/en-cascade.wav` (any rate;
  `--reply` resamples to 24 kHz mono) and `GALION_EN_CASCADE_WAV=` on stdout. No other hook needed for *speaking*.
  Verified: FP-03's Piper output (`A21 neural TTS (Piper VITS en_US-ljspeech-medium; CPU; MIT)`, 1.36 s) already drives the dot.
- **Audible playback is NOT wired:** the words are in the me band and in `live_output_stereo.wav`, but nothing plays
  them to the speakers. Simplest correct hook: in `--realtime` mode, write each frame's me band to a local sink
  (`paplay`/`aplay` raw pipe) from `run_frames` — same frame as the dot, so they stay in sync. FP-03 to decide.
- `V1_SPEECH_CMD` / `scripts/v1/speech.sh` branch in the script is dead (file absent) — left for FP-03 per FP-01.
- `docs/V1-DEMO.md` §7 "Real speech" bullet still says flite/A21 ABSENT — FP-03's to update.

## 7. Notes for FP-04 (gate)

- `--headless` path and `galion-gates/v1-demo.sh` / `scripts/v1/soft-checks.sh` contract **unchanged**. The headless
  EN turn still uses `--in <wav>` (words in the you band); switching it to `--reply` would make C-checks see speaking —
  FP-04's call. C5 still passes in web mode via `state.json`.
- soft-checks C9 text "live me band still Mimi IS_STUB tone voice" is stale for the live path (me band = words via
  `--reply`); still true for the headless `--in` EN turn. Wording fix is FP-04's.
- `--check` now = one live loop (~16 s) + curl `/health`, `/dot`, `/`; exit 69 propagated from the binary.

## 8. Verification (2026-11-02, Europe/Zagreb)

- `cargo test --offline`: galion-render (web) 26 ok · galion-console (dot-viz-web) 39 ok · galion-live-demo (dot-viz-web) 8+1+1 ok
  (new `reply_words_hold_me_voiced_and_yield_on_barge_in`) · galion-heartbeat, galion-presence ok · `check-locks.sh` OK.
- `dot_console_demo` example (fallback) builds with dot-viz-web.
- `FEED_PORT=17903 V1_OUT=/tmp/fp02-v1 scripts/run-v1-demo.sh --check` → PASS (side port; :7878 left to other lanes).
- `V1_OUT=/tmp/fp02-headless scripts/run-v1-demo.sh --headless` → `V1 RESULT: fails=0 soft=0 speech=words`, rc 0.
- Busy port → `galion-live-demo` exit 69. `bash -n scripts/run-v1-demo.sh` OK.

— Maciej Grajczyk
