# Self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb · 2026-11-03.

Ledger for [`EUREKA.md`](EUREKA.md). A symphony section is complete when its
gate passes and a row in the table below records the command, the exit, and
the measured number. This file does not invent a pass. `LOCKS.toml` is not
edited here.

Scores are logged numbers. The heartbeat crate is the inner clock.
[`docs/eval/PRESENCE-EVAL.md`](../../eval/PRESENCE-EVAL.md) is the phone
loopback. Neither one is a model grading itself.

## Standing harness

| Loop | What it measures | Command |
|---|---|---|
| Frame | `Heartbeat::step`, late = pause, Rika off by default, lean-in 160 ms on the v0 stub, zero allocations | `cargo test --offline -p galion-heartbeat` |
| Presence replay | P1–P6 through the real heartbeat (`lean_in_ms` on P6) | `cargo run --offline -p galion-heartbeat --bin galion-heartbeat-presence -- specs/presence.json` |
| R1–R9 | G6, RMS/ZCR stub, no weights | `galion-gates/g6-rika.sh` |
| P1–P8 mock | PS gate, no weights | `galion-gates/ps-presence.sh` |
| Phone T0–T2 | Loopback virtual user. TTFA only when a reply WAV exists. PSTN is not run. | `scripts/eval/virtual-user.sh` |

Crate and tests: [`crates/galion-heartbeat`](../../../crates/galion-heartbeat),
[`tests/heartbeat.rs`](../../../crates/galion-heartbeat/tests/heartbeat.rs),
[`tests/zero_alloc.rs`](../../../crates/galion-heartbeat/tests/zero_alloc.rs),
[`tests/presence_p6.rs`](../../../crates/galion-heartbeat/tests/presence_p6.rs).

Forbidden metric names, same as `specs/presence.json`: engagement, retention,
session length, time in session, dwell, speech elicited.

## Symphony rows

The Eureka landing records the standard and the harness links. It does not
mark a section done. Add a row only after the command has been run.

| Section | Gate (see EUREKA.md) | Result |
|---|---|---|
| A Compiler | Compiles, round-trip fixture, Chatterbox raw is not the primary runtime path | open |
| B Clone landing | Worker PCM and a sha256 of a LEDGER-cleared ref | open — rights gates closed |
| C Jarvis texture | Texture band retained | open — no clip |
| D Persona / emotion | Persona docs, emotion mapped to prosody and wash, no scripted empathy line | open |
| E One-dot | One `/` surface, mic without a second panel, workflow hook in a test | open |
| F Docs / runbooks | Commands in EUREKA.md run as written | open |
| G Latency | `frame_ms = 80` asserted, streaming budget measured or stub-measured | open |

## How to add a row

One line, after a real run: date (Europe/Zagreb), section, command, exit
code, the number that was measured, commit. Leave the row open when the
command was not run. A green exit on a different machine does not fill a
row that this file did not see.

— Maciej Grajczyk
