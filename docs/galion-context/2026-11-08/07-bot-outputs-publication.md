# Bot outputs publication — Nebula V3 swarm, stopped

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

| | |
|---|---|
| Date | 2026-11-08, Europe/Zagreb (CET) |
| Written | 2026-11-08 04:29 CET |
| Author | Specialist 07, publication pass. Pair: context 08 `bc-4066bdd7-c5f2-5944-9212-0dd221231872` |
| Lead | `bc-43d26d8a-4e4c-5a18-973e-22de9b04b3fd` |
| Base checked | `origin/main` `3e1f5a12c53cd8e0dd47d111676d61d608540559` |
| This file | `docs/galion-context/2026-11-08/07-bot-outputs-publication.md` |
| Evidence beside it | `docs/galion-context/2026-11-08/07-bot-outputs-evidence.json` |

This page inventories what the twelve-bot Nebula V3 swarm actually produced before it was stopped, plus the unfinished lead tree the audit describes. It is a publication manifest. It does not implement the renderer, the profiling harness, or a frame-rate run.

## Evidence this page uses

Read here, in full:

- Stopped-swarm audit, attached as `nebula-swarm-stopped-audit`. The audit’s own clock is Europe/Zagreb. It limits the Nebula window to lead transcript lines 584–686 (04:18–04:22 CET) and specialist activity about 04:19–04:22 CET. Lines 1–583 of the lead transcript are the 7 Nov recovery read and are outside this inventory.
- One-line JSON summary attached with that audit: `fps_measured` null, `commits` 0, `prs` 0, `tests_run` 0, `builds` 0, edited agent “01 lead (uncommitted, incomplete: missing v3/field.rs)”.
- Cloud-agent status list attached as `nebula-agent-status-20261108`: 84 bullets, 83 stored as finished, 1 stored as error.

Checked on this machine at 2026-11-08 04:28 CET, against the same `origin/main`, without building:

- `7e8819dcd619b76d7989fa0be840a1923bdfd442` (emotion PR 84) and `6a516c0eb9b1248000dbffc06cfb76f60d29139b` (Nebula PR 102 head) are ancestors of `HEAD`.
- `docs/nebula-v3/` and `apps/nebula-voice-v2/src/v3/` are absent.
- `git ls-remote --heads origin cursor/nebula-v3-integration-b3fd` returned no head.
- `/dev/dri` is absent. CPU is 4 vCPUs, `Intel(R) Xeon(R) Processor`. Chrome is `Google Chrome 156.0.8078.47`. `rustc` / `cargo` are 1.83.0. `wasm-bindgen` is absent. `/opt/galion` is absent.
- `apps/nebula-voice-v2/src/shade.rs` still holds the warm-stack numbers quoted below.
- `galion-emotion/src/detect.rs` still opens with the Phase 2 stub sentence. `galion-emotion/src/film.rs` still names `naive_mirror_policy_is_rejected_for_negative_callers`. `galion-demo/galion_demo.py` still documents a counterbalance port and still defines `mirror` at line 140.

Not re-opened: the jsonl transcripts under the audit’s `/workspace/cloud-agent-transcripts/`. Line numbers below are the audit’s citations. This page does not treat them as a second reading.

## Planned work and measured output

The 8 Nov owner request to the lead asked for a visual Nebula V3, a sustained minimum of 120 fps (8.33 ms) with real frame times, emotion-blend from PR 84 driving gas, the PR 102 warm stack reused, and design reasoning before implementation. The swarm split that across twelve agents. None of that delivery was measured.

| Planned | Measured before the stop |
|---|---|
| Sustained 120 fps on the integrated gas renderer | No frame time. Audit JSON `fps_measured` is null. No browser render and no benchmark in any of the twelve transcripts. |
| p50 / p95 / p99, dropped frames, GPU time, refresh, viewport, DPR | None recorded. |
| Commits, pushes, pull requests from the twelve | Audit: zero commits, zero pushes, zero pull requests. |
| `cargo` build, wasm build, tests | Audit: zero builds, zero tests. |
| Profiling harness and a 120 pass / fail label | Specialist 07 wrote no harness. |
| Combined V3 gas renderer | Lead files described below were local and incomplete. They are not on `origin/main`. |
| Hubble / JWST assets | Specialist 09 searched. No asset file was fetched. |
| Live emotion bridge run | Specialist 11 did not run one. `/opt/galion/SAMANTHA-V2-BRIEFING.md` was not on the machines that looked. |

A 60 Hz cap, a software renderer, or a synthetic loop was never scored, because no run existed to score. There is no 120 fps result to publish, pass or fail.

## Twelve bots

Status text is the attached list’s stored field (`finished` or `error`). It is not a CI re-check.

| # | Agent | Stored status | Actual output before the stop | Repo change |
|---|---|---|---|---|
| 01 | Lead `bc-43d26d8a-4e4c-5a18-973e-22de9b04b3fd`. Status-list title remains “Samantha V2 recovery: full repository review before build”. | finished | Nebula window 04:18–04:22 CET. Confirmed `origin/main` `3e1f5a1`, PR 84 and PR 102 as ancestors. Read sim, frame, shade, CIE, render, and `galion-emotion`. Local branch `cursor/nebula-v3-integration-b3fd` created from that main. Uncommitted edits listed in the next section. Audit quotes a local `DESIGN.md` line `fps_120: UNVERIFIED`. | Not on origin. This checkout does not contain the edits. |
| 02 | Render `bc-0421640c-f485-5437-be4c-1f14d9b07b1e` | finished | `git log`, hardware probe (4-vCPU Xeon, no `/dev/dri`, no GPU), read `wasm_api.rs`. | None. |
| 03 | Gas density `bc-d884070b-071c-5878-8bb1-2ff14b982a8d` | finished | `git status` / `git log`, read `nebula-elements` `lib.rs`. | None. |
| 04 | Magnetic fields and waves `bc-6edf5b3c-d7b3-5f73-b8a8-fc748d7490ee` | finished | `git` / `gh pr list`, search in `EMOTION-BLEND.md`. | None. |
| 05 | Emotion mapping `bc-ac80a4cb-3e55-5a3c-a9bd-57beb0da7d6b` | finished | `git` stats for PR 84 and PR 102, read `apps/nebula-voice-v2/RESEARCH.md`. | None. |
| 06 | Warm color provenance `bc-af4582f7-36f5-58df-b21b-0d15b12ac0a0` | finished | `git`, `gh` branch and open-PR listing, `gh pr view 84`. | None. No new swatch. |
| 07 | Profiling harness `bc-70e00867-7bbf-5d94-ae4c-acd84aebb4f0` | finished | `git`, `gh pr view 102`, parse of the lead transcript, toolchain probe: Rust 1.83, `wasm-bindgen` absent. | None. No harness, no raw frame arrivals, no GPU timestamps, no percentiles. |
| 08 | Handoff and architecture review `bc-4066bdd7-c5f2-5944-9212-0dd221231872` (this page’s pair) | finished | Before the stop: `git` and PR 84. After a later owner request the same transcript listed commit history (the audit calls that an audit continuation). | No edits in the audited window. CI was assigned to 08 and was not checked inside that window. This file does not replace 08’s own page. |
| 09 | Scientific imagery and physics provenance `bc-a10b6402-632d-5f18-9a9d-1f5a2d13c17b` | finished | `git` / PR 102 stat, nine web searches (Hubble, JWST, ESA Webb; the audit’s example is Orion Bar NIRCam). | None. No image, no FITS, no license file. A search hit is not a cleared asset. |
| 10 | Mobile and GPU regression `bc-2d21c215-643f-5de9-9539-e9ae0aa6f784` | finished | `git`, `gh pr list`, PR 102 detail, a headless Chrome probe whose captured output was empty. | None. Empty output is not a phone or GPU measurement. |
| 11 | Live emotion bridge `bc-0b6d0589-a28a-5026-98ce-41c8dab26208` | finished | Parsed the lead transcript, `git`, check for `/opt/galion/SAMANTHA-V2-BRIEFING.md`, PR 84 stat, color history. | None. Briefing path was absent. No bridge process was run. |
| 12 | Validation and visual acceptance `bc-7e5fba95-d191-5a56-9e40-0a184a322942` | finished | `git`, `gh pr view 102`, hardware probe. | None. No test and no acceptance screenshot were added. |

Specialist 07’s publication pass (this document) started from a clean `main` after that stop. It did not resume the harness.

## Unfinished lead code

The audit is the only witness for these paths. They were local and uncommitted on the lead VM. They are not in `3e1f5a1`, and `cursor/nebula-v3-integration-b3fd` is not an origin head. This publication does not reconstruct them from a transcript.

| Path the audit names | Audit’s state |
|---|---|
| `docs/nebula-v3/DESIGN.md` | Written locally, more than once. Contains the planned gas mapping and `fps_120: UNVERIFIED`. |
| `docs/nebula-v3/ROSTER.md` | Written locally. |
| `apps/nebula-voice-v2/src/v3/mod.rs` | Declares `mod field` with no `field.rs` beside it. |
| `apps/nebula-voice-v2/src/v3/input.rs` | Written locally. |
| `apps/nebula-voice-v2/src/v3/field.rs` | Never written. |

The audit’s conclusion: that module would not compile, and the gas integrator was not written. Publishability of those sources is **withheld**. Shipping a reconstructed copy would invent the missing field module and would present an unfinished tree as the V3 renderer.

Planned mapping, quoted from the audit’s reading of that local `DESIGN.md`, and not executed:

- weights to species density
- arousal to magnetic alignment and wave amplitude
- valence / warmth to radiation pressure
- energy × VAD to shock threshold
- dominance to clump
- hold when `present` is false or `sense` is false
- an 80 ms emotion clock kept apart from the render clock

Those sentences are a design note that never became a measured render.

## Warm stack already on main

Verified in `shade.rs` on this `HEAD`. These numbers are the existing V2 illuminant, not a swarm measurement and not a new palette.

| Voice | `star_linear` | `grade_linear` |
|---|---|---|
| Samantha | `[1.0, 0.72, 0.38]` | `[1.05, 0.86, 0.55]` |
| Jarvis | `[0.35, 0.48, 1.0]` | `[0.55, 0.70, 1.10]` |

Spectral and display hex for the Demo 13 rows stay in `nebula_elements`. The browser CIE table stays `apps/nebula-voice-v2/web/cie.js`. PR 102 (`3e1f5a1`, merged 2026-11-07 20:02 CET, head `6a516c0` at 18:51 CET) added the GPU path, cache-bust, and phone tier. It did not add swatches. Commits on that merge, Zagreb time: `abde48e` 18:24, `1922788` 18:36, `6a516c0` 18:51. Earlier the same evening: PR 101 `8d6e556` 17:58 CET, PR 100 `b7049d9` 17:58 CET.

## Emotion on main, scores unmeasured

Checked as source text only. No scorer was run.

- `galion-emotion/src/detect.rs` lines 3–14: “Phase 2 stub: RMS energy + zero-crossing rate”. Valence is applied only when supplied. The file states that nothing there is a measured accuracy.
- `galion-emotion/src/film.rs` contains the test `naive_mirror_policy_is_rejected_for_negative_callers`.
- `galion-demo/galion_demo.py` header says the policy is counterbalance and “never mirror”, and the same file defines `mirror` at line 140. The audit calls that conflict unresolved. This page did not execute the demo, so it does not decide which path runs.
- The audit also cites `band.rs` consent bits `sense` / `adapt` / `remember` / `share`, default deny, and `W20-04-STATUS` dated 2026-11-02 saying the Python demo still mirrors. Those two citations were not re-opened in this pass. No transcript shows a persistent-storage run.

PR 84 (`7e8819d`, “cloning-v2: emotion-blend tooling”) is an ancestor of main. The swarm did not produce a new emotion-blend measurement.

## Owner pricing direction

Recorded for this publication, because the status list includes older SKU docs and because those docs already sit on main:

- Research Ultra: **$220**
- One: **$20**

Already published on `main`, from earlier symphony work, and left untouched by this commit:

- Plus at $20 per month (`docs/galion-voice-v2/symphony/PLAN-PLUS.md` and the pages that repeat it)
- Ultra at $220, including the stored t284 string “Ultra $220 / 2GB SSD” (`PLAN-ULTRA.md`)

This page does not rename Plus to One inside those files, does not treat the two $20 labels as one measured product, and does not treat the Ultra disk line as a new figure. The stopped swarm published no pricing artifact.

## Other agents on the status list

The attachment has 84 agents. Thirteen are the lead and the twelve specialists above. The other 71 are earlier runs. Seventy of the full list are stored as finished. One is stored as error: `bc-d7e9980b-2372-5520-9c84-c2fd4dc702fb` (W7 hard-test), with PR 52 last stored as merged. “Last stored: merged” is the attachment’s field. This page did not re-query CI.

Those 71 rows already point at their own pull requests, including older voice work and two rows on other repositories (`galion-operating-system`, `Galion-Voice-`). Their diffs, audio, and weights are not copied here.

## Publishability

| Item | Decision | Why |
|---|---|---|
| This markdown and `07-bot-outputs-evidence.json` | Publish on the publication branch | Owner-authorized safe notes. No secret, no audio, no weight, no dataset. |
| Audit facts restated above | Publish as this prose | The jsonl logs themselves stay out of git. |
| Lead `DESIGN.md`, `ROSTER.md`, `v3/mod.rs`, `v3/input.rs` | Withhold | Uncommitted, incomplete, absent here, not reconstructed. |
| Frame-time tables, screenshots, wasm, `dist/` | Nothing to publish | No run, no build. |
| Specialist 09 search hits and any Hubble / JWST / ESA pixels | Withhold | No file was saved, and no license was cleared into the tree. |
| `/opt/galion/SAMANTHA-V2-BRIEFING.md` and the VPS archive | Withhold | Path absent. Not read. |
| Piper WAVs and other generated audio from the lead’s 7 Nov recovery artifacts | Withhold | Audio stays out. Those files are outside this commit. |
| `LOCKS.toml`, `jarvis.toml`, `REFS.sha256`, credentials, clone weights, own-voice refs | Withhold | Excluded by the publish rule. Not opened for this page. |
| Full 84-row status dump and jsonl transcripts | Withhold | Bulk logs. Summary only. |
| Held PR 78 and PR 83 bodies | Withhold | Not part of this commit. |

## Excluded from the commit

Reviewed before push. The commit contains only the two files named in the header.

Left out on purpose:

- jsonl transcripts and the raw status dump
- any `git add -A` of the worktree
- unfinished lead Rust and the local design docs that are not in this tree
- audio, weights, datasets, secrets, lockfiles, and ref hashes
- browser profiles, Chrome logs, and `/tmp` transcripts
- a profiling harness, tests, or FPS output (none were produced; the stopped implementation stays stopped)

## Recent main, for the pair

Same commits the audit assigns to specialist 08’s local listing. CI was not re-run here.

| CET on 2026-11-07 | Commit | What landed |
|---|---|---|
| 20:02 | `3e1f5a1` | PR 102 merge |
| 18:51 | `6a516c0` | GPU nebula path, strong-phone high tier |
| 18:36 | `1922788` | Cache-bust of browser assets |
| 18:24 | `abde48e` | Phone canvas sharpness, spectrum preview scale |
| 17:58 | `8d6e556` | PR 101, Nebula Voice V2 app |
| 17:58 | `b7049d9` | PR 100, offline nebula scoring |

VPS briefing and archive remain unread. No earlier local handoff was found that this page can date to the evening of 7 Nov.
