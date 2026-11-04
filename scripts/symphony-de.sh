#!/bin/sh
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Symphony sections D and E. CPU-only gates. Exits 0 only when every row passes.
set -eu
cd "$(dirname "$0")/.."

cargo test -p galion-emotion --lib persona::
cargo test -p galion-heartbeat --test heartbeat feeling_
cargo test -p galion-heartbeat --test zero_alloc feeling_step_allocates_nothing
cargo test -p galion-console --features phone-turn --lib workflow::
cargo test -p galion-console --features phone-turn --lib phone_host::tests::drive_posts_mic_level_and_utterance_on_the_bus
cargo test -p galion-console --features phone-turn --lib phone_host::tests::words_choose_a_program_without_a_second_screen

echo "symphony D+E PASS"
