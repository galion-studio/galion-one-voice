# Swarm 10 persona-v2v — `/one` Samantha default (candidate)

Perpetual Open MIT License. (c) 2026 Galion Labs.
Written 2026-11-15 22:07 +01:00 (Europe/Zagreb).

The spelling is **Galion**. The product is Galion Voice.

## Flag for Maciej

This pull request is a **candidate**. The orchestrator does not merge it. `docs/galion-voice-v2/symphony/PERSONA.md` says Galion One is the product voice and Samantha is test/internal, and that presenting a test voice as the product is FAIL. The owner instruction for this lane is that Voice V2 stays **Piper + the Samantha persona** until a clone mouth is actually live. Those two lines disagree. This change follows the lane instruction and leaves the disagreement on the record.

## What was checked

On `8980e1a`, `mountSession` in `crates/galion-console/assets/v1-screen.html` set `cv.dataset.persona="galion"` when `productIsOne()` (`/one` or `/one/`) and returned. The canvas markup already said `data-persona="samantha"` and `data-mouth="piper"`. `e2e/one/server.mjs` defaulted `/dot` `persona` to `galion`, and `e2e/one/one.spec.js` posted that value and expected the attribute.

The Rust `/dot` handler in `galion-render` annotates the floor id. The floor ids are `samantha` and `jarvis` (`PersonaId` in `crates/galion-console/src/bus.rs`). A production `/dot` does not emit `galion`. The flip was the page, then the desktop stub echoing it.

`productIsOne()` still gates the 432 Hz tones, the boot/shutdown cues, and the early return that skips the decide box. This change does not remove that. The document title stays `Galion One`. `data-mouth` stays `piper` from the markup. Piper is not described as the original clone.

## What changed

- `/one` now sets `data-persona` to `samantha` instead of `galion`.
- The desktop stub's default and the idle `/__state` post use `samantha`.
- The desktop assertion expects `data-persona="samantha"`.
- `screen_page_is_phone_path_and_v2_orb` refuses `dataset.persona="galion"`.

No edit to `SOUL.md`, `config.yaml`, `LOCKS.toml`, weights, or reference pins. No wav committed.

## Tests

Host `cursor`, rustc 1.85.0, 2026-11-15 22:07 +01:00.

| Command | Result |
|---|---|
| `cargo +1.85.0 test -p galion-console --features dot-viz --lib screen_page_is_phone_path_and_v2_orb` | 1 passed, exit 0 |
| `cargo +1.85.0 test -p galion-render --lib page_shares_the_lattice_and_the_hold` | 1 passed, exit 0 |
| `npx playwright test` in `e2e/one` (Chromium 141.0.7390.37, Playwright 1.56.1) | 12 passed, exit 0, 17.2 s |

The desktop spec opened `/one`, expected the title `Galion One`, one canvas, `data-persona="samantha"`, then posted listening and saw the dot radius grow. Halo, contrast, and the 432 Hz cue specs passed on the same server. `cargo test --workspace` was not run. GitHub Actions for this pull request were not run yet at the time of the commit.

## CI

Not run locally as GitHub Actions. The pull request checks are the record.

## Open questions

1. Should `/one` keep Samantha (this candidate) or go back to a product id once a Galion One bus id exists? PERSONA.md and `docs/galion-context/2026-11-08/06-products-map.md` say there is no `galion` persona id on the bus.
2. Merging this is Maciej's decision. Lane C's C2–C4 merges do not include it.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
