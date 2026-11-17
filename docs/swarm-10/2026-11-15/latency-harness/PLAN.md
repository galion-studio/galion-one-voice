# Swarm 10 — latency-harness plan

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-15 22:05 +01:00.

The spelling is **Galion**. The product is Galion Voice.

Lane slug: `latency-harness` (Lane B). Owner: Maciej Grajczyk. Repo: `galion-studio/galion-one-voice`.

| | |
|---|---|
| Baseline | `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` (merge of #144) |
| Plan branch | `cursor/latency-harness-plan-68d6` |
| Orchestrator | Swarm 10 latency-harness orchestrator, run `bc-60b588a5-5c71-5db6-8a3b-ac7e5afc68d6` |
| Sources | `docs/swarm-10` briefs and backlog as given at launch; local untracked `NEXT-ITERATION.md` (2026-11-15 21:56 +01:00, host `grok-bot-vm-511777615`). That file is cited. It is not committed. |
| This file | The only file on the plan PR. Workers do not edit it. |

Worker branch names below are the contract. The orchestrator fills the actual ref and PR number on this file when each PR opens. At 22:06 +01:00 no Lane B worker PR was open.

## What this lane owns

The hearable-latency bar and the 80 ms heartbeat evidence on the current tip.

| Role | Backlog | PR title | Notes file | Done when |
|---|---|---|---|---|
| ttfa | B1 | `Swarm 10 latency-harness ttfa: <short goal>` | `docs/swarm-10/2026-11-15/latency-harness/ttfa.md` | Piper receipt preserved and labelled Piper. No Nano claim. |
| heartbeat | B2 | `Swarm 10 latency-harness heartbeat: <short goal>` | `docs/swarm-10/2026-11-15/latency-harness/heartbeat.md` | Current-tip `verdict=` line from `scripts/eval/symphony-heartbeat.sh`, plus the heartbeat crate result. 80 ms, allocs=0, late=pause, or an honest FAIL/BLOCKED. |
| stream-clone | B3 | `Swarm 10 latency-harness stream-clone: <short goal>` | `docs/swarm-10/2026-11-15/latency-harness/stream-clone.md` | Measured Nano original-ref first-PCM / `ttfa_ms`, or BLOCKED on A0/A1. |
| one-percent | B4 | `Swarm 10 latency-harness one-percent: <short goal>` | `docs/swarm-10/2026-11-15/latency-harness/one-percent.md` | p99 from `galion-gates/op-one-percent.sh` on the worker host. `product_claim` only if that run earned it. |

First reading: [TTFA.md](../../../galion-voice-v2/symphony/TTFA.md), [PIPER-STREAM-TTFA.md](../../../galion-voice-v2/PIPER-STREAM-TTFA.md), [80MS.md](../../../galion-voice-v2/symphony/80MS.md), [HEARTBEAT.md](../../../galion-voice-v2/symphony/HEARTBEAT.md), [STREAM-TTFA.md](../../../galion-voice-v2/STREAM-TTFA.md), [ONE-PERCENT.md](../../../galion-voice-v2/symphony/ONE-PERCENT.md).

## What this lane does not own

- A0 (t273 vs t623u) and A1 (Nano PCM). Lane A. B3 waits on both. Do not flip `LOCKS.toml`, `scripts/clone/REFS.sha256`, or the voice TOMLs.
- A5, the seven `galion-ml-voice` failures when t273 wavs or pinned weights sit on disk. Do not delete operator files, commit WAVs, or rewrite tests to hide them.
- Self-eval ledger rows Lane D is updating. Put Lane B evidence in the role notes. Touch `SELF-EVAL.md` only for a row this worker actually ran, and stop if another open PR already edits that file.
- Emotion V2, One-app / Research-desk, payments, V3 code, deploys, parked PRs #78 and #83, and open drafts #145, #149, #97, #86, #87.
- Locked files: `SOUL.md`, `config.yaml`, `LOCKS.toml`. No edits to `voices/` weights, `weights/`, or `runtimes/` binaries. Do not flip `reference_clips`, `ref_clip_decision`, `paid_tiers`, `chrome_allowed`, or the 80 ms clock.

## Roster

```
latency-harness: O bc-60b588a5-5c71-5db6-8a3b-ac7e5afc68d6 | ttfa bc-38462fe4-ceba-5f61-bcd3-ef42058ad251 | heartbeat bc-73380dfd-7f66-5df8-93b3-6043631005c6 | stream-clone bc-821103fb-65d5-5676-a3f3-0d5bb951533b | one-percent bc-917d0c77-dac6-5a91-896f-3cdc1103b942
```

Read 2026-11-15 22:06 +01:00 from the cloud-agent list. All four were `RUNNING` with no branch yet. Lane A orchestrator is `bc-e03b289a-1b1e-5e90-a8d6-722adad494c0`. Lane A plan PR was not open at that read (open Swarm 10 plans: #150 persona-v2v, #151 this file). Identity for review stays the PR title.

## Worker branches

Base every branch on current `origin/main`. One concern per PR. Draft until the worker's own local tests have been re-run and the notes file quotes what those commands printed.

| Order | Role | Branch | Status at 23:00 +01:00 |
|---|---|---|---|
| 1 | ttfa | `cursor/latency-harness-ttfa-d251` | Merged. [#157](https://github.com/galion-studio/galion-one-voice/pull/157) head `46087e9`, merge commit `2ecca8645adb1bf069af912cb7066334f97d1b3c`. Piper only: cited cold 398 ms / warm 20 ms, this host cold 339 ms / warm 21 ms. |
| 2 | heartbeat | `cursor/latency-harness-heartbeat-05c6` | Merged. [#154](https://github.com/galion-studio/galion-one-voice/pull/154) head `ada0a92`, merge commit `c33dd4a681fc1d0e2eeff2dc7116cd9439a4ded2`. Harness `verdict=FAIL reason=stub-frames-are-not-a-voice`. Separate unit line `frame_ms=80 allocs=0 late=pause`. |
| 3 | one-percent | `cursor/latency-harness-one-percent-b942` | Merged. [#155](https://github.com/galion-studio/galion-one-voice/pull/155) head `d450bb3`, merge commit `8a629a328447332dd597c3dd055b6f97b798997c`. Notes quote `p99=1438ns`, `product_claim=false`. Not an EPYC, CX23, or CX43 claim. |
| 4 | stream-clone | `cursor/latency-harness-stream-clone-533b` | Merged. [#164](https://github.com/galion-studio/galion-one-voice/pull/164) head `c4c35dd`, merge commit `dec55d572a65f0f98b5d9460152d4ccd724ffc9c`. Re-run exit 2, `gate` `BLOCKED`, `claim_80ms` false, no `ttfa_ms`. A1 [#170](https://github.com/galion-studio/galion-one-voice/pull/170) is BLOCKED with no Nano PCM. Texture [#161](https://github.com/galion-studio/galion-one-voice/pull/161) is BLOCKED without the original clip. A0 remains BLOCKED-OWNER. |

Stay on that branch. Do not push to the plan branch. Do not push to `main`.

## Merge order

From the backlog suggested order, the part that applies to this lane:

1. **B1 ttfa.** File the Piper receipt as Piper-only. No wait on Nano.
2. **B2 heartbeat.** Remeasure the heartbeat harness on the worker host. No wait on Nano.
3. **B4 one-percent.** Remeasure p99 on the worker host. No wait on Nano.
4. **B3 stream-clone, last.** Waits on lane A: an owner decision for A0, then A1 real Nano PCM from the original t273 refs. Until both exist, B3's only acceptable landing is an honest BLOCKED note. It is not merged ahead of B1.

The orchestrator merges one PR at a time, through the normal merge API (merge commit, the same shape as #144). After each merge the next branch updates from `main` and CI runs again on that new head. Workers do not merge. No force-push, no history rewrite, no branch-protection or admin bypass, no skipped or disabled checks.

If B2 or B4 is green before B1 exists, they wait. If a PR is still a draft when its checks are green and the diff matches this plan, the orchestrator marks it ready and then merges. A draft is not merged while it is still a draft.

## Merge bar

`main` at `8980e1a` is not a protected branch (`protection.enabled` false, required status checks empty), read 2026-11-15 22:04 +01:00. That does not loosen the bar. A PR merges only when every check that actually ran on **that head** has conclusion `success`, and no required check was skipped to get there.

Checks that run on a pull request in this repo:

| Workflow | Job name seen on `8980e1a` | On a docs-only PR |
|---|---|---|
| `.github/workflows/gates.yml` | `test` | runs |
| `.github/workflows/one-desktop.yml` | `playwright` | runs |
| `.github/workflows/emotion-v2-ci.yml` | `emotion-v2 harness (CPU stub)` | runs |
| `.github/workflows/nebula-voice-v2.yml` | path-filtered | does not run unless those paths change; absence is not a failure |

On `8980e1a` those three jobs completed `success` (Actions runs 43660665688, 43660665724, 43660665722). CI on `main` was green before this lane. Do not "fix" a red check by skipping it. If `main` goes red later, say so with the run URL and do not bypass.

Pending, failure, cancelled, or timed-out checks block the merge. A skipped job that the workflow itself skipped by a path filter does not. `product_claim` inside a script is not a GitHub check.

Review is comment-only. The orchestrator rejects a diff that touches a locked file, commits a WAV or a weight, labels Piper or a protocol stub as the original clone, or files a stub clock as a product TTFA or a HARD-PASS.

## Rules for every worker

1. New branch from current `main`. Title as in the table. Notes file in the same PR as the change it describes.
2. Local tests re-run. Unrun = "not run". Unmeasured = "not measured". Quote the command and the lines it printed. Timestamps in Europe/Zagreb with offset +01:00. Name the host (`hostname`) and the commit.
3. NO LIES. Protocol-stub PCM, `GALION_CLONE_STUB_*`, and a 15 ms stub first-PCM are **FAIL** for product TTFA or HARD-PASS. Piper labelled as the original Samantha or Jarvis clone is a lie. Flite labelled as Piper is a lie. A whole-utterance time is not a first-PCM time.
4. CPU only. No GPU-mandatory path. No Chatterbox or PyTorch inside a Voice process. No GGUF in Voice crates. No paid model call, no Qwen, no Groq, no cloud LLM.
5. No tokens, keys, passwords, or server IPs in commits, PR text, or logs (redact IPv4). Never commit Samantha or Jarvis WAVs, CC0 donor WAVs, or clone weights. Do not delete gitignored wavs on a hash mismatch.
6. CI uses rustc 1.85.0 (`dtolnay/rust-toolchain@1.85.0`). `scripts/eval/symphony-heartbeat.sh` prefers `cargo +1.85.0`. Install that toolchain locally before calling the harness BLOCKED for a missing compiler. The plan host (below) is not the measurement host.
7. Copyright line is Maciej Grajczyk. Do not sign `ADMIN-UI-CHECK.md`. Do not sign a V3 start.

## B1 — ttfa

Scope: documentation and, when the pack is already on the worker host, a re-run. Keep Piper labelled Piper.

Cited from `NEXT-ITERATION.md` §4.3–§4.5. Measured 2026-11-15 on `grok-bot-vm-511777615`, commit `8980e1a`. **Not re-run by this orchestrator.** Not a clone of the originals.

Piper stream (`scripts/eval/piper-stream-ttfa.sh`, 21:53 +01:00, exit 0, wall 8.579 s):

```text
engine=piper voice=en_US-ljspeech-medium pack=present sha256=pinned
runtime=weights/runtime/piper/piper cpu_only=true measured=true stub=false product_claim=true
SpeechStack cold_first_pcm_ms=398 warm_first_pcm_ms=20 worker_reused=true piper_spawns=1
raw_spawn cold_spawn_first_pcm_byte_ms=363.8 warm_line_first_pcm_byte_ms=23.8
drain in_rate=22050 out_rate=24000 frame_samples=1920 tick_ms=80 frames=42 late_ticks=0 late_policy=pause audio_ms=3343.7
```

`product_claim=true` is for Piper `en_US-ljspeech-medium` only. The drain WAV stayed under `/tmp` and is not in git.

Virtual-user loopback (21:55 +01:00): `ttfa_ms=78`, `ttfa_status=measured`, `ttfa_kind=loopback-full-utterance`, `llm=canned`, `speech_backend=a21-weights`. That is canned plus Piper, end of inject until a reply WAV, not a streaming first frame and not a clone clock.

Default `scripts/eval/flash-ttfa.sh` on that run: `measured=false`, `product_claim=false`, no `ttfa_ms`.

B1 files that receipt in `ttfa.md` and updates the stale "this session" block in `docs/galion-voice-v2/symphony/TTFA.md` so the Piper line is Piper and the Nano product bar stays open. Do not copy 398 / 20 / 78 into a Nano or `chatterbox-nano` field. Do not set a measured-file `ttfa_ms` of 80 from these clocks. The 80 ms first-playable-frame bar applies only to a measured Nano file (B3).

Re-run `scripts/eval/piper-stream-ttfa.sh` and the default `scripts/eval/flash-ttfa.sh` when the A21 pack and runtime are already on the worker host. A new number from a different host is reported beside the cited receipt, with its own host, and still as Piper. If the pack is absent, write "not measured" for this host and keep the cited receipt. Do not download weights into git. A credential-free in-repo fetch that writes only gitignored paths is allowed; a failed fetch stays "not measured". No stub flag.

## B2 — heartbeat

Run on the worker host, rustc 1.85:

```bash
cargo test --offline -p galion-heartbeat
scripts/eval/symphony-heartbeat.sh
```

Paste the script's `verdict=` line into `heartbeat.md` with host, commit, and Europe/Zagreb time. The frame is 80 ms, 24 kHz, 1920 samples. A late frame pauses. The heartbeat hot path allocates nothing (`tests/zero_alloc.rs`). Do not edit `LOCKS.toml` or `FRAME_MS`.

The shell script prints `verdict=PASS` only when the heartbeat tests already printed `verdict=PASS`, the frame tests passed, and `piper_drain` printed `piper_measured=true`. Without the A21 pack the script ends `verdict=FAIL reason=stub-frames-are-not-a-voice`. That FAIL is the verdict. Quote it. The unit-test lines for allocs and late=pause may still be quoted from the log above that line. Do not patch the script so a missing pack becomes PASS. A Piper drain of 0 late ticks from B1 is the Piper path only; it is not this verdict.

`cargo` missing after a real attempt to use 1.85 is `verdict=BLOCKED reason=cargo-missing` (the script's own line). Do not invent a microsecond figure.

## B3 — stream-clone

Wait. Read the lane A plan PR (`Swarm 10 clone-compiler plan`) and the A1 PR before writing a number.

A0 is BLOCKED-OWNER. `LOCKS.toml` has `ref_clip_decision = "t623u"`. `REFS.sha256` and the voice TOMLs pin the t623u CC0 donors. Symphony landing still names the t273 originals. Tonight's in-tree generate, cited from `NEXT-ITERATION.md` §4.1, refused both personas before torch: exit 3, 0 PCM bytes, `galion-clone-refuse: reference sha256 is not allowlisted`. The orchestrator has not re-run that command. Independent gaps on that baseline host, also not re-checked here: no CPU torch/chatterbox venv, tokenizer tree missing under `weights/clone/tokenizer/`.

Until Maciej decides A0 and A1 has emitted real PCM (`engine` `chatterbox-nano` or `nano`, `stub=false`) from the original refs, B3 stays **BLOCKED**. The notes say so and name A0 and A1. They do not substitute Piper, Flite, or a protocol stub.

If A1 has landed:

- Log first-PCM / `galion-clone-first` and `ttfa_ms` with host, engine, both t273 sha256 values, `stub=false`.
- Claim the 80 ms first-playable frame only when that measured file has `ttfa_ms` ≤ 80. Otherwise report the real number.
- A whole-utterance-only figure ≥ 1.5 s is HARD-FAIL. The 2026-11-03 smoke (~1.8–2.4 s) does not clear the stream bar.
- No WAV in git. No `GALION_CLONE_STUB_PCM`. Do not add a ref-sha override that bypasses `REFS.sha256` or `LOCKS.toml`. `nano_stream.py` already has an operator override; using it to score t273 while the lock still says t623u is not an A0 decision. Do not do it.

## B4 — one-percent

Run on the worker host:

```bash
galion-gates/op-one-percent.sh
```

Log p99, the instrument, the CPU source, and `product_claim` exactly as the script printed them. The budget is 1% of 80 ms = 800 µs. `product_claim` stays false unless that run's own output sets it true. The gate doc says there is no code path that sets it true, and a release run on a build machine is not an EPYC, CX23, or CX43 sample. Say that when it is true of the worker host. Do not invent a CPU percent. Do not copy an older ledger row onto this host.

An older ledger line, not re-run here and not this plan host, is quoted only so it is not recycled by accident: `OP PASS — P1≤1% (ok) P3 p99=3206ns ratio=249 much_less=yes cpu=schedstat stub=8 measured=256 product_claim=false` in `SELF-EVAL.md`. B4's note carries the new run, or "not run" with the reason.

## Blockers

| Id | State at plan time | Effect on this lane |
|---|---|---|
| A0 t273 vs t623u | BLOCKED-OWNER. Agents do not flip the lock or `REFS.sha256`. | B3 cannot measure original-ref Nano. |
| A1 Nano PCM from the originals | On main as [#170](https://github.com/galion-studio/galion-one-voice/pull/170). Verdict BLOCKED. No Nano PCM. Wavs, pins, and tokenizer absent. A CPU venv was staged on that host only and was not committed. | B3 has no PCM to time. The harness on main exits 2. |
| Piper pack on the plan host | absent. `weights/` contains only `.gitignore`. | Orchestrator did not re-measure TTFA or the Piper half of the heartbeat script. Workers measure on their own host. |
| rustc on the plan host | 1.83.0. CI pin is 1.85.0. | Orchestrator did not run `cargo test` for this plan. |
| EPYC / CX23 / CX43 tick | not this class of machine | B4 must not call a local p99 a capacity-class product claim. |
| `gqvoice`, Vosk, student `.gq` | out of this lane | No action. |

## Decisions for Maciej

Record only. This lane does not settle them.

1. **A0.** Re-open the t273 originals on the live clone path (a lock-and-ledger edit Maciej makes), or state that `main` stays on t623u and the symphony B/C landing bar must be re-stated. The B3 harness is on main as BLOCKED. It does not label Piper as the clone.
2. Live `/one` stays Piper plus the Samantha persona until a clone mouth is actually live. B1's `product_claim` does not change that.
3. The 80 ms first-playable frame is not claimed from the Piper clocks (cold 398 ms, warm 20 ms, loopback 78 ms).

## Lane record

2026-11-15 23:00 +01:00. All four worker pull requests are on main. This plan file is the remaining draft.

| Item | Merge | What the notes actually say |
|---|---|---|
| B1 | `2ecca8645adb1bf069af912cb7066334f97d1b3c` | Piper only. Cited cold 398 ms / warm 20 ms. This host cold 339 ms / warm 21 ms. Loopback `ttfa_ms=78` is loopback, not Nano. |
| B2 | `c33dd4a681fc1d0e2eeff2dc7116cd9439a4ded2` | Harness `verdict=FAIL reason=stub-frames-are-not-a-voice` because the A21 pack was absent. Separate unit line `frame_ms=80 allocs=0 late=pause`. |
| B4 | `8a629a328447332dd597c3dd055b6f97b798997c` | `p99=1438ns`, `product_claim=false`. Not an EPYC, CX23, or CX43 claim. |
| B3 | `dec55d572a65f0f98b5d9460152d4ccd724ffc9c` | Exit 2, `gate` `BLOCKED`, no `ttfa_ms`, `claim_80ms` false. 80 ms is not claimed. |

## Plan host (not a measurement)

Checked while writing this file, 2026-11-15 22:04 +01:00. Hostname `cursor`. Linux. 4 CPUs. rustc 1.83.0. No `weights/tts`, no `weights/runtime/piper`. These facts are not a TTFA, not a heartbeat verdict, and not a p99.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
