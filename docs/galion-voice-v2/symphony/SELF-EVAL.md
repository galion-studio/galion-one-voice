# Symphony self-evaluation

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A section is complete when its gate passes and the run is written here.
Incomplete stays blocked. This ledger does not edit `LOCKS.toml`.

## D — Samantha emotions and sounds

Spec: [`SAMANTHA-EMOTIONS.md`](SAMANTHA-EMOTIONS.md).

Bar: personality, behavior, and emotion in the voice, not timbre alone. Emotion mapped to sound, prosody, and wash. Empathy is the live 80 ms weight: OKLCH and prosody from `voice_color`. No scripted empathy line.

Run: 2026-11-03. Host rustc 1.83.0 (`90b35a623`, 2024-11-26).

| Gate | Result | Evidence |
|---|---|---|
| D1 Personality, behavior, and emotion, with timbre named as the mouth | Pass | `SAMANTHA-EMOTIONS.md` §1–§3 |
| D2 Eight `Affect` labels plus a missing frame, each with sound, both prosody paths, and wash. OKLCH `#F5F5F7` at `e = 0`, `#B695CF` at `e = 1` | Pass | §6 tables. Anchors match `empathy_at_one_is_the_profile_and_heather` and `circle_stays_locked_while_empathy_only_names_a_wash` |
| D3 No fake empathy. Words on grief do not change when `e` goes from 0 to 1 | Pass | `cargo test -p galion-render --lib v2_persona::` — 10 passed, 0 failed. `soul_probes_stay_on_the_matter`, `high_e_changes_prosody_not_the_words`. Forbidden list is `v2_persona::FORBIDDEN` |
| D4 Live `e`, prosody, OKLCH, duplex feeling band | Pass | `cargo test -p galion-emotion --lib` — 76 passed, 0 failed. Includes `angry_caller_gets_calm_galion`, `joyful_caller_is_shared`, `full_duplex_loop` |
| D5 No lock edit | Pass | Diff is `docs/galion-voice-v2/symphony/` only |
| D6 Re-run `galion-heartbeat` and `galion-console` persona tests on this box | Not run | Both crates require rustc 1.85. `FRAME_MS = 80` cited from `crates/galion-heartbeat/src/lib.rs`. Register row cited from `bus.rs` `SAMANTHA_E0` / `SAMANTHA_E1` and checked via `configs_match_the_design_table` on `samantha.toml` |

Observation, not a gate failure: `Floor::finish` traps a forbidden phrase with `debug_assert`. Release builds drop that assert. The reply table and the test helper `no_script` are what keep the lines out.

Commands:

```text
cargo test -p galion-emotion --lib
cargo test -p galion-render --lib v2_persona::
```

Other symphony sections are not claimed by this run.

— Maciej Grajczyk
