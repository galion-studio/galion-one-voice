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
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

This file is the ledger. A section is complete when its gate is **PASS** and the row below has been filled in. The blank table in the index is the form. Write the result here.

Demo, spec, test, and teacher are the same artifact. The model does not grade itself without a human-calibrated anchor. A missing support is an honest gap, and that honesty is a success. Scores are numbers in this file. They are not a mood.

Three loops, from the Eureka note:

| Loop | When | What is scored |
| Inner | each turn | confidence and groundedness of that turn |
| Middle | each session | the session held together |
| Outer | each release | the gates in the table below |

## Harnesses already in the tree

```bash
cargo run --offline -q -p galion-heartbeat --bin galion-heartbeat-presence
cargo test --offline -p galion-heartbeat
scripts/eval/virtual-user.sh
scripts/eval/flash-ttfa.sh
BASE=http://127.0.0.1:7878 scripts/eval/v2-persona-selftest.sh
scripts/run-v1-screen.sh --check
```

Presence reports land in `galion-gates/results/presence-eval/` (gitignored). The flash harness prints `measured: false` unless `GALION_FLASH_EVAL=1`. A protocol-stub clock is evidence about the pipe. It is a product TTFA only when `engine` is a model. See [TTFA.md](TTFA.md) and [FLASH-EVAL.md](../FLASH-EVAL.md).

## Ledger

Replace `PASS / FAIL / BLOCKED` with one token. Evidence is a path, a command, or a hash. When is Europe/Zagreb.

| Section | Gate | Result | Evidence | When |
|---|---|---|---|---|
| Compiler | Round-trip fixture. The Rust path opens the Galion voice file. | PASS / FAIL / BLOCKED | | |
| Clone | Worker PCM. Ref sha256 matches. Smoke speak logged. | PASS / FAIL / BLOCKED | | |
| Persona | Emotion maps to prosody and the wash. Words stay on the matter. | PASS / FAIL / BLOCKED | | |
| One-dot | `GET /` is the only screen. Mic records. `GET /v2` redirects to `/`. | PASS / FAIL / BLOCKED | | |
| Runbooks | Commands in [RUNBOOKS.md](RUNBOOKS.md) match their outputs. | PASS / FAIL / BLOCKED | | |
| 80 ms | Frame is 80 ms. Streaming budget measured or marked stub-measured. | PASS / FAIL / BLOCKED | | |
| Self-eval | The rows in this file are the record of the gates. | PASS / FAIL / BLOCKED | | |
| Eureka | Presence replay and the heartbeat harness ran. Scores are in the log. | PASS / FAIL / BLOCKED | | |
| Plus / Ultra / Research | Local, Plus, Ultra, and Research are written with their figures. | PASS / FAIL / BLOCKED | | |
| Rust translation | A Galion voice file loads on the Rust path. PyTorch stays a child process. | PASS / FAIL / BLOCKED | | |
| Jarvis texture | Hub clicks, alarms, and ambience remain against a clean-speech baseline. | PASS / FAIL / BLOCKED | | |
| Samantha emotions | Her prosody column is the one the feeling band applies. | PASS / FAIL / BLOCKED | | |
| One-percent | Non-inference CPU ≤ 1% of the host. Tick p99 well under 800 µs. Late pauses. | PASS / FAIL / BLOCKED | | |
| Heartbeat | `Heartbeat::step` advances one frame and allocates nothing on that path. | PASS / FAIL / BLOCKED | | |
| Clip store | Both WAV refs exist. sha256 matches. Git holds no reference audio. | PASS / FAIL / BLOCKED | | |
| Auto-workflow | A spoken turn records itself and the workflow hook fires. | PASS / FAIL / BLOCKED | | |
| TTFA | Default harness prints `measured: false`. A product claim names a model. | PASS / FAIL / BLOCKED | | |
| Staging | Loopback bind holds. Public name has auth. CORS origin is exact. | PASS / FAIL / BLOCKED | | |

No row above has been scored by the landing of this index. Fill a row when its gate has been run.
# Symphony self-evaluation

Europe/Zagreb. A section is complete when its gate passes and the run is written here.
Incomplete stays blocked. This ledger does not edit `LOCKS.toml`.

## D — Samantha emotions and sounds

Spec: [`SAMANTHA-EMOTIONS.md`](SAMANTHA-EMOTIONS.md).

Bar: personality, behavior, and emotion in the voice, not timbre alone. Emotion mapped to sound, prosody, and wash. Empathy is the live 80 ms weight: OKLCH and prosody from `voice_color`. No scripted empathy line.

Run: 2026-11-03. Host rustc 1.83.0 (`90b35a623`, 2024-11-26).

| Gate | Result | Evidence |
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

Other symphony sections are not claimed by this run.
# Symphony self-eval

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

Europe/Zagreb.

A section is complete when its gate passes and the receipt below matches the script.
`LOCKS.toml` is read by the gate and left unchanged.

## G · Latency / TTFA

Gate: `docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh`.
Detail: [`TTFA.md`](TTFA.md).

The stub block is the mock presence replay (onset, barge-in, e-to-a) on an 80 ms frame. `measured.present` is false until a measured file is passed on a later run. `--write-eval` refuses while `GALION_TTFA_MEASURED` is set, so a host clock cannot overwrite this record.

<!-- TTFA-RECEIPT:BEGIN -->
```json
{
  "frame_bar": "pass",
  "frame_ms": 80,
  "kind": "stub",
  "locks_edited": false,
  "measured": {
    "present": false
  },
  "owner": "Maciej Grajczyk",
  "program": "Galion Voice",
  "schema": "galion-ttfa-gate/v1",
  "stub": {
    "backend": "mock",
    "barge_in": "pass",
    "barge_in_ms": 80,
    "barge_in_one_frame_should": "pass",
    "e_to_a": "pass",
    "e_to_a_frames": 1,
    "e_to_a_ms": 80,
    "frame_ms": 80,
    "kind": "stub",
    "measured": false,
    "onset": "pass",
    "onset_p50_ms": 240,
    "runs": [
      {
        "backend": "mock",
        "barge_in": "pass",
        "barge_in_ms": 80,
        "barge_in_one_frame_should": "pass",
        "digest_fnv1a64": "9f657f360c565dbf",
        "e_to_a": "pass",
        "e_to_a_frames": 1,
        "e_to_a_ms": 80,
        "frame_ms": 80,
        "kind": "stub",
        "measured": false,
        "onset": "pass",
        "onset_p50_ms": 240,
        "spec": "presence",
        "verdict": "green"
      },
      {
        "backend": "mock",
        "barge_in": "pass",
        "barge_in_ms": 80,
        "barge_in_one_frame_should": "pass",
        "digest_fnv1a64": "db7541ec9f6f42c7",
        "e_to_a": "absent",
        "e_to_a_frames": null,
        "e_to_a_ms": null,
        "frame_ms": 80,
        "kind": "stub",
        "measured": false,
        "onset": "pass",
        "onset_p50_ms": 240,
        "spec": "presence-wav",
        "verdict": "green"
      }
    ]
  },
  "verdict": "pass"
}
<!-- TTFA-RECEIPT:END -->

2026-11-03 · Europe/Zagreb

Europe/Zagreb. Gate ledger. A section is complete when its gate passes here.

This landing records **RC**, the Galion Research console plan. Other symphony sectors append their own sections.

## RC · Research console

Plan: [`PLAN-RESEARCH-CONSOLE.md`](PLAN-RESEARCH-CONSOLE.md).
Recorded: 2026-11-03 Europe/Zagreb.
Owner: Maciej Grajczyk.
`LOCKS.toml` was not edited.

The plan section is complete when RC-1 through RC-4 pass. RC-5 and RC-6 are the product gates. They stay open on purpose: this landing is the plan.

| Gate | Criterion | Result |
| RC-1 | Research is the async side band. The plan names the turn-boundary handoff, the 3 s voice cap, and the tests that already refuse a blocking offer. | **PASS** |
| RC-2 | Galion Voice stays one dot. The Research console is its own product surface: own process, own page, seven state words, no second AI panel on `v1-screen.html`. | **PASS** |
| RC-3 | CPU and RAM come from the t284u summary of HARDWARE.md. Higher tok/s is the optimistic end of the published LLM band (300 / 1 300), nominal planning point 250 / 1 000, floor 200 / 800 when the node is at the voice peak. Residual arithmetic is shown. No extra tok/s was invented. | **PASS** |
| RC-4 | This change does not edit `LOCKS.toml`. `paid_tiers` stays false. `[ui]` stays one dot. | **PASS** |
| RC-5 | A CPU benchmark of research tok/s on the async queue, written to the ledger against the published band. | **NOT RUN** |
| RC-6 | A console process a person can open, separate from the voice page. | **NOT RUN** |

### RC-1 witness

rustc 1.85.0. The workspace default on this image is 1.83.0, which refuses `galion-openmind` (`rust-version = "1.85"`). The run used the 1.85.0 toolchain.

cargo +1.85.0 test -p galion-openmind --test handoff
test offline_hermes_is_honest_unreachable ... ok
test affect_is_clamped_before_it_reaches_hermes ... ok
test money_is_not_offered ... ok
test offer_never_blocks ... ok
# 4 passed; 0 failed; finished in 0.20s

cargo +1.85.0 test -p galion-sideband --test client
test unknown_turn_is_none ... ok
test null_transport_gives_honest_unreachable ... ok
test submit_and_poll_never_block ... ok
# 3 passed; 0 failed; finished in 0.20s

`offer_never_blocks` is six offers on a 200 ms transport with queue depth 1, finished in under 80 ms. `submit_and_poll_never_block` is the same shape and ends in `QueueFull`. Neither test is a `Heartbeat::step` trace. The plan's rule is stricter: `offer` is not called from `step`.

### RC-3 arithmetic

From the t284u summary only: 806 voice streams, ~190 GB, ~5 app nodes, 192 threads, 768 GB on the App/Voice/Research node.

| | |
| Voice vCPU per node | 806 / 5 = 161.2 |
| Thread residual | 192 − 161.2 = 30.8 |
| Voice share of threads | 84% |
| 70% of 192 | 134.4 threads |
| Voice RAM per node | 190 / 5 = 38 GB |
| RAM residual | 768 − 38 = 730 GB |
| 8 000 source tokens at 1 000 tok/s | 8.0 s, past the 3 s voice cap |
| 8 000 source tokens at 800 tok/s | 10.0 s |
| 8 000 source tokens at 1 300 tok/s | 6.2 s |
| 300 decode tokens at 250 tok/s | 1.2 s, inside the cap once prefill is done |

At the voice peak the higher tok/s queue does not share those app-node threads. It runs where the 70% band is free. The desk control plane fits in the ~31 thread residual. The token worker does not.

### RC-4 witness

The diff for this landing is the two files under `docs/galion-voice-v2/symphony/`. `LOCKS.toml` is untouched.

### Still open

RC-5 needs a `.gq` run on CPU, host and model id written down, compared with 200/800, 250/1000, and 300/1300. RC-6 needs the separate console process. Neither is claimed here.

# Symphony self-eval · Eureka A–G

Europe/Zagreb. t287. The teacher is a run of `scripts/symphony/self-eval.sh`, not this prose.

Demo = Spec = Test. The harness is the spec. A section stays **open** until a real run of its gate exits 0 and that run is written between the markers below. **PASS** is the only closed state. **OPEN** means the gate ran and the section is not done. An open section is not shipped. The script exits 1 while any row is open, and exits 2 if `LOCKS.toml` changes under it. This file does not edit `LOCKS.toml`.

CPU bar carried by gate G: `frame_ms = 80` (EPYC-class hosts still use that frame). The one-percent tick and the vCPU session budget are not rows A–G.

## Run

scripts/symphony/self-eval.sh
scripts/symphony/self-eval.sh --only G
scripts/symphony/gates/a.sh

`self-eval.sh` with no `--only` rewrites the matrix and the run log. A single letter does not.

## What closes a row

| ID | Section | The run that closes it |
| A | Compiler | A clone-to-Galion clip compiler, outside this harness, round-trips a Galion clip (`.gq` / `GALIONGQ`) and refuses Chatterbox raw as the primary runtime. The weight reader in `crates/galion-tensor/src/gq.rs` is not that compiler while it says reader-only. |
| B | Clone landing | `personas/refs/samantha.wav` sha256 `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` and `jarvis.wav` sha256 `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60`. No mp3 refs. A smoke-speak log names both hashes. The protocol stub may emit PCM and must refuse a reference clip; that stub is not the smoke speak. |
| C | Jarvis texture toggle | `personas/voice/jarvis.toml` has `[texture] preserve = true` (hub clicks, alarms, and ambience stay). `jarvis.wav` keeps a high-band ratio at least the clean-speech baseline `jarvis-clean-baseline.wav`. The synthetic checker in the gate is not the Jarvis recording. |
| D | Persona and emotion | Samantha, Jarvis, and Galion One docs have an emotion section and forbid scripted empathy. Samantha names `[laugh]` and `[chuckle]`. Both voice tomls map `e0` / `e1` prosody and wash. `voice_color` blends prosody. The feeling band sits on the 80 ms heartbeat. Canned replies do not speak the banned empathy lines. |
| E | One-dot | LOCKS stays one dot, chrome off. The phone page is one canvas, the mic records there, `/v2` redirects to `/`. A test outside this harness fires a workflow hook after auto-record. |
| F | Runbooks | `README.md` tells how to run the demo, and every `scripts/*.sh` it names is executable and passes `bash -n`. |
| G | 80 ms frame | LOCKS `frame_ms = 80`, `frame_hz = 12.5`, compute budget ≤ 60 ms. Heartbeat constants match. A late frame pauses. The streaming note names 80 ms. `scripts/eval/flash-ttfa.sh` stub-measures: no `ttfa_ms` unless a real Flash eval is asked for. |

## Matrix

<!-- SYMPHONY-MATRIX-START -->
Recorded 2026-11-03 10:51 CET. A row closes only when this run says PASS.

| ID | Section | Status | Evidence |
| A | Compiler | OPEN | .gq container is reader-only in galion-tensor; no clone-to-Galion clip compiler round-trip that refuses Chatterbox as the primary path |
| B | Clone landing | OPEN | protocol stub emitted 402 B PCM (not a ref speak); worker refused a reference clip; samantha.wav absent; jarvis.wav absent; no smoke-speak log names both ref sha256 values |
| C | Jarvis texture toggle | OPEN | checker self-test passed on a synthetic tone; jarvis.toml has no [texture] preserve toggle |
| D | Persona and emotion | PASS | persona docs, e0/e1 prosody and wash, voice_color, feeling band at 80 ms; canned replies have no scripted empathy lines |
| E | One-dot | OPEN | no test shows speak, auto-record, then a workflow hook |
| F | Runbooks | PASS | bash -n ok for 8 README scripts: scripts/asr/fetch-vosk.sh, scripts/a21/fetch-a21.sh, scripts/run-v1-screen.sh, scripts/run-v1-demo.sh, scripts/eval/virtual-user.sh, scripts/check-phone-pipeline.sh, scripts/check-locks.sh, scripts/check-no-forbidden.sh |
| G | 80 ms frame | PASS | frame_ms=80, 24 kHz, p95 budget <= 60 ms, late frame pauses; streaming doc names 80 ms; flash-ttfa stub-measures and prints no ttfa_ms |

Closed: D, F, G. Open: A, B, C, E.
<!-- SYMPHONY-MATRIX-END -->

## Last run

<!-- SYMPHONY-RUN-START -->
stamp: 2026-11-03 10:51 CET
locks_sha256: 6b69ffe258bffab8608566d808aefad4dba310d691aa97d1216c7844c0eb6397
command: scripts/symphony/self-eval.sh
A OPEN — .gq container is reader-only in galion-tensor; no clone-to-Galion clip compiler round-trip that refuses Chatterbox as the primary path
B OPEN — protocol stub emitted 402 B PCM (not a ref speak); worker refused a reference clip; samantha.wav absent; jarvis.wav absent; no smoke-speak log names both ref sha256 values
C OPEN — checker self-test passed on a synthetic tone; jarvis.toml has no [texture] preserve toggle
D PASS — persona docs, e0/e1 prosody and wash, voice_color, feeling band at 80 ms; canned replies have no scripted empathy lines
E OPEN — no test shows speak, auto-record, then a workflow hook
F PASS — bash -n ok for 8 README scripts: scripts/asr/fetch-vosk.sh, scripts/a21/fetch-a21.sh, scripts/run-v1-screen.sh, scripts/run-v1-demo.sh, scripts/eval/virtual-user.sh, scripts/check-phone-pipeline.sh, scripts/check-locks.sh, scripts/check-no-forbidden.sh
G PASS — frame_ms=80, 24 kHz, p95 budget <= 60 ms, late frame pauses; streaming doc names 80 ms; flash-ttfa stub-measures and prints no ttfa_ms
exit: 1
<!-- SYMPHONY-RUN-END -->

