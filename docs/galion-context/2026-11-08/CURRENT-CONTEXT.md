# Current context — Nebula and emotion on main

Verified against voice `main` `3e1f5a12c53cd8e0dd47d111676d61d608540559` on 2026-11-08 (CET). Historical numbers are labelled. No new measurement of accuracy or frame rate was taken.

## Palette roles

Three different colours are easy to mix up. They are separate fields.

| Role | Where | What it is |
|---|---|---|
| Spectral hex | `crates/nebula-elements` `DEMO` / `UNIVERSAL`, copied from `docs/emotion-v2/UNIVERSAL-EMOTION-GAS-TABLE-20261107.md` | CIE 1931 gamut-clipped swatch of the wavelength. Not a display colour. |
| Display hex | `DemoLine.display_hex` only (13 rows) | Hand-chosen semantic swatch. Research note says these were not moved. Closest pair cited there is Amused `#FF9C64` and Excited `#FF6300`. |
| Voice grade | `apps/nebula-voice-v2/src/shade.rs` `star_linear` and `grade_linear` | Samantha gold and Jarvis blue as the illuminating star and a small grade. They tint thin dust. They do not replace the emotion colour. |

Pull request 102 does not edit `shade.rs` or the element table. Its GPU path (`render.rs` `write_sprites`) mixes about 22 percent of the star colour into glow sprites. That is a compositor step on top of the existing grade, not a new swatch.

`map_frame` in `frame.rs` computes a warmth value from valence. `drive_from` drops it. The simulator never reads warmth. The README sentence that valence scales hue warmth is ahead of the code.

## Two gas-line vocabularies

The live feed (`tools/emotion-v2/nebula/gas.py` and `NEBULA-LIVE-FEED.md`) uses ids such as `Hbeta-486.1`, `[NII]-575.5`, `Halpha-656.3`.

The visualiser table uses `H-beta`, `NII-575.5`, `H-alpha`. `label_weight` rejects a supplied id that is not the table id.

`parse_frame` also expects a weight to be a number. The live feed sends `{ "weight", "gas_line" }`. A number read fails, the mass stays zero, and the frame falls back to neutral. `--stdin` prints dominant names and does not draw. The browser loop calls `frame_at`, a scripted cycle, and has no socket for the feed.

So the feed, the score, and the picture are three contracts:

| Contract | Id | Reads audio? |
|---|---|---|
| Live feed | `galion-nebula-live-feed/1` | No. It smooths a posterior the caller already has. |
| Offline score | `galion-emotion-nebula/1` in `tools/cloning-v2/scorers/persona/py/nebula.py` | No. `affects_gate` is false. |
| Picture | `galion-nebula/1` in `apps/nebula-voice-v2/README.md` | No. Demo weights come from `demo.rs`. |

## Physics that is actually in the simulator

`noise.rs` builds a curl of a value-noise potential. Octave weights follow `f^(-4/3)` on the potential so the curl is near `f^(-1/3)`. A test checks that the divergence stays small. That is procedural curl noise, not a magnetohydrodynamic solve.

`sim.rs` seeds particles on five open curves (`spine_point`), deposits mass on a 12³ grid, and adds a mild gradient plus a spine spring. Speed is capped. There is no magnetic field and no acoustic wave solver. The “waves” in the spine are sines used as a shape.

`shade.rs` applies Rayleigh `λ^-4` optical depth and a Henyey–Greenstein phase function, then mixes spectral and display colour (84 percent toward display). Weak lines carry a low `intrinsic` so the code does not pretend they are bright. Excited is the laboratory neon line (`lab_line`).

Universal has 62 lookup rows. The picture uses the 13 demo rows only. `label_weight("love")` errors. Love is not a particle layer.

## Frame pacing, not a measured 120

`quality.rs` documents that the repository had no Nebula fps constant before that file. `normal` is an automatic ladder `{30, 45, 60, 90, 120}` capped by the refresh. `high` is a fixed 120 target, then the browser constructor clamps it with `ladder_at_most(refresh)`, so a 60 Hz panel does not keep 120. `ultra` is 240 and is refused without a discrete GPU. No run in this review recorded an achieved frame rate. The optional HUD (`?fps=1`) is a display of the running average, not a stored profile.

## What was not remeasured

The 7 November handoff says Samantha `emotion_match` sat about 0.46–0.81 against a 0.83 gate. That is a historical claim in `docs/emotion-v2/HANDOFF.md` and the foundation spec. This review did not rescore audio.
