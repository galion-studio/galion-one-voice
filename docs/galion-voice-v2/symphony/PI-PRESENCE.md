# Pi presence

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-06. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Pi here is the 64-bit Raspberry Pi GNU target `aarch64-unknown-linux-gnu`. This page records commands run on this checkout. A score this checkout did not print stays **BLOCKED**. WER, ECAPA, MOS, latency, and TTFA stay **BLOCKED**.

## Bar

This landing meets Pi when all three are true:

1. `cargo test -p galion-presence-eval` exits 0.
2. `cargo check -p galion-presence-eval --target aarch64-unknown-linux-gnu` exits 0.
3. A linked `galion-presence-eval` ELF for `aarch64-unknown-linux-gnu` exists, and `stat` recorded its size in bytes.

This session meets the first two. The third is unmeasured. **Not yet.**

## This session

Host `GALION-STUDIO`. `uname -srm` printed `Linux 6.6.87.2-microsoft-standard-WSL2 x86_64`. `rustc -vV` printed `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, host `x86_64-unknown-linux-gnu`. Branch `cursor/symphony-v2-emotion42-presence-a4-d379`. Worktree `/home/galion/workspace/galion/src/swarms/e42/presence-a4`. Environment: `source /home/galion/workspace/galion/env.sh`. Cargo ran under `flock -w 7200 /home/galion/workspace/galion/tmp/cargo-build.lock`. No Raspberry Pi ran these commands.

`aarch64-linux-gnu-gcc` is absent on this host. `cargo check` typechecks. It does not link an executable.

`rustup target add aarch64-unknown-linux-gnu` exited 1:

```text
info: downloading component rust-std
info: rolling back changes
error: component download failed for rust-std-aarch64-unknown-linux-gnu: could not rename 'downloaded' file from '/home/galion/workspace/galion/rustup/downloads/e25668477083d6a9ddb87b9beb9c00742225f38233ccb4bfba3afa7a07427f52.partial' to '/home/galion/workspace/galion/rustup/downloads/e25668477083d6a9ddb87b9beb9c00742225f38233ccb4bfba3afa7a07427f52': No such file or directory (os error 2): No such file or directory (os error 2)
```

After that failure, `rustup target list --installed` listed `aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu`. The sysroot directory `lib/rustlib/aarch64-unknown-linux-gnu/lib` held 62 files. The check below used that target.

## Commands

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p galion-presence-eval` | 0 | 7 lib tests passed. Bin tests 0. Doc-tests 0. Finished in 6.51s. |
| `cargo check -p galion-presence-eval --target aarch64-unknown-linux-gnu` | 0 | Finished `dev` profile in 4.95s. One dead-code warning in `galion-ml-voice`. |
| `galion-gates/rg-offline.sh` | 0 | RG PASS. Metric rows stay BLOCKED. |

### `cargo test` result lines

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.51s
     Running unittests src/lib.rs (target/debug/deps/galion_presence_eval-e88d2ee7d1b9a5b6)

running 7 tests
test score::tests::denylist_hits_dwell_and_spares_ttfa ... ok
test score::tests::interim_does_not_claim_recognition ... ok
test loopback::tests::loopback_guard_rejects_wildcard ... ok
test score::tests::task_facts_match_the_canned_table ... ok
test listen::tests::silence_has_no_voice_and_a_tone_does ... ok
test tests::headless_eval_scores_labels_without_inventing_presence ... ok
test tests::virtual_user_writes_a_green_loopback_report ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s

     Running unittests src/main.rs (target/debug/deps/galion_presence_eval-dcd9591f87a11fb4)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests galion_presence_eval

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The words `0 measured` in those three lines are the Rust test harness benchmark counter. This page does not turn them into a latency, a TTFA, or a presence score. The test log printed no `ttfa_ms`.

### `cargo check` result lines

```text
warning: field `counsel` is never read
  --> crates/galion-ml-voice/src/speech/clip_store.rs:72:5
warning: `galion-ml-voice` (lib) generated 1 warning
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.95s
```

### `galion-gates/rg-offline.sh`

Stdout:

```text
RG PASS — honesty-contract exit 0; wer ecapa mos latency ttfa microphone weights network BLOCKED
```

Receipt `galion-gates/results/rg-20261106T175423+0100.json` (gitignored):

```json
{"ts": "20261106T175423+0100", "gate": "RG", "status": "PASS", "note": "honesty-contract exit 0; wer ecapa mos latency ttfa microphone weights network BLOCKED", "commands": [{"cmd": "rustc --edition 2021 --test -O galion-gates/rg/offline_gate.rs", "exit": 0}, {"cmd": "galion-gates/.build/rg_offline_test --test-threads=1", "exit": 0}, {"cmd": "rustc --edition 2021 -O galion-gates/rg/offline_gate.rs", "exit": 0}, {"cmd": "galion-gates/.build/rg_offline", "exit": 0}], "ledger": {"gate": "RG", "used_network": false, "used_weights": false, "used_microphone": false, "fails": 0, "rows": [{"name": "honesty-contract", "status": "PASS", "exit": 0}, {"name": "wer", "status": "BLOCKED"}, {"name": "ecapa", "status": "BLOCKED"}, {"name": "mos", "status": "BLOCKED"}, {"name": "latency", "status": "BLOCKED"}, {"name": "ttfa", "status": "BLOCKED"}, {"name": "microphone", "status": "BLOCKED"}, {"name": "weights", "status": "BLOCKED"}, {"name": "network", "status": "BLOCKED"}]}}
```

The gate keeps `wer`, `ecapa`, `mos`, `latency`, and `ttfa` at **BLOCKED**. It writes no numeric value for those rows.

## Sizes

`stat -c '%n %s'` on 2026-11-06. `file` on the two host harnesses reported `ELF 64-bit LSB pie executable, x86-64`, dynamically linked, with debug info, not stripped.

| Path | Bytes | What `file` and the path say |
|---|---:|---|
| `target/debug/deps/galion_presence_eval-e88d2ee7d1b9a5b6` | 37270520 | x86-64 debug lib test harness |
| `target/debug/deps/galion_presence_eval-dcd9591f87a11fb4` | 7207656 | x86-64 debug bin test harness (0 tests) |
| `target/aarch64-unknown-linux-gnu/debug/deps/libgalion_presence_eval-6089469d2b62aa4a.rmeta` | 72089 | `cargo check` metadata for `src/lib.rs` |
| `target/aarch64-unknown-linux-gnu/debug/deps/libgalion_presence_eval-88979d76914102c8.rmeta` | 0 | `cargo check` metadata file for `src/main.rs` |
| `target/debug/galion-presence-eval` | unmeasured | path absent |
| `target/aarch64-unknown-linux-gnu/debug/galion-presence-eval` | unmeasured | path absent |

`find target/aarch64-unknown-linux-gnu -type f -executable` counted 0. The linked aarch64 executable size is **unmeasured**. The `.rmeta` byte counts are metadata from `cargo check`. They are not an executable size.

## Verdict

**Not yet.** Host tests passed. The aarch64 check passed. The Pi executable was not linked, and its size is unmeasured. Product scores stay **BLOCKED**.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
