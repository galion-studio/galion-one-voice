# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03.

A section is complete when its gate exits 0 and the result is written here.
The gate does not edit `LOCKS.toml`.

| Section | Gate | Result | Date |
|---|---|---|---|
| AUTO-WORKFLOW | `scripts/symphony/auto-workflow-eval.sh` | PASS | 2026-11-03 |

Witness, 2026-11-03, Rust 1.85. `scripts/symphony/auto-workflow-eval.sh`
printed `PASS: one-dot auto-record workflow`. These tests passed:

- `auto_workflow::tests::mic_without_a_hint_is_the_auto_program`
- `auto_workflow::tests::a_token_launches_a_skill_and_junk_does_not`
- `auto_workflow::tests::the_page_proceeds_on_the_mic_path_and_draws_no_second_ui`
- `phone_host::tests::drive_posts_mic_level_and_utterance_on_the_bus` (feature `phone-turn`)
- `dotviz::tests::screen_page_is_phone_path_and_v2_orb` (feature `phone-turn`)
- `backends::web::tests::cors_preflight_and_x_sample_rate` (feature `web`)

The proceed call is one token scan when a take ends. Quiet 80 ms frames leave
it alone. This gate does not claim a time-to-first-audio number.

— Maciej Grajczyk
