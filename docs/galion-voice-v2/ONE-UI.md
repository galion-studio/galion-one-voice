# One-UI

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
The spelling is Galion. This is the phone surface for V2: one screen, one
mark. Motion contract: `ORB.md`. Integrator: `ANIMATION-ENGINE.md`.

## One screen

The page is `crates/galion-console/assets/v1-screen.html`. It is raw HTML,
CSS, and JavaScript in that one file. No React, no Vue, no Svelte, no npm,
no build step for the UI.

What you see is the fluid circle. It is a vector ellipse on a canvas
(`ellipse`, one fill, `#F5F5F7`). The amber → violet wash is the background,
painted small and scaled. Aspect squash is the same mark stretching a
little. There is no second ring, no face, no menu, and no counter drawn
on the screen.

The hint ("Tap the dot to talk") and the Art. 50 line are the existing
phone path, so a mic can be granted and the disclosure is visible. They
are not a second graphic. `LOCKS.toml` `[ui]` is not edited. A latched fault is `ERROR-STATE.md`: the wash goes full-bleed and one `GV-` code is drawn in that gradient. The circle fill stays `#F5F5F7`, and Luti gains no error row.

## Luti

Luti is the easing profile. Semi-implicit Euler, position and velocity,
one gain pair per motion. The page binds it as `LUTI`. `SPRING` is the
same object, kept so the historical name still resolves. Rust calls it
`Luti` (`Luti::radius`, `Luti::brightness`).

| Motion | `w` | `z` |
|---|---|---|
| idle | 2.4 | 0.86 |
| listening | 11 | 0.70 |
| thinking | 6.5 | 0.48 |
| speaking | 18 | 0.58 |
| offline | 3.2 | 1.05 |

`DisplayHz::Hz240` is unchanged. The page does not pick a rate. It
integrates the `dt` from `requestAnimationFrame`. A 240 Hz panel delivers
about `1/240` s, which is inside the spring clamp (`1` ms .. `50` ms), so
that panel runs the 240 Hz path. A 60 Hz panel stays at `1/60`. The
heartbeat stays 80 ms. Luti does not resample onto it.

## One-percent power

The engine counts its own work. Nothing is bolted on beside it: no
Prometheus, no analytics host, no extra crate.

`OrbEngine::step` keeps an `OrbAccount`:

- `frames` — accepted steps
- `dt_s` — seconds integrated last call (`0` if the call was refused)
- `settle_err` — larger channel error against the last accepted target
- `cost_hint` — `48` (`Luti::STEP_COST`) after an accepted step, else `0`

`48` is a fixed hint for a few dozen flops. It is not a measured time.

`GET /dot` adds `"engine"` from one Luti step at 240 Hz inside
`WebSurface::present`. `radius` and `brightness` in that object are still
`DotParams`, so the golden the page reads is unchanged and G4 is unchanged.
The witness is how a `curl` of `/dot` can see that the integrator is alive.
It is not the pixels.

The page's real display account sits on the canvas, not in the picture:
`data-profile="luti"` and `data-luti` as `frames,dt,settle,cost`. That
`dt` is the panel's frame. The server `engine.dt_s` is one `1/240` s
witness step per heartbeat handoff. They are not the same counter.

`Heartbeat::step` does not call `OrbEngine`. `NullSurface` still records
`DotParams`.

## No virtualenv

The UI needs no Python virtualenv. Rust serves the page. The page is the
file above. CI does not create an environment for it.

```bash
scripts/run-v1-screen.sh          # http://127.0.0.1:7878/
scripts/one-ui.sh                 # prints this fact and exits 0
```

Optional, and unused by the page and by CI: an empty local venv, created
only when you pass the flag. Nothing is installed into it.

```bash
scripts/one-ui.sh --venv
# or: GALION_ONE_UI_VENV=/tmp/galion-one-ui-venv scripts/one-ui.sh --venv
```

Do not point the orb at that environment. It does not run the mark.

## Auto-record

After the first tap the microphone stays open. Speech starts the take.
Quiet ends it. The page then proceeds on the same `POST /turn`. There is
no second button and no second AI page. The workflow is
`docs/galion-voice-v2/symphony/AUTO-WORKFLOW.md`.

— Maciej Grajczyk
