# HORBM 100% plan C: evals and machines

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Agent | C of 3. Planning only. This file does not change product code, manifests, or locks. |
| Base read | `main` `9fa188d551eef902428dd006b07f6c45480340b7` (merge of #389). `git rev-parse` on this checkout. |
| Page | Owner progress text in the task: `horbm.json`, synced 27 Nov 2026 15:35 Europe/Zagreb. That file is not in this repository. Status A means Accepted, not Built. |
| Parts | (76) (77) (79) (80) (83) (84) (85) (91). |
| Locks | Read, not edited. `LOCKS.toml` `[horbm]` `hot_path = "retrieval"`, `student_on_tick = false`. |

`Drop? (owner decides)` means this part cannot become Built without breaking a ban already in the tree, or it is a fact or a process rather than a missing program. Removing it from the 66 is the owner's choice. This pass did not find `sync-progress.mjs` in `galion-one-voice` or in a GitHub code search of `org:galion-studio` for that filename. The claim that only a Dropped chip leaves the count is therefore unverified here. If that claim is true, Accepted and Later stay in the denominator until the owner marks Dropped or a build lands.

A claim below is either a file this pass read, a pull request the GitHub API returned, or a licence file fetched from the repository named. Where a check was not done, the line says so.

## Layer 10 — evals

The layer note on the progress page says no quality-turn evals were found. What is on main is a closed turn-taking gate, not a return rate and not conversation-turns per session.

`horbm/crates/horbm-eval` scores pause takeover, turn-taking latency, backchannel, interruption stop, overlap, and false barge-in on a virtual 80 ms clock (`src/turn.rs`, `src/lib.rs`). Latency is scored on trials where the speaker answers (`docs/horbm/foundations/F-turn-eval-s43.md`). Pause takeover and backchannel stay ungated (`PAUSE_TOR_P95_MAX` and the backchannel limits are `None` in `src/turn.rs`). That is timing quality. It is not parts 76, 77, or 80.

### (76) Earned return: comes back by choice within 7 days

**1. First principles.** Not needed for a great CPU conversationalist now. "Comes back within 7 days" is a return rate. Domain 08, in this repo, excludes it in writing: "Explicitly excluded: session duration, turns per session, return rate, any engagement score" (`docs/research/galion-voice-v2/domains/08-evaluation.md`, Maciej Grajczyk, 2026-11-03). The presence harness rejects the word `retention` in a metric, a spec key, or an assertion (`crates/galion-presence/src/spec.rs` `FORBIDDEN`; `crates/galion-presence/tests/replay.rs` poisons `user_retention` and expects the word `retention` in the error). R4 §12 puts the same ban on the HORBM eval: session length, retention, and affect-steering must be absent from the score file (closed draft #207, commit `dd902bbcfe34e23f63745a19fc2fed8e468a3b09`, file `docs/horbm/research/R4-cpu-realtime-integration.md`; that pull request is closed and unmerged). A score that proves "came back within 7 days" would fail the harness that already exists. **Drop? (owner decides).**

**2. Reuse search.** GitHub repository search `retention analytics language:Rust license:mit` returned one repository: [ForgeRewardz/solscope](https://github.com/ForgeRewardz/solscope). Its `LICENSE` file is the MIT licence, copyright 2026 Rewardz Protocol. The repository description is Solana wallet analytics, including retention. It does not fit: it is chain analytics, and adopting a retention counter would trip `FORBIDDEN`. Full-Duplex-Bench was checked because the turn eval already follows its metric names. [DanielLin94144/Full-Duplex-Bench](https://github.com/DanielLin94144/Full-Duplex-Bench) `LICENSE` is Creative Commons Attribution-NonCommercial 4.0. That is not MIT or Apache-2.0. Do not copy that code. The in-tree scorer stays our own text. A second search, `dialogue task success language:Rust license:mit`, returned no repositories. A Linux package index was not queried. I don't know of a distro package for this.

**3. Already in this repo.** No return-rate counter. `rg` over this tree for an earned-return scorer found the denylist, not a 7-day metric. `horbm-eval` does not name retention.

**4. Smallest build.** None. Do not add a 7-day counter, a cohort table, or an analytics client.

**5. Acceptance gate.** There is no passing test that should exist. The existing proof that the ban holds is `cargo test -p galion-presence` on the poisoned-key test in `crates/galion-presence/tests/replay.rs`. This plan did not run that test.

**6. Swarm.** No build swarm. Owner marks Dropped, or the part stays Accepted and keeps counting.

### (77) Quality turns: CPS that counts only real turns

**1. First principles.** The name on the page is CPS. R4 §1, same unmerged note, says XiaoIce's reported product score is conversation-turns per session, that this is an engagement metric, and that "HORBM does not adopt CPS." This pass did not re-read the XiaoIce paper. The exclusion used here is the sentence in that note plus domain 08's "turns per session" line, which was read. "Counts only real turns" does not take CPS out of that exclusion: it is still turns per session. A filler-aware quality clock is a different metric. Domain 08 calls that TTFCW (time to first content word) and says a "Hmm" must not count. `horbm-eval` already drops unanswered trials out of the latency score. That is the real-turn filter for timing. It is not a session tally. **Drop? (owner decides)** for CPS as named. Do not replace it with a hidden turns-per-session counter.

**2. Reuse search.** Nothing to vendor. The same Full-Duplex-Bench repository is CC BY-NC 4.0, so its code stays out. Search `mean opinion score blind rating language:Rust license:apache-2.0` was for part 80 and is recorded there. No MIT/Apache CPS library was selected, because selecting one would implement the banned score.

**3. Already in this repo.** Turn-taking scores: `horbm/crates/horbm-eval/src/turn.rs`. Grounding and unknown-reply checks: `horbm/crates/horbm-eval/src/score.rs`, thresholds in `thresholds.toml` via `src/thresholds.rs`. None of these is CPS. The progress page's "no quality-turn evals" matches this reading: there is no scorer whose pass condition is conversation-turns per session.

**4. Smallest build.** None for CPS. TTFCW, if the owner wants that name later, is a timing measurement on the existing virtual clock (first content frame, not a session count). It is not this part. It is not started here.

**5. Acceptance gate.** No new gate. Shipping CPS would be a failed presence file, same denylist as (76).

**6. Swarm.** No build swarm.

### (79) Good endings: calls end with the want met

**1. First principles.** Needed now, and it is small. A conversationalist that tracks an open want and never notices the ending is unfinished. This is task success, which domain 08 allows as "required facts present and forbidden phrases absent," not a return rate and not turns per session. The tracker already runs on a normal turn. The missing piece is a check that the call ended with that want met or explicitly dropped.

**2. Reuse search.** GitHub repository search `dialogue task success language:Rust license:mit` returned zero repositories. Nothing external to add. The code to reuse is already in this tree: `voice-stack/empathy` `WantList` (merged #238, `2aa633c6cbf76dc51ef1f82b2e35d84b98d685c6`). Authors: Galion Labs, MIT, `voice-stack/empathy/src/wants.rs`. Do not add a dependency.

**3. Already in this repo.** Read on this base.

- `WantState` is `Open`, `Met`, `Dropped`. The list is in memory for the process. Text is the caller's words and is not spoken (`voice-stack/empathy/src/wants.rs`).
- A final line opens on `i want `, `i need `, or `i wish `. Met phrases include `that works` and `that's what i wanted`. Drop phrases include `never mind` and `cancel that`. Goodbye is not in either list.
- `Empathy::tick` calls `wants.observe` when `asr_final` is set (`voice-stack/empathy/src/layer.rs`).
- `Brain::turn_with_ear` calls that tick with `asr_final: !heard.is_empty()` (`horbm/crates/horbm/src/brain.rs`). So a non-empty turn does update the list.
- `prepare_lookup` returns `open`, `met`, and `dropped` (`LookupPrep` in `layer.rs`). `turn_with_ear` keeps `prep.query` and does not read the counts. `empathy` on `Brain` is private. No function under `horbm/crates/horbm/src` calls `view()` or `wants()`.
- `meta_of` copies mood and prosody and drops the want counts.
- Inner-life "wants" (`Brain::inner_turn`, four token slots) are a different store. They are not `WantState::Met`.
- Unit coverage of the phrase table is `voice-stack/empathy/src/wants.rs` tests and `voice-stack/empathy/tests/signals.rs` (`mark_want` to `Met`). There is no test that a `Brain` dialogue ends with `open == 0`.

**4. Smallest build.** One accessor and one fixture test. No new crate, no network, no audio, no lock edit.

- Add a method on `Brain` that returns the three counts from `Empathy::view`. It does not speak them and does not write the diary.
- Fixture A: `turn("I want the gate checked")` then `turn("that works")`. Expect `met >= 1` and `open == 0`.
- Fixture B: `turn("I want tea")` and no met phrase. Expect `open >= 1`. That script is the not-met ending. The test must be able to see it. Do not paper over it with a stock comfort line.
- A dropped want (`never mind`) is a closed ending of a different kind. Count it as not open. Do not call it met.
- Leave `voice-stack/local-server` alone. The live process is not this gate.

**5. Acceptance gate.** On the laptop, rustc 1.85 as the tree's CI comment states (`crates/horbm-device/Cargo.toml`; this pass did not print `rustc -V`):

```text
cargo test --manifest-path horbm/Cargo.toml -p horbm --test want_end -- --nocapture
```

The test name is the one the build adds. Pass is fixture A `open=0` and fixture B `open>=1`, printed. Then the existing brain commands in `docs/horbm/PLAN-brain.md`: `cargo test --manifest-path horbm/Cargo.toml --workspace` and clippy `-D warnings`. Do not raise `HEAP_GROWTH_KIB` (128, live heap bytes). This plan did not run these commands.

**6. Swarm.** Swarm C1, two agents, one PR, after this plan. Agent 1 owns the accessor on `Brain`. Agent 2 owns `horbm/crates/horbm/tests/want_end.rs`. No overlap with swarm C2. Depends on #238, which is already merged. Does not depend on a drop decision.

### (80) Feels heard: blind human ratings

**1. First principles.** Needed before anyone can claim the machine feels like it listened. A CPU fixture cannot be that claim. It is not needed to make the current brain work, and R4 §12 says human MOS stays out of phase 9 until a listener protocol exists: "This note does not pick a rater." Domain 08 T4 is "20+ raters, MOS and CMOS against the previous release." No such sheet is in this tree. **Later.** This is not forbidden by `FORBIDDEN` (those words are engagement, retention, session length, time in session, dwell, speech elicited). It also cannot become Built on a cargo gate alone. If the unverified chip rule is real, the percentage does not move until a study lands or the owner marks Dropped.

**2. Reuse search.** GitHub repository search `mean opinion score blind rating language:Rust license:apache-2.0` returned zero repositories. Nothing found that runs a blind panel on the laptop. UTMOS is named in domain 08 and in R4 §12 as a later proxy that needs a licensed voice. This pass did not open an UTMOS repository and does not pick one. Full-Duplex-Bench's code is CC BY-NC 4.0 and is not a rater.

**3. Already in this repo.** `attunement` is an automatic affect score, not a human rating. `crates/galion-presence/src/replay.rs` sets it only when sense and adapt are both on, as the minimum of a non-escalating share and a response share. `spec.rs` says attunement replaces engagement. That is not "feels heard." `horbm-eval` has no rater id, no MOS field, and no blind protocol. I don't know of a sealed rating file outside this repository.

**4. Smallest build.** None now. A later build, after the owner writes the protocol, is a reader for a rating sheet the owner collected: rater count, MOS, the commit rated, and a denylist check that the sheet has none of the `FORBIDDEN` words. The sheet is not generated by the agent. Do not invent raters or scores.

**5. Acceptance gate.** Not a cargo pass today. When a protocol exists, the laptop gate is: the sheet's commit equals the commit under test, rater count is the number the protocol requires, and `galion-presence` rejects the sheet if a forbidden key appears. This pass saw no sheet, so there is no number.

**6. Swarm.** No swarm until the protocol exists. Do not put (80) in the same PR as (79).

## Layer 11 — machines

### (83) galion-boot: hardware probe; CPU-only is local-only

**1. First principles.** Needed. A CPU box must stay on local retrieval. The probe that exists today does the opposite for the desk model: the last matching rule is `cpu-only`, and it selects `Llm::Api` (`crates/openmind-survival/src/degrade.rs` in [galion-studio/obsidian-open-mind](https://github.com/galion-studio/obsidian-open-mind), read from the default branch during this pass; `LINE_CPU` is "CPU only. Hermes stays on the laptop model. Weights stay off."). R4 §7 states the same conflict. The voice must not follow that `api` line. The mouth must not follow a GPU flag. That is the part. Detecting the machine again is not the part.

**2. Reuse search.** Do not write a second probe.

- [galion-studio/obsidian-open-mind](https://github.com/galion-studio/obsidian-open-mind) `LICENSE` is the MIT licence, copyright 3000 Galion / galion-studio. `crates/galion-boot/src/hardware.rs` (SPDX MIT) is the probe: meminfo, `nvidia-smi` when it returns a row, else a DRM card, then `degrade::profile`, written as `hardware.json` with `llm` and `rule`. Credit that file. Call it read-only. Do not edit Open Mind in the voice PR.
- `sysinfo` 0.36.1 is already pinned by `crates/horbm-device/Cargo.toml` (0.37 needs rustc 1.88; CI comment says 1.85). [GuillaumeGomez/sysinfo](https://github.com/GuillaumeGomez/sysinfo) `LICENSE` is the MIT licence, copyright 2015 Guillaume Gomez. `horbm-device` already uses it for logical processors and RAM. Do not add another copy.
- `nvml-wrapper` 0.13.0 is an optional feature, off by default. [rust-nvml/nvml-wrapper](https://github.com/rust-nvml/nvml-wrapper) has `LICENSE-MIT` (MIT, copyright 2017 Jarek Samic) and `LICENSE-APACHE` (Apache License 2.0, January 2004, the standard text). The voice plan does not need the GPU library. Brain and mouth stay CPU without it.

**3. Already in this repo.**

- Merged #280 (`43a6ee166bbf0013c8a0bde36eb1556c292a4256`): `crates/horbm-device`. `plan` sets `brain` and `mouth` to `Backend::Cpu` for every sample (`src/plan.rs`). `GpuChoice::Cpu` reports a GPU and does not select it. `GpuChoice::OptIn` can set `compute` to GPU. The crate does not load a weight and does not open a socket (`src/lib.rs`).
- No Rust file on this base contains `hardware.json`. Checked by search. `horbm-device` does not read the Open Mind document and does not look at an `llm` field. So #280 is not this part. The hint that (83) is already built via #280 does not hold for the `api` line.
- Open #257 (`cursor/horbm-r4-phase8-boot-4da6`, head `fdd11e4c10fb73d42bc69e78c5cc4bc378f85dc2`, updated 2026-11-24, not merged). Its body says it adds `LOCKS.toml` `[boot]` and changes `scripts/check-locks.sh`. This plan does not merge it and does not touch `LOCKS.toml`. I did not re-read the branch diff in this pass; the lock edit is from the pull request body.

**4. Smallest build.** A reader in `horbm-device`, no new dependency, no `LOCKS.toml` edit, no change to `degrade.rs`.

- Accept a fixture shaped like `hardware.json`: the keys `llm`, `rule`, and an optional `gpu` object. A line-oriented or small scan is enough. Do not pull serde into this crate if the existing profile reader can be extended without it. The current profile format is the crate's own `HDP1` text (`src/profile.rs`), which is a different file from `hardware.json`.
- `voice_over_probe` returns `brain = cpu` and `mouth = cpu` when `llm` is `api`, `local-gpu`, `lan`, or `cpu-light`. The `api` value is reported and does not select a backend.
- A GPU object is copied into the report. It does not change `brain` or `mouth`. `compute` stays CPU unless the caller passes the existing `GpuChoice::OptIn`. Default the tests to `GpuChoice::Cpu`.
- Do not shell out to `nvidia-smi`. Do not write `machine.json`. Do not bind a socket.

**5. Acceptance gate.** Laptop, two fixtures, no live machine required:

```text
cargo test -p horbm-device voice_ignores_the_api_line_and_keeps_brain_and_mouth_on_cpu -- --nocapture
```

The test prints `brain=cpu mouth=cpu llm=api rule=cpu-only` and `brain=cpu mouth=cpu llm=local-gpu` (gpu present). Exit 0. Then `cargo test -p horbm-device` and `cargo clippy -p horbm-device --all-targets -- -D warnings`. `scripts/check-locks.sh` must still pass with `LOCKS.toml` unchanged. This plan did not run these commands. #257's Windows spawn failure is not this gate.

**6. Swarm.** Swarm C2, two agents, one PR. Agent 1 owns the fixture bytes and the reader. Agent 2 owns the test and the clippy run. Files stay inside `crates/horbm-device`. Can run beside C1. Do not stack on #257. Closing or rebasing #257 is a separate owner decision because that branch edits the lock.

### (84) Titan: gateway and the site; offline, the VPS expired 20 Nov 2026

**1. First principles.** A remote site is not needed for the CPU conversationalist. Local first is the rule, and the replacement gateway is already on main. The progress page says the VPS expired 20 Nov 2026. This pass did not find that date in the repository and did not contact a host. I don't know the expiry from a contract or a bill. What was read: `voice-stack/research-gateway/README.md` and `src/lib.rs` say Titan is gone and this process is the local Rust gateway. It binds loopback only, dials no remote host, and calls no paid API. **Drop? (owner decides).** Do not rebuild the remote site to turn the row green.

**2. Reuse search.** Nothing to add. The local gateway is this repo. Search for a second gateway was not useful once this crate was read. I don't know of an external MIT gateway that should replace it.

**3. Already in this repo.**

- Package `galion-research-gateway` (`voice-stack/research-gateway`).
- `RESEARCH_CAP` is 3 seconds (`src/lib.rs`). A miss, a timeout, or a closed port is no answer; the router says `I don't know.` (`README.md`).
- `NoRemoteLookup` opens no socket. `HermesLookup` is the binary's client and refuses every host other than its one loopback Hermes address (`src/hermes.rs`). This plan does not repeat addresses.
- `Brain::turn` arms a follow-up only after `TurnOut` is dropped (`horbm/crates/horbm/src/brain.rs` `turn` docs). That is the side band, not a site.

Deploy notes under `docs/deploy/` and `docs/galion-voice-v2/` still describe a staging host. This plan does not quote their addresses. Those notes are not a running gateway.

**4. Smallest build.** None. Keep the loopback process. Do not add a public listener, a tunnel, or a site.

**5. Acceptance gate.** No new test. The existing refusal is the bind check in `voice-stack/research-gateway` (non-loopback rejected before listen, `src/lib.rs`). This plan did not run it. A green remote deploy would not make HORBM a better local conversationalist.

**6. Swarm.** No build swarm.

### (85) Laptop G:\galion: main storage and CPU runs

**1. First principles.** The laptop is where the owner runs the CPU build. It is not a missing feature of the brain. This pass did not see that machine, so it does not claim the disk or a CPU run there. **Drop? (owner decides)** as a counted part. The path check that can be tested is already merged. Counting the computer itself as unbuilt does not leave a Rust change that would make the conversationalist better.

**2. Reuse search.** Nothing to vendor. A drive-letter policy is a few lines, and they are already here.

**3. Already in this repo.** `voice-stack/empathy/src/paths.rs`: `WINDOWS_GALION_HOME` is `G:\galion`. `require_g_drive` accepts only that drive letter. `galion_home` on Windows uses `GALION_HOME` when it is on that drive, otherwise the constant. The test `the_owner_windows_home_is_g_and_other_drives_are_refused` refuses other letters. `voice-stack/empathy/README.md` states the same rule. Windows install scripts under `voice-stack/windows-install/` match a Samantha path on that drive. This plan did not run those scripts and did not see the drive.

**4. Smallest build.** None. Do not add another path crate. Do not copy install scripts into the brain.

**5. Acceptance gate.** If the owner wants the row marked Built instead of Dropped, the existing test is the gate:

```text
cargo test --manifest-path voice-stack/Cargo.toml -p galion-sam-empathy the_owner_windows_home_is_g_and_other_drives_are_refused
```

That test does not prove the laptop has the tree or that a CPU conversation was run. It proves the letter check. This plan did not run it. A claim that the laptop ran the stack needs a laptop-gate comment, which this session does not have.

**6. Swarm.** No build swarm. Mapping the row to the existing test is an owner edit of the progress page, not a pull request in this repo.

### (91) Build order: R4's 9 phases, gate per phase

**1. First principles.** The nine phases are a sequence the unmerged R4 note wrote so each step would end with a command and a commit (`#207`, §11). They are not a ninth program. Most of the behaviour has already landed as separate crates. A new "build order" runner would restate work other parts already are. **Drop? (owner decides).** What is still unbuilt that this agent owns is phase 8's voice override, which is part (83), and the want-ending check, which is part (79). Phase 9 is the eval in §12, and that section says to run it only when the owner asks. It also requires the presence bans to be absent. Do not start phase 9 from this plan.

**2. Reuse search.** Nothing to vendor. The phase table is the note. The gates that exist are `cargo test` targets already in the tree.

**3. Already in this repo.** Read against `9fa188d5`. This pass did not re-run any phase gate.

| Phase | What R4 asked | What this base shows |
|---|---|---|
| 0 | The note, including the S0 conflict and the `degrade.rs` conflict. | The note is not on `main`. It is closed draft #207. `LOCKS.toml` `[horbm]` now records retrieval and `student_on_tick = false`. `degrade.rs` still maps `cpu-only` to `api` (Open Mind, this pass). |
| 1 | Domain 04 stamps, lengths not transcripts. Dry run prints zeros or absent. | `crates/galion-cascade/src/stamps.rs` is that sheet and forbids PCM and transcript in the log line. `crates/galion-conductor/src/stamps.rs` has the same names and its own comment says the phase 1 module from #250 is not on `main`. I did not open #250. I did not run a dry log. |
| 2 | Retrieval thread, consent deny, no disk when denied. | `horbm/crates/horbm/src/retrieve.rs` module comment calls itself phase 2. `PLAN-brain.md` on this base describes `remember` deny until `GALION_HORBM_REMEMBER=grant`. Tests: `horbm/crates/horbm/tests/retrieve.rs`. Not re-run. |
| 3 | Mapped int8, scalar oracle, SIMD behind `galion-tensor`. | `crates/horbm-keys` and `docs/horbm/foundations/S57-honest-simd.md`. I did not re-read the dot kernel this pass. |
| 4 | Heuristic pick instead of a canned chat model. | `Brain::turn` (`horbm/crates/horbm/src/brain.rs`). I did not check the original "first clause ≤ 2 words" limit. |
| 5 | Conductor, barge-in, no cancel on a listed backchannel. | `horbm-duplex` and `horbm-eval` `src/turn.rs`. Backchannel gate is still off. I did not re-measure the 200 ms lock. |
| 6 | Side band, 3 s cap, no research call inside the tick. | `RESEARCH_CAP` and `Brain::turn` docs, as in (84). |
| 7 | Learn: fresh writes nothing, grant writes one sourced row, withdrawal leaves none. | `horbm/crates/horbm/tests/learn.rs` function `phase7_gate_fresh_writes_nothing_granted_writes_one_sourced_row_withdrawal_leaves_no_row`. Not re-run. |
| 8 | Fixture with no GPU and fixture with a GPU both select the CPU brain, and the `api` line does not. | Not on `main`. #280 keeps brain and mouth on CPU but does not read `hardware.json`. #257 is open and its body edits `LOCKS.toml`. This is part (83). |
| 9 | The §12 eval, only when the owner asks. | `horbm-eval` covers a duplex subset. It is not the §12 table (ear RTF, 100k retrieval, recall@5, Piper RTF, receipt with cgroup). I did not run it. |

**4. Smallest build.** None as its own part. Phase 8's smallest build is (83). Do not add a meta-runner that shells out to every phase.

**5. Acceptance gate.** There is no single command that means "build order is Built." Each phase gate stays the command already written next to that phase. A laptop comment on one commit does not close the other phases.

**6. Swarm.** No swarm. C1 and C2 are the only build PRs this plan proposes.

## Swarm order

1. Owner decision, no code: (76), (77), (84), (85), (91) as Drop?, and (80) as Later. This plan does not edit the progress page.
2. Swarm C1, part (79), two agents, one PR. `horbm` crate and its tests only.
3. Swarm C2, part (83), two agents, one PR. `crates/horbm-device` only. Parallel with C1 is safe on files. Do not merge #257 to satisfy (83).

No third swarm. Two agents each, not three, because each change is one crate plus its test.

## Table

| Part | Keep / drop / later | Reuse pick | Size | Swarm |
|---|---|---|---|---|
| (76) Earned return | Drop? (owner decides) | none; solscope is MIT and is the banned metric | — | none |
| (77) Quality turns (CPS) | Drop? (owner decides) | none; in-tree `horbm-eval` is timing, not CPS | — | none |
| (79) Good endings | Keep | in-tree `WantList` (#238), no new dependency | S | C1, 2 agents |
| (80) Feels heard | Later | nothing found | — | none until a protocol |
| (83) galion-boot, CPU-only local | Keep | Open Mind `galion-boot` MIT, read-only; `sysinfo` MIT already pinned | S | C2, 2 agents |
| (84) Titan | Drop? (owner decides) | in-tree `galion-research-gateway` | — | none |
| (85) Laptop `G:\galion` | Drop? (owner decides) | in-tree `require_g_drive` | — | none |
| (91) R4 build order | Drop? (owner decides) | the phase table in unmerged #207; gates already scattered in tests | — | none |

## Sources

- This checkout, `main` `9fa188d551eef902428dd006b07f6c45480340b7`.
- `LOCKS.toml` `[horbm]`. `crates/galion-presence/src/spec.rs`, `crates/galion-presence/src/replay.rs`, `crates/galion-presence/tests/replay.rs`.
- `docs/research/galion-voice-v2/domains/08-evaluation.md`.
- `docs/horbm/PLAN-brain.md`, `docs/horbm/foundations/F-turn-eval-s43.md`, `docs/horbm/README.md`.
- `horbm/crates/horbm-eval/src/lib.rs`, `src/turn.rs`, `src/thresholds.rs`.
- `horbm/crates/horbm/src/brain.rs`, `horbm/crates/horbm/tests/learn.rs`.
- `voice-stack/empathy/src/wants.rs`, `src/layer.rs`, `src/paths.rs`, `README.md`.
- `voice-stack/research-gateway/README.md`, `src/lib.rs`, `src/hermes.rs`.
- `crates/horbm-device/src/lib.rs`, `src/plan.rs`, `Cargo.toml`.
- `crates/galion-cascade/src/stamps.rs`, `crates/galion-conductor/src/stamps.rs`.
- Pull requests, GitHub API this pass: [#207](https://github.com/galion-studio/galion-one-voice/pull/207) closed unmerged, file at `dd902bbcfe34e23f63745a19fc2fed8e468a3b09`; [#238](https://github.com/galion-studio/galion-one-voice/pull/238) merged; [#257](https://github.com/galion-studio/galion-one-voice/pull/257) open; [#280](https://github.com/galion-studio/galion-one-voice/pull/280) merged.
- [obsidian-open-mind](https://github.com/galion-studio/obsidian-open-mind) `LICENSE`, `crates/galion-boot/src/hardware.rs`, and `crates/openmind-survival/src/degrade.rs`, default branch, read this pass.
- [sysinfo](https://github.com/GuillaumeGomez/sysinfo) `LICENSE` (MIT). [nvml-wrapper](https://github.com/rust-nvml/nvml-wrapper) `LICENSE-MIT` and `LICENSE-APACHE`. [solscope](https://github.com/ForgeRewardz/solscope) `LICENSE` (MIT). [Full-Duplex-Bench](https://github.com/DanielLin94144/Full-Duplex-Bench) `LICENSE` (CC BY-NC 4.0, not reused). Each licence file was fetched from that repository's default branch this pass.
