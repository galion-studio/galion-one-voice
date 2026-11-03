# V2-S2 · Orb motion contract

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Master slogan: "The seamless connection with the human being, on the voice only."
The orb is the mark that connection has. It is still one mark. The slogan is
not drawn on it.
`LOCKS.toml` `[ui]` is unchanged: one dot, `#F5F5F7`, amber→violet on the
background only. The live mark's body stop is that locked pigment. Brightness
is glow and a little size. It does not multiply the fill.

## Why V1 felt stiff

`galion-render` `DotParams::compute` quantises speaking to the 80 ms grid
("one step per frame, never in between"). The first screen page also drew
whatever `/dot` returned on that same grid. A phone looking at that sees a
dot that jumps. V1 later smoothed radius with an exponential lerp on a 60 fps
clock. That removed the stutter. It did not give the mark mass, and speaking
did not follow the sound coming out of the speaker.

## What the page does now

`crates/galion-console/assets/v1-screen.html` keeps the V1 phone pipeline and
replaces the lerp with a spring-damper.

- State is position and velocity for radius and for brightness (`spring()`).
- The easing profile is **Luti**. Each mode has a soft aim (`LUTI` on the
  page; `SPRING` is the same object). The live gain eases toward that aim
  (480 ms). A mode change does not rebind ω and ζ in one frame. The numbers
  are the ones below. They are not a `LOCKS.toml` value.
- Idle is a slow breath (soft spring, low frequency). Rika lean-in stays on
  this motion. The lean is a little extra radius, not the speaking spring.
- Listening couples radius to microphone energy, local RMS first, `/dot` level
  as well. The mark grows while you talk, before the server round trip.
  The drive is low-passed.
- Thinking is the gap after `POST /turn` returns and before playback starts.
  It is a motion of the same mark, not a new widget. `LOCKS.toml` `dot_states`
  is still idle / listening / speaking. Thinking is a client motion pending an
  owner decision if it should become a fourth locked state.
- Speaking, while a reply WAV is playing, follows `samplePlayEnv()`: an
  `AnalyserNode` on the playback graph, low-passed (about 200 ms attack,
  380 ms release). Radius tracks that envelope calmly. An empty WAV does not
  pump the mark. Server `/dot` radius is the fallback when nothing is playing
  locally yet, and that target is low-passed before the spring.
- Aspect stays 1. The mark is a circle. The old `radius_velocity * 6` squash
  (draw clamp 0.82..=1.18) flipped the disc between wide and tall ovals and
  is gone.
- The fill is a near-white radial gradient. The body stops are `#F5F5F7`.
  A 1-device-pixel stroke `rgba(255,255,255,0.55)` is the contour. The inner
  highlight follows `GET /dot` `"emotion"`. The surround wash follows that
  same feeling. No second
  ring, no face, no caption on the mark.

The server me band is the other half. `PhoneHost::drive_me` drains reply PCM
into the 24 kHz heartbeat frame, so `/dot` `level` moves with the reply even
for a client that is not running this page.

## Luti

Luti is the name of that spring-damper. It is not a second curve, and it is
not a CSS easing keyword. It is the gain table the page and `OrbEngine`
already shared, now named so a later change cannot retune it by accident.

| Motion | `w` (rad/s) | `z` |
|---|---|---|
| idle | 2.4 | 0.86 |
| listening | 11 | 0.70 |
| thinking | 6.5 | 0.48 |
| speaking | 18 | 0.58 |
| offline | 3.2 | 1.05 |

Brightness uses `w * 0.85` and `min(1.2, z + 0.08)`, as the page does.
`DotParams` stays the 80 ms golden. Luti eases toward it. The gains are not
in `DotParams::to_json`, so the G4 hash does not move.

`DisplayHz::Hz240` is unchanged (`1/240` s). That step sits inside the
integrator clamp (`0.001..=0.05` s), same as 60 Hz and 120 Hz. The phone
loop is `requestAnimationFrame`: a 240 Hz panel is integrated at the `dt`
the panel just delivered, not resampled onto 80 ms.

One screen. The graphic is the one mark. The hint and the Art. 50 line are
the existing phone-path text, not a second shape. Detail:
`docs/galion-voice-v2/ONE-UI.md`.

## What this is not

Not a shader engine, not a scene graph, not V3. The shared integrator lives
in `galion-render` (`spring`, `OrbEngine`, profile Luti). Design:
`docs/galion-voice-v2/ANIMATION-ENGINE.md`. PNG, the terminal, and
`NullSurface` do not call it. `WebSurface::present` takes one 240 Hz Luti
step so `/dot` can carry the in-process account; the radius it publishes
is still `DotParams`. The page is still what the phone sees, and
`DotParams` stays the golden for G4. Pointing PNG or the window at the
spring is a later owner step.

## When the surface is in a fault

Acceptance item 4 is the healthy room: `#0A0A0C` and the amber → violet
wash, with the circle a solid `#F5F5F7`. A fault is a different draw of
that same pair. The vignette comes off, the field is the gradient, and
one `GV-` code is rendered in the gradient, clear of the circle. Luti,
the spring feel on this mark, does not gain an error row and does not
paint the fill. Contract: `ERROR-STATE.md`. The page does not draw this yet.

## Acceptance

1. The page source contains `function spring` and `samplePlayEnv` (console test
   `screen_page_is_phone_path_and_v2_orb`).
2. On a 60 fps display, idle radius does not jump once per `/dot` poll.
3. During playback, radius changes inside a syllable, not only on the 80 ms grid.
4. One canvas, one filled mark, background still `#0A0A0C` with the amber → violet wash.
5. G4 NullSurface golden unchanged, because `dot.rs` was not retuned.
6. `OrbEngine` steps at 1/60, 1/120, and 1/240 without NaNs, and
   `NullSurface` still records `DotParams`.
7. The gain set is named Luti (`Luti::radius` matches the table above,
   `LUTI` on the page). `DisplayHz::Hz240` is still 240.

— Maciej Grajczyk
