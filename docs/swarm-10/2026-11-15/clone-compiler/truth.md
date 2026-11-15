# Swarm 10 · clone-compiler · truth

**Written:** 2026-11-15 22:13 +01:00 (Europe/Zagreb)
**Role:** worker `truth` (Lane A, A5 then A4)
**Base:** `8980e1a` (`Merge pull request #144`)
**Branch:** `cursor/clone-compiler-truth-4ead`
**Plan:** orchestrator draft PR “Swarm 10 clone-compiler plan” (PR #153). This PR does not edit `PLAN.md`.
**Host:** `cursor`, Linux 6.12.113+ x86_64, 4 CPUs, 15 GiB RAM, rustc 1.85.1 (4eb161250 2025-03-15). This is not `grok-bot-vm-511777615`.

## What was checked

The seven `galion-ml-voice` failures in `NEXT-ITERATION.md` (host `grok-bot-vm-511777615`, commit `8980e1a`) are a present unpinned wav, a publish script that already verifies, and a packet the stack never wrote. This host has none of the operator files: no `docs/galion-voice-v2/personas/refs/*.wav` (placeholders only), no `weights/clone/*.safetensors`, and `gqvoice` is not on `PATH`.

`LOCKS.toml` `[formats.exceptions.v2s14_chatterbox]` stays `reference_clips = true`, `ref_clip_decision = "t623u"`. Uncommented `scripts/clone/REFS.sha256` rows stay the t623u CC0 pins. Commented t273 rows stay commented. Neither file was edited. No wav and no weight was deleted or committed.

A0 stays with Maciej. t273 originals are not the live allowlist. This change does not add `GALION_CLONE_REF_SHA` or `GALION_CLONE_REF_ALLOW`.

## What changed

`resolve_reference` still hashes the first existing candidate and still refuses a digest that is not the uncommented pin. The file is not attached and not deleted.

- A commented t273 original (Samantha `bdcb0c29…`, Jarvis `c2200b59…`) is named inside that refuse: `reference sha256 mismatch for {persona}: on-disk file is the superseded t273 original and is not allowlisted; active pin remains t623u`.
- Any other digest stays `reference sha256 mismatch for {persona}`.
- A protocol child (`GALION_CLONE_STUB_PCM=1`, or a script whose basename is not `chatterbox_worker.py`) omits the path and continues. That is the same outcome as a temp root with no wav. It does not allowlist t273. The repo worker (`chatterbox_worker.py`, stub off) still returns the error.
- `scripts/clone/fetch-clone-weights.test.sh` accepts exit 0 when the checkout weights verify and the script does not upload, and it always keeps an empty-directory case that expects exit 2 `BLOCKED`. A fake `gh` is on `PATH` for those cases and must not run. `--upload` is not used by the new cases.
- `scripts/clone/test_worker_gate.py` stops claiming the uncommented rows are the t273 pins. The allowlist prefixes stay `e4bc60c3` / `60ad5457`.
- Symphony pages that described the child as always unwired now match `scripts/clone/chatterbox_worker.py`: missing weights exit 2 `galion-clone-absent`; stub PCM is `protocol-stub` and is not generate; weights present and `GALION_CLONE_LOCAL_REFS` unset is `galion-clone-unwired` after the pin check; generate runs only for an uncommented pin with `GALION_CLONE_LOCAL_REFS=1`. A t273 file is not that pin.

Left alone on purpose:

- `speech::tests::landed_clone_is_compiled_before_the_stack_plays_it` and `gqvoice.rs`. That test is the compiler’s (A2). On the operator host the mismatch became a hard Piper fallback, no packet was written, and `GqVoice::open` reported `No such file or directory`. That string is the missing packet, not a `gqvoice` binary on `PATH`. There is an example `crates/galion-ml-voice/examples/gqvoice.rs`. A fake script (basename not `chatterbox_worker.py`) now omits the unpinned wav, so the packet can still be written. This host has no t273 wav, so that dirty-disk path was not executed here. The test body was not edited. On this clean tree the test passed.
- `scripts/clone/chatterbox_worker.py`. The generate path already says `reference sha256 is not allowlisted`. The stub and unwired pin check already says `reference sha256 mismatch`. Both apply to every non-pin digest, including a t273 original. Exit 3 is `galion-clone-refuse`.
- `LOCKS.toml`, `scripts/clone/REFS.sha256`, `SOUL.md`, `config.yaml`, voice TOML, `weights/`, runtime binaries.
- `docs/galion-voice-v2/symphony/HEARTBEAT-WIRE.md` still says the model call is unwired. Lane B owns that page (heartbeat PR #154).
- `SELF-EVAL.md` is the D3 ledger. This PR does not mark a clone row PASS.

## Tests

Run on this host at 2026-11-15 22:12 +01:00, rustc 1.85.1.

| Command | Result |
|---|---|
| `cargo test -p galion-ml-voice --features speech` | lib **110 passed, 0 failed, 1 ignored** (`python_written_file_verifies`). bin 0. `a21_piper` 1, `clone_drain_alloc` 1, `fallback_speak` 1, `gqvoice_handoff` 1, `no_cuda_default` 10, `piper_drain` 1, `roundtrip_wav` 7, `scaffold` 7, `voice_artifact` 3. doc-tests 0. |
| `python3 scripts/clone/test_worker_gate.py` | 13 tests, OK, 0.276 s |
| `scripts/check-locks.sh` | OK, all Eureka locks hold |
| `scripts/check-no-forbidden.sh` | OK |
| `cargo test --workspace` | not run |

The five former ref failures and `fetch_script_release_pins_stay_local` are in the 110. `landed_clone_is_compiled_before_the_stack_plays_it` is also in the 110. One pre-existing warning: `clip_store.rs` field `counsel` is never read. Not changed.

The t273 classifier was unit-tested with the commented digests. No t273 wav bytes were on this disk, so the on-disk hash of an original was not measured here.

`GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --check --nano` (no `--upload`, `GALION_CLONE_PUBLISH` unset): **exit 1**. Stdout `skip turbo/t3_turbo_v1.safetensors (--nano)`. Stderr `MISSING/BAD` for `shared/s3gen.safetensors`, `shared/s3gen_meanflow.safetensors`, `shared/ve.safetensors`, `nano/t3_nano_v1.safetensors`, then `CLONE=ABSENT (check incomplete)`. Weights are absent on this host. The check created gitignored empty `weights/clone/` directories. Nothing was uploaded.

`scripts/eval/v2-hard-loop.sh` with rustc 1.85.1 on `PATH`: **exit 1**.

```text
overall: HARD-FAIL
HARD-PASS of the loop: refused
report-line: HARD-FAIL ttfa_ms=absent allocs=0 late_frames=proved persona_http=absent dot=absent product_claim=false missing=tree-hygiene,ref-hashes,weight-pins,piper-pcm,ttfa-streaming,real-llm,real-thinking,real-persona,full-chain,persona-http,live-one-dot
```

This is not STUB-PASS and not HARD-PASS. `product_claim=false`. Protocol-stub PCM was not promoted. `ref-hashes` absent (no `GALION_CLONE_LOCAL_REFS=1`, no clip store). `weight-pins` absent (safetensors not on disk; fetch was the `--check` above, not a download). `tree-hygiene` failed because 46 `docs/galion-voice-v2/evals/listening-set/emotion-sweep/*.wav` files are already tracked on `8980e1a`. They were not added or deleted here. `real-sampling` and `late-pause` met under the 1.85.1 toolchain. An earlier loop on the default rustc 1.83 failed `late-pause` with `galion-heartbeat requires rustc 1.85`. That run is not the line quoted above. Piper PCM, TTFA, a live server, a Hermes reply, and a thought trace were absent. No Nano PCM was measured. Torch was not run.

## CI

Not run at this writing. The branch was not pushed yet. Required checks once pushed: `test` (gates), emotion-v2 harness (CPU stub), playwright (`one-desktop`). Main on `8980e1a` was already green (runs 43660665688, 43660665722, 43660665724). `nebula-voice-v2` is path-filtered.

## Open questions for Maciej

- A0: re-open the t273 originals himself, or keep t623u and restate symphony B and C. This PR does not choose.
- Live `/one` stays Piper plus the Samantha persona until a clone mouth is actually live. No Nano smoke speak was logged.
- A renamed copy of `chatterbox_worker.py` is treated as a protocol child and omits an unpinned wav. The repo path still refuses. Say if that filename rule should be narrower.
- `#78` and `#83` stay parked. No agent signs `ADMIN-UI-CHECK.md`.
- `HEARTBEAT-WIRE.md` and `SELF-EVAL.md` still have older “unwired” / open rows. Left for their lanes.
