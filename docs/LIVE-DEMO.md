# Galion Voice — Live demo runbook (Wave-21)

Perpetual Open MIT License. Local only: no network, no VPS, no weight downloads, no training.
Target: working local live demo by EOD 2026-11-02 (Europe/Zagreb).

## 1. One command

```bash
cd /workspace/galion-voice-code
scripts/run-live-demo.sh               # synthetic "you" band → 80 ms heartbeat → stereo + capture
scripts/run-live-demo.sh --showcase    # 3 variants: default / opener (Rika) / workplace hard-OFF
```

`scripts/run-live-demo.sh` (W21-01) builds `galion-live-demo` (`cargo build --offline --release`), feeds a
WAV (or a seeded synthetic "you" band) as 80 ms frames @ 24 kHz through `Heartbeat::step` +
`RikaReplayBackend`, renders the "me" band through the W21-02 Mimi lane (synthetic, ZeroWeights, DryBypass),
and writes a stereo WAV + console capture. Last stdout line: `GALION_LIVE_DEMO_OUT=<dir>`.

Flags: `--in you.wav` · `--out DIR` · `--consent none|sense,adapt` · `--purpose personal|workplace|education` ·
`--rika-opener` · `--realtime` (sleep 80 ms/frame) · `--mimi off|bypass|codec|nokernels` · `--dot` (W21-04 PNG dot frames in `<out>/dot/`) · `--quiet`.
Env: `LIVE_OUT` (default `galion-demo/out/live`), `LIVE_REALTIME=1` (wall-clock pacing), `LIVE_EN_CASCADE=1` (use the EN cascade WAV as input), `LIVE_DOT=1` (build with `dot-viz` and add `--dot`).

### Companion scripts

| script | owner | what | last line |
|---|---|---|---|
| `scripts/run-en-cascade-demo.sh [--test] [--play] [--text "hello galion"]` | W21-03 | EN cascade lane: ASR-stub → route → MockLlm → TTS-stub → WAV (fallback lane, never default) | `GALION_EN_CASCADE_WAV=<path>` |
| `scripts/run-dot-console-demo.sh [--headless] [--web] [--seconds N]` | W21-04 | one-dot render + live console from the real heartbeat; TTY live view, headless PNG sequence; `--web` serves http://127.0.0.1:7878/dot | `GALION_DOT_DIR=<dir>` |

Suggested live-demo order (≈2 min): `run-live-demo.sh --showcase` → play `opener/live_output_stereo.wav`
→ `run-dot-console-demo.sh` in a terminal (or `--web` in a browser) → `run-en-cascade-demo.sh --play`.
Attaching the visualizer inside the live binary: `docs/agent-wave21/W21-04-ATTACH.md`.

## 2. Prerequisites

| need | check | note |
|---|---|---|
| Rust stable + cargo | `cargo --version` | all crates build `--offline` from the local cargo cache |
| Python 3 (stdlib only) | `python3 --version` | canned fallback path `galion-demo/` |
| bash, coreutils, `timeout`, `sha256sum` | — | used by the script and gates |
| optional: display (X11/Wayland) | `echo $DISPLAY` | only for the one-dot window; headless is fine |
| optional: `aplay`/`ffplay` | — | to listen to the output WAV |

No model file is needed. If `GALION_MODEL` is unset or points nowhere, the demo uses the synthetic
student/canned responder and says so in the capture header.

## 3. Pipeline (what you are watching)

`band "you" (WAV in) → 80 ms frames → Mimi streaming codec (synthetic, W21-02) → emotion proxy +
consent gate → counterbalance policy → student backend stub → band "me" (stereo R) ;
band "feeling" → EmotionWire-v1-8B per frame → console (W21-04) + Rika (opt-in, wordless only)`.
Optional EN cascade lane (W21-03, feature `cascade`): ASR → small LLM → TTS **mock traits**, fallback
only, never default.

## 4. Expected artifacts

| artifact | what | from |
|---|---|---|
| `galion-demo/out/live/live_input_24k.wav` | "you" band as fed (24 kHz mono) | run-live-demo.sh |
| `galion-demo/out/live/live_output_stereo.wav` | **L = you, R = me**, full duplex | run-live-demo.sh |
| `galion-demo/out/live/console_capture.txt` | per-frame you/me/feeling bars, Rika state, summary (`frames=… yields=… lean_in_latency=…`) | run-live-demo.sh |
| `galion-demo/out/live/SHA256SUMS` | checksums | run-live-demo.sh |
| `galion-demo/out/live/dot/` (with `--dot` / `LIVE_DOT=1`) | PNG one-dot frames | run-live-demo.sh |
| `galion-demo/out/live/{default,opener,workplace}/…` | same four files per variant; workplace me band byte-identical to default | `--showcase` |
| `target/galion-demo/en-cascade.wav`, `en-cascade.json`, `en-roundtrip.wav` | EN cascade earcon + turn summary | run-en-cascade-demo.sh |
| `target/dot-demo/latest.png`, `frame_NNNNNN.png`, `summary.txt` (+ `dot.mp4` with `VIDEO=1` and ffmpeg) | one-dot frames | run-dot-console-demo.sh |

Canned reference (pure Python, no build): `galion-demo/run_demo.sh` → `galion-demo/out/`; golden set
`galion-demo/golden.sh` → `galion-demo/out/canned/`.

## 5. Honest limits

- **No real speech model yet.** The "me" band is synthetic tones, not words. It shows timing, duplex
  overlap, barge-in yield, Rika lean-in and emotion-following, not language.
- **No microphone.** Input is a WAV file (`--in`) or the seeded synthetic "you" band; nothing is captured live.
- **Mimi is `IS_STUB`** (ZeroWeights, DryBypass): the codec lane is wired but bypasses real kernels.
- Student backend is a stub; cascade traits (ASR/LLM/TTS) are mocks.
- Emotion is a heuristic proxy, not a trained classifier.
- RTF printed is for the stub path; it is **not** a model benchmark.
- R0–R3 (VPS) gates stay PENDING; this is a local CPU demo only.
- No legacy container formats anywhere — `.gq` / safetensors only (gate G2).

## 6. Gates

`galion-gates/run-gates.sh` runs G0–G3 hard and G4/G6/PS/LK/DG/**LV** soft.
**LV** = `galion-gates/lv-live-demo.sh`: runs `scripts/run-live-demo.sh --showcase` headless into a temp
dir (DISPLAY cleared, `GALION_HEADLESS=1`, timeout `GALION_LV_TIMEOUT`, default 180 s) and checks the
stereo WAV + capture of all three variants. PASS on rc 0, SKIP if the script is absent or exits 3,
SOFTFAIL otherwise. Measured: ~11 s warm. It only counts toward the exit code with `GALION_GATES_HARD=1`;
`GALION_GATES_NO_LV=1` skips it. A missing display never fails CI.

## 7. Troubleshooting

| symptom | fix |
|---|---|
| `cargo` tries the network | run with `--offline`; the gates already retry offline first |
| no window appears | expected without `$DISPLAY`; check the terminal dot / console capture instead |
| `run-live-demo.sh` build fails | fall back to `galion-demo/run_demo.sh` (canned, same artifact shape) and report the cargo error |
| `live-demo: FAIL workplace is not hard-OFF` | consent gate regression — stop the demo, check `galion-heartbeat` / Rika gate |
| `--web` port busy | another dot demo is running on 127.0.0.1:7878; stop it |
| capture says `ZeroWeights` / `mock` / `canned` | expected today — no trained weights |
| Rika never fires | opt-in: needs SENSE+ADAPT consent and a personal/accessibility purpose; workplace/education are hard OFF |
| DG golden mismatch | canned output changed; re-pin only with an owner decision: `galion-demo/golden.sh --write` |
| G2 FAIL | a doc or code line names a legacy container format; reword (".gq only") — gate scripts are the only allowed place |
| LV times out | raise `GALION_LV_TIMEOUT`; the first release build is the slow part |
| no sound | `aplay`/`paplay` missing — open the WAV in any player; `--play` skips quietly |

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
