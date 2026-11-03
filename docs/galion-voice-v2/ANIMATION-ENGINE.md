# V2-S2 · Animation engine

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
The spelling is Galion. This is the design for the CPU orb integrator, and
the note for the Rust foundation that landed beside it. It is not V3.
There is no separate white paper in this repository. This file is the
engine note. Motion contract: `ORB.md`. Brand: `docs/brand/GALION-VOICE-BRAND.md`.

The mark stays one filled orb. Fill `#F5F5F7`, ground `#0A0A0C`, wash
amber `#F5A524` → violet `#7C3AED` on the background only.
`chrome_allowed = false`. `LOCKS.toml` `[ui]` is not edited.

## Two clocks

| Clock | Rate | Who owns it |
|---|---|---|
| Heartbeat | 12.5 Hz, 80 ms, 24 kHz, 1920 samples | `Heartbeat::step`. Audio, Rika, the me band. 60 ms compute budget inside the frame. |
| Display | 60 Hz baseline. 120 Hz and 240 Hz when the panel's vsync actually delivers them. | The animation thread only. Phone page: `requestAnimationFrame`. Future window: the softbuffer thread. |

The heartbeat does not speed up because a monitor is 240 Hz. The display
does not drop to 12.5 Hz to match the codec. Physics uses the frame `dt`
the display just measured. A 60 Hz frame is `1/60` s, a 120 Hz frame is
`1/120` s, a 240 Hz frame is `1/240` s. If vsync jitters, the next step
uses that shorter or longer `dt`. Nothing in the integrator resamples the
motion onto an 80 ms grid.

`DotParams::compute` is still quantised for speaking ("one step per frame,
never in between"). That function is the G4 golden. The spring reads it as
a target and fills the time between heartbeats. It does not write back.

## Where the step runs

```text
audio thread                         animation thread
  Heartbeat::step (80 ms)              vsync dt  (60 / 120 / 240, or whatever arrived)
  build VizFrame                       take latest VizFrame (FrameMailbox, latest wins)
  Fanout::present                      DotParams::from_frame  → target held until the next heartbeat
        │                              OrbEngine::step(motion, target, dt)
        └─ mailbox / /dot JSON         draw one filled circle on the wash
```

`OrbEngine::step` is a few dozen floating-point operations and allocates
nothing. It is still forbidden inside `Heartbeat::step`. A 240 Hz loop
inside the audio clock would multiply work by the display rate and would
stall the voice on a late vsync. `Fanout::present` stays the cheap handoff.
PNG keeps its default `max_fps` of 12.5. The window stub's `present` only
stores the latest frame.

The phone already does the right split. `v1-screen.html` polls `/dot` on
the 80 ms grid and integrates `spring()` on `requestAnimationFrame`.
Listening targets include local microphone energy. Speaking targets follow
`samplePlayEnv()` on the playback `AnalyserNode`. This crate does not take
that page's job away. The page is still what the phone draws.

## The integrator

Semi-implicit Euler, the same step as `function spring` on the page.
`p` position, `v` velocity, `target` the held pose, `h` seconds, `w`
angular frequency (rad/s), `z` damping ratio:

```text
a = -w² (p − target) − 2 z w v
v = v + a h
p = p + v h
```

Radius and brightness are two independent springs. Brightness uses
`w * 0.85` and `min(1.2, z + 0.08)`, as the page does. Aspect is not a
third spring. A little radius velocity becomes ellipse squash, then an
exponential ease (`dt` in milliseconds, time constant 90 ms), clamped at
draw to `0.82..=1.18`. Still one filled shape.

Gains are the **Luti** profile (the page's `LUTI` table; `SPRING` is the
same object). `Luti::radius` / `Luti::brightness` in `orb_engine.rs`:

| Motion | `w` | `z` | What it feels like |
|---|---|---|---|
| idle | 2.4 | 0.86 | Slow breath. Soft. |
| listening | 11 | 0.70 | Opens with the microphone. |
| thinking | 6.5 | 0.48 | Tighter pulse while a turn is in flight. |
| speaking | 18 | 0.58 | Follows the voice that is playing. |
| offline | 3.2 | 1.05 | Small and still when `/dot` has gone quiet. |

Thinking and offline are motions. They are not `DotState` values.
`dot_states` in `LOCKS.toml` stays idle / listening / speaking. Rika
lean-in stays idle: `DotParams::compute_lean` already changed the target
radius, and the idle spring eases toward it.

`h` for a positive finite `dt` is `clamp(dt, 0.001, 0.05)` seconds. The
three display rates sit inside that window, so they are integrated at the
true frame time. The 50 ms ceiling is hitch recovery, not a hidden
heartbeat. Passing the raw 80 ms period is a different step from five
frames at 1/60, and the unit test locks that. A non-positive or
non-finite `dt` does not move the state. The page's `Math.max(0.001, …)`
also fires when `dt` is 0; the Rust step does not. A non-finite target
holds the last finite pose. The page can return NaN when `target` is NaN.
The Rust step must not.

Draw clamps: radius `0..=0.5`, brightness `0..=1`, aspect `0.82..=1.18`.
The integrator position may sit slightly outside that for one overshoot.
Targets are clamped into the legal range before the force is computed, so
a bad number cannot wind the spring up.

Code, always compiled, no feature flag, no new dependency:

- `galion-render/src/spring.rs` — one degree of freedom.
- `galion-render/src/orb_engine.rs` — Luti gains, radius, brightness, aspect, `DisplayHz`, `OrbEngine::step_toward_dot`, `OrbAccount`.
- `galion-render/examples/orb_spring.rs` — one second at 240 Hz, targets from `DotParams`, stdout JSON only.

`OrbSample::to_json` is a draw snapshot (`motion`, `profile` `luti`,
`radius`, `brightness`, `aspect`, velocities, `dt_s`, plus the in-process
account: `frames`, `settle_err`, `cost_hint`). It is how a later client can
share state without a framebuffer. Wasm of the same functions can replace
the page's `spring()` later. The page still draws. `/dot` carries a witness
account; it does not carry the spring's radius.

## What draws the pixels

CPU. One circle. The wash is background.

The page already does the cheap version: the wash is painted at 96×54 and
scaled, then one vector ellipse is filled. A future `softbuffer` + `winit` backend
(the `window` feature is still a stub, zero dependencies, no wgpu, no glow,
no skia) should copy that structure:

- Keep the wash in a small buffer and blit it.
- Repaint the previous circle's box from that wash.
- Fill the new circle.

That is trivial at 240 Hz for a 1080p framebuffer. A circle at the idle
radius is on the order of 10⁵ pixels. At 240 Hz that is about 10⁷ writes
per second.

The existing `rasterize` walks every pixel and takes a square root. It is
the correct path for tests and for the PNG sequence (default 320×180 at
12.5 Hz). It is the wrong tool for 1080p at 240 Hz. Do not call it from
the audio thread, and do not pretend it is the display loop.

Full-HD video decode at 240 Hz is not this path. The orb is not a movie.

## Video, if it is ever a feature

Not in this change. No `video` feature flag yet. If an owner adds one, it
stays optional, default off, and CPU-only:

- In process: a non-intensive decode. MJPEG, or a low-resolution preview.
  Not full-HD 240 Hz.
- Out of process: ffmpeg, the way `scripts/run-dot-console-demo.sh` already
  works. `VIDEO=1` reads the PNG sequence at `-framerate 12.5` and writes
  `dot.mp4`. That is an offline archive of the heartbeat-rate frames, not
  a live ingest.

The feature must not become a camera bus, and it must not run inside
`Heartbeat::step`.

## ngrok is the phone UI

ngrok is HTTPS so a phone can call `getUserMedia` and reach the loopback
app (`docs/agent-v1-demo/EDGE-NGROK.md`). The tunnel carries `/dot` JSON,
`/mic`, `/turn`, and `/reply` WAV. The browser integrates and draws.

ngrok is not a video ingest bus. Do not stream the framebuffer through it.
Do not add a second tunnel for pixels. The server sends targets at the
heartbeat. The display, on the phone or on a future local window, draws
the mark itself.

## Budget

`LOCKS.toml` `[clock]` gives the heartbeat 60 ms inside an 80 ms frame.
Animation is outside that budget.

- `Heartbeat::step` does not call `OrbEngine`, does not rasterise, and
  does not wait on vsync.
- `Fanout` may drop a slow surface. It does not substep the spring.
- The display thread may fall behind. It integrates the `dt` it has, up
  to the 50 ms clamp, and moves on. It never blocks the voice to catch up.

## One-percent power

The integrator is the analytics. There is no second agent, no Prometheus
scrape, and no telemetry host. Each accepted `OrbEngine::step` updates an
`OrbAccount` in the same struct:

| Field | Meaning |
|---|---|
| `frames` | Accepted display steps. A refused `dt` or a non-finite target does not increment it. |
| `dt_s` | Seconds actually integrated on the last call. `0` when the call was refused. |
| `settle_err` | After the last accepted step, the larger of `|radius − target|` and `|brightness − target|`. |
| `cost_hint` | `Luti::STEP_COST` (`48`) when the last call integrated, else `0`. A fixed stand-in for a few dozen flops. Not a timer. |

`/dot` adds `"engine"` from one Luti step at `DisplayHz::Hz240` inside
`WebSurface::present`. That is a witness next to the heartbeat handoff, not
the phone's vsync loop, and not `Heartbeat::step`. `radius` and `brightness`
in that JSON stay `DotParams`. The page keeps its own account on the canvas
(`data-profile="luti"`, `data-luti` = frames, dt, settle error, cost) because
the panel, not the server, knows the real frame time. Neither account is
drawn. The mark stays one ellipse.

Work stays on the animation side of the 60 ms budget. One step is the whole
sample. Catching the display up inside the audio clock is still forbidden.

## G4

`NullSurface::present` still stores `DotParams::from_frame`. PNG, the
terminal dot, and `rasterize` still consume `DotParams`. This change does
not retune those numbers and does not re-bless `galion-gates/golden/g4-render.sha256`.

Wiring a surface so that the recorded dot is the spring output would be a
different change. That one re-blesses G4 on purpose, with an owner.

## Acceptance

Landed with the foundation:

1. `spring` and `OrbEngine` tests: no NaN on non-finite input, equilibrium
   holds, each motion settles at `1/60`, `1/120`, and `1/240`, a mixed
   `dt` stays bounded, and a finer `dt` tracks a fine reference more closely.
2. Inside one 80 ms speaking frame, `DotParams` is unchanged and the spring
   position moves.
3. An 80 ms `dt` is not the same step as substeps at 1/60 covering a
   similar wall time.
4. `NullSurface` still records `DotParams` after an engine step on the side.
5. Thinking is an `OrbMotion` only. `step_toward_dot` never invents it.
6. `cargo run -p galion-render --example orb_spring` prints JSON samples
   for one second at 240 Hz and writes nothing else.
7. G4 golden file is untouched.
8. `Luti::NAME` is `luti`. An accepted 240 Hz step increments `frames` and
   charges `cost_hint`. A refused step does neither. `/dot` `engine.hz` is
   `240` and the golden `state` is still `DotParams`.

Still the page's job, not this crate's yet:

1. `screen_page_is_phone_path_and_v2_orb` keeps passing.
2. On a 60 Hz display, idle radius does not jump once per `/dot` poll.
3. During playback, radius moves inside a syllable.
4. One canvas, one filled mark, ground `#0A0A0C`, wash amber → violet.
5. A later owner may point PNG or the softbuffer thread at `OrbEngine`.
   Until then those surfaces draw the discrete golden.

## Not in this step

- No edit to `LOCKS.toml`.
- No wgpu, glow, or skia.
- No shader, no scene graph, no second shape.
- No claim that this is V3.
- The phone page is not switched over to the Rust step.
- No video decoder.

— Maciej Grajczyk
