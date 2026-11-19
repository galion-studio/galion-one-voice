# Swarm 10 — clone-compiler — compiler (A2)

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. Written 2026-11-15 22:10 +01:00.

The spelling is **Galion**. The product is Galion Voice.

Role: compiler. Lane plan: [PR #153](https://github.com/galion-studio/galion-one-voice/pull/153) `docs/swarm-10/2026-11-15/clone-compiler/PLAN.md`. This PR does not edit that file.

## What was checked

`COMPILER.md`, `galion-voice-compile`, `speech::gqvoice`, and `speech::tests::landed_clone_is_compiled_before_the_stack_plays_it` on `8980e1a`.

The string `gqvoice: No such file or directory (os error 2)` is `GqVoice::open` formatting `std::io::Error`. It is not a missing `gqvoice` executable. The example `crates/galion-ml-voice/examples/gqvoice.rs` is not on the PATH of this test. `handoff_at` writes `dir/<persona>.gqvoice` and reads it back, in process.

Reproduce, this host, before the change. Unallowlisted bytes (not a recording) were placed at the gitignored pin paths and deleted afterwards. They were not committed.

```text
cargo +1.85.0 test -p galion-ml-voice --features speech --lib landed_clone_is_compiled_before_the_stack_plays_it -- --test-threads=1
```

```text
thread 'speech::tests::landed_clone_is_compiled_before_the_stack_plays_it' panicked at crates/galion-ml-voice/src/speech/mod.rs:1064:52:
called `Result::unwrap()` on an `Err` value: Backend("gqvoice: No such file or directory (os error 2)")
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 109 filtered out; finished in 0.04s
```

`speak` calls `resolve_reference` first. The unallowlisted file returns `reference sha256 mismatch`. That is not the soft "not allowlisted" path, so `SpeechStack` falls back. This host has libflite and no Piper pack, so the utterance was Flite. The clone packet was never written. The same command on a clean tree (no wav at the pin) passed in 0.03 s before the change.

`LOCKS.toml` and `scripts/clone/REFS.sha256` were not edited. The production hash compare was not weakened. With the same unallowlisted file still on disk, `two_turns_reuse_one_clone_process_and_the_clock_is_not_a_product_claim` still panicked: `Backend("reference sha256 mismatch for jarvis")`. That test, and the other four pipe tests, stay with truth (A5).

## What changed

- `CloneWorker::with_ref_root` points reference lookup at a directory the caller chooses. The compile-then-play tests pass an empty directory (no wav, no voice TOML), which is the same absent-ref path CI already takes. The live slot leaves the root unset and still hashes the repo pin. An unallowlisted wav is not treated as allowlisted.
- `landed_clone_is_compiled_before_the_stack_plays_it` uses that empty root, refuses a Flite/Piper fallback, and lands `.gqvoice` for Samantha and Jarvis under the temp dir. A missing `python3` or a packet that was not written panics with `compile-then-play BLOCKED`, not a bare `os error 2`. The test checks `voices/gq` did not gain a file.
- `GqVoice::open` / `land` map `NotFound` to `gqvoice BLOCKED: packet was not written at <path>. The compiler is in-process Rust, not a gqvoice executable on PATH.`
- `tests/gqvoice_handoff.rs` uses the same empty root so the existing protocol-stub handoff still compiles when an unallowlisted file sits at the pin. That PCM is protocol-stub. It is not Chatterbox Nano and it is not a product pass.

Raw PCM, RIFF, and a `.gq` weight are still refused as a `.gqvoice` runtime file. Nothing was loosened.

## Tests

Host: `cursor`, Linux 6.12.113+ x86_64, 4 CPUs, 15 GiB. rustc 1.85.0 (4d91de4e4 2025-02-17). Not `grok-bot-vm-511777615`. Not an EPYC 9655P, CX23, or CX43.

| Command | Result |
|---|---|
| `cargo +1.85.0 test -p galion-ml-voice --features speech --lib` (clean tree, no refs, no weights) | **110 passed, 0 failed, 1 ignored**, 0.65 s |
| same filter `landed_clone_is_compiled_before_the_stack_plays_it`, unallowlisted bytes at both pin paths, then deleted | **ok**, 0.05 s |
| `cargo +1.85.0 test -p galion-ml-voice --features speech --test gqvoice_handoff` clean and with those bytes | **ok**, 0.15 s each |
| `cargo +1.85.0 test -p galion-ml-voice --features speech --test voice_artifact -- --test-threads=1 --nocapture` | **3 passed**. Tick line below |
| `scripts/check-locks.sh` | `OK: all Eureka locks hold (LOCKS.toml)` |
| `scripts/check-no-forbidden.sh` | `OK: forbidden-format + forbidden-runtime gates pass` |

Workspace `cargo test --workspace` was **not run** on this host.

## Measurements

Fixture tick only, debug profile, this host, `voice_artifact::fill_frame_allocates_nothing_and_meets_the_tick_budget`:

```text
section-a tick p99_ns=6775 max_ns=6878 budget_ns=800000
```

p99 6.8 µs, max 6.9 µs, budget 800 µs. Late frames are inside that loop. This is not a hosted voice session and not A13.

| Gate | This run |
|---|---|
| Persona `.gq` from original-ref Nano PCM | **BLOCKED**. A0 is still open. A1 has not emitted PCM. No torch, no chatterbox, no tokenizer, no weights, and no reference wav on this VM. Nothing was invented. |
| SELF-EVAL A14 | Left **BLOCKED**. Protocol-stub and the fake-child buffer are not that row. |
| MOS | not measured |
| TTFA | not measured |
| Speaker similarity | not measured |
| EPYC / CX23 / CX43 tick | not measured |

No `.wav`, `.pcm`, `.gq`, `.gqvoice`, or weight file was committed. Packets from the tests stayed under the temp dir and were removed.

## CI

Not recorded in this file at authoring. The PR stays draft until `test`, `emotion-v2 harness (CPU stub)`, and `playwright` are success on this head. `nebula-voice-v2` is path-filtered.

## Open questions

Truth has not landed the shared isolation for the five pipe tests. This PR does not edit those tests. If truth's hook replaces `with_ref_root`, rebase this test onto it. Do not merge this ahead of truth if the hunks collide; the plan's order is truth, then compiler.

Real Samantha/Jarvis `.gq` from Nano PCM waits on Maciej's A0 decision and on A1. This page does not close it.

## Decisions for Maciej

A0 is unchanged: re-open the t273 originals yourself, or state that main stays on the t623u CC0 pins and symphony B/C must be re-stated. This PR does not choose. Live `/one` stays Piper until a clone mouth is actually live. This packet is not that mouth.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
