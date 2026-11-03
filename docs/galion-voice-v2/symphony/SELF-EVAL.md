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



A section is complete when its gate passes and the result is written here.
A skip of a missing gitignored wav is a pass of that gate. A missing model
weight is blocked, not a fake pass. Measurable lines only.

Harness: `scripts/clone/clone-landing-smoke.sh`. Re-run it. Do not copy a
TTFA number from a protocol stub.

## B · Clone landing

| Current Samantha and Jarvis pins, WAV only, no model swap (`nano`) | PASS | `scripts/clone/REFS.sha256`; persona TOML `model = "nano"`; unit `reference_pins_keep_the_current_wavs_and_jarvis_texture` |
| `present` / `clip_path` / `send_reference` wired; `GALION_CLIP_STORE` optional | PASS | `docs/galion-voice-v2/personas/voice/{samantha,jarvis}.toml` |
| sha256 checked when the wav is present; clean skip when absent | PASS | smoke line `status=skip` on this machine (files not in the work tree); unit `absent_reference_is_skipped_and_a_bad_file_is_refused` |
| Unconsented path, MP3, hash mismatch, denoise: no PCM | PASS | smoke exits 3 on those requests; stdout empty |
| Jarvis hub texture is a separate toggleable layer, not stripped | PASS | `texture_layer=false` logs `present=1 enabled=0 stripped=0`; `strip_texture` exits 3; clip round-trip keeps the layer |
| CPU-only Chatterbox Nano, out of process, **1 vCPU** this child | PASS | smoke `threads=1 device=cpu model=nano`; `GALION_CLONE_THREADS=8` logs `asked=8 applied=1`; GPU device exits 3 |
| No GGUF, no GPU, PyTorch not linked into the crate | PASS | smoke `gguf=false`; worker source has no `import torch`, no `.gguf`; `in_tree_ggml` stays false because `LOCKS.toml` was not edited |
| `frame_ms = 80` | PASS | read from `LOCKS.toml`; not edited |
| Voice-session planning budget recorded | PASS | ~1.0 vCPU average, burst ~4 across the cascade (not this child), RAM 0.65–1.15 GB planning only, not measured |
| Owner t273 local send; public demo still closed | PASS | `LEDGER.md`; page `rights` string unchanged |
| Galion-format emit (`GALIONVC`), not safetensors-as-runtime, not GGUF, not `.gq` weights | PASS | `speech/galion_clip.rs`; magic `GALIONVC`; `.gq` writer still deferred |
| Lock flip `reference_clips=true` | BLOCKED-on-owner | `LOCKS.toml` still `reference_clips = false`. This PR does not edit it. Owner flips the key. |
| Model PCM / Chatterbox generate | BLOCKED | weights not fetched (~3 GB pinned safetensors). Worker does not import torch and does not call generate. No TTFA product claim. |

Section B gate: **PASS** for wiring, the Galion clip, and the texture toggle. Two items stay blocked: the lock flip (**BLOCKED-on-owner**) and Chatterbox generate (**BLOCKED**, weights not fetched). Generate stays out of process and CPU-only when an owner fetches the pack. It must not strip the hub layer.

Smoke on this machine, refs absent: `PASS frame_ms=80 session_vcpu=1 device=cpu gguf=false denoise=false texture_toggle=separate galion_clip=GALIONVC lock_flip=BLOCKED-on-owner`. Both sha256 rows printed `status=skip`. `generate=blocked reason=weights-not-fetched`. Speech unit tests 30/30. Persona config test passed.

This file does not grade sections A, C, D, E, F, or G. C's texture rule is enforced here for the reference bytes (they are not cleaned). It is not a spectral measurement against a clean-speech baseline.


Europe/Zagreb. A section is complete only when its gate passes and the result is written here.

Master slogan: "The seamless connection with the human being, on the voice only."

Demo = Spec = Test. The gate is `scripts/symphony/self-eval-a.sh`.

## Section A — compiler

Verdict: **PASS**

Host for the numbers below: the build CPU for this change (Rust 1.85.0, `cargo test` debug profile). Not an AMD EPYC 9655P and not a CX23/CX43. The budget those hosts must meet is what the test asserts.

Command:

cargo test -p galion-ml-voice --lib artifact::
cargo test -p galion-ml-voice --test voice_artifact -- --test-threads=1 --nocapture

Tick line from the integration test (debug, unoptimized):

section-a tick p99_ns=7316 max_ns=7970 budget_ns=800000

p99 7.3 µs, max 8.0 µs, budget 800 µs (1% of 80 ms). Late frames are inside that loop. Debug profile, so this is the slow build.

| # | Gate | Result |
| A1 | Crate tests compile and the 16 `artifact::` tests pass | **PASS** |
| A2 | Round trip: synthetic s16le → `.gq` → `GqFile::verify_all` → same samples. Output is not raw PCM and not RIFF | **PASS** |
| A3 | `.safetensors` / `.pt` / `.wav` / `.pcm` / `.gci` / `.ggml` are not `open_runtime`. CLI exit 3, no output file | **PASS** |
| A4 | Safetensors bytes or a GGML-family magic renamed to `.gq` are refused | **PASS** |
| A5 | `chatterbox_runtime` flag and a GPU flag are refused. `CHATTERBOX_IS_RUNTIME` is false | **PASS** |
| A6 | Rate 24000, frame 80 ms, 1920 samples, equal to `galion-heartbeat` | **PASS** |
| A7 | `fill_frame` allocates 0 times (counting allocator), including late and past-end | **PASS** |
| A8 | Late frame is silence and the same index still plays. No catch-up | **PASS** |
| A9 | p99 and max of `fill_frame` under 800 µs | **PASS** (7.3 µs / 8.0 µs) |
| A10 | Sidecar sha256 required. Stem must match the persona. Atomic write leaves no `.tmp` | **PASS** |
| A11 | Stereo is mixed, not stripped. A 48 kHz click survives resample | **PASS** |
| A12 | `LOCKS.toml` not edited. PyTorch not linked. No GGML-family container written | **PASS** |
| A13 | Tick measured on EPYC 9655P-class or CX23/CX43 | **BLOCKED** — those hosts are not in this run. The budget gate ran here instead |
| A14 | Live Chatterbox `generate` for Samantha/Jarvis | **BLOCKED** — section B. Weights are not in git. This gate uses synthetic PCM in the worker's s16le 24 kHz shape |

`LOCKS.toml` diff: none.

## Sections B–G

Not this change. No verdict.

| Section | State |
| B Clone landing | Not run |
| C Jarvis texture | Not run. The compiler does not denoise; that is not the texture gate |
| D Persona / emotion | Not run |
| E One-dot | Not run |
| F Runbooks beyond the compiler | Compiler run steps are in `COMPILER.md` |
| G Latency on a live stream | Frame constant and the clip tick are gated here. A streaming session is not measured |


Europe/Zagreb. A section is complete only when its gate PASSes and the
result is written here.

| Section | Gate | Status | Provenance |
| ONE-PERCENT | `galion-gates/op-one-percent.sh` | PASS | stub script + measured local release |

## ONE-PERCENT

Recorded run: **2026-11-03 10:53 Europe/Zagreb**. Gate line:

OP PASS — P1≤1% (ok) P3 p99=3206ns ratio=249 much_less=yes cpu=schedstat stub=8 measured=256 product_claim=false

Host: this build machine (Intel Xeon, 4 threads, x86_64), release.
`product_claim` is false. This is not an EPYC node, not a CX23 or CX43,
not a phone, and not a VPS.

**Measured** (`schedstat` for P1, `instant` for P3), 256 silence frames,
Rika v0 on, sense and adapt granted:

| Field | Value |
| kind | measured |
| P1 share | 0.00004113 (cpu 842273 ns, note `ok`) |
| P1 pass | true |
| P3 p99 | 3206 ns |
| P3 max | 24522 ns |
| P3 ratio | 249 (800 µs / p99) |
| p3_much_less | true (p99 under 80 µs) |
| pauses | 0 |

**Stub** (`injected`), the fixed script of seven 50 µs frames and one late
40 µs frame:

| Field | Value |
| kind | stub |
| n | 8 |
| pauses | 1 |
| p99 | 50000 ns |
| share | 0.00060937 |
| P1 pass | true |

`late_is_pause` is true. `catch_up_steps` is 0.

Three other release runs the same morning, same binary shape, had p99 of
3177 ns, 3342 ns, and 5104 ns. The number moves. The gate run in the table
is the one this section records. Debug builds are slower and are not this
table. The crate tests in debug only hold the 800 µs ceiling.


# Symphony heartbeat self-eval

Europe/Zagreb · 2026-11-03 · t287.


Harness: `docs/galion-voice-v2/symphony/HEARTBEAT-WIRE.md`.
Command: `scripts/eval/symphony-heartbeat.sh`.
Toolchain: Rust 1.85.0. Profile: `cargo test` (debug). This file records that run.


**PASS**

| Check | Verdict | Evidence |
| `FRAME_MS` is 80, three bands, three sectors | PASS | `frame_is_80ms_and_three_bands_and_three_sectors` |
| `Heartbeat::step` and `SymphonyWire::step` allocate nothing | PASS | counting allocator, `allocs=0`, including late frames (`steps_allocate_nothing`, `zero_alloc`) |
| Neither step blocks | PASS | source scan has no lock, channel, sleep, clock, or heap call; warm p99 under 800 µs |
| Late frame is a pause, and the next frame does not catch up | PASS | pause copies 0 and leaves the me buffer; the following step copies 1920, and the queued tail stays |
| Persona hold is 8 frames; feeling lands on the next frame; one frame never flips | PASS | `persona_needs_eight_frames_and_feeling_lands_next_frame`; floor test `empathy_switch_needs_eight_unpinned_frames` |
| One-dot mark: speak wins, pause holds the mark | PASS | `one_dot_speak_wins_and_a_pause_holds_the_mark`; `sectors_match_heartbeat_frame` |
| Yield copies nothing and asks the host to release the slot | PASS | `yield_releases_the_slot_and_copies_nothing` |
| Clone drain size is one heartbeat frame | PASS | `frame_matches_heartbeat`; `drain_takes_one_heartbeat_frame_and_leaves_the_tail` |
| Live frame loop still runs with the wire in the loop | PASS | `galion-live-demo` `tests/live.rs` (feature `dot-viz`) |
| Chatterbox `generate` (model PCM) | BLOCKED | child stays off the step; generate is still unwired; weights are not in the tree. The drain size is wired. This row does not fail the harness. |


## Measured step

Gate line from `harness_budget_and_verdict` (warmup 64 frames dropped, then 4032 frames, Rika on, one late frame in 64):

verdict=PASS frame_ms=80 bands=3 sectors=3 allocs=0 late=pause p99_us=118.009 max_us=148.642 budget_us=800 n=4032

The ceiling is 800 µs. Three debug passes in this session stayed under it: p99 106.639 / 118.009 / 130.342 µs, max 138.387 / 148.642 / 158.090 µs. A later run may print another pair. The harness fails when either number reaches 800.

The clock read is outside `step`. The counting allocator is armed only around the two `step` calls.

## Fail and blocked

A failing assertion, or a missing `verdict=PASS` line, is **FAIL**. The script exits 1.

`cargo` missing on the machine is **BLOCKED**. The script exits 2.

Model PCM from Chatterbox is **BLOCKED** for this gate, as the table says. The 80 ms harness itself passed.


Europe/Zagreb. Produced by `cargo run -p galion-symphony -- eval` on Rust 1.85.1.
`cargo test -p galion-symphony` passed (5 unit tests, 1 pack test).

Section: Rust translation of the clone interchange into the Galion voice pack and clip ABI.
A missing owner file is BLOCKED. A broken gate is FAIL. Infra is PASS only when Fails is 0 and the `.gq` and clip gates passed.

| Gate | Status | Detail |
| gq-roundtrip | PASS | GqFile::verify_all and payload bytes match the fixture |
| namespace | PASS | every .gq name is in sym.; the interchange name stays in the manifest |
| output-clean | PASS | pack directory holds .gq, SYMPHONY.txt, SHA256SUMS, and clip.gclip |
| clip-abi | PASS | GALIONCL payload is the WAV; stereo samples stay distinct; hub chunk kept |
| texture-keep | PASS | jarvis policy keep; clicks, alarms, ambience declared; strip flag 0 |
| clip-not-lane | PASS | clip stays 48 kHz stereo; the heartbeat lane stays 24 kHz |
| presence | PASS | schema galion-presence-report/v1, bands you/me/feeling, no engagement field |
| empathy | PASS | live OKLCH weight; scripted empathy false |
| clock | PASS | 80 ms, 24 kHz, 1920 samples, same constants as galion-heartbeat and galion-ml-voice |
| foreign-refused | PASS | foreign weight magic and refused extensions do not enter the pack |
| cpu-budgets | PASS | EPYC 9655P, CX23, CX43; session about 1 vCPU, burst 4, 0.65–1.15 GB, non-inference ceiling 1%, tick ceiling 800 µs |
| pins-table | PASS | five interchange pins match scripts/clone/PINS.sha256 |
| heartbeat-pause | PASS | on-time 1920-sample frame runs; a late frame and a short buffer pause |
| landing-samantha-wav | BLOCKED | docs/galion-voice-v2/personas/refs/samantha.wav is not on this machine |
| landing-jarvis-wav | BLOCKED | docs/galion-voice-v2/personas/refs/jarvis.wav is not on this machine |
| landing-clone-weights | BLOCKED | weights/clone/shared/ve.safetensors is not on this machine |
| clone-generate | BLOCKED | this crate does not spawn the clone child and does not call generate; weights are not in the tree |

Infra: PASS

Fails: 0

Fixture translate: 4347 µs. Fixture load: 626 µs. The 80 ms bar is the heartbeat frame (`late` pauses). It is not the wall clock of packing a weight file.


Signed: Maciej Grajczyk

Generated by `scripts/symphony/self-eval.sh` at 2026-11-03 09:55 UTC.

Champion: a conversation is one heartbeat, not a pipeline.
Demo = Spec = Test = Teacher. Harness before model.

**Result: PASS**

`SELF-EVAL PASS`

A section is complete only when its check exits 0. This file is that record.
A PASS with a gap is a passed check that refused to invent the missing measurement.

| Section | Check | Evidence |
| A Compiler | PASS | cited galion-gates/g5-gq-synthetic.sh (SYNTHETIC-PASS, real_g5 PENDING) and crates/galion-tensor/tests/gq_synthetic_g5.rs; cargo G5 not run; Chatterbox stays out of process and is not the heartbeat runtime |
| B Clone landing | PASS | cited scripts/clone/chatterbox_worker.py and crates/galion-ml-voice/src/speech/clone_worker.rs (clock is not a product claim); protocol-stub pcm_bytes=402; ref_sha256=not-measured; landing=not-a-voice |
| C Jarvis texture | PASS | worker has no denoise step; clip-store runbook says do not strip; spectral=not-measured |
| D Persona / emotion | PASS | cited scripts/eval/v2-persona-selftest.sh (HTTP half not run), EMPATHY.md, and galion-emotion voice_color; prosody and OKLCH mapped; scripted empathy lines absent from code |
| E One-dot | PASS | cited scripts/run-v1-screen.sh and crates/galion-console/assets/v1-screen.html; ONE_DOT_WORKFLOW fired on the VAD to /turn contract; live browser not run |
| F Docs / runbooks | PASS | runbooks list local demo, clone worker, one-dot, clip store, and self-eval; one-ui and check-locks ran; bash -n clean |
| G Latency / CPU tick | PASS | measured-stub profile=debug p99_us=105 max_us=151 step_duty_ppm=1312 budget_us=800 frame_ms=80 late=pause; host CPU and EPYC/CX23/CX43 capacity not measured |

## Tick

The non-inference bar is 1% of the 80 ms tick, which is 800 µs.
The number below is `Heartbeat::step` on the stub affect path (Rika v0, no model, no codec kernels).
It is not host CPU%, not mouth-to-ear latency, and not an EPYC, CX23, or CX43 run.

`GALION_FRAME_BUDGET kind=measured-stub profile=debug frame_ms=80 budget_us=800 n=256 p50_us=92 p95_us=99 p99_us=105 max_us=151 step_duty_ppm=1312 late_pause=1`

Late frame = pause. `Heartbeat::step` is also covered by `tests/zero_alloc.rs` (no allocation) and `late_frame_is_a_pause_no_extra_trigger_no_burst`.

## CPU plan encoded here

Flagship app node: AMD EPYC 9655P (96 cores / 192 threads). Plus: CX23-class. Ultra: CX43-class.
CPU only, no GPU. Do not import GGUF. Those SKUs and the tok/s and vCPU figures in `CPU.md` are planning figures. They were not measured on this host.

## Cited gates

| Section | Gate |
| A Compiler | `galion-gates/g5-gq-synthetic.sh`, `crates/galion-tensor/tests/gq_synthetic_g5.rs` |
| B Clone | `scripts/clone/chatterbox_worker.py`, `crates/galion-ml-voice/src/speech/clone_worker.rs` |
| D Persona | `scripts/eval/v2-persona-selftest.sh`, `docs/galion-voice-v2/EMPATHY.md`, `galion-emotion/src/express.rs` |
| E One-dot | `scripts/run-v1-screen.sh`, `crates/galion-console/assets/v1-screen.html` |
| G Tick | `crates/galion-heartbeat` `FRAME_MS`, `galion-gates/sy-frame.sh` |

## Not measured

- Reference wav sha256, unless the bytes were on disk for section B.
- Jarvis spectral retention against a clean-speech baseline, unless both files were on disk for section C.
- Real-weights G5. The cited gate is the synthetic round-trip, and its own script keeps real G5 pending.
- A landed clone. Protocol-stub PCM is not a voice.
- `scripts/eval/v2-persona-selftest.sh` against a live page. The file and prosody map were checked. The HTTP half was not started.
- A browser session of the one-dot page.
- Host CPU% of ASR, TTS, or the clone child.
- EPYC 9655P tok/s, voice-session vCPU, and CX23 / CX43 capacity.
- VPS R0–R3.

