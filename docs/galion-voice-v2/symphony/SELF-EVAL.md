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
# Symphony self-eval — Ultra

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb, 2026-11-03.

Subject: `docs/galion-voice-v2/symphony/PLAN-ULTRA.md`.
Other SKUs are not scored. This file does not score them by omission.

A gate passes only with evidence from the plan text or from a command run
for this eval. No CX43-class machine was booted. No voice session was timed.
Indicative numbers in the CPU pack stay indicative.

## ULTRA-PRICE — PASS

The Ultra monthly price is $220, in section 1 and in the proposed note.
No setup fee, discount, or replacement monthly price is stated.

The string `$20` occurs only as the Plus figure inside the t284 quotation,
labelled as not specified on this page. It is not an Ultra price and it is
not a Plus plan.

Command: `rg -n '\$[0-9]' docs/galion-voice-v2/symphony/PLAN-ULTRA.md`
Expected hits: `$220` as the Ultra price, `$20` only inside the t284
quotation. Result recorded in the log below.

## ULTRA-DISK — PASS as a record, OPEN as a quota

PASS: section 5 quotes t284 as `Ultra $220 / 2GB SSD`, sets that string
beside the 160 GB class disk and beside the 100 GB Plus disk from the same
sentence, rejects a literal 2 GB workspace, and does not adopt 160 GB,
100 GB, or any other size. The proposed note keeps $220 and leaves the
quota unnamed.

OPEN: the workspace quota is not decided. The Ultra section is not complete
on disk until the owner confirms reading 1 or reading 2 (scratch/cache, or
bad copy). Reading 3 stays rejected.

## ULTRA-CPU — PASS

Section 3 separates the fleet node (AMD EPYC 9655P, 96 cores / 192 threads,
pack sizing 384 GB and 768 GB) from the tenant (CX43 class: 8 shared vCPU,
16 GB RAM, 160 GB NVMe, no GPU, public label Intel or AMD). The tenant is
not described as a 96-core part. GPU is absent from both. Foreign weight
containers are not imported. Letter A on the phone path is unchanged.

The CX43 row cites the Hetzner page, the Gen3 table, compute.info, and the
26 October 2026 unavailability note. Orderability is not claimed.

## ULTRA-COMPUTER — PASS

Section 2 is one computer: Galion OS, agents, voice. Research stays a
separate console. The fleet's ~806 streams are not the tenant's load.
Section 4 keeps the pack's ~1.0 vCPU / ~4 vCPU burst / 0.65–1.15 GB RAM as
unmeasured sizing, and keeps heavy speech off the 80 ms tick.

## ULTRA-FILE — PASS

`LOCKS.toml` is not in the diff for this plan. `paid_tiers` is not flipped.
The plan says a licence change is a later owner edit.

Command: `git diff --name-only origin/main...HEAD` includes the symphony
pages and the pointer lines, and does not include `LOCKS.toml`. Result in
the log below.

## ULTRA-HONESTY — PASS

Section 4 and section 6 say nothing was measured for this plan. Token rates
and a COGS figure from the pack are left out so they cannot be read as an
Ultra result. Titanaxe `51.38.137.193` is not called this SKU. No SSH.

## ULTRA-SIGN — PASS

`PLAN-ULTRA.md` and this file both end with Maciej Grajczyk. Date
2026-11-03, Europe/Zagreb.

## ULTRA-HOST — NOT RUN

No Titanaxe Max 3XL was provisioned. No steal time, no voice RTF, no disk
`df`, no 80 ms tick histogram. The 1% / 800 µs bar is a requirement, not a
result. This gate stays NOT RUN until a human has a machine and a LEDGER
line records a run.

## Verdict

| Gate | Result |
|---|---|
| ULTRA-PRICE | PASS |
| ULTRA-DISK record | PASS |
| ULTRA-DISK quota | OPEN |
| ULTRA-CPU | PASS |
| ULTRA-COMPUTER | PASS |
| ULTRA-FILE | PASS |
| ULTRA-HONESTY | PASS |
| ULTRA-SIGN | PASS |
| ULTRA-HOST | NOT RUN |

The plan text is complete. Ultra is not complete as a host, and the
workspace quota is not complete as a number. $220 stays.

## Command log

Run on 2026-11-03 against the plan text before commit. Working tree, this branch.

`rg -n '\$[0-9]' docs/galion-voice-v2/symphony/PLAN-ULTRA.md`

Dollar amounts present: `$220` (Ultra price, the t284 Ultra string, the proposed note) and `$20` (t284 Plus quotation only, sections 1 and 5, and the source line). No other dollar amount.

`rg -ni 'gguf' docs/galion-voice-v2/symphony` — no hits.

`git diff --name-only -- LOCKS.toml` — empty. The lock file is not in the change.

`tail` of `PLAN-ULTRA.md` is the sign-off `— Maciej Grajczyk`.

Re-run this log if the plan changes. A stale PASS is a fail.

— Maciej Grajczyk
