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
cargo test --manifest-path voice-stack/one-dot-ui/Cargo.toml
cargo run --manifest-path voice-stack/one-dot-ui/Cargo.toml -- serve --mock
```

Open `http://127.0.0.1:7881/`. The dot talks to `ws://127.0.0.1:7881/v1/samantha` (`galion-samantha-ws/1`). `GALION_SAMANTHA_WS` overrides the bind, and it still has to be loopback.

Preview with no server at all:

```bash
cargo run --manifest-path voice-stack/one-dot-ui/Cargo.toml -- serve
```

Then open `http://127.0.0.1:7881/?mock=1`. The page does not open a socket.
`file://` also forces that preview.

Clock check (prints the measured tick gaps):

```bash
cargo run --manifest-path voice-stack/one-dot-ui/Cargo.toml -- check
```

## Run on Windows

From the repository root, in `cmd` or PowerShell. If the tree lives under
`G:\galion`, `cd` there first. The commands are the same either way.

```bat
cargo test --manifest-path voice-stack\one-dot-ui\Cargo.toml
cargo run --manifest-path voice-stack\one-dot-ui\Cargo.toml -- serve --mock
```

Open `http://127.0.0.1:7881/` in the browser. The microphone works on
`localhost` / `127.0.0.1`. A `file://` open is preview only and does not
ask for the mic until you tap, and the tap may still be refused by the
browser off localhost.

`GALION_SAMANTHA_WS` overrides the bind. Anything other than a loopback
address is refused.

## The page

- One violet dot. It is the only visible control. Tap it, or press Enter or Space, to talk. Tap it again, or press Escape, to stop and drop the microphone.
- The microphone is requested inside that tap, not on load.
- Her reply shows as text near the dot while she is speaking, then goes away. There is no captions button.
- The dot's accessible name follows the state, and a screen reader hears that state.
- `prefers-reduced-motion: reduce` drops the breath and the spring. The
  dot still changes size when the state changes.
- Mood (arousal, valence, dominance) changes the breath, the violet, and
  a little of the size. Valence stays inside the violet family. Rest is
  `{0.3, 0.25, 0.5}`.

## Measured on this workspace

These numbers are from this cloud VM (Linux, rustc 1.83, debug build),
not from the owner's laptop. `samantha-dot check`:

```
tick_samples=9
tick_min_ms=79.952
tick_median_ms=79.981
tick_max_ms=83.957
http_us=393
index_bytes=1059
caption_ms=645.245
```

The tick target is 80 ms. `caption_ms` is the preview think window after a
typed `text` frame (the constant is 640 ms), not a brain latency.

Headless Chrome on the same VM, `http://127.0.0.1:7881/?mock=1`:

- The only button is the dot. Paint interval over 120 frames, median 16.7 ms.
- `getUserMedia` count before any tap: 0.
- A click walked idle → listening → thinking → speaking. The line
  `Hi. I'm here.` was on screen 2247 ms after the click, then a second
  click returned to idle and cleared it.
- Enter started listening. Escape returned to idle and the mic flag to off.
- With Chrome's fake microphone, the mic flag stayed off until the click,
  then `on`, then `off` after the second tap.

No Samantha audio was played. The page said "No voice on this preview."

## What this is not

No HORBM, no ASR, no TTS, no cloned voice, no audio files. The mock line
is text. The speaking envelope is a sine, not her voice. A later mouth
can send `GVPC` frames and the page will play them.
