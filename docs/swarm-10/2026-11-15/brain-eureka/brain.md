# Swarm 10 — brain-eureka — brain (D1)

**Written:** 2026-11-15 22:15 +01:00 (Europe/Zagreb)
**Role:** brain. Lane plan: draft PR [#152](https://github.com/galion-studio/galion-one-voice/pull/152) (`cursor/swarm10-brain-eureka-plan-b8c3`). This branch is from `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0`, not from the plan branch. `PLAN.md` was not edited.
**Owner:** Maciej Grajczyk. Copyright line stays Maciej Grajczyk.

## What was checked

- `crates/galion-cascade/src/reflect.rs`, `galion-render/src/v2_persona.rs` (`SayWork::model_reply`), `crates/galion-cascade/src/phone.rs`, `scripts/eval/v2_hard_loop.py` `classify_llm` / `llm_source`, `docs/galion-voice-v2/GALION-VOICE-V2.md` V2-S5, `galion-student/README.md`, `galion-student/configs/s0.toml`, `crates/galion-student-backend/src/lib.rs` (`load_slot` still returns `CounselPending` / `NotPlaced`; it was not changed).
- This host: Linux 6.12.113+ x86_64, 4 CPUs, rustc 1.85.1. RAM not measured. `weights/` contains only `.gitignore`. `weights/student/s0.gq` is absent. `docs/galion-voice-v2/personas/refs/` has `samantha.placeholder` and `jarvis.placeholder` only. No student `.gq` was found. No teacher was downloaded. `s0.toml` still names `kyutai/moshiko` as a plan string; that string was not fetched.
- `LOCKS.toml`, `SOUL.md`, `config.yaml`, and `scripts/clone/REFS.sha256` were not edited.

## What changed

The persona floor no longer speaks `reflect::reply` on content turns. `GALION_LLM=canned|inhouse|in-house|student|reflect`, with no Hermes client, probes an S0 `.gq` through `galion_tensor::GqFile`.

- Path: `GALION_STUDENT_GQ` if set, otherwise `weights/student/s0.gq` (gitignored with `weights/`). The probe does not create the file and does not call `GqFile` when the path is missing.
- Missing file: `Student brain unavailable. The S0 student .gq is not on this host.`
- Present but not a readable `.gq`: `Student brain unavailable. The S0 student .gq did not open.`
- A `.gq` that opens and verifies, including a synthetic fixture in the unit test: `Student brain unavailable. The S0 .gq opened and cannot speak.` `INFERENCE_IMPLEMENTED` is false. There is no token generator. The line does not restate the caller and is not `reflect::reply`.
- `"llm"` on that turn is `galion-student-blocked`. It is not `galion-reflect` and not `student-gq`.
- `reflect::reply` remains for its unit tests. It is not relabelled as a model.
- Phone default stays `CannedLlm`. `GALION_LLM=echo` still selects `MockLlm`. `GALION_LLM=student` selects the same blocked probe. `reflect` is an error on the phone, not a fallback to echo or canned.
- `llm_source` treats a reply that starts with `student brain unavailable.` as `student-blocked` even if `GALION_STUDENT_GQ` points at a file. `classify_llm` scores that source `stub`, not `met`.

Harness: `cargo run -p galion-cascade --example student_probe`.

On this host, 2026-11-15 22:15 +01:00:

```text
ready=false label=galion-student-blocked source=student-blocked reason=weights-absent tensors=0 path=weights/student/s0.gq line=Student brain unavailable. The S0 student .gq is not on this host.
```

## Tests

Host as above. rustc 1.85.1. Commands that say "exit 0" printed that exit code.

| Command | Result |
|---|---|
| `cargo test -p galion-cascade --features cascade --lib` | 100 passed, 0 failed, 0 ignored, exit 0. Re-run after the final tree. Includes `student::tests::missing_file_is_blocked_and_does_not_reflect` and `opened_gq_cannot_speak_and_does_not_reflect`. |
| `cargo test -p galion-render --features web --lib -- v2_persona` | 18 passed, 0 failed, exit 0. Includes `inhouse_floor_says_the_student_is_blocked`. |
| `cargo test --workspace` | exit 0. Sum of the per-target lines cargo printed: 887 passed, 0 failed, 5 ignored, 94 targets. That run was before a rustfmt-only rewrite of unrelated cascade files was reverted. The student probe was already in that tree. The post-revert confirmation is the cascade and `v2_persona` rows above, not a second workspace run. |
| `cargo test -p galion-ml-voice --features speech --lib` | 109 passed, 0 failed, 1 ignored, exit 0. The seven failures in NEXT-ITERATION (2026-11-15, the other host, t273 wavs and clone weights on disk) were not reproduced here. Placeholders only, no clone weights. |
| `cargo test -p galion-console --features phone-turn --lib` | 88 passed, 0 failed, exit 0. |
| `cargo test --manifest-path tools/emotion-v2/refcheck/Cargo.toml` | 22 passed across the targets cargo printed (5 + 17), exit 0. |
| `scripts/check-locks.sh` | `OK: all Eureka locks hold (LOCKS.toml)`, exit 0. |
| `scripts/check-no-forbidden.sh` | `OK: forbidden-format + forbidden-runtime gates pass`, exit 0. |
| `scripts/eval/v2_hard_loop.py --self-check` | exit 0. Printed `HARD-PASS of the loop: refused`. Not a product verdict. |
| `scripts/eval/flash-ttfa.sh` with rustc 1.85.1 on `PATH` | exit 0. `{"measured":false,"product_claim":false,"engine":null,"note":"GALION_FLASH_EVAL is not 1; no TTFA number"}`. No TTFA number. |
| `scripts/eval/virtual-user.sh` with rustc 1.85.1 on `PATH` | exit 0. `verdict=green`, 15 cases, 0 failed. Report `galion-gates/results/presence-eval/20261115T211344Z.json` (gitignored). `llm=canned`, `reply_model=canned-llm`, `speech_backend=system-flite`, `asr_mode=interim`, `pstn=not_run`. `ttfa_ms=240`, `ttfa_status=measured`, `ttfa_kind=loopback-full-utterance`. That is canned plus flite, full utterance, not a streaming first frame and not a student or clone clock. `git rev-parse` inside the script still printed `8980e1a` because this commit did not exist yet. |

`cargo test --workspace --offline` was tried first and failed: `failed to download allocator-api2` because `--offline` was set. The workspace row above is the online retry. Not a test failure.

A first `scripts/eval/flash-ttfa.sh` used the default `cargo` (rustc 1.83.0) and exited 2 with `rustc 1.83.0 is not supported` (`galion-ml-voice` requires 1.85). That run is not a TTFA result. The 1.85.1 row is the one that ran the harness.

## CI

Not observed at the time this file was written. The PR checks are the record.

## Open questions

- S0 teacher / training source is still an owner decision. This PR does not pick one.
- A `.gq` that opens still cannot speak. A generator has to land before `ready=true` is honest. Flipping the label without that generator would be a lie.
- Persona-http in the hard loop still wants the grief reply to name the fact (`mother`). The blocked line does not. A demo launched with `GALION_LLM=canned` and no Hermes client will fail that gate. That is a failed content turn, not a pass. The words were not stuffed back in to keep the gate green.
- Phone default remains the canned table, as the lane plan requires. Tonight's virtual-user shape (`llm=canned`) is still what this host prints. Echo is not the default.

## Decisions for Maciej

1. No S0 weights on this host, and no teacher chosen. The live in-house content turn says the student brain is unavailable.
2. Phone default stays `CannedLlm`. Say if the phone default should become the blocked line instead.
3. A0 (t273 vs t623u) was not touched.
4. No deploy. No merge from this worker.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
