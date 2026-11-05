# Local demo

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. From the repository root.

The live demo feeds a you-band through `galion-heartbeat` at `FRAME_MS` (80 ms, 24 kHz, 1920 samples) and writes a stereo WAV plus a console capture. The me band on this path is synthetic. It is not a speech model and not a latency benchmark of one. Wave-21 detail lives in `docs/LIVE-DEMO.md`. This page is the symphony entry.

CPU only, no GPU. The process runs on whatever Linux CPU you have. It does not claim to be an EPYC 9655P, a CX23, or a CX43. Those classes are in [CPU.md](CPU.md).

## Run

```bash
bash scripts/run-live-demo.sh
bash scripts/run-live-demo.sh --showcase
bash scripts/run-dot-console-demo.sh --headless
```

`scripts/run-live-demo.sh` builds `galion-live-demo` offline if the cache allows, steps the heartbeat, and prints `GALION_LIVE_DEMO_OUT=<dir>` on the last line. `--showcase` writes `default`, `opener`, and `workplace`. Workplace stays hard-off.

Useful flags, same as the script header: `--in you.wav`, `--out DIR`, `--consent none|sense,adapt`, `--purpose personal|workplace|education`, `--rika-opener`, `--realtime`, `--mimi off|bypass|codec|nokernels`, `--dot`, `--quiet`.

`--realtime` sleeps to the wall clock, one 80 ms frame at a time. A frame that misses that deadline is passed as `late`. Late means pause: the me band starts nothing new, and the next frame does not catch up. Without `--realtime` the demo does not pretend the wall clock is the product latency.

## What you should see

| Path | What it is |
|---|---|
| `galion-demo/out/live/live_output_stereo.wav` | left = you, right = me |
| `galion-demo/out/live/console_capture.txt` | per-frame bars and a `summary:` line (`paused=`, `late=`) |
| `target/dot-demo/latest.png` | one-dot frames from the headless console demo |

The summary's `lean_in_latency=` on the Rika path is the stub heartbeat's own lean-in (the locked 160 ms land), not a measured mouth-to-ear time.

## Limits

The student, the codec kernels, and the emotion head on this command are stubs. RTF printed here is not a model number. Do not quote it as EPYC, CX23, or CX43 performance. The tick gate that does measure a step is [LATENCY.md](LATENCY.md), and it labels the result `measured-stub`.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
