# galion-console

Terminal-default (ratatui, SSH-friendly) console for Galion Voice: bench history,
tensor activity, job status — and a minimal YES / NO control on the 80 ms heartbeat.

```
cargo run -p galion-console                                # interactive (q / Esc to quit)
cargo run -p galion-console -- --headless 20               # CI smoke frame, no TTY
cargo run -p galion-console -- --headless 12 --answers y..n # scripted presses, 1 char per tick
```

## YES / NO (heartbeat widget)

| key                 | answer |
|---------------------|--------|
| `y` / `Y` / `Left`  | YES    |
| `n` / `N` / `Right` | NO     |

A press only *arms* an answer. On the next 80 ms tick `ConsoleState::advance()`
commits it as one `HeartbeatEvent { tick, kind, at_ms = tick * 80, presses }`.
Coalescing: queue capacity 1, **last press in a frame wins** — at most one event
per tick. The committed answer is inverted for 4 ticks (320 ms), then rests dim.
See `src/heartbeat.rs`.

## Live feeds (A11)
- `galion-console --live` — deterministic stub feed (`--sense-off` ⇒ `no reading`).
- `galion-console --feed real` — real galion-tensor kernel timing + galion-emotion `AffectStore` SENSE (features `tensor-feed` + `emotion-feed`, on by default). `--no-default-features` builds stub-only.

### Mic input + SENSE estimator (A11 wave 2)
- `--mic-wav PATH` — CAPTURE from a WAV fixture (integer PCM 8/16/24/32-bit, mono/stereo, 8–192 kHz), decoded by the shared `galion_presence::wav` loader (feature `wav-mic`, in `real-feeds`), linearly resampled to 24 kHz and looped. Implies `--feed real`. Example: `--mic-wav specs/fixtures/p7-turn-mono-8k16.wav`.
- `--mic synth` (default) — deterministic tone (unchanged). `--mic live` — **stubbed**: no audio device on the box and no audio backend compiled in → exits 2 with the reason. A device backend implements `mic::MicSource` later.
- `--affect loudness` (default; valence unknown = 0) | `synthetic:<neutral|sweep|joy|angry|sad|tired|calm|valence:V>` | `reference:PATH` (replays an offline label track, e.g. `galion-emotion/fixtures/affect-ref-synthetic.txt`). Implies `--feed real`. No model is loaded, downloaded or trained.

## Scope & gates (W20-07, 2026-11-02)
- **Developer console only** — not an end-user UI; no user-facing surface ships from this crate.
- Local gate: `cargo test -p galion-console` → 37/37 pass (stub + real feeds, WAV mic, headless frames).
- Synthetic by design: tensor weights synthetic, `--mic live` stubbed (no audio device), no model loaded/trained.
- Still open: Rika glyph row + `RIKA: off (no SENSE)` badge (`FrameOut::glyph()` ready, not drawn); pinned headless console golden.

## One-dot + live console demo (W21-04, feature `dot-viz`)
`scripts/run-dot-console-demo.sh` — real 80 ms heartbeat → `DotConsole` (galion-render one-dot: PNG sequence,
terminal; the web one-dot screen is `scripts/run-v1-screen.sh`) + TIMING / EMOTION / BANDS / RIKA panel. Headless-safe. Embedding guide:
`docs/agent-wave21/W21-04-ATTACH.md`.
