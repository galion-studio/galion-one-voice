# Swarm 10 — brain-eureka — seam (D2)

**Written:** 2026-11-15 22:13 +01:00 (Europe/Zagreb)
**Role:** seam. Lane D, backlog D2. Plan: [#152](https://github.com/galion-studio/galion-one-voice/pull/152) (`docs/swarm-10/2026-11-15/brain-eureka/PLAN.md`). This file does not edit `PLAN.md`.
**Host:** `cursor`, Linux x86_64, 4 CPUs, rustc 1.85.0 (`4d91de4e4`, 2025-02-17). The default `rustc` on the box was 1.83.0; tests below used `cargo +1.85.0`.
**Base:** `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0`. Branch from that commit, not from the plan branch.

## This host is BLOCKED

Checked 2026-11-15 22:12 +01:00, before the tests, and again at 22:13 +01:00:

| Piece | Result |
|---|---|
| `/opt/galion/openmind` | absent (`test -d` exit 1). The seam test `deploy_tree_is_not_created_or_written` left it absent. Nothing was written there. |
| `GALION_OPENMIND_STORE` | unset |
| `GALION_STUDENT_GQ` | unset |
| `GALION_OPENMIND_REMEMBER` | unset (deny) |
| `GALION_OPENMIND_SEAM` | unset (the phone worker does not consult the seam) |
| `*.gq` / `*.gqvoice` under `galion-student/` and `voices/` | 0 files. `galion-student/configs/s0.toml` is present. That is a config, not a weight. |
| `flite`, `piper` | not on `PATH` |

Later-turn memory on this host is **BLOCKED**. The missing pieces are the store and the student `.gq`. `remember` is also deny, which is the default. No memory line was invented for this machine. A temp directory created inside a unit test is not a store this host has.

No TTFA number. Not measured. `scripts/eval/virtual-user.sh` was not run. `cargo test --workspace` was not run.

## What changed

`TurnSeam` in `crates/galion-openmind/src/seam.rs`.

- `GALION_OPENMIND_STORE` counts only when the path is already a file or a directory. A missing path is not created. A directory uses `turns.jsonl` inside it. Lines are `v1`, turn id, role `user` or `voice`, and the text. A later `consult` returns that text unchanged. A bad line is skipped.
- `GALION_STUDENT_GQ` counts only when the path is a non-empty file ending in `.gq`. The bytes are not loaded and are not spoken. D1 owns a speaking student. This PR does not label a turn `student-gq`.
- `GALION_OPENMIND_REMEMBER` defaults to deny. Deny does not write and does not return stored text.
- Path state is `present` only when both files exist. Otherwise it is `blocked` and names `store`, `weights`, or `store+weights`. There is no thought-trace field. `hermes_called` is always false. Hermes stays on `CortexHandoff`.
- A write whose path is `/opt/galion/openmind` or under it is refused. A read of a file a human already placed there is allowed and does not add a file.
- The phone worker (`crates/galion-console`, feature `phone-turn`) calls the seam only when `GALION_OPENMIND_SEAM` is `1` / `true` / `on` / `yes`, on the worker thread, after `Heartbeat::step` has already returned. When the path is blocked, the spoken words stay the lane reply (`CannedLlm`, or the lane's own Hermes client if `GALION_LLM=hermes`). This crate does not synthesize the unreachable sentence. When `remember` is granted and an earlier line is in the store, the reply text is that exact line and the llm label is `local-store` (not `student-gq`, not `galion-reflect`). `classify_llm` treats `student-gq` as `real-llm: met`. This label is not that source.
- `student_present` in the phone host now uses the same `.gq` file check. An empty file or a non-`.gq` file is absent, so the nerve still records the student→CPU fallback. Presence is not speech.
- `galion-sideband` documents that C6 (the Open Mind memory server) is still open. This file is not that server.

D1 had not landed on `main` when this branch was cut. The blocked fallback is therefore the lane reply that is already on `8980e1a`, not a sentence D1 has not written. No student label was added.

The flag defaults off so a turn that did not ask for the seam keeps today's `turn_wav` path. Set the flag when a store or a `.gq` is actually on disk.

## Tests

`cargo +1.85.0`, 2026-11-15 22:13 +01:00, this host. Exit 0 for each command.

| Command | Result |
|---|---|
| `cargo +1.85.0 test -p galion-openmind` | 32 passed, 0 failed (lib 9, `from_env` 1, `handoff` 4, `note_alloc` 1, nerve `seam.rs` 5, `turn_seam.rs` 12) |
| `cargo +1.85.0 test -p galion-sideband` | 9 passed, 0 failed (client 3, lang 5, types 1) |
| `cargo +1.85.0 test -p galion-console --features phone-turn --lib` | 89 passed, 0 failed, including `openmind_seam_blocks_when_the_store_is_missing_and_witnesses_when_it_exists` |
| `scripts/check-locks.sh` | `OK: all Eureka locks hold (LOCKS.toml)`, exit 0 |
| `scripts/check-no-forbidden.sh` | `OK: forbidden-format + forbidden-runtime gates pass`, exit 0 |

The phone-worker seam test sets `GALION_SPEECH=off`. With no store, stages are `seam=blocked:store+weights memory=blocked:store fallback=canned` and the reply text is empty. With a temp directory the test created, turn 1 stores `the lamp is on` and turn 2's stages contain `memory=hit:1`. The file is exactly:

```text
v1	1	user	the lamp is on
v1	2	user	hello
```

The mouth on that run is the tone (`NoSpeech`). The stored sentence was not played. That is not a spoken-memory pass.

`cargo test --workspace` — not run. `scripts/eval/virtual-user.sh` — not run. `scripts/symphony/self-eval.sh` — not run (D3).

## CI

Not run at the time this note was written. The pull request's `gates` job is the run that counts. This note will not be edited to invent a green check.

## Not done

- No deploy. `/opt/galion/openmind` was not created.
- No clone of `obsidian-open-mind`. No git dependency on it.
- No weight download. No paid API. No cloud LLM. No Qwen / Groq.
- `LOCKS.toml`, `SOUL.md`, `config.yaml`, `scripts/clone/REFS.sha256` untouched.
- No WAV committed. The temp `turns.jsonl` files stay under the system temp dir and are deleted by the tests.
- D1 files (`reflect.rs`, `v2_persona.rs`) not edited. D3 ledger not edited. D4 briefing not edited. `PLAN.md` not edited.
- #78, #83, #86, #87, #97, #145, #149 not touched.

## Decisions for Maciej

1. This host has no Open Mind store and no student `.gq`. Later-turn memory stays BLOCKED until those files exist. Do not treat a unit-test temp file as a product memory.
2. `GALION_OPENMIND_SEAM` defaults off. Turn it on only when you want the worker to consult the seam. The blocked path keeps the phone lane's own reply.
3. `remember` defaults deny. A store on disk is not spoken back until `GALION_OPENMIND_REMEMBER` is granted.
4. A non-empty `.gq` is a file check here. It does not speak. D1's readiness probe is the place that may open it. This PR will not call that file `student-gq`.
5. C6, the Open Mind memory server, is still open. This seam is a local line file, not that server.
6. Merge order in the plan puts this PR after the brain PR. If D1's blocked sentence should replace `CannedLlm` on the seam's blocked path, that is a follow-up after D1 is on `main`. This branch does not guess that sentence.
