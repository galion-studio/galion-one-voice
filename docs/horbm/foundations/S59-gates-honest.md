# S59 — gates that say what they ran

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S59. Make `galion-gates/run-gates.sh` exit 0 on `main` with no VPS settings, and still exit 1 on a real GGUF load. |
| Written | 2026-11-25. No weight download, no voice file, no paid API, no merge. |
| Base | `main` `3c6751141f510f4c6538cd1cb07bafa2f63a98b5` |
| Persona | Samantha only. Aria is not loaded. Not edited. |
| Consent | `remember=deny`, `learn=deny`, `learn_in_band=false`. Not edited. Ear prosody stays on (`GALION_EAR_SENSE` is the off switch). |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. |

## What it decides

Three results on `main` were not the thing they claimed.

G2 failed on `crates/horbm-ear/tests/frames.rs:596`. That line is the string `"gguf",` inside `let banned = [ ... ]`. The test then asserts `!text.contains(needle)` over the crate's `src/*.rs`. The word is an entry in a forbidden-word list. It is not a load of a container. The ear crate was not edited.

G4 soft-failed on a host whose active `rustc` is 1.83. The throwaway snapshot package path-depends on `galion-render`, and that crate's path dependencies set `rust-version = "1.85"` (`galion-cascade`, `galion-consent`, `galion-heartbeat`, `horbm-brain`). Cargo stops before it can hash a snapshot. The golden file was not wrong. The summary kept only the first error line, which ends before the package names.

R0–R3 were `PENDING` whenever `GALION_VPS_HOST` was unset. Titan is down on purpose during the cloud move. There is no remote run to wait for. Pending read as unfinished work. A local run with no VPS settings now records `SKIP` and the note `skipped (no VPS)`.

`FAIL` is still the only status that makes `run-gates.sh` exit 1. `SKIP`, `SOFTFAIL`, and `PENDING` do not.

## G2

`galion-gates/g2_scan.py` is the rule. `run-gates.sh` calls it. `run-gates.sh --g2-scan ROOT` is the same scan and nothing else, so the self-test's exit code is the runner's.

A file whose name contains `gguf` is a container, except under `target/` or `.git/`. Docs, `galion-gates/`, `results/`, and `.build/` are not line-scanned. `LOCKS.toml`, `scripts/check-no-forbidden.sh`, and `*.off` are not line-scanned. The older ban phrases stay ignored when the line is not a load: a full-line comment, `refuse(`, an absence `contains("gguf")`, `gguf=false`, `do not import`, the magic-byte check `b"GGUF"`, and the rest of the list in the scanner.

A forbidden-word list element is not a load. Every `gguf` on the line is a string literal whose entire contents are that token, and either:

- the line is only that literal (optional commas), or it is a one-line denylist declaration such as `let banned = ["gguf"];`, and
- the previous 80 lines bind a denylist (`banned`, `forbidden`, `denylist`, `deny_list`, `blocklist`, `ban_list`, `prohibited`, `forbidden_words`) and contain `[`, and the next 80 lines assert the token is absent (`!…contains(`, `contains(needle`, or `not in`), or
- this line, or the previous non-empty line, carries `galion-gates:allow-forbidden-word`.

The marker applies only to that bare shape. `frames.rs` needs no marker: the binding is `banned` and the assert is `!text.contains(needle)`.

These still fail, including under `tests/`:

- `std::fs::read("model.gguf")`, and the same call on one line with `refuse(` or with `contains(".gguf")`
- `include_bytes!("fixture.gguf")` and `include_str!` of a container
- a string whose entire contents are a relative filename ending in `.gguf` (`model.gguf`, not the bare extension `.gguf`, not a longer grep pattern)
- an unquoted `name.gguf` that is not the shell glob `*.gguf`
- `let fmt = "gguf";` and `format!("model.{}", "gguf")`
- `let banned = ["gguf"];` with no absence assert and no marker
- a file named `weights.gguf`

`std::fs` or `File::open` on the same line as the token is a load even when the line also matches an old ban phrase. The ban phrase does not clear it.

The scan is textual. This is not a load it can see, and it does not pretend to:

```rust
let banned = ["gguf"];
assert!(!text.contains(needle));
std::fs::read(format!("model.{}", banned[0]));
```

The read line no longer spells the token. Naming that limit is the point of the self-test's positive cases: the lines that still spell a container or an open stay hits.

The old advisory grepped `/workspace` and called the count non-code. That path is not the checkout on every machine. The advisory now counts files under the repo root and says the count includes docs and ban lines. It is not a failure.

## G4

Reproduced on this machine before the fix.

Active `rustc` 1.83.0, `galion-gates/g4-render.sh` exit 1:

```text
error: rustc 1.83.0 is not supported by the following packages:
  galion-cascade@0.0.1 requires rustc 1.85
  galion-consent@0.0.1 requires rustc 1.85
  galion-heartbeat@0.0.1 requires rustc 1.85
  horbm-brain@0.0.1 requires rustc 1.85
```

`run-gates.sh` stores that as `SOFTFAIL` unless `GALION_GATES_HARD=1`. The golden sha was not compared.

`rustc` 1.85.0, same script, exit 0. Snapshot sha `618a6b2646da5d8dd13ef6ba89f7ca65ef1fccd04e4daf2fa31edfdf5ddc21dd`, which is `galion-gates/golden/g4-render.sha256`. `present()` p99 was 10275 ns. The golden file was not regenerated.

The harness now picks the compiler before it builds.

- Active `rustc` is at least 1.85: `cargo build --release --offline`, then one online retry if the failure is not the rust-version error.
- Active `rustc` is older and `rustup run 1.85.0 rustc --version` works: `cargo +1.85.0`. The PASS note names the switch. On this machine, default 1.83.0 produced PASS, the same sha, p99 10661 ns, and the note `[rustc 1.85.0; active 1.83.0 is below rust-version 1.85]`.
- Neither is available: exit 3, `G4 SKIP — skipped (rustc 1.83.0 < 1.85; galion-render path deps set rust-version 1.85; snapshot not checked)`. Checked by putting a `rustup` that exits 1 first on `PATH`. That skip is not snapshot drift. `run-gates.sh` records `SKIP`, not `SOFTFAIL`.

If the snapshot is deterministic and `NullSurface` accepted every frame, and the only miss is `present()` p99 over 1 ms, the harness runs once more. A second miss still fails. The 1 ms check stays.

`sha256sum` is used when it exists. `shasum -a 256` is the fallback. This host has `sha256sum`.

## G6

The same rustc 1.85 run recorded `G6 SOFTFAIL — structural: no SENSE guard`. The guard is in `galion-emotion/src/rika.rs` (`consent_bits::SENSE` before `#[cfg(test)]`). The script did `echo "$CODE" | grep -q ...` under `set -o pipefail`. `grep -q` exits at the first match and closes the pipe. `echo` then dies with SIGPIPE, the pipeline's status is non-zero, and a present guard is reported missing. It is a race: a later run of the same script reached the cargo step instead.

The structural checks now use here-strings. Five runs of `g6-rika.sh` on rustc 1.85.0 each exited 0: `R1–R9 pass (9 passed, 0 failed)` and the SENSE and ADAPT guards. The crate was not edited. G6 stays a soft gate.

## R0–R3

`vps_mode` in `galion-gates/gates_lib.sh` returns `remote` when `GALION_VPS_HOST` is non-empty, and `skip` when it is unset or empty. `run-gates.sh` then marks R0, R1, R2, and R3:

| Host | Status | Note |
|---|---|---|
| unset or empty | `SKIP` | `skipped (no VPS)` |
| set, ssh BatchMode fails | `PENDING` | `VPS <host> unreachable (ssh BatchMode)` |
| set, remote `run-gates.sh --strict-cv` exits 0 | `PASS` | remote run rc |
| set, remote run exits non-zero | `SEE_LOG` | remote run rc |

`is_hard_fail` is true only for `FAIL`. The self-test checks `SKIP`, `SOFTFAIL`, and `PENDING` against that function, and checks that the runner still contains both the skip note and the unreachable-host note.

Older notes still say R0–R3 stay `PENDING` when the variable is unset. They were not edited. They are `docs/galion-voice-v2/GALION-VOICE-V2.md` (V2-S10), `docs/LIVE-DEMO.md`, `docs/V1-DEMO.md`, `docs/agent-wave20/R0-R3-DRY-RUN.md`, and the wave status notes that quote a past run. The runner is the contract this change updates. An agent run still must not set `GALION_VPS_HOST`.

## Self-test

```bash
bash galion-gates/run-gates.sh --self-test
```

No cargo, no network, no microphone. It scans this repo (exit 0, so `frames.rs:596` is not a hit) and temp trees. The load planted at `crates/example/tests/evil.rs` as `std::fs::read("model.gguf")` exits 1. So does that load with the allow-marker on the line, and so does `refuse(...); std::fs::read("model.gguf");` on one line. The `frames.rs` shape exits 0. `rustc_at_least_185` accepts 1.85 and 2.0 and rejects 1.83 and 1.84.

## Measured

rustc 1.85.0 on Linux, `GALION_VPS_HOST` unset. `cargo test --workspace`: 1062 passed, 0 failed. `cargo clippy --workspace --all-targets -- -D warnings`: clean. `cargo test --manifest-path horbm/Cargo.toml --workspace`: 267 passed, 0 failed (one ignored card test stays ignored unless `GALION_HORBM_REMEMBER=grant`). `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings`: clean.

After the G6 here-string fix, `galion-gates/run-gates.sh` exited 0:

```text
G0=PASS G1=PASS (cargo test 1062 passed, 0 failed) G2=PASS G3=PASS
G4=PASS G6=PASS PS=PASS LK=PASS DG=PASS LV=PASS V1=PASS OP=PASS SY=PASS RG=PASS
R0-R3=SKIP skipped (no VPS)  fails=0
```

G4 sha `618a6b2646da5d8dd13ef6ba89f7ca65ef1fccd04e4daf2fa31edfdf5ddc21dd`, p99 10903 ns. `bash galion-gates/run-gates.sh --self-test` exited 0.

## Files

- `galion-gates/g2_scan.py` — G2 rule
- `galion-gates/gates_lib.sh` — `vps_mode`, `is_hard_fail`, `rustc_at_least_185`
- `galion-gates/selftest.sh` — the cases above
- `galion-gates/run-gates.sh` — calls the scanner; R0–R3 skip when no host is set
- `galion-gates/g4-render.sh` — toolchain selection, one p99 retry, `shasum` fallback
- `galion-gates/g6-rika.sh` — structural checks read the source with here-strings so `pipefail` cannot turn a present SENSE guard into a miss
- this note

Not edited: `crates/**` (including `crates/horbm-ear`), `horbm/**`, `galion-render/**`, `LEDGER.md`, root `Cargo.toml`, `Cargo.lock`, `.github/workflows/*`, `LOCKS.toml`, `SOUL.md`. No new dependency.

Linux notes, unchanged except where this change touches them: G0 still reads `/proc` and `lscpu`. The VPS block still uses `ssh` and only runs when a host is set. G4's hash prefers `sha256sum` and falls back to `shasum -a 256`. G4's `cargo +1.85.0` needs rustup when the active compiler is older than 1.85. The required green run is rustc 1.85.
