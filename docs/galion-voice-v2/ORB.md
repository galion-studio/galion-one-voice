# V2-S2 · Orb motion contract

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
This is the foundation of the graphic engine. It is still one mark.
`LOCKS.toml` `[ui]` is unchanged: one dot, solid `#F5F5F7`, size and brightness
only, gradient on the background only.

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
- Each mode has its own stiffness and damping (`SPRING`).
- Idle is a slow breath (soft spring, low frequency).
- Listening couples radius to microphone energy, local RMS first, `/dot` level
  as well. The mark grows while you talk, before the server round trip.
- Thinking is the gap after `POST /turn` returns and before playback starts.
  It is a motion of the same mark, not a new widget. `LOCKS.toml` `dot_states`
  is still idle / listening / speaking. Thinking is a client motion pending an
  owner decision if it should become a fourth locked state.
- Speaking, while a reply WAV is playing, follows `samplePlayEnv()`: an
  `AnalyserNode` on the playback graph. Radius and brightness track that
  envelope, so the orb moves with the voice the phone is actually playing.
  Server `/dot` radius is only the fallback when nothing is playing locally yet.
- A little velocity becomes ellipse aspect (`aspect`). Still one filled shape.
  No second ring, no face, no caption on the mark.

The server me band is the other half. `PhoneHost::drive_me` drains reply PCM
into the 24 kHz heartbeat frame, so `/dot` `level` moves with the reply even
for a client that is not running this page.

## What this is not

Not a shader engine, not a scene graph, not V3. The next graphic step, when
an owner wants it, is to move this spring into `galion-render` so terminal,
PNG, and web share one integrator. Until then the page is the place the phone
sees, and `DotParams` stays the golden for G4.

## Acceptance

1. The page source contains `function spring` and `samplePlayEnv` (console test
   `screen_page_is_phone_path_and_v2_orb`).
2. On a 60 fps display, idle radius does not jump once per `/dot` poll.
3. During playback, radius changes inside a syllable, not only on the 80 ms grid.
4. One canvas, one filled mark, background still `#0A0A0C` with the amber → violet wash.
5. G4 NullSurface golden unchanged, because `dot.rs` was not retuned.

— Maciej Grajczyk
