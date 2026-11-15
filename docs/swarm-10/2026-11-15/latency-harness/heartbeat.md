# Swarm 10 — latency-harness heartbeat (B2)

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-15 22:06 +01:00.

The spelling is **Galion**. The product is Galion Voice.

Lane: `latency-harness`, worker heartbeat, backlog B2. Plan: [PR #151](https://github.com/galion-studio/galion-one-voice/pull/151) (`cursor/latency-harness-plan-68d6`). This file does not edit that plan.

| | |
|---|---|
| Measured commit | `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` (`origin/main` at measure time) |
| Host | `cursor`, Linux 6.12.113+, x86_64, `Intel(R) Xeon(R) Processor`, 4 CPUs, MemTotal 16387328 kB |
| Toolchain used | `cargo 1.85.0 (d73d2caf9 2024-12-31)`, `rustc 1.85.0 (4d91de4e4 2025-02-17)` via `cargo +1.85.0` |
| Profile | `cargo test` debug (`unoptimized + debuginfo`). Not a release build. Not EPYC, CX23, or CX43. |
| Clock | `FRAME_MS` is 80. `LOCKS.toml` `[clock] frame_ms` is 80. Neither file was edited. |

The image default before this run was rustc 1.83.0, which cannot build `crates/galion-heartbeat` (`rust-version = "1.85"`). `rustup toolchain install 1.85.0 --profile minimal` was run on this host so the harness could compile. That install is not a deploy.

`weights/` on this host contains only `.gitignore`. No `en_US-ljspeech-medium.onnx` was found under `/opt`, `/workspace`, or `/home/ubuntu`. Weights were not downloaded. The script was not patched.

## Verdict

`scripts/eval/symphony-heartbeat.sh` exit **1**. This is the harness verdict:

```text
verdict=FAIL reason=stub-frames-are-not-a-voice
piper_measured=false reason=pack-absent
```

The script prints `verdict=PASS` only when the heartbeat tests already printed `verdict=PASS`, the frame tests passed, and `piper_drain` printed `piper_measured=true`. The A21 pack is absent, so `a21_piper_pcm_feeds_one_heartbeat_frame` returned without measuring Piper. A missing pack is not a voice. This FAIL is not a Nano clock, not a product TTFA, and not the Piper drain cited from another host.

## Commands

### 1. Offline crate test, before the index existed

2026-11-15 22:04:11 +01:00.

```bash
cargo +1.85.0 test --offline -p galion-heartbeat -- --test-threads=1
```

Exit 101. Exact error:

```text
error: no matching package named `ratatui` found
location searched: crates.io index
required by package `galion-console v0.0.1 (/agent/repos/galion-one-voice/crates/galion-console)`
```

`--offline` cannot resolve the workspace while the crates.io index is empty. No microsecond figure came from this attempt.

`cargo +1.85.0 fetch --locked` then completed (crates.io only, no weights). It ran after that failure and before the re-run below.

### 2. Offline crate test, after fetch

2026-11-15 22:04:26 +01:00 to 22:04:28 +01:00. Exit 0.

```bash
cargo +1.85.0 test --offline -p galion-heartbeat -- --test-threads=1 --nocapture
```

`--nocapture` is extra so the `verdict=` line is in the log. `--test-threads=1` matches the harness script. The package filter is the whole `galion-heartbeat` test set.

| Target | Result |
|---|---|
| lib unit tests | 9 passed, 0 failed |
| `galion-heartbeat-presence` bin | 0 tests |
| `tests/frame_budget.rs` | 1 passed, 0 failed |
| `tests/heartbeat.rs` | 16 passed, 0 failed |
| `tests/presence_p6.rs` | 2 passed, 0 failed |
| `tests/symphony_wire.rs` | 11 passed, 0 failed |
| `tests/zero_alloc.rs` | 2 passed, 0 failed |
| doc-tests | 0 tests |

Late-frame tests in that run passed, including `late_frame_is_a_pause_no_extra_trigger_no_burst`, `short_pcm_frame_is_treated_as_late_pause`, `late_frame_pauses_and_the_next_frame_does_not_catch_up`, and `nano_ring_drains_one_frame_and_a_late_tick_does_not_catch_up`. `feeling_step_allocates_nothing` and `step_allocates_nothing_with_rika_on` passed. `steps_allocate_nothing` passed.

Gate line from `harness_budget_and_verdict` on this invocation (debug, median of 5 passes, ceiling 800 µs). This line is the unit gate. It is not the script verdict above.

```text
verdict=PASS frame_ms=80 bands=3 sectors=3 allocs=0 late=pause p99_us=5.590 max_us=26.095 budget_us=800 n=4032 trials=5 gate=median worst_p99_us=5.766 worst_max_us=32.267 trial_max_us=26.095,32.267,31.688,12.194,15.911
```

`tests/frame_budget.rs` printed a stub-path line. The test source labels it stub. It is not a model, codec, clone, or host-CPU product figure:

```text
GALION_FRAME_BUDGET kind=measured-stub profile=debug frame_ms=80 budget_us=800 n=256 p50_us=4 p95_us=4 p99_us=5 max_us=14 step_duty_ppm=62 late_pause=1
```

### 3. Heartbeat harness

2026-11-15 22:04:37 +01:00 to 22:04:57 +01:00. Exit 1.

```bash
bash scripts/eval/symphony-heartbeat.sh
```

The script selected `cargo +1.85.0`. Heartbeat tests inside it passed and printed a second unit gate line (a later run, not a replacement of the line above):

```text
verdict=PASS frame_ms=80 bands=3 sectors=3 allocs=0 late=pause p99_us=6.505 max_us=36.077 budget_us=800 n=4032 trials=5 gate=median worst_p99_us=7.534 worst_max_us=39.713 trial_max_us=38.929,18.873,39.713,36.077,27.039
```

Frame checks in the same script, all exit 0:

| Test | Result |
|---|---|
| `speech::clone_worker::tests::frame_matches_heartbeat` | ok (1 passed, 109 filtered) |
| `dotviz::tests::sectors_match_heartbeat_frame` | ok (1 passed, 56 filtered) |
| `v2_persona::tests::empathy_switch_needs_eight_unpinned_frames` | ok (1 passed, 79 filtered) |
| `phone_host::tests::drain_takes_one_heartbeat_frame_and_leaves_the_tail` | ok (1 passed, 87 filtered) |
| `galion-live-demo` `tests/live.rs` | 9 passed, 0 failed |
| `a21_piper_pcm_feeds_one_heartbeat_frame` | test ok, `piper_measured=false reason=pack-absent` |

Then the script printed the FAIL line in the verdict section and exited 1. `galion-ml-voice` emitted one dead-code warning (`clip_store.rs` field `counsel`). It did not change the verdict.

## Not measured

- Piper PCM into the ring (`piper_measured=true`). Pack absent. Not measured.
- `LIVE_REALTIME=1 scripts/run-live-demo.sh`. Not run. [80MS.md](../../../galion-voice-v2/symphony/80MS.md) CPU and mouth-to-ear figures stay unmeasured on this page.
- `cargo run -p galion-heartbeat --bin galion-heartbeat-presence`. Not run.
- Nano / `chatterbox-nano` first-PCM and the 80 ms first-playable frame. Not this item. Not measured.
- `galion-gates/op-one-percent.sh`. Not this item. Not run.
- A product TTFA. Not measured.

Prior ledger lines (p99 118.009 µs / 107.175 µs, and the 2026-11-03 `SELF-EVAL.md` heartbeat section) are other hosts and other times. They are not this run. `SELF-EVAL.md` was not edited. A GitHub text search for open pull requests containing `SELF-EVAL.md` returned 0 at 22:05 +01:00. Diffs of other open PRs were not scanned. Lane D still owns the ledger.

## What changed

This notes file only. `LOCKS.toml`, `SOUL.md`, `config.yaml`, `FRAME_MS`, `STEP_BUDGET_US`, and `scripts/eval/symphony-heartbeat.sh` were not edited. No WAV, weight, token, or address was committed.

## Decisions for Maciej

1. The 80 ms clock stays locked. This worker did not change it.
2. On this host the harness ship line is `verdict=FAIL reason=stub-frames-are-not-a-voice` until an A21 pack is present and `piper_measured=true` is printed by a real run. Turning that FAIL into PASS without the pack would be a lie. It was not done.
3. The unit gate on this host did clear 80 ms, `allocs=0`, `late=pause`, under the 800 µs ceiling, in debug. That does not make the script PASS, and it is not a Nano first-playable-frame claim.
4. A0 (t273 vs t623u) is still the owner decision. This lane does not flip `LOCKS.toml` or `REFS.sha256`. B3 stays outside this PR.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
