# Nebula Galion Voice V2

A standalone nebula visualiser for Samantha and Jarvis. It is its own Cargo
workspace at `apps/nebula-voice-v2` and is not a member of the root workspace.

The emission lines are hard-coded in `crates/nebula-elements`. Spectral hex
and the display target are separate fields. `UNIVERSAL` (62 rows) is lookup
only. The picture uses the Demo 13 rows.

## Frame pacing

Before this file, the repository had **no** nebula FPS constant. There was
no laptop value and no desktop value to preserve. Phone, laptop, and desktop
now share one pacing tier, `normal`.

Constants live in `apps/nebula-voice-v2/src/quality.rs`.

| Tier | Constant | Value | Pacing constant | Mode |
|---|---|---|---|---|
| pi | `PI_FPS_TARGET` line 22 | 30 | `PI_PACING` line 24 | `FixedCap` |
| normal | `NORMAL_FPS_MIN` line 27, `NORMAL_FPS_MAX` line 29, `NORMAL_FPS_LADDER` line 32 | 30, 120, `{30, 45, 60, 90, 120}` | `NORMAL_PACING` line 35 | `AutoLadder` |
| high | `HIGH_FPS_TARGET` line 38 | 120 | `HIGH_PACING` line 40 | `FixedCap` |
| ultra | `ULTRA_FPS_TARGET` line 44, `ULTRA_FRAME_BUDGET_MS` line 47 | 240, `1000/240` ms | `ULTRA_PACING` line 49 | `UncappedMailbox` |

`normal` is the default for phone, laptop, and desktop. The ladder step is
chosen from the display refresh (winit monitor refresh, or `requestAnimationFrame`
sampling on the web), the GPU class, device memory, and measured frame time.
A step is never above the refresh. Particles move to hold the chosen step.
Laptop and desktop do not have an FPS constant of their own.

`high` is an opt-in fixed 120 cap (`--quality high`, `?quality=high`).

`ultra` is opt-in (`--quality ultra`, `?quality=ultra`) and is refused on an
integrated or unknown GPU. It targets 240 fps with mailbox or immediate
present when the backend has it, otherwise vsync. Particles search upward
while frame time stays within `ULTRA_FRAME_BUDGET_MS`. Resolution starts at
`ULTRA_RENDER_SCALE` (1.0) and may rise to `ULTRA_SUPERSAMPLE` (1.5). It uses
three bloom passes, an HDR `rgba16float` target when the device allows it,
tonemapping, dithering, four scatter samples, and six curl octaves.

`pi` stays a fixed 30 cap, 2500 particles, 0.5× scale, one bloom pass, and
ordered dither. `--quality phone|laptop|desktop` keeps `normal` pacing and
only selects the particle class.

## Run

```bash
# headless PNG frames, no display
cargo run --manifest-path apps/nebula-voice-v2/Cargo.toml --release -- --headless --frames 4 --out /tmp/nebula

# Pi tier
cargo run --manifest-path apps/nebula-voice-v2/Cargo.toml --release -- --headless --quality pi --frames 1 --voice samantha

# stdin, one JSON object per line
cargo run --manifest-path apps/nebula-voice-v2/Cargo.toml --release -- --stdin

# window (wgpu low-power adapter, CPU softbuffer fallback)
cargo run --manifest-path apps/nebula-voice-v2/Cargo.toml --release --features window -- --demo
```

`--voice both` is the default of `--headless` when `--voice` is omitted.

### Frame schema `galion-nebula/1`

```json
{
  "voice": "samantha",
  "timestamp": 1.25,
  "vad": 1.0,
  "energy": 0.4,
  "valence": 0.2,
  "arousal": 0.5,
  "dominance": 0.1,
  "weights": [
    {"emotion": "happy", "weight": 0.7, "element": "NII-575.5"}
  ]
}
```

`vad` is voice activity in `[0, 1]`. It may also be a boolean, or an object
`{"active": 1, "valence": 0, "arousal": 0, "dominance": 0}`. `valence`,
`arousal`, and `dominance` may sit at the top level. A mismatched `element`
is rejected. An omitted `element` is filled from the const. Seductive is
Samantha-only.

Arousal scales turbulence, valence scales hue warmth, dominance scales
clumping, and speech `energy` gated by `vad` scales the pulse.

A line whose `schema` is `galion-nebula-live-feed/1` or
`galion-voice-v2-emotion-feed/1` is the live contract in
`docs/galion-voice-v2/EMOTION-NEBULA-FEED.md`. `estimate: none` (and
`present: false`) leaves every weight at 0. It does not fill Neutral.
`drive_from` then keeps turbulence, clump, and pulse at 0. Bind particles
with `element_id` (`H-beta`, `H-alpha`). The feed's `gas_line`
(`Hbeta-486.1`, `Halpha-656.3`) is a different string.

## Browser

`apps/nebula-voice-v2/build-web.sh` writes a static `dist/` (gitignored).
Serve that directory. `index.html` links to `samantha.html`, `jarvis.html`,
and `spectrum.html`. The voice pages are a pure nebula until `S` or a
three-finger tap shows the spectrum strip.

## Pi / aarch64

```bash
rustup target add aarch64-unknown-linux-gnu
cargo check --manifest-path apps/nebula-voice-v2/Cargo.toml --no-default-features --target aarch64-unknown-linux-gnu
cargo check --manifest-path apps/nebula-voice-v2/Cargo.toml --features gles --target aarch64-unknown-linux-gnu
```

`gles` is the window build. wgpu 23 includes the GL backend in its default
features. The runtime asks for a low-power Vulkan adapter (Pi V3D) and then
GL/GLES 3.1. There is no compute pass. Pi draws instanced point sprites with
one additive downsample bloom and Bayer dither.

## Tests

```bash
cargo test --manifest-path crates/nebula-elements/Cargo.toml
cargo test --manifest-path apps/nebula-voice-v2/Cargo.toml --no-default-features --lib
PYTHONPATH=tools/emotion-v2 python -m unittest tools/emotion-v2/harness/tests/test_elements_sync.py
```
