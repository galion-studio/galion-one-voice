<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S11 · Samantha one-dot

A single breathing violet dot for Samantha. Plain HTML, CSS, and JS. No
framework and no build step. The Rust crate serves the page on loopback
and speaks the socket in [PROTOCOL-ASSUMED.md](PROTOCOL-ASSUMED.md).

This directory is its own Cargo workspace. It is not a member of the
root workspace, and the root `Cargo.toml` is not edited.

## What it reuses

The Galion One page stays the near-white disc (`#F5F5F7`). This mark uses
the violet stop (`#7C3AED`) because this sector is the violet dot. The
motion and the wash are the existing ones:

| Piece | Taken from |
|---|---|
| Idle / listening / speaking radii and brightness | `galion-render/src/dot.rs` `DotParams` |
| Thinking, as a motion of the same mark | `crates/galion-console/assets/v1-screen.html` and `docs/galion-voice-v2/ORB.md` |
| Luti spring (`w`, `z`, 480 ms gain ease) | `galion-render/src/orb_engine.rs` `OrbMotion::radius_gain` and `function spring` in `v1-screen.html` |
| Wash: obsidian `#0A0A0C`, amber `#F5A524`, violet `#7C3AED`, strength 0.38, 24 s | `galion-render/src/palette.rs` `background_at` |
| 80 ms tick | `galion-heartbeat` `FRAME_MS` |
| Mic only after the first tap, 640 ms quiet, 15 s cap | the phone path in `v1-screen.html` |
| Reconnect backoff 80, 160, 320, 640, 1280 ms | `apps/nebula-voice-v2/web/boot.js` |
| Greeting `Hi. I'm here.` | `docs/galion-voice-v2/personas/SAMANTHA.md` |
| Loopback bind | the same rule as `galion-render` `WebSurface` and the systemd units (those units are not installed or edited here) |

`ReplyEngine` is the seam for `horbm::Brain::turn`. The real crate is not
linked. Reference audio is not loaded. `GALION_SAMANTHA_REF` belongs to
the mouth, not this page.

## Run on Linux

From the repository root:

```bash
cargo test --manifest-path voice-stack/s11-one-dot-ui/Cargo.toml
cargo run --manifest-path voice-stack/s11-one-dot-ui/Cargo.toml -- serve --mock
```

Open `http://127.0.0.1:8788/`. The dot talks to `ws://127.0.0.1:8788/v1`.

Preview with no server at all:

```bash
cargo run --manifest-path voice-stack/s11-one-dot-ui/Cargo.toml -- serve
```

Then open `http://127.0.0.1:8788/?mock=1`. The page does not open a socket.
`file://` also forces that preview.

Clock check (prints the measured tick gaps):

```bash
cargo run --manifest-path voice-stack/s11-one-dot-ui/Cargo.toml -- check
```

## Run on Windows

From the repository root, in `cmd` or PowerShell. If the tree lives under
`G:\galion`, `cd` there first. The commands are the same either way.

```bat
cargo test --manifest-path voice-stack\s11-one-dot-ui\Cargo.toml
cargo run --manifest-path voice-stack\s11-one-dot-ui\Cargo.toml -- serve --mock
```

Open `http://127.0.0.1:8788/` in the browser. The microphone works on
`localhost` / `127.0.0.1`. A `file://` open is preview only and does not
ask for the mic until you tap, and the tap may still be refused by the
browser off localhost.

`SAMANTHA_DOT_ADDR` overrides the bind. Anything other than a loopback
address is refused.

## The page

- One violet dot. Tap it, or press Enter, to talk.
- Stop is always on screen. Escape stops too.
- The microphone is requested inside that tap, not on load.
- Captions is a toggle. On, it shows her reply and the source labels.
- `C` toggles captions. The state is also announced to a screen reader.
- `prefers-reduced-motion: reduce` drops the breath and the spring. The
  dot still changes size when the state changes.
- Mood (arousal, valence, dominance) changes the breath, the violet, and
  a little of the size. Valence stays inside the violet family. Rest is
  `{0.3, 0.25, 0.5}`.

## Measured on this workspace

These numbers are from this cloud VM (Linux, rustc 1.83, debug build),
not from the owner's laptop. `samantha-dot check` on the final binary:

```
tick_samples=10
tick_min_ms=75.872
tick_median_ms=80.013
tick_max_ms=84.089
http_us=309
index_bytes=1301
caption_ms=645.857
```

The tick target is 80 ms. The median landed on it. `caption_ms` is the
preview think window after `end` (the constant is 640 ms), not a brain
latency.

Headless Chrome on the same VM, page at `http://127.0.0.1:8788/` with
`--mock`:

- Paint interval over 120 frames: min 16.6 ms, median 16.7 ms, max 16.8 ms.
  That is the display clock, about 60 fps, not the 80 ms tick.
- `getUserMedia` count before any tap: 0. Stop was visible, 44 px tall.
- Enter with Talk focused asked for the mic and the state became listening.
  Escape returned to idle and the mic flag to off.
- A click then walked idle → listening → thinking → speaking. The caption
  `Hi. I'm here.` and source `fake:reply-engine` were on screen 2257 ms
  after the click. That wait is the 1600 ms quiet cap plus the 640 ms
  think, because the headless mic was denied and the preview treated the
  turn as silence.
- With Chrome's fake microphone, the mic flag stayed off until the click,
  then `on`, then `off` after Stop.
- `prefers-reduced-motion: reduce` set `data-motion="reduced"` and did not
  ask for the mic.

No Samantha audio was played. The page said "No voice on this preview."

## What this is not

No HORBM, no ASR, no TTS, no cloned voice, no audio files. The mock line
is text. The speaking envelope is a sine, not her voice. A later mouth
can send `GVPC` frames and the page will play them.
