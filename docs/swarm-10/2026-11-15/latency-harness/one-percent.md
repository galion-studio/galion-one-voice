# Swarm 10 — latency-harness / one-percent (B4)

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Written 2026-11-15 22:06 +01:00.

The spelling is **Galion**. The product is Galion Voice.

| | |
|---|---|
| Lane | `latency-harness` (Lane B), worker `one-percent`, backlog **B4** |
| Plan | [PR #151](https://github.com/galion-studio/galion-one-voice/pull/151) `Swarm 10 latency-harness plan`, branch `cursor/latency-harness-plan-68d6`, commit `36b1e8bedfead6ea0882713655e2be22408f18b6`. This note does not edit `PLAN.md`. |
| Base | `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` |
| Branch | `cursor/latency-harness-one-percent-b942` |
| Gate | `galion-gates/op-one-percent.sh` |
| Design | [ONE-PERCENT.md](../../../galion-voice-v2/symphony/ONE-PERCENT.md) |

This PR is the note. No lock, no clock, no weight, no WAV, no gate-script edit.

## Host

This is the worker host. It is not the capacity class in `crates/galion-symphony/src/host_cpu.rs`.

| Field | Value |
|---|---|
| hostname | `cursor` |
| OS | Linux 6.12.113+ x86_64, KVM |
| CPU | `Intel(R) Xeon(R) Processor`, family 6 model 207, 4 CPUs, 1 thread per core |
| RAM | 15 GiB (`free -h` total) |
| rustc | `rustc 1.85.0 (4d91de4e4 2025-02-17)` |
| cargo | `cargo 1.85.0 (d73d2caf9 2024-12-31)` |

The image default was rustc 1.83.0. CI pins `dtolnay/rust-toolchain@1.85.0` (`.github/workflows/gates.yml`). `rustup toolchain install 1.85.0 --profile minimal` and `rustup override set 1.85.0` were run in this checkout. The override is local rustup state. No `rust-toolchain` file is committed.

`HostId` in the tree is only `epyc9655p` (96 cores / 192 threads / 768 GB), `cx23` (Plus), and `cx43` (Ultra). This VM is none of those. A13 (tick on EPYC 9655P-class or CX23/CX43) stays **BLOCKED**.

A host-wide CPU percent was **not measured**. Immediately before the gate, a `cut` of `/proc/loadavg` mis-parsed and printed `cut: '"': No such file or directory`, then the shell printed `LOAD=0.09 0.06 0.02 1/254 2785`. That is not a clean loadavg read. A later `cat /proc/loadavg` at 2026-11-15 22:05:42 +01:00 printed `0.16 0.10 0.03 1/255 4034`. Neither line is the P1 figure below.

## Command

Started 2026-11-15 22:04:40 +01:00 (UTC `2026-11-15T21:04:40+0000`), on commit `8980e1a`, in this checkout.

```bash
bash galion-gates/op-one-percent.sh
```

Last stdout line, exit 0:

```text
OP PASS — P1≤1% (ok) P3 p99=1438ns ratio=556 much_less=yes cpu=schedstat stub=8 measured=256 product_claim=false
```

`product_claim` is **false**. The run did not earn a product claim. `PRODUCT_CLAIM` in `crates/galion-symphony/src/lib.rs` is the constant `false`, and ONE-PERCENT.md says there is no path that sets it true. This note does not flip it.

The script writes `galion-gates/results/op-$TS.json` and `galion-gates/.build/op-one-percent.log`. Both are gitignored (`galion-gates/.gitignore`). They are not in this commit. The JSON line below is the file `galion-gates/results/op-20261115T210440+0000.json` as written (one line):

```json
{"ts": "20261115T210440+0000", "gate": "OP", "status": "PASS", "note": "P1≤1% (ok) P3 p99=1438ns ratio=556 much_less=yes cpu=schedstat stub=8 measured=256 product_claim=false", "report": {"engine": "v6-one-percent", "product_claim": false, "tick_ns": 80000000, "p1_budget_ns": 800000, "p3_ll_ns": 80000, "p3_ll_factor": 10, "late_is_pause": true, "catch_up_steps": 0, "stub": {"kind": "stub", "n": 8, "pauses": 1, "over_budget": 0, "p99_ns": 50000, "max_ns": 50000, "p3_under_budget": true, "p3_much_less": true, "p3_ratio": 16, "p3_instrument": "injected", "share": 0.00060937, "share_hi": 0.00060937, "p1_pass": true, "p1_note": "ok", "p1_instrument": "injected", "cpu_ns": null, "cpu_resolution_ns": null, "cpu_source": null}, "measured": {"kind": "measured", "n": 256, "pauses": 0, "over_budget": 0, "p99_ns": 1438, "max_ns": 7621, "p3_under_budget": true, "p3_much_less": true, "p3_ratio": 556, "p3_instrument": "instant", "share": 1.852e-05, "share_hi": 1.852e-05, "p1_pass": true, "p1_note": "ok", "p1_instrument": "schedstat", "cpu_ns": 379194, "cpu_resolution_ns": 1, "cpu_source": "schedstat"}}}
```

## What the numbers are

Budget unchanged: tick 80 ms (`tick_ns` 80000000), 1% of that tick is 800 µs (`p1_budget_ns` 800000). `p3_much_less` is the flag for p99 strictly under 80 µs (`p3_ll_ns` 80000). The gate's exit uses the 800 µs ceiling.

**Measured** series. This process, release example `one-percent`, 256 silence frames, Rika path inside `gate_report`. Clock is outside `Heartbeat::step`. Not inference. Not Piper. Not Nano. Not a clone.

| Field | Printed value |
|---|---|
| kind | `measured` |
| n | 256 |
| p99 | **1438 ns** (1.438 µs). Instrument `instant`. |
| max | 7621 ns |
| p3_ratio | 556 (`p1_budget_ns / p99_ns`) |
| p3_under_budget | true (1438 < 800000) |
| p3_much_less | true (1438 < 80000) |
| pauses | 0 |
| over_budget | 0 |
| P1 instrument | `schedstat` |
| cpu_source | `schedstat` |
| cpu_ns | 379194 |
| cpu_resolution_ns | 1 |
| share | 1.852e-05 (window share, note `ok`, `p1_pass` true) |
| product_claim | false |

**Stub** series. Caller-injected script inside the same JSON: seven 50 µs frames and one late 40 µs frame. Instrument `injected`. p99 50000 ns, n 8, pauses 1, share 0.00060937. This checks the arithmetic and the pause rule. It is not this host's clock and it is not a product claim.

`late_is_pause` is true. `catch_up_steps` is 0. Engine string `v6-one-percent`.

`share` is on-CPU time of this thread over `frames × 80 ms`, from the gate. It is not a host-wide CPU percent. No host-wide CPU percent is claimed.

## Tests the script ran

The retained log starts with `Updating crates.io index` and a compile, so the successful `cargo test` was the script's online retry. An earlier `--offline` attempt, if it failed, was overwritten by that retry and is **not retained**. Not quoted as a separate result.

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 2.93s
```

`cargo test -p galion-symphony` result lines in `galion-gates/.build/op-one-percent.log`:

| Binary | Result line |
|---|---|
| `unittests src/lib.rs` | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `unittests src/main.rs` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `tests/one_percent.rs` | `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `tests/pack.rs` | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| doc-tests `galion_symphony` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |

Those five lines sum to **20 passed, 0 failed**. The release example binary `target/release/examples/one-percent` was produced by the same script invocation (mtime 2026-11-15 21:04 UTC). `target/` is gitignored.

`cargo test --workspace` was **not run** on this branch. The seven `galion-ml-voice` failures in NEXT-ITERATION.md are lane A (A5), not this note.

## What was not run

| Item | State |
|---|---|
| Piper stream TTFA | not run (B1) |
| `scripts/eval/symphony-heartbeat.sh` | not run (B2) |
| Nano original-ref first-PCM / `ttfa_ms` | not run (B3; A0 still BLOCKED-OWNER) |
| EPYC 9655P, CX23, CX43 tick | not measured |
| Host-wide CPU percent | not measured |
| `SELF-EVAL.md` edit | not done. Lane D owns the ledger. The plan says Lane B evidence lives in this note. |
| `LOCKS.toml`, `SOUL.md`, `config.yaml` | not edited |
| Paid model, Qwen, Groq, cloud LLM, deploy | not done |

An older ledger line is a different run and is not this host's p99: `SELF-EVAL.md` records `OP PASS — … p99=3206ns … product_claim=false` on 2026-11-03. It was not re-run here and it is not copied forward as this result.

## CI

At authoring time (2026-11-15 22:06 +01:00) GitHub checks on this branch had not finished. The PR body is updated when they finish. This file is not rewritten just to paste a check URL.

## Decisions for Maciej

1. **B4 on this host:** gate PASS, measured p99 **1438 ns**, instrument `instant`, CPU source `schedstat`, `product_claim=false`. The claim was not earned. This VM is not EPYC 9655P, CX23, or CX43.
2. **A13** stays open until someone runs the same gate on one of those classes and logs that host. This note does not close it.
3. **A0** (t273 vs t623u) is not this lane. Nothing here speaks from the original refs or from Piper.
4. This worker does not merge.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
