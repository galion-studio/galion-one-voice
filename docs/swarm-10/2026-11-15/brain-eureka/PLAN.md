# Swarm 10 — Lane D plan (brain-eureka)

**Written:** 2026-11-15 22:04 +01:00 (Europe/Zagreb)
**Orchestrator:** Swarm 10 brain-eureka (`bc-86012b24-3104-5325-b690-7c6fed22b8c3`)
**Owner:** Maciej Grajczyk (galion-studio). Copyright line stays Maciej Grajczyk.
**Repo:** [galion-studio/galion-one-voice](https://github.com/galion-studio/galion-one-voice)
**Base:** `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` (merge of #144)
**This PR:** draft, this file only. Workers do not edit `PLAN.md`.

Sources followed, not committed: Swarm 10 briefs (common rules + Lane D), backlog §6 D1–D4 and §8, and the local untracked `NEXT-ITERATION.md` (2026-11-15 21:56 +01:00, host `grok-bot-vm-511777615`, same tip). Do not add that file to git.

## What this lane owns

Replace the instruction-floor pretence with a real in-house brain only as far as weights that already exist allow, and re-run Eureka / self-eval honestly on a cargo-capable host.

| Role | Backlog | Done when |
|---|---|---|
| brain | D1 | Student `.gq` path on `galion-tensor` when a file is present and a readiness test passes, or an honest blocked label plus a harness. Echo is not the phone default. `galion-reflect` is not described as the product brain. |
| seam | D2 | Local Open Mind / student seam tested. Missing store or weights is BLOCKED, not a fake memory or a fake thought trace. Hermes stays off the 80 ms tick. |
| self-eval | D3 | Ledger rows updated only from runs on the worker host and tip. Sections B and C close only on the original t273 refs. Otherwise they stay OPEN/BLOCKED and name A0. |
| clearance | D4 | Written t273 vs t623u briefing for Maciej. No consent fabrication. No `LOCKS.toml` or `REFS.sha256` edit. No merge of #78 or #83. |

Workers are launched separately. Their run ids are not in this plan. Find this PR by the title `Swarm 10 brain-eureka plan`. Stay in this lane. Do not edit open drafts #145, #149, #97, #86, #87, #78, #83.

## Worker PR contract

1. Branch from current `origin/main` (fetch first). Do not branch from this plan branch. Preferred name: `cursor/swarm10-brain-eureka-<role>-<your-suffix>`.
2. One draft PR, one concern. Title: `Swarm 10 brain-eureka <role>: <short goal>`. Roles are `brain`, `seam`, `self-eval`, `clearance`.
3. Notes file in the same PR: `docs/swarm-10/2026-11-15/brain-eureka/<role>.md` (`brain.md`, `seam.md`, `self-eval.md`, `clearance.md`). Write what you checked, what changed, tests with results, CI, and open questions. Timestamps in Europe/Zagreb with `+01:00`. Unrun stays "not run". Unmeasured stays "not measured".
4. Mark the PR ready for review when every required, non-skipped check is green on the head you want merged. Workers do not merge.
5. Update from `main` by merge. No force-push, no history rewrite. After the orchestrator merges another lane PR, update and let CI run again before the next merge.
6. Forbidden for every role: force-push; admin merge; skipping or disabling checks; deploys; server credentials; paid model calls; Qwen, Groq, or any cloud LLM; DNS / Cloudflare / HADES / firewall changes; editing `SOUL.md`, `config.yaml`, or `LOCKS.toml`; editing `scripts/clone/REFS.sha256`; flipping `reference_clips`, `ref_clip_decision`, `paid_tiers`, `chrome_allowed`, or the 80 ms clock; committing WAVs, clone weights, tokens, or IPv4 addresses; GGUF in Voice crates; GPU-mandatory paths; Chatterbox or PyTorch inside a Voice process; signing `ADMIN-UI-CHECK.md`; V3 implementation; Emotion V2 training; One-app / Research-desk feature work.

Phone default stays `CannedLlm`. `GALION_LLM=echo` stays a test mode. Hermes stays the side band. Piper is not a clone of the originals. Protocol-stub PCM and `GALION_CLONE_STUB_*` are FAIL for a product or HARD-PASS claim.

## Merge order

From backlog §8 as it applies to this lane. The orchestrator merges one PR at a time, only when every required non-skipped check is green on that PR's current head, through the normal merge API (merge commit, the same form as #144). No bypass.

1. **self-eval (D3).** Remeasure what this host can run without Nano. Do not wait for A0, A1, or the brain PR. Sections B and C ship OPEN/BLOCKED.
2. **clearance (D4).** Docs only, after D3, so it does not race the self-eval ledger edit. D4 does not edit `SELF-EVAL.md`.
3. **brain (D1).**
4. **seam (D2).** After D1, so the fallback names the same blocked student state. If D1 is still open when D2 is green, D2 may merge against current main and must not invent a student label D1 has not defined. The orchestrator will ask for a main merge and a fresh CI run before each merge.

File ownership, so the four PRs do not collide:

| Path | Owner |
|---|---|
| `docs/swarm-10/2026-11-15/brain-eureka/PLAN.md` | orchestrator |
| `docs/swarm-10/2026-11-15/brain-eureka/self-eval.md` and the self-eval ledger rows you actually ran in `docs/galion-voice-v2/symphony/SELF-EVAL.md` | self-eval |
| `docs/swarm-10/2026-11-15/brain-eureka/clearance.md` | clearance |
| `crates/galion-cascade/src/reflect.rs`, the persona reply path in `galion-render/src/v2_persona.rs`, and a new S0 `.gq` readiness probe | brain |
| `crates/galion-openmind`, `crates/galion-sideband` | seam |

Do not edit another role's notes file.

## Checks that must be green

On a pull request, GitHub runs `gates`, `emotion-v2`, and `one-desktop`. `nebula-voice-v2` runs only when its path filter matches; a skip from that filter is not a failure. A red required check blocks the merge. A check that was already red on `main` before the branch is reported with the run URL and is not "fixed" by skipping it.

`gates` (`.github/workflows/gates.yml`) includes `scripts/check-locks.sh`, `scripts/check-no-forbidden.sh`, `cargo test --workspace`, the emotion-v2 refcheck, `galion-gates/op-one-percent.sh`, `cargo test -p galion-ml-voice --features speech`, flash TTFA, the hard-loop self-check, Jarvis texture, clone-stream TTFA (exit 2 is allowed only as the workflow already writes it: not a HARD-PASS), `galion-render` web, `galion-cascade` cascade, `galion-console` phone-turn, and `scripts/eval/virtual-user.sh`. CI uses Rust 1.85. This orchestrator VM had rustc 1.83.0 at 22:04 +01:00; match CI's 1.85 when you compile. Do not commit a `rust-toolchain` file to paper over that.

`scripts/symphony/self-eval.sh` is not a GitHub job. D3 runs it locally and pastes the result. Exit 1 while B or C is open is expected and must be reported. Exit 2 means `LOCKS.toml` changed during the run; that is a failed run.

## D1 — brain

Read `crates/galion-cascade/src/reflect.rs`, `GALION-VOICE-V2.md` V2-S5 and V2-S9, `galion-student/README.md`, `galion-student/configs/s0.toml`, `crates/galion-student-backend/src/lib.rs`, `galion-render/src/v2_persona.rs` (`SayWork::model_reply`), and `scripts/eval/v2_hard_loop.py` `classify_llm`.

Already true on `8980e1a`:

- Phone default is `CannedLlm` (`LlmMode` unset or `canned`). Echo is `GALION_LLM=echo`. Hermes is a loopback client and is not the phone default.
- `reflect::reply` is a rule engine. `LLM_NAME` is `galion-reflect`. The module says it is not a language model. `classify_llm` already marks that text `real-llm: stub`.
- The live pretence is `SayWork::model_reply`: with no Hermes client and `inhouse`, it returns `reflect::reply`, which restates the caller's sentence in second person. `Floor::new` labels that path `galion-reflect` when `GALION_LLM` is `canned`, `inhouse`, or `reflect`.
- `galion-student/` is config stubs. No student `.gq` is in git. `load_slot` returns `CounselPending` or `NotPlaced` and does not mmap `.gq`. `galion-tensor` `GqFile` can open a `.gq`.
- Owner ruling 2026-11-04: no Qwen, no Groq, no cloud LLM. The S0 teacher source is an owner decision. Do not pick one and do not download one. The `kyutai/moshiko` string already in `s0.toml` is not a licence to fetch it.

Land this:

- A readiness probe for an S0 student `.gq` on a documented gitignored path. It uses `GqFile` only when the file is present. It never downloads, never writes a weight, and never treats an empty or missing file as loaded.
- When the file is absent, a content turn that today would speak `reflect::reply` speaks an explicit blocked/unavailable line that names the missing student `.gq`. The `"llm"` label on that turn must not be `galion-reflect` and must not be `student-gq`. `classify_llm` must keep that turn off `real-llm: met`.
- When a `.gq` opens and there is still no decode/speak path, the line stays blocked ("opened, cannot speak"). Do not put reflect text under a student label.
- `reflect::reply` stays available as the instruction floor for its unit tests. Do not delete it and do not relabel it as a model.
- `MockLlm` / `you said:` stays off the phone default.
- Do not route the phone through Hermes to make `real-llm` look met. Hermes `met` in the hard loop is a side-band classification, not this lane's brain.

Tests to run and paste: `cargo test -p galion-cascade --features cascade --lib`, the `galion-render` tests that cover `v2_persona`, and a new test that a missing `.gq` does not return the second-person reflection of the user sentence. `cargo test --workspace` if you can. Weights on this orchestrator VM at plan time: absent. Say so if your host matches.

## D2 — seam

Read `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md`, `crates/galion-openmind`, `crates/galion-sideband`.

Already true: nerve events and cortex handoff exist; the 3 s retrieval cap exists; money intents are not offered; `Heartbeat::step` does not call them; `CortexHandoff::local()` does not dial a gateway. `/opt/galion/openmind` is not something this repo creates.

Land a tested local seam:

- A voice turn can take a local Open Mind / student path when that path is present.
- When the store or the weights are absent, the fallback is the honest phone reply (`CannedLlm` or the D1 blocked/unavailable line). No fabricated thought trace, no fabricated later-turn memory.
- Hermes remains side-band only. No Hermes call inside `Heartbeat::step`. No new Research-desk UI.
- A later-turn memory witness is in scope only if a local store exists on the worker host. If it does not, write BLOCKED and name the missing piece. Do not clone `obsidian-open-mind`. Do not deploy to `/opt/galion/openmind`. Do not add a git dependency on that repo.

Tests: `cargo test -p galion-openmind`, `cargo test -p galion-sideband`, plus `scripts/check-locks.sh` and `scripts/check-no-forbidden.sh`.

## D3 — self-eval

Read `scripts/symphony/self-eval.sh`, `scripts/symphony/eval.py` (`gate_b`, `gate_c`), `docs/galion-voice-v2/symphony/SELF-EVAL.md`, `EUREKA.md`, `GATES.md`, and NEXT-ITERATION gap 3.

Run `bash scripts/symphony/self-eval.sh` on your host. The script rewrites the matrix between `SYMPHONY-MATRIX-START` and `SYMPHONY-MATRIX-END` and the last-run block. Review that diff before you commit.

Swarm 10 bar for B and C, which is stricter than a green script:

- **B closes only** with a smoke-speak log from this host that names both original t273 sha256 values: Samantha `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693`, Jarvis `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60`. Protocol-stub PCM is not that log. Piper is not that log. The t623u CC0 donors are not those refs.
- **C closes only** on that original Jarvis clip: hub texture kept (`preserve-hub`), `GALION_JARVIS_TEXTURE` as a layer toggle that does not denoise the asset, and a spectral check against a clean-speech baseline, with command and host. A synthetic tone self-test is not that hearing. Do not add `[texture] preserve` to `jarvis.toml` to chase the gate. Texture policy is lane A3. On `8980e1a` the texture keys live under `[reference]`, and `gate_c` looks for a `[texture]` table, so the script leaves C OPEN. Keep it OPEN/BLOCKED.
- `eval.py` currently compares wavs to the **t623u** pins (`e4bc60c33f697108cf5585dead213da1065979010f5be1d513574893584a4641` and `60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6`). Those are the active rows in `scripts/clone/REFS.sha256`. The prose in SELF-EVAL "what closes a row", `CLONE-LANDING.md`, and the symphony README still names the t273 hashes. Do not edit `eval.py`'s hash constants, `REFS.sha256`, or `LOCKS.toml` to force either side. If the script would PASS B on the t623u hashes, on a skip, or on a stub, change the committed B row back to OPEN/BLOCKED, quote the script line, and name A0. Record the disagreement. Do not resolve it.

Update only rows you actually ran, with a Europe/Zagreb timestamp and the evidence path. Leave unrun rows as they were. Cite other lanes' numbers without rewriting them. If lane A has not put original-ref smoke on `main`, B and C stay OPEN/BLOCKED. Script exit 1 is the honest result.

At plan time on this orchestrator VM the ref directory held `samantha.placeholder` and `jarvis.placeholder` only, and `weights/clone` was absent. That is this VM, not the NEXT-ITERATION host. Re-check. Do not copy the NEXT-ITERATION Piper clocks (cold 398 ms / warm 20 ms, loopback 78 ms, `llm=canned`) into a clone or brain claim.

Also run, if cargo is available: the pieces the ledger rows you touch require. Do not claim `cargo test --workspace` unless you ran it. The 2026-11-15 NEXT-ITERATION workspace total on the other host was 842 passed, 7 failed, 5 ignored, all seven in `galion-ml-voice`. Those seven are lane A5. Do not "fix" them here by deleting refs or weights or by flipping pins. If you hit them, quote them and leave them to lane A.

## D4 — clearance

Read `docs/galion-voice-v2/RIGHTS-CLEARANCE.md`, `LOCKS.toml` `ref_clip_decision` (read only), `scripts/clone/REFS.sha256` (read only), `LEDGER.md` t273 line (2026-11-03 10:37 CET), symphony README reference table, and parked PRs #78 and #83.

Write the briefing in `clearance.md`. Do not tick a box in `RIGHTS-CLEARANCE.md`. Do not paste a LEDGER template that still has a field, and do not invent counsel, a rightsholder, a date, or a sha256. The t273 digests above are already in the tree; citing them is not inventing them.

The briefing states, without choosing:

1. **A0 is BLOCKED-OWNER.** Either (a) Maciej re-opens the t273 originals on the live clone path by editing `LOCKS.toml` and `REFS.sha256` himself, or (b) main stays on the t623u CC0 donors (Amy Koenig / Alan Davis Drake) and symphony B and C are re-stated off the t273 hashes. Agents do not do either edit.
2. **In force on `8980e1a`:** `ref_clip_decision = "t623u"`. Active `REFS.sha256` rows are the t623u hashes. The t273 rows are commented with "do not re-enable them." `reference_clips` stays as the lock file already has it. RIGHTS-CLEARANCE boxes stay unchecked. The 2026-11-03 LEDGER line records private local-send clearance for the t273 wavs and says public demo stays closed. That line is not a lock flip and not a film-voice claim.
3. Product voices stay Galion One and Galion Research. Test ids `samantha` and `jarvis` are INTERNAL registers. No PR in this lane claims a public film voice. No new celebrity clip. Originals, if a worker hashes them, stay gitignored.
4. #78 and #83 stay parked. Do not merge them and do not push to `swarm/samantha-exp` or `swarm/piper-cc0-finetune`.

## Blockers

| Blocker | Who moves it | Lane D behaviour |
|---|---|---|
| A0 t273 vs t623u | Maciej | Record it in D4. Do not edit locks or REFS. D3 leaves B and C OPEN/BLOCKED. |
| No S0 student `.gq` on disk, teacher source unchosen | Maciej | D1 ships the blocked harness. No weight download. |
| No local Open Mind store (`/opt/galion/openmind` absent) | nobody in this swarm | D2 writes BLOCKED for later-turn memory. No deploy. |
| Original wavs and clone weights are not on this orchestrator VM (placeholders only, weights absent) | operator files, gitignored | Do not invent PCM. Do not commit wavs. Lane A owns generate. |
| Self-eval B/C cannot close until A1/A3 produce original-ref evidence | lane A, after A0 | D3 does not wait to merge, and does not close the rows. |
| Seven `galion-ml-voice` failures on a host that has t273 wavs or weights | lane A (A5) | Do not fix them in this lane. |

## Decisions left for Maciej

Record only. No merge in this lane settles these.

1. A0 — re-open t273 on the live clone path, or keep t623u and re-state symphony B and C.
2. S0 teacher / training source. No third-party model is assumed.
3. Admin UI sign-off, after he has heard live generations. Agents do not sign.
4. When, if ever, the live mouth leaves Piper. This lane does not switch it.
5. Merge or continued park of #83. Continued park from this lane.
6. V3 start. Out of scope.
7. Workspace quota / Ultra disk. Leave it alone.

## Orchestrator after this PR

Review each worker PR. Merge in the order above, one at a time, only on a green current head, then ask the next branch to merge `main` and re-run CI. Comment-only review. No lock edits. No deploys.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
