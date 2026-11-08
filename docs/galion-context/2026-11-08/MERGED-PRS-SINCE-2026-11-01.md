# Merged pull requests since 2026-11-01

Evidence class: **verified from GitHub and local git on 2026-11-08 04:25 CET**, not a re-run of CI.

- Repository: [galion-studio/galion-one-voice](https://github.com/galion-studio/galion-one-voice)
- `main` tip at query: `3e1f5a12c53cd8e0dd47d111676d61d608540559` (same as `origin/main`)
- Window: 2026-11-01 00:00 Europe/Zagreb through 2026-11-08T04:25:33+01:00
- Merged PRs in that window: **95** (GraphQL, two pages, state MERGED)
- Raw rows: `evidence/merge-inventory-20261108.json` and `evidence/ci-compact.json`

## How to read CI

Each SHA was queried once with `GET /repos/galion-studio/galion-one-voice/commits/{sha}/check-runs` and `/status`.

- **Head checks** are the runs stored on the pull-request head commit.
- **Merge-commit checks** are the runs stored on the merge commit. Workflows also trigger on `push` to `main` (`.github/workflows/gates.yml`, `emotion-v2-ci.yml`, and path-filtered `nebula-voice-v2.yml`), so a merge SHA can fail even when the head was green.
- The legacy combined status is **`pending` on every row** and the status list is empty. That is the Commit Status API, not Actions. Do not read `pending` as red or as still running.
- **Green-at-the-instant-of-merge is unknown.** These conclusions are whatever GitHub still stores. This audit did not re-run jobs and did not read historical logs line by line.
- No check conclusion in this pull was `skipped`. Soft gates that are **absent as jobs** are named in the notes, not counted as skipped checks.
- A failing job name in this dataset is the GitHub check `test` unless a second name is listed.

## Not merged (same number range, excluded from the table)

| PR | State | Why it is not in the merged table |
|---|---|---|
| [#69](https://github.com/galion-studio/galion-one-voice/pull/69) | CLOSED, not merged | Local Qwen2.5 path. Closed 2026-11-04 21:34 CET. Owner ruling the same evening removes Qwen. |
| [#78](https://github.com/galion-studio/galion-one-voice/pull/78) | OPEN | Title says do not merge. Proposed CC0 ref swap. |
| [#83](https://github.com/galion-studio/galion-one-voice/pull/83) | OPEN | Consent addendum, title says do not merge yet. |
| [#86](https://github.com/galion-studio/galion-one-voice/pull/86) | OPEN | Expression / prosody plan. |
| [#87](https://github.com/galion-studio/galion-one-voice/pull/87) | OPEN | Reference fidelity. |
| [#96](https://github.com/galion-studio/galion-one-voice/pull/96) | OPEN | Rollout and consent blockers. |
| [#97](https://github.com/galion-studio/galion-one-voice/pull/97) | OPEN | Emotion gap plan and live nebula pipeline doc. |

First-parent commits on `main` in the window that are not those merge commits: `81fdbe5578` Initial commit (no check runs stored); `0dd661bd9e` Add the Samantha V2 session spec (one stored success).

## Table

| PR | Merged (CET) | Merge commit | On main first-parent | PR diff | Head check-runs | Merge-commit check-runs | Title |
|---|---|---|---|---|---|---|---|
| [#1](https://github.com/galion-studio/galion-one-voice/pull/1) | 2026-11-02T19:00:50+01:00 | `8a6636de823cd487190c1989c104eaaa981e81da` | yes | +31940/−1, 262 files | no runs stored | no runs stored | Add the V1 one-dot demo, README, and MIT LICENSE |
| [#2](https://github.com/galion-studio/galion-one-voice/pull/2) | 2026-11-02T19:02:19+01:00 | `253c1d1a9cee4250be14ed2f153c3a59d8ebeadd` | yes | +161/−0, 2 files | no runs stored | no runs stored | Add CREDENTIALS.md and CONTRIBUTIONS.md |
| [#3](https://github.com/galion-studio/galion-one-voice/pull/3) | 2026-11-02T22:19:49+01:00 | `9a5ebe187097f6810c44cec515eec90ae828b467` | yes | +4281/−194, 33 files | success×2 | success×2 | Galion Voice V2 plan + Plan V5 + phone path + orb foundation |
| [#4](https://github.com/galion-studio/galion-one-voice/pull/4) | 2026-11-03T01:55:45+01:00 | `8973e6fb8620685a8b5f3794c9e58f630f61f2a1` | yes | +1125/−11, 11 files | success×1 | success×1 | Add the CPU orb spring foundation (V2-S2) |
| [#5](https://github.com/galion-studio/galion-one-voice/pull/5) | 2026-11-03T02:12:44+01:00 | `ba3ce81b9e22dc5e492b4489ed2238379f61af2e` | yes | +1989/−7, 19 files | success×1 | success×1 | Add an offline loopback virtual user for presence-eval T0–T2 |
| [#6](https://github.com/galion-studio/galion-one-voice/pull/6) | 2026-11-03T02:21:20+01:00 | `80b6fdedac0daa94564b254044c7c3d81153d5a2` | yes | +1289/−97, 14 files | success×1 | success×1 | Harden the Galion Voice edge (V2-S3) |
| [#9](https://github.com/galion-studio/galion-one-voice/pull/9) | 2026-11-03T02:54:56+01:00 | `8f97cabf31bfad95b9e9a5395131b33f8150d041` | yes | +1062/−62, 15 files | success×1 | success×1 | Warm Piper worker on the phone lane (V2-S6) |
| [#10](https://github.com/galion-studio/galion-one-voice/pull/10) | 2026-11-03T03:20:19+01:00 | `a75e065ff685eef873a8db84b2ac1245ebf49029` | yes | +441/−95, 16 files | success×1 | success×1 | Record V2 owner decisions: Piper pin, Vosk A, Chatterbox |
| [#11](https://github.com/galion-studio/galion-one-voice/pull/11) | 2026-11-03T04:32:55+01:00 | `12108e16ae1b7b93bf10668c59cb1b6670d6524e` | yes | +1680/−2, 18 files | success×1 | success×1 | Add the Open Mind nerve and cortex seam |
| [#13](https://github.com/galion-studio/galion-one-voice/pull/13) | 2026-11-03T04:33:13+01:00 | `e362050f45abefa490d2ea4d623bf817dc5d1353` | yes | +413/−35, 11 files | success×1 | success×1 | Add the empathy weight to prosody and the background wash |
| [#7](https://github.com/galion-studio/galion-one-voice/pull/7) | 2026-11-03T04:35:45+01:00 | `5ab3d38f55975eb75f6053dfc5643f1da7e1d57d` | yes | +1454/−70, 5 files | success×1 | success×1 | Add the V2-S1 process bus next to the turn worker |
| [#8](https://github.com/galion-studio/galion-one-voice/pull/8) | 2026-11-03T04:37:47+01:00 | `5fd164210ea3329d9f32be63baab9201767d3054` | yes | +1239/−0, 18 files | success×1 | success×1 | Galion Voice V2 research docs (2026-11-03) |
| [#12](https://github.com/galion-studio/galion-one-voice/pull/12) | 2026-11-03T04:51:23+01:00 | `b584a56db60b0e77ef185b6b8b1bd6123fed6689` | yes | +497/−64, 16 files | success×1 | success×1 | Name the Luti easing profile and count the orb step in-process |
| [#15](https://github.com/galion-studio/galion-one-voice/pull/15) | 2026-11-03T04:53:38+01:00 | `d1db9a401ab134d3a2f504ed7de82123e4f93ee7` | yes | +1965/−66, 23 files | success×1 | success×1 | Scaffold the Galion Voice V2-S14 clone worker and Persona bus body |
| [#14](https://github.com/galion-studio/galion-one-voice/pull/14) | 2026-11-03T05:21:59+01:00 | `eb3e80ef2f751241f5d1b91b1551b2ecbfd1a807` | yes | +539/−0, 8 files | success×1 | success×1 | Add Galion Voice persona instruction files |
| [#17](https://github.com/galion-studio/galion-one-voice/pull/17) | 2026-11-03T05:22:05+01:00 | `04cc193c78e78cdc18bff7e3d3c74a6f1279b7ca` | yes | +276/−0, 7 files | success×1 | success×1 | Draft Samantha and Jarvis V2 voice configs |
| [#18](https://github.com/galion-studio/galion-one-voice/pull/18) | 2026-11-03T05:28:24+01:00 | `8ea7a6561ecd29a284fa988b1f8cdc54db100f34` | yes | +377/−4, 7 files | success×1 | success×1 | Add the V2-S14 rights-clearance checklist for Samantha and Jarvis |
| [#19](https://github.com/galion-studio/galion-one-voice/pull/19) | 2026-11-03T05:29:28+01:00 | `4ecf0d6f5946275a94d9db28305538cbe2db1c60` | yes | +269/−4, 6 files | success×1 | success×1 | Document the closed film-clip ingest outline |
| [#16](https://github.com/galion-studio/galion-one-voice/pull/16) | 2026-11-03T05:42:58+01:00 | `eed19730fd989ab1d0d9aa698723e6d169c7373d` | yes | +296/−1, 7 files | success×1 | success×1 | Document the Galion Voice V2 error state |
| [#21](https://github.com/galion-studio/galion-one-voice/pull/21) | 2026-11-03T05:46:22+01:00 | `4f537b48a785d61578519719f62646d2b263ad69` | yes | +319/−128, 11 files | no runs stored | success×1 | Keep the one-dot mark a calm near-white circle |
| [#22](https://github.com/galion-studio/galion-one-voice/pull/22) | 2026-11-03T06:03:36+01:00 | `986a6dbd331003350cbb5e9e83b19b2dcc3a8db1` | yes | +1485/−30, 17 files | success×1 | success×1 | Add the V2 persona dot and mood wash |
| [#26](https://github.com/galion-studio/galion-one-voice/pull/26) | 2026-11-03T09:50:07+01:00 | `235481b0f2ffd24ed0e93eb571bfc77b6ed6b413` | yes | +2086/−113, 27 files | success×1 | success×1 | Use out-of-process Vosk for the phone mic |
| [#25](https://github.com/galion-studio/galion-one-voice/pull/25) | 2026-11-03T09:50:35+01:00 | `b1f5e0c5712f4498048b37d41a541a040787e74d` | yes | +634/−309, 12 files | success×1 | success×1 | One phone-ready circle: mic, text, persona, and Piper |
| [#51](https://github.com/galion-studio/galion-one-voice/pull/51) | 2026-11-03T12:54:36+01:00 | `ed933060e80a4b665d376b8390e46f7078b655cc` | yes | +897/−39, 15 files | success×1 | success×1 | Fetch Chatterbox clone weights from GitHub Release assets |
| [#58](https://github.com/galion-studio/galion-one-voice/pull/58) | 2026-11-03T13:12:05+01:00 | `8162456f988beffcdbe12015efbe015f9dfdb5e8` | yes | +274/−0, 3 files | success×1 | success×1 | Piper CPU stream TTFA live demo (t301) — refs #54 #55 |
| [#27](https://github.com/galion-studio/galion-one-voice/pull/27) | 2026-11-03T17:26:16+01:00 | `4bca8c2e93df012ae4971378e8ef9fa3f48c380d` | yes | +117/−0, 1 files | success×1 | failure×1 | Add the Plus SKU plan for Galion Voice V2 symphony |
| [#28](https://github.com/galion-studio/galion-one-voice/pull/28) | 2026-11-03T17:26:24+01:00 | `efbde9bfdf7fe0321c79cd102f5593dc741c5e29` | yes | +200/−0, 3 files | success×1 | success×1 | Wire Eureka into the Galion Voice V2 symphony docs |
| [#29](https://github.com/galion-studio/galion-one-voice/pull/29) | 2026-11-03T17:26:26+01:00 | `0dbe96a92baa751f984b230097a896e518ce822a` | yes | +396/−0, 1 files | success×1 | success×1 | Add the Galion Voice V3 symphony plan (docs only) |
| [#38](https://github.com/galion-studio/galion-one-voice/pull/38) | 2026-11-03T17:26:28+01:00 | `0d615c429bf5a0c07c50fa34c77d3203efc82d8c` | yes | +1442/−84, 11 files | success×1 | success×1 | Compile Chatterbox PCM into a .gqvoice packet |
| [#40](https://github.com/galion-studio/galion-one-voice/pull/40) | 2026-11-03T17:26:30+01:00 | `b01c80508c0032c85886c7b6eca0cbef940b1baa` | yes | +37/−1, 2 files | success×1 | success×1 | Ignore Samantha and Jarvis indoor voice references |
| [#46](https://github.com/galion-studio/galion-one-voice/pull/46) | 2026-11-03T17:26:32+01:00 | `179a884e50542165645e2d1a7623925834c9e26c` | yes | +440/−0, 1 files | success×1 | success×1 | Document Symphony puzzle pieces (MIT / Apache inventory) |
| [#50](https://github.com/galion-studio/galion-one-voice/pull/50) | 2026-11-03T17:26:34+01:00 | `e173fbb8ff968fd6570c18b3c67565958d2d2c78` | yes | +834/−3, 8 files | success×1 | success×1 | Wire the Samantha and Jarvis voice-to-voice outline |
| [#57](https://github.com/galion-studio/galion-one-voice/pull/57) | 2026-11-03T17:26:38+01:00 | `0f3c463b9155682b35267593046b34d689e1b162` | yes | +892/−0, 8 files | success×1 | success×1 | Add Galion Voice V3 planning docs |
| [#59](https://github.com/galion-studio/galion-one-voice/pull/59) | 2026-11-03T18:31:05+01:00 | `ac6065ce600ac93eaab753f06eab939a86e46da3` | yes | +10898/−174, 97 files | success×1 | success×1 | Merge open PRs #20 #30-36 #39 #41 #42 #45 #52 (markdown conflicts resolved by union) |
| [#30](https://github.com/galion-studio/galion-one-voice/pull/30) | 2026-11-03T18:31:07+01:00 | `44b334792ae9d51ac75ed63699f31f52847fca28` | no (landed inside a later rollup) | +237/−2, 4 files | success×1 | no runs stored | Plan Titanaxe staging CPU cascade (Plus CX23 / Ultra CX43) |
| [#52](https://github.com/galion-studio/galion-one-voice/pull/52) | 2026-11-03T18:31:07+01:00 | `88e9796c4e44861a251b627ae79f3a270d93705d` | no (landed inside a later rollup) | +1857/−4, 10 files | success×1 | success×1 | Add a hard Galion Voice loop harness and schedule the Admin UI check |
| [#45](https://github.com/galion-studio/galion-one-voice/pull/45) | 2026-11-03T18:31:07+01:00 | `db6835dde9ac11a1f16431c061a5ae8682c9bf55` | no (landed inside a later rollup) | +1317/−4, 14 files | success×1 | no runs stored | Wire V6 one-percent engine metrics into symphony |
| [#42](https://github.com/galion-studio/galion-one-voice/pull/42) | 2026-11-03T18:31:07+01:00 | `bb821dbb654f2af46f3b2c7c3c15946919ad3f88` | no (landed inside a later rollup) | +1776/−2, 12 files | success×1 | no runs stored | Compile clone PCM into a Galion .gq voice clip |
| [#41](https://github.com/galion-studio/galion-one-voice/pull/41) | 2026-11-03T18:31:07+01:00 | `08698c62fbc5b53b178be1287eddd17c149fa50e` | no (landed inside a later rollup) | +1659/−147, 22 files | success×1 | no runs stored | Land Chatterbox Nano clone wiring and Galion clip emit |
| [#39](https://github.com/galion-studio/galion-one-voice/pull/39) | 2026-11-03T18:31:07+01:00 | `dc07f4ffad3f42efe42c3ab196d00cd2b0aafd7a` | no (landed inside a later rollup) | +690/−0, 10 files | success×1 | no runs stored | Add Eureka self-eval gates for symphony A–G |
| [#36](https://github.com/galion-studio/galion-one-voice/pull/36) | 2026-11-03T18:31:07+01:00 | `5113ec1dd9e2d2770c3f65f6f37b7b78400bd330` | no (landed inside a later rollup) | +314/−0, 2 files | success×1 | no runs stored | Plan the Galion Research console as its own async desk |
| [#35](https://github.com/galion-studio/galion-one-voice/pull/35) | 2026-11-03T18:31:07+01:00 | `322f6ecca2ddf632eecdf08017b0696ebf664e8f` | no (landed inside a later rollup) | +967/−0, 9 files | success×1 | no runs stored | TTFA gate: 80 ms live-sector frame, presence onset / barge-in / e-to-a |
| [#34](https://github.com/galion-studio/galion-one-voice/pull/34) | 2026-11-03T18:31:07+01:00 | `e833dbdf87915e96c897c70f772ea17f14a37509` | no (landed inside a later rollup) | +373/−5, 11 files | failure×1 | no runs stored | One-dot auto-record proceeds on the mic path |
| [#33](https://github.com/galion-studio/galion-one-voice/pull/33) | 2026-11-03T18:31:07+01:00 | `aa41d959be94f547e06c9ca9e3738af01e55c869` | no (landed inside a later rollup) | +279/−0, 2 files | success×1 | no runs stored | Document Samantha emotions as live prosody, not a script |
| [#32](https://github.com/galion-studio/galion-one-voice/pull/32) | 2026-11-03T18:31:07+01:00 | `372c5d0cc89990bb79d9d581c1388346023d6250` | no (landed inside a later rollup) | +967/−0, 19 files | success×1 | no runs stored | Add the Galion Voice V2 symphony index |
| [#31](https://github.com/galion-studio/galion-one-voice/pull/31) | 2026-11-03T18:31:07+01:00 | `73662830da106b82cc6c1b9bc3cbb6566df44382` | no (landed inside a later rollup) | +289/−7, 6 files | success×1 | no runs stored | Add the Ultra symphony host plan ($220, CX43-class) |
| [#20](https://github.com/galion-studio/galion-one-voice/pull/20) | 2026-11-03T18:31:07+01:00 | `071a73f5e6eae55a09ab75df2e5189db8e273cea` | no (landed inside a later rollup) | +231/−3, 6 files | no runs stored | no runs stored | Plan the Chatterbox Nano out-of-process clone worker |
| [#60](https://github.com/galion-studio/galion-one-voice/pull/60) | 2026-11-03T18:32:30+01:00 | `cc4e6e81da3d2f331feaa236d240c60a7a78e027` | yes | +8151/−38, 73 files | success×1 | success×1 | Merge open PRs #37 #44 #47 #48 #49 (code conflicts resolved) |
| [#48](https://github.com/galion-studio/galion-one-voice/pull/48) | 2026-11-03T18:32:32+01:00 | `7366a3382ad321e6d46a61b4a308e190c4302fab` | no (landed inside a later rollup) | +4052/−1, 22 files | success×1 | no runs stored | Translate clone interchange into the Galion symphony load path |
| [#49](https://github.com/galion-studio/galion-one-voice/pull/49) | 2026-11-03T18:32:32+01:00 | `b8a981af75c123722ef8bcf27f87a03ecb200502` | no (landed inside a later rollup) | +977/−25, 17 files | success×1 | no runs stored | Sit clone, persona, and the one dot on the 80 ms heartbeat |
| [#44](https://github.com/galion-studio/galion-one-voice/pull/44) | 2026-11-03T18:32:32+01:00 | `cefd7120a89e6399c4cc7e6b77fc80098807cd3f` | no (landed inside a later rollup) | +1245/−2, 12 files | success×1 | no runs stored | Add a toggleable Jarvis hub texture layer |
| [#37](https://github.com/galion-studio/galion-one-voice/pull/37) | 2026-11-03T18:32:32+01:00 | `90307fee74cb5d20f8a87fd40038116bbaad9927` | no (landed inside a later rollup) | +892/−8, 13 files | success×1 | no runs stored | Persona feeling on the heartbeat, one-dot workflow hook |
| [#47](https://github.com/galion-studio/galion-one-voice/pull/47) | 2026-11-03T18:32:34+01:00 | `927abeae1635908699b97fc032f4152086b31212` | no (landed inside a later rollup) | +1060/−3, 20 files | success×1 | no runs stored | Add symphony runbooks and the 80 ms self-eval |
| [#61](https://github.com/galion-studio/galion-one-voice/pull/61) | 2026-11-03T19:11:41+01:00 | `5114f6550896429f69af352f2943093424842264` | yes | +7898/−497, 69 files | success×1 | failure×1 | Merge clone cluster + Hermes PRs (#53 #55 #54 #43 #56 #24 #23) |
| [#55](https://github.com/galion-studio/galion-one-voice/pull/55) | 2026-11-03T19:11:43+01:00 | `7a73229cc5b89a64d445b054dded1636d9a0e644` | no (landed inside a later rollup) | +1993/−39, 24 files | success×1 | no runs stored | Drain one Chatterbox PCM frame on the 80 ms heartbeat |
| [#54](https://github.com/galion-studio/galion-one-voice/pull/54) | 2026-11-03T19:11:43+01:00 | `d6d604a91a90504dda9bb93cb1315a7cd7832276` | no (landed inside a later rollup) | +1993/−81, 21 files | success×1 | no runs stored | Stream TTFA: Piper script is the live path, stub clocks fail |
| [#43](https://github.com/galion-studio/galion-one-voice/pull/43) | 2026-11-03T19:11:43+01:00 | `5fa00771f21dfd75097a7796f24e23623c7cc691` | no (landed inside a later rollup) | +1253/−49, 20 files | success×1 | no runs stored | Unlock the clip store on the t273 private clearance |
| [#24](https://github.com/galion-studio/galion-one-voice/pull/24) | 2026-11-03T19:11:43+01:00 | `d3604147f0ef89f1d59d94a76a17f970a5bb0539` | no (landed inside a later rollup) | +1310/−129, 19 files | success×1 | no runs stored | Run Chatterbox Nano out of process, gated on consented refs |
| [#23](https://github.com/galion-studio/galion-one-voice/pull/23) | 2026-11-03T19:11:43+01:00 | `3f39560de36b2a3cf0483a7d1abee3011b6d5bc2` | no (landed inside a later rollup) | +1340/−234, 14 files | success×1 | success×1 | Wire Hermes into the live reply path |
| [#56](https://github.com/galion-studio/galion-one-voice/pull/56) | 2026-11-03T19:11:43+01:00 | `21932c77137d026ef8ca357d126f5280d49c72f5` | no (landed inside a later rollup) | +1209/−122, 17 files | success×1 | no runs stored | Make Chatterbox Nano the V2 clone backend, Piper fallback |
| [#53](https://github.com/galion-studio/galion-one-voice/pull/53) | 2026-11-03T19:11:44+01:00 | `dda7a60106d4bf88e5e3ad6d9311e7d3d1c56639` | no (landed inside a later rollup) | +1222/−124, 15 files | success×1 | no runs stored | Wire Chatterbox Nano generate in the clone worker |
| [#63](https://github.com/galion-studio/galion-one-voice/pull/63) | 2026-11-04T03:01:27+01:00 | `3b1acc9e9c2c44d76f125bed1c84727bac57579c` | yes | +1919/−69, 27 files | success×1 | success×1 | Consent check at clone time only (Rust, in process); no per-turn consent subprocess |
| [#62](https://github.com/galion-studio/galion-one-voice/pull/62) | 2026-11-04T07:46:27+01:00 | `5fe846b628f34a23744c9d6d174aa0d2517d63df` | yes | +1435/−37, 22 files | success×1 | failure×1 | latency-240/01: galion-piper-rt (ORT 1.31.1) default Piper runtime + streaming/pre-warm (owner t613s2992) |
| [#64](https://github.com/galion-studio/galion-one-voice/pull/64) | 2026-11-04T17:51:38+01:00 | `508cfe64d001fc1ec11419b9f7828815e5c3d2e5` | yes | +279/−38, 2 files | success×1 | success×1 | Hard-test harness: apply the six REPORT §5b fixes (local LLM run: HARD-FAIL, gaps named) |
| [#72](https://github.com/galion-studio/galion-one-voice/pull/72) | 2026-11-05T03:36:34+01:00 | `4b87ed5902be6c40a4ed008d26508c43a79015e9` | yes | +6/−1, 1 files | success×1 | success×1 | a21: build-piper-rt.sh builds and copies from the same target dir |
| [#66](https://github.com/galion-studio/galion-one-voice/pull/66) | 2026-11-05T03:36:41+01:00 | `9b6ffeed1830b188e8d4e6157155749ba67e8240` | yes | +608/−21, 5 files | success×1 | failure×1 | latency-240 lane 02: V2 core map + ClonePlan CPU pinning (clone off during live calls) |
| [#68](https://github.com/galion-studio/galion-one-voice/pull/68) | 2026-11-05T03:36:54+01:00 | `fc4d72d7d66484b3853c93f668648820b6062c21` | yes | +888/−275, 17 files | success×1 | success×1 | Core map v3 (owner map): bus+retrieval 0, Vosk 1, in-house reasoning 2-5, Piper Samantha 6 / Jarvis 7; no Qwen |
| [#65](https://github.com/galion-studio/galion-one-voice/pull/65) | 2026-11-05T03:37:01+01:00 | `86c5d4466f83b1687d4265a015497f645c5687b5` | yes | +2154/−0, 18 files | success×1 | success×1 | latency-240 lane 04: pipeline (LLM at ASR final, first-clause cap fix, two-stage endpointer, warm client) |
| [#75](https://github.com/galion-studio/galion-one-voice/pull/75) | 2026-11-05T03:37:08+01:00 | `3e6583a641c76a2d7bf89aa6413446640fbc1f54` | yes | +2158/−0, 18 files | success×1 | failure×1 | lat240 wt-14: GALION_LLM=canned local reply + in-tree Piper runtime default (eos->first audio 178.9/221.9 ms canned, n=24) |
| [#67](https://github.com/galion-studio/galion-one-voice/pull/67) | 2026-11-05T03:37:16+01:00 | `d12f9cc09706bf44ede068401d6565200bfde1c4` | yes | +568/−0, 8 files | success×1 | success×1 | latency-240 lane 03: Chatterbox Nano clone efficiency (bench-only; q8t3+s1 RTF 0.553→0.358) |
| [#70](https://github.com/galion-studio/galion-one-voice/pull/70) | 2026-11-05T03:37:23+01:00 | `6c706ff9353d11b54b5abf346d70d0bd3c72e313` | yes | +1241/−67, 29 files | success×1 | failure×1 | Clone: CC0 donors ON (owner t628u) + CC0 licence checkbox enforced per session; re-cut tool, TOKENIZER.sha256 fix, clone_probe |
| [#71](https://github.com/galion-studio/galion-one-voice/pull/71) | 2026-11-05T03:39:45+01:00 | `382e2e45f9e13e658b4d0291c1e308f7b23c8ee1` | yes | +1269/−3, 5 files | success×1 | success×1 | Clone: cache Chatterbox conditioning as safetensors in the clip store (consent-gated, sha+pin keyed) |
| [#73](https://github.com/galion-studio/galion-one-voice/pull/73) | 2026-11-05T03:39:51+01:00 | `d729f0000855a3886e78404db7301f96f1a65fcb` | yes | +171/−0, 1 files | success×1 | success×1 | Plan: re-anchor V2 on a PersonaPlex-style integrated dual-band model with RL (doc only, owner item 9) |
| [#74](https://github.com/galion-studio/galion-one-voice/pull/74) | 2026-11-05T03:42:00+01:00 | `bbb34376526e46d338b14b4a30b96cfa5bcc196f` | yes | +600/−0, 150 files | success×1 | success×1 | docs: add one-line G.A.L.I.O.N. footer to README and Markdown docs |
| [#76](https://github.com/galion-studio/galion-one-voice/pull/76) | 2026-11-05T03:43:58+01:00 | `4245a80553e20eec862ae04d24ccba5510af09cf` | yes | +742/−21, 11 files | success×1 | failure×1 | hardtest-sweep: ban 'so sorry' class, in-house reflect floor, Jarvis hub on playback, nerve thought trace; pinned persona drives the mouth |
| [#77](https://github.com/galion-studio/galion-one-voice/pull/77) | 2026-11-05T03:49:09+01:00 | `8bcbf85ba285c93c46b1fe56c2f7033985142b63` | yes | +2827/−18, 27 files | success×1 | failure×1 | V2: persona mood (PersonaPlex-style) + persona-conditioned Piper voice; premium Piper voice eval + listening-set results |
| [#79](https://github.com/galion-studio/galion-one-voice/pull/79) | 2026-11-05T03:49:33+01:00 | `3f9ae80e4d3271586f709b2c3e6490e885cf4a6a` | yes | +10806/−205, 256 files | success×1 | failure×1 | V2 voice screen: pure voice-to-voice (one dot + one full-screen object, no text/UI) + integration of the V2 PR stack (deployed build) |
| [#80](https://github.com/galion-studio/galion-one-voice/pull/80) | 2026-11-05T03:49:45+01:00 | `2302be1fd943d398135507bf1064a6b0bec6061f` | yes | +19513/−0, 259 files | success×1 | failure×1 | docs: V2 sprint results, bake-off t648 report + scoring tools, research notes, listening set |
| [#81](https://github.com/galion-studio/galion-one-voice/pull/81) | 2026-11-05T03:57:12+01:00 | `b72e6a5da3d8646a0b0d1cf53fe63fa658c27b55` | yes | +509/−1, 6 files | success×1 | failure×1 | galion-clone-cbx: Chatterbox Turbo ONNX fp32 clone worker (separate unit, loopback, consent-gated, off during live calls) |
| [#82](https://github.com/galion-studio/galion-one-voice/pull/82) | 2026-11-05T04:23:27+01:00 | `3479f23fa4e3063f73408252f11d6e9554e9b850` | yes | +19/−3, 2 files | success×1 | failure×1 | galion-clone-cbx: consent gate without Nano weight check; CC0 clip store in unit |
| [#84](https://github.com/galion-studio/galion-one-voice/pull/84) | 2026-11-06T07:12:46+01:00 | `7e8819dcd619b76d7989fa0be840a1923bdfd442` | yes | +12457/−0, 80 files | success×1 | failure×1 | cloning-v2: emotion-blend tooling (Samantha coherent-rotation) |
| [#89](https://github.com/galion-studio/galion-one-voice/pull/89) | 2026-11-07T13:32:34+01:00 | `4e19b4b4052d1a9ac864547080fe443332891c4b` | yes | +1360/−0, 4 files | success×1 | failure×1 | Emotion V2 A: Samantha 12-emotion taxonomy |
| [#90](https://github.com/galion-studio/galion-one-voice/pull/90) | 2026-11-07T13:35:46+01:00 | `d07ab70b663faef53f51d6d4fed21dccdc204aec` | yes | +2343/−0, 42 files | success×2 | failure×1, success×1 | Emotion V2 C: per-emotion CI test harness |
| [#91](https://github.com/galion-studio/galion-one-voice/pull/91) | 2026-11-07T13:36:08+01:00 | `5ba85c347d8f63a821dceb2a47b7dbcbad7022e1` | yes | +3909/−0, 28 files | success×1 | failure×1, success×1 | Emotion V2 D: generic training-data pipeline |
| [#94](https://github.com/galion-studio/galion-one-voice/pull/94) | 2026-11-07T14:43:55+01:00 | `f648ca7328855478991d3cf93756477116aac52b` | yes | +569/−0, 2 files | success×2 | failure×1, success×1 | docs: regenerated Emotion V2 foundation spec and landscape |
| [#93](https://github.com/galion-studio/galion-one-voice/pull/93) | 2026-11-07T14:49:48+01:00 | `bec463dcaf3b8cd31ce76a68b7fad2371b5038d9` | yes | +3509/−4, 16 files | success×2 | success×2 | Emotion V2 B: controlled reference capture + checker |
| [#95](https://github.com/galion-studio/galion-one-voice/pull/95) | 2026-11-07T15:18:39+01:00 | `1a2be8529406c05154e4676ae50f8e585f585477` | yes | +36/−5, 1 files | success×2 | failure×1, success×1 | Reap a Piper child that exits before the first audio |
| [#92](https://github.com/galion-studio/galion-one-voice/pull/92) | 2026-11-07T15:18:43+01:00 | `9c2b94d6a68d6fabff86397070559609ac4545dc` | yes | +57/−5, 2 files | success×2 | success×2 | Emotion V2 handoff: starting point for new account |
| [#85](https://github.com/galion-studio/galion-one-voice/pull/85) | 2026-11-07T15:18:48+01:00 | `b9bd890a304656188993022d3a64e37c28d21abe` | yes | +606583/−39, 1408 files | success×2 | success×2 | save: cloning-v2 persona scorer and pipeline sources |
| [#88](https://github.com/galion-studio/galion-one-voice/pull/88) | 2026-11-07T16:00:57+01:00 | `754643b7aab24a8c02bb65bb118aa6b05ec1e277` | yes | +8225/−668, 78 files | success×2 | success×2 | Symphony V2 emotion42: unified symphony stack (alpha) |
| [#99](https://github.com/galion-studio/galion-one-voice/pull/99) | 2026-11-07T16:03:25+01:00 | `598e1da815176928ec1071c23a410ee8803112c4` | yes | +1236/−22, 12 files | success×2 | success×2 | Fix heartbeat budget flake and add laptop runbooks |
| [#98](https://github.com/galion-studio/galion-one-voice/pull/98) | 2026-11-07T16:04:22+01:00 | `6e9eb85cc3987a7ec0b37a62fee6080408792820` | yes | +1528/−0, 12 files | success×2 | failure×1, success×1 | Add the Nebula live emotion feed |
| [#100](https://github.com/galion-studio/galion-one-voice/pull/100) | 2026-11-07T17:58:08+01:00 | `b7049d99879f2ffde0459ed14f76a71dc9e6087b` | yes | +889/−4, 6 files | success×2 | success×2 | Add offline nebula emotion scoring beside emotion_match |
| [#101](https://github.com/galion-studio/galion-one-voice/pull/101) | 2026-11-07T17:58:17+01:00 | `8d6e55646916badd26f2f9309b9cff45a41695ad` | yes | +6508/−1, 36 files | success×3 | success×3 | Nebula Galion Voice V2 (draft) |
| [#102](https://github.com/galion-studio/galion-one-voice/pull/102) | 2026-11-07T20:02:31+01:00 | `3e1f5a12c53cd8e0dd47d111676d61d608540559` | yes | +1044/−58, 14 files | success×3 | success×3 | Fix phone canvas sharpness and spectrum preview scale |

## Per pull request

### #1 Add the V1 one-dot demo, README, and MIT LICENSE

- URL: https://github.com/galion-studio/galion-one-voice/pull/1
- Merged: 2026-11-02T19:00:50+01:00 (UTC 2026-11-02T18:00:50Z)
- Base `main`, head branch `cursor/v1-demo-readme-b954`
- Merge commit `8a6636de823cd487190c1989c104eaaa981e81da`, head `3747bcc9ee633a3bfc468b3712f21312fbaa0ff2`
- GitHub pull diff: +31940 / −1 across 262 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 262 files changed, 31940 insertions(+), 1 deletion(-) (262 paths; list capped at 80 in the JSON).
- Head: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- No stored Actions evidence. Green-at-merge: **unknown**.

### #2 Add CREDENTIALS.md and CONTRIBUTIONS.md

- URL: https://github.com/galion-studio/galion-one-voice/pull/2
- Merged: 2026-11-02T19:02:19+01:00 (UTC 2026-11-02T18:02:19Z)
- Base `main`, head branch `cursor/credentials-contributions-b954`
- Merge commit `253c1d1a9cee4250be14ed2f153c3a59d8ebeadd`, head `aa9bc0f78f574278e7491e17f74ae836379d57a3`
- GitHub pull diff: +161 / −0 across 2 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 2 files changed, 161 insertions(+) (2 paths; list capped at 80 in the JSON).
- Head: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- No stored Actions evidence. Green-at-merge: **unknown**.

### #3 Galion Voice V2 plan + Plan V5 + phone path + orb foundation

- URL: https://github.com/galion-studio/galion-one-voice/pull/3
- Merged: 2026-11-02T22:19:49+01:00 (UTC 2026-11-02T21:19:49Z)
- Base `main`, head branch `cursor/galion-voice-v2-f1e5`
- Merge commit `9a5ebe187097f6810c44cec515eec90ae828b467`, head `9a5ebe187097f6810c44cec515eec90ae828b467`
- GitHub pull diff: +4281 / −194 across 33 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #4 Add the CPU orb spring foundation (V2-S2)

- URL: https://github.com/galion-studio/galion-one-voice/pull/4
- Merged: 2026-11-03T01:55:45+01:00 (UTC 2026-11-03T00:55:45Z)
- Base `main`, head branch `cursor/orb-animation-engine-074a`
- Merge commit `8973e6fb8620685a8b5f3794c9e58f630f61f2a1`, head `7db837ea8d47231c3175e0b84cf68709cf4701b4`
- GitHub pull diff: +1125 / −11 across 11 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #5 Add an offline loopback virtual user for presence-eval T0–T2

- URL: https://github.com/galion-studio/galion-one-voice/pull/5
- Merged: 2026-11-03T02:12:44+01:00 (UTC 2026-11-03T01:12:44Z)
- Base `main`, head branch `cursor/presence-eval-virtual-user-9e19`
- Merge commit `ba3ce81b9e22dc5e492b4489ed2238379f61af2e`, head `ab2c2861df198a7bd887accc47177a8a00c89498`
- GitHub pull diff: +1989 / −7 across 19 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #6 Harden the Galion Voice edge (V2-S3)

- URL: https://github.com/galion-studio/galion-one-voice/pull/6
- Merged: 2026-11-03T02:21:20+01:00 (UTC 2026-11-03T01:21:20Z)
- Base `main`, head branch `cursor/v2-s3-security-edge-ea77`
- Merge commit `80b6fdedac0daa94564b254044c7c3d81153d5a2`, head `9bc4c706e10f20e7003f4031b3c8ad5d5fe6cabc`
- GitHub pull diff: +1289 / −97 across 14 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #9 Warm Piper worker on the phone lane (V2-S6)

- URL: https://github.com/galion-studio/galion-one-voice/pull/9
- Merged: 2026-11-03T02:54:56+01:00 (UTC 2026-11-03T01:54:56Z)
- Base `main`, head branch `cursor/warm-piper-s6-bfb2`
- Merge commit `8f97cabf31bfad95b9e9a5395131b33f8150d041`, head `0803c0bb662dd85b2c86dcc3214cf0ff7ca8bf9a`
- GitHub pull diff: +1062 / −62 across 15 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #10 Record V2 owner decisions: Piper pin, Vosk A, Chatterbox

- URL: https://github.com/galion-studio/galion-one-voice/pull/10
- Merged: 2026-11-03T03:20:19+01:00 (UTC 2026-11-03T02:20:19Z)
- Base `main`, head branch `cursor/owner-approvals-v2-1632`
- Merge commit `a75e065ff685eef873a8db84b2ac1245ebf49029`, head `c56585edd90870e7d694fa35e5ce647f6ed206c8`
- GitHub pull diff: +441 / −95 across 16 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #11 Add the Open Mind nerve and cortex seam

- URL: https://github.com/galion-studio/galion-one-voice/pull/11
- Merged: 2026-11-03T04:32:55+01:00 (UTC 2026-11-03T03:32:55Z)
- Base `main`, head branch `cursor/openmind-nerve-cortex-9f19`
- Merge commit `12108e16ae1b7b93bf10668c59cb1b6670d6524e`, head `105f929028be80172871cce19cfb98a07ebdf6a8`
- GitHub pull diff: +1680 / −2 across 18 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #13 Add the empathy weight to prosody and the background wash

- URL: https://github.com/galion-studio/galion-one-voice/pull/13
- Merged: 2026-11-03T04:33:13+01:00 (UTC 2026-11-03T03:33:13Z)
- Base `main`, head branch `cursor/empathy-voice-color-4169`
- Merge commit `e362050f45abefa490d2ea4d623bf817dc5d1353`, head `04ffe43242e949f4f75fe3a0406cfff7ab5be11a`
- GitHub pull diff: +413 / −35 across 11 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #7 Add the V2-S1 process bus next to the turn worker

- URL: https://github.com/galion-studio/galion-one-voice/pull/7
- Merged: 2026-11-03T04:35:45+01:00 (UTC 2026-11-03T03:35:45Z)
- Base `main`, head branch `cursor/v2-s1-process-bus-b8ed`
- Merge commit `5ab3d38f55975eb75f6053dfc5643f1da7e1d57d`, head `31922895a60ff1e8ae2c393149fa5345dbfc1c7f`
- GitHub pull diff: +1454 / −70 across 5 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #8 Galion Voice V2 research docs (2026-11-03)

- URL: https://github.com/galion-studio/galion-one-voice/pull/8
- Merged: 2026-11-03T04:37:47+01:00 (UTC 2026-11-03T03:37:47Z)
- Base `main`, head branch `cursor/galion-voice-v2-research-docs-3e48`
- Merge commit `5fd164210ea3329d9f32be63baab9201767d3054`, head `fef5e04e535d8400732c29ca133abf0b6f95237b`
- GitHub pull diff: +1239 / −0 across 18 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #12 Name the Luti easing profile and count the orb step in-process

- URL: https://github.com/galion-studio/galion-one-voice/pull/12
- Merged: 2026-11-03T04:51:23+01:00 (UTC 2026-11-03T03:51:23Z)
- Base `main`, head branch `cursor/one-ui-luti-engine-4351`
- Merge commit `b584a56db60b0e77ef185b6b8b1bd6123fed6689`, head `c6bcfac45962e8d02b2a74fcb67477e49da1d9f7`
- GitHub pull diff: +497 / −64 across 16 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #15 Scaffold the Galion Voice V2-S14 clone worker and Persona bus body

- URL: https://github.com/galion-studio/galion-one-voice/pull/15
- Merged: 2026-11-03T04:53:38+01:00 (UTC 2026-11-03T03:53:38Z)
- Base `main`, head branch `cursor/v2-s14-clone-scaffold-447b`
- Merge commit `d1db9a401ab134d3a2f504ed7de82123e4f93ee7`, head `3b587f58bb865d9ea26c7a530e8fc23a69a9d4b9`
- GitHub pull diff: +1965 / −66 across 23 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #14 Add Galion Voice persona instruction files

- URL: https://github.com/galion-studio/galion-one-voice/pull/14
- Merged: 2026-11-03T05:21:59+01:00 (UTC 2026-11-03T04:21:59Z)
- Base `main`, head branch `cursor/voice-persona-instructions-8735`
- Merge commit `eb3e80ef2f751241f5d1b91b1551b2ecbfd1a807`, head `2aa7b418e8e1fefcefb6ecb59e5b807c79abe809`
- GitHub pull diff: +539 / −0 across 8 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 8 files changed, 539 insertions(+) (8 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #17 Draft Samantha and Jarvis V2 voice configs

- URL: https://github.com/galion-studio/galion-one-voice/pull/17
- Merged: 2026-11-03T05:22:05+01:00 (UTC 2026-11-03T04:22:05Z)
- Base `main`, head branch `cursor/persona-voice-configs-790b`
- Merge commit `04cc193c78e78cdc18bff7e3d3c74a6f1279b7ca`, head `c0dd0f77e6601af794785f49870cc58318ea1bcd`
- GitHub pull diff: +276 / −0 across 7 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 7 files changed, 276 insertions(+) (7 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #18 Add the V2-S14 rights-clearance checklist for Samantha and Jarvis

- URL: https://github.com/galion-studio/galion-one-voice/pull/18
- Merged: 2026-11-03T05:28:24+01:00 (UTC 2026-11-03T04:28:24Z)
- Base `main`, head branch `cursor/rights-clearance-checklist-cf62`
- Merge commit `8ea7a6561ecd29a284fa988b1f8cdc54db100f34`, head `c3830a0663df25b7012722313b064c61f6549b14`
- GitHub pull diff: +377 / −4 across 7 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 7 files changed, 377 insertions(+), 4 deletions(-) (7 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #19 Document the closed film-clip ingest outline

- URL: https://github.com/galion-studio/galion-one-voice/pull/19
- Merged: 2026-11-03T05:29:28+01:00 (UTC 2026-11-03T04:29:28Z)
- Base `main`, head branch `cursor/clip-ingest-outline-1391`
- Merge commit `4ecf0d6f5946275a94d9db28305538cbe2db1c60`, head `845f46ca4bc882f09f7084a3f3442fa1afc8ddb2`
- GitHub pull diff: +269 / −4 across 6 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 6 files changed, 270 insertions(+), 5 deletions(-) (6 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #16 Document the Galion Voice V2 error state

- URL: https://github.com/galion-studio/galion-one-voice/pull/16
- Merged: 2026-11-03T05:42:58+01:00 (UTC 2026-11-03T04:42:58Z)
- Base `main`, head branch `cursor/v2-error-state-250f`
- Merge commit `eed19730fd989ab1d0d9aa698723e6d169c7373d`, head `bdf5e1fad354316d068c465615749a531bf98990`
- GitHub pull diff: +296 / −1 across 7 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 7 files changed, 296 insertions(+), 1 deletion(-) (7 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #21 Keep the one-dot mark a calm near-white circle

- URL: https://github.com/galion-studio/galion-one-voice/pull/21
- Merged: 2026-11-03T05:46:22+01:00 (UTC 2026-11-03T04:46:22Z)
- Base `main`, head branch `cursor/calm-circle-mark-2658`
- Merge commit `4f537b48a785d61578519719f62646d2b263ad69`, head `6dbf97f00a4b8a8c1a7a45101aba8167130bdd54`
- GitHub pull diff: +319 / −128 across 11 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 11 files changed, 384 insertions(+), 341 deletions(-) (11 paths; list capped at 80 in the JSON).
- Head: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).

### #22 Add the V2 persona dot and mood wash

- URL: https://github.com/galion-studio/galion-one-voice/pull/22
- Merged: 2026-11-03T06:03:36+01:00 (UTC 2026-11-03T05:03:36Z)
- Base `main`, head branch `cursor/v2-persona-demo-eval-4481`
- Merge commit `986a6dbd331003350cbb5e9e83b19b2dcc3a8db1`, head `215943ef06dc1b7f152f5ed01de555c693eff679`
- GitHub pull diff: +1485 / −30 across 17 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 17 files changed, 1485 insertions(+), 30 deletions(-) (17 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #26 Use out-of-process Vosk for the phone mic

- URL: https://github.com/galion-studio/galion-one-voice/pull/26
- Merged: 2026-11-03T09:50:07+01:00 (UTC 2026-11-03T08:50:07Z)
- Base `main`, head branch `cursor/vosk-mic-asr-2146`
- Merge commit `235481b0f2ffd24ed0e93eb571bfc77b6ed6b413`, head `12cb30d58d18454c696ed88f2e96aaf588465f2e`
- GitHub pull diff: +2086 / −113 across 27 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 27 files changed, 2086 insertions(+), 113 deletions(-) (27 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #25 One phone-ready circle: mic, text, persona, and Piper

- URL: https://github.com/galion-studio/galion-one-voice/pull/25
- Merged: 2026-11-03T09:50:35+01:00 (UTC 2026-11-03T08:50:35Z)
- Base `main`, head branch `cursor/one-circle-page-09a4`
- Merge commit `b1f5e0c5712f4498048b37d41a541a040787e74d`, head `dbb45142a9a286fee1822e99e6305ab493d7c083`
- GitHub pull diff: +634 / −309 across 12 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 12 files changed, 634 insertions(+), 309 deletions(-) (12 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #51 Fetch Chatterbox clone weights from GitHub Release assets

- URL: https://github.com/galion-studio/galion-one-voice/pull/51
- Merged: 2026-11-03T12:54:36+01:00 (UTC 2026-11-03T11:54:36Z)
- Base `main`, head branch `cursor/clone-weights-release-f570`
- Merge commit `ed933060e80a4b665d376b8390e46f7078b655cc`, head `29923a01463259dfb26ad555dbca667aa5329b61`
- GitHub pull diff: +897 / −39 across 15 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #58 Piper CPU stream TTFA live demo (t301) — refs #54 #55

- URL: https://github.com/galion-studio/galion-one-voice/pull/58
- Merged: 2026-11-03T13:12:05+01:00 (UTC 2026-11-03T12:12:05Z)
- Base `main`, head branch `cursor/piper-stream-live-t301`
- Merge commit `8162456f988beffcdbe12015efbe015f9dfdb5e8`, head `8c92aef1f7b89758d54e79d296167508a347e088`
- GitHub pull diff: +274 / −0 across 3 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: n/a (? paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #27 Add the Plus SKU plan for Galion Voice V2 symphony

- URL: https://github.com/galion-studio/galion-one-voice/pull/27
- Merged: 2026-11-03T17:26:16+01:00 (UTC 2026-11-03T16:26:16Z)
- Base `main`, head branch `cursor/plus-sku-plan-25f5`
- Merge commit `4bca8c2e93df012ae4971378e8ef9fa3f48c380d`, head `4ccedb52c8ae646bb662b290fd47e950d3e04317`
- GitHub pull diff: +117 / −0 across 1 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1 file changed, 117 insertions(+) (1 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #28 Wire Eureka into the Galion Voice V2 symphony docs

- URL: https://github.com/galion-studio/galion-one-voice/pull/28
- Merged: 2026-11-03T17:26:24+01:00 (UTC 2026-11-03T16:26:24Z)
- Base `main`, head branch `cursor/symphony-eureka-dddb`
- Merge commit `efbde9bfdf7fe0321c79cd102f5593dc741c5e29`, head `8a79e1f68cc0f2e7d8f874b9626d1000360f9f8a`
- GitHub pull diff: +200 / −0 across 3 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 3 files changed, 200 insertions(+) (3 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #29 Add the Galion Voice V3 symphony plan (docs only)

- URL: https://github.com/galion-studio/galion-one-voice/pull/29
- Merged: 2026-11-03T17:26:26+01:00 (UTC 2026-11-03T16:26:26Z)
- Base `main`, head branch `cursor/v3-symphony-plan-10df`
- Merge commit `0dbe96a92baa751f984b230097a896e518ce822a`, head `9bdaa5c8a93a52c3203b90c8f6ef1cf00659b93e`
- GitHub pull diff: +396 / −0 across 1 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1 file changed, 396 insertions(+) (1 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #38 Compile Chatterbox PCM into a .gqvoice packet

- URL: https://github.com/galion-studio/galion-one-voice/pull/38
- Merged: 2026-11-03T17:26:28+01:00 (UTC 2026-11-03T16:26:28Z)
- Base `main`, head branch `cursor/gqvoice-compiler-3281`
- Merge commit `0d615c429bf5a0c07c50fa34c77d3203efc82d8c`, head `d210fba23cccf51d5122b1612b663212f6964274`
- GitHub pull diff: +1442 / −84 across 11 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 11 files changed, 1442 insertions(+), 84 deletions(-) (11 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #40 Ignore Samantha and Jarvis indoor voice references

- URL: https://github.com/galion-studio/galion-one-voice/pull/40
- Merged: 2026-11-03T17:26:30+01:00 (UTC 2026-11-03T16:26:30Z)
- Base `main`, head branch `cursor/ignore-samantha-jarvis-voice-0a39`
- Merge commit `b01c80508c0032c85886c7b6eca0cbef940b1baa`, head `7849b868c449e355b1bc7c5ce610d5235face27e`
- GitHub pull diff: +37 / −1 across 2 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 2 files changed, 37 insertions(+), 1 deletion(-) (2 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #46 Document Symphony puzzle pieces (MIT / Apache inventory)

- URL: https://github.com/galion-studio/galion-one-voice/pull/46
- Merged: 2026-11-03T17:26:32+01:00 (UTC 2026-11-03T16:26:32Z)
- Base `main`, head branch `cursor/symphony-puzzle-pieces-c6f2`
- Merge commit `179a884e50542165645e2d1a7623925834c9e26c`, head `bf41952001a4dbcc0ab3221cb7bf61717a621999`
- GitHub pull diff: +440 / −0 across 1 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1 file changed, 440 insertions(+) (1 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #50 Wire the Samantha and Jarvis voice-to-voice outline

- URL: https://github.com/galion-studio/galion-one-voice/pull/50
- Merged: 2026-11-03T17:26:34+01:00 (UTC 2026-11-03T16:26:34Z)
- Base `main`, head branch `cursor/v2v-showcase-emotion-6430`
- Merge commit `e173fbb8ff968fd6570c18b3c67565958d2d2c78`, head `be8c9f3958c6f6a7bdd3a40f2af16da62e92c388`
- GitHub pull diff: +834 / −3 across 8 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 8 files changed, 834 insertions(+), 3 deletions(-) (8 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #57 Add Galion Voice V3 planning docs

- URL: https://github.com/galion-studio/galion-one-voice/pull/57
- Merged: 2026-11-03T17:26:38+01:00 (UTC 2026-11-03T16:26:38Z)
- Base `main`, head branch `cursor/galion-voice-v3-plan-ac11`
- Merge commit `0f3c463b9155682b35267593046b34d689e1b162`, head `ce1bc0f25706c1f5bb7313d413f0e4509524ca24`
- GitHub pull diff: +892 / −0 across 8 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 8 files changed, 892 insertions(+) (8 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #59 Merge open PRs #20 #30-36 #39 #41 #42 #45 #52 (markdown conflicts resolved by union)

- URL: https://github.com/galion-studio/galion-one-voice/pull/59
- Merged: 2026-11-03T18:31:05+01:00 (UTC 2026-11-03T17:31:05Z)
- Base `main`, head branch `swarm/merge-all-prs`
- Merge commit `ac6065ce600ac93eaab753f06eab939a86e46da3`, head `5e7a9232ff6a8719265b5721baab09b1d58f0df1`
- GitHub pull diff: +10898 / −174 across 97 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 97 files changed, 10898 insertions(+), 174 deletions(-) (97 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #30 Plan Titanaxe staging CPU cascade (Plus CX23 / Ultra CX43)

- URL: https://github.com/galion-studio/galion-one-voice/pull/30
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/staging-cpu-cascade-a357`
- Merge commit `44b334792ae9d51ac75ed63699f31f52847fca28`, head `1de760b0442cc7fe422476c312d892e50b1d9093`
- GitHub pull diff: +237 / −2 across 4 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #52 Add a hard Galion Voice loop harness and schedule the Admin UI check

- URL: https://github.com/galion-studio/galion-one-voice/pull/52
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/hard-loop-harness-02fb`
- Merge commit `88e9796c4e44861a251b627ae79f3a270d93705d`, head `a1568c634f5d5c5d4482ba924b6adcf1695ab91d`
- GitHub pull diff: +1857 / −4 across 10 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #45 Wire V6 one-percent engine metrics into symphony

- URL: https://github.com/galion-studio/galion-one-voice/pull/45
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/symphony-one-percent-ecbe`
- Merge commit `db6835dde9ac11a1f16431c061a5ae8682c9bf55`, head `39991df53dd9974e9cf28fcd951cd9a74919b443`
- GitHub pull diff: +1317 / −4 across 14 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #42 Compile clone PCM into a Galion .gq voice clip

- URL: https://github.com/galion-studio/galion-one-voice/pull/42
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/voice-clip-compiler-0fb1`
- Merge commit `bb821dbb654f2af46f3b2c7c3c15946919ad3f88`, head `d616e5e2100f9864bb3b2efaaf504c06db299c6f`
- GitHub pull diff: +1776 / −2 across 12 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #41 Land Chatterbox Nano clone wiring and Galion clip emit

- URL: https://github.com/galion-studio/galion-one-voice/pull/41
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/clone-landing-7174`
- Merge commit `08698c62fbc5b53b178be1287eddd17c149fa50e`, head `9927fe14e7949804e389d86d86b3bcf5a53825bb`
- GitHub pull diff: +1659 / −147 across 22 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #39 Add Eureka self-eval gates for symphony A–G

- URL: https://github.com/galion-studio/galion-one-voice/pull/39
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/symphony-self-eval-ag-19ce`
- Merge commit `dc07f4ffad3f42efe42c3ab196d00cd2b0aafd7a`, head `155ef6046da44282705a8b31cdec536e6a42817e`
- GitHub pull diff: +690 / −0 across 10 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #36 Plan the Galion Research console as its own async desk

- URL: https://github.com/galion-studio/galion-one-voice/pull/36
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/research-console-plan-20b3`
- Merge commit `5113ec1dd9e2d2770c3f65f6f37b7b78400bd330`, head `30c1decf0345b2f0a9be4ee19c30c8fcd5558d8e`
- GitHub pull diff: +314 / −0 across 2 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #35 TTFA gate: 80 ms live-sector frame, presence onset / barge-in / e-to-a

- URL: https://github.com/galion-studio/galion-one-voice/pull/35
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/ttfa-streaming-gates-f57c`
- Merge commit `322f6ecca2ddf632eecdf08017b0696ebf664e8f`, head `39e87a2f5bd55c303514655f5054b5be75bff6ce`
- GitHub pull diff: +967 / −0 across 9 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #34 One-dot auto-record proceeds on the mic path

- URL: https://github.com/galion-studio/galion-one-voice/pull/34
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/one-dot-auto-workflow-9169`
- Merge commit `e833dbdf87915e96c897c70f772ea17f14a37509`, head `627ce857f901d77e6013031c62722bfc82447b43`
- GitHub pull diff: +373 / −5 across 11 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #33 Document Samantha emotions as live prosody, not a script

- URL: https://github.com/galion-studio/galion-one-voice/pull/33
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/samantha-emotions-sounds-a799`
- Merge commit `aa41d959be94f547e06c9ca9e3738af01e55c869`, head `77f51127415424de5dac9df7ca6810ab4ea1b71a`
- GitHub pull diff: +279 / −0 across 2 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #32 Add the Galion Voice V2 symphony index

- URL: https://github.com/galion-studio/galion-one-voice/pull/32
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/symphony-index-9cab`
- Merge commit `372c5d0cc89990bb79d9d581c1388346023d6250`, head `b54e44fc6acdf66cf60f11c0cf210650be44cbcd`
- GitHub pull diff: +967 / −0 across 19 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #31 Add the Ultra symphony host plan ($220, CX43-class)

- URL: https://github.com/galion-studio/galion-one-voice/pull/31
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/ultra-symphony-plan-0ef9`
- Merge commit `73662830da106b82cc6c1b9bc3cbb6566df44382`, head `c53dc3325658303e5b86e64da5267abbc09654ef`
- GitHub pull diff: +289 / −7 across 6 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #20 Plan the Chatterbox Nano out-of-process clone worker

- URL: https://github.com/galion-studio/galion-one-voice/pull/20
- Merged: 2026-11-03T18:31:07+01:00 (UTC 2026-11-03T17:31:07Z)
- Base `main`, head branch `cursor/chatterbox-worker-plan-b1b0`
- Merge commit `071a73f5e6eae55a09ab75df2e5189db8e273cea`, head `92975fc5951e7b6b65b4f826c3d87fb828c10dab`
- GitHub pull diff: +231 / −3 across 6 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- No stored Actions evidence. Green-at-merge: **unknown**.

### #60 Merge open PRs #37 #44 #47 #48 #49 (code conflicts resolved)

- URL: https://github.com/galion-studio/galion-one-voice/pull/60
- Merged: 2026-11-03T18:32:30+01:00 (UTC 2026-11-03T17:32:30Z)
- Base `main`, head branch `swarm/merge-code-prs`
- Merge commit `cc4e6e81da3d2f331feaa236d240c60a7a78e027`, head `5c936ac77a480ad6e75739732ff26906044cbec4`
- GitHub pull diff: +8151 / −38 across 73 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 73 files changed, 8151 insertions(+), 38 deletions(-) (73 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #48 Translate clone interchange into the Galion symphony load path

- URL: https://github.com/galion-studio/galion-one-voice/pull/48
- Merged: 2026-11-03T18:32:32+01:00 (UTC 2026-11-03T17:32:32Z)
- Base `main`, head branch `cursor/symphony-rust-translation-f704`
- Merge commit `7366a3382ad321e6d46a61b4a308e190c4302fab`, head `8dee2457eccefe0bfcae37fc4c2b1687fa2cc308`
- GitHub pull diff: +4052 / −1 across 22 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #49 Sit clone, persona, and the one dot on the 80 ms heartbeat

- URL: https://github.com/galion-studio/galion-one-voice/pull/49
- Merged: 2026-11-03T18:32:32+01:00 (UTC 2026-11-03T17:32:32Z)
- Base `main`, head branch `cursor/symphony-heartbeat-wire-be0f`
- Merge commit `b8a981af75c123722ef8bcf27f87a03ecb200502`, head `e58381092067042413380bc4552a39e53da84ca7`
- GitHub pull diff: +977 / −25 across 17 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #44 Add a toggleable Jarvis hub texture layer

- URL: https://github.com/galion-studio/galion-one-voice/pull/44
- Merged: 2026-11-03T18:32:32+01:00 (UTC 2026-11-03T17:32:32Z)
- Base `main`, head branch `cursor/jarvis-hub-texture-88e1`
- Merge commit `cefd7120a89e6399c4cc7e6b77fc80098807cd3f`, head `98506005ed776475ca544d140b34acbe10b91782`
- GitHub pull diff: +1245 / −2 across 12 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #37 Persona feeling on the heartbeat, one-dot workflow hook

- URL: https://github.com/galion-studio/galion-one-voice/pull/37
- Merged: 2026-11-03T18:32:32+01:00 (UTC 2026-11-03T17:32:32Z)
- Base `main`, head branch `cursor/symphony-persona-onedot-5a4c`
- Merge commit `90307fee74cb5d20f8a87fd40038116bbaad9927`, head `c49a9bc7978fd2c94f66a9a29301b897b811550d`
- GitHub pull diff: +892 / −8 across 13 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #47 Add symphony runbooks and the 80 ms self-eval

- URL: https://github.com/galion-studio/galion-one-voice/pull/47
- Merged: 2026-11-03T18:32:34+01:00 (UTC 2026-11-03T17:32:34Z)
- Base `main`, head branch `cursor/symphony-runbooks-self-eval-6f36`
- Merge commit `927abeae1635908699b97fc032f4152086b31212`, head `50d835589e5fe2a924d6020062c8b4064c6592c9`
- GitHub pull diff: +1060 / −3 across 20 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #61 Merge clone cluster + Hermes PRs (#53 #55 #54 #43 #56 #24 #23)

- URL: https://github.com/galion-studio/galion-one-voice/pull/61
- Merged: 2026-11-03T19:11:41+01:00 (UTC 2026-11-03T18:11:41Z)
- Base `main`, head branch `swarm/merge-clone-prs`
- Merge commit `5114f6550896429f69af352f2943093424842264`, head `3f39560de36b2a3cf0483a7d1abee3011b6d5bc2`
- GitHub pull diff: +7898 / −497 across 69 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 69 files changed, 7898 insertions(+), 497 deletions(-) (69 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #55 Drain one Chatterbox PCM frame on the 80 ms heartbeat

- URL: https://github.com/galion-studio/galion-one-voice/pull/55
- Merged: 2026-11-03T19:11:43+01:00 (UTC 2026-11-03T18:11:43Z)
- Base `main`, head branch `cursor/chatterbox-pcm-drain-06c3`
- Merge commit `7a73229cc5b89a64d445b054dded1636d9a0e644`, head `8132c07612496395f546afe5d07f2556e0cb92d1`
- GitHub pull diff: +1993 / −39 across 24 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #54 Stream TTFA: Piper script is the live path, stub clocks fail

- URL: https://github.com/galion-studio/galion-one-voice/pull/54
- Merged: 2026-11-03T19:11:43+01:00 (UTC 2026-11-03T18:11:43Z)
- Base `main`, head branch `cursor/chatterbox-nano-stream-ttfa-e7db`
- Merge commit `d6d604a91a90504dda9bb93cb1315a7cd7832276`, head `29e12be5f83fa530f2eae50ce4d5164a7d4ada0c`
- GitHub pull diff: +1993 / −81 across 21 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #43 Unlock the clip store on the t273 private clearance

- URL: https://github.com/galion-studio/galion-one-voice/pull/43
- Merged: 2026-11-03T19:11:43+01:00 (UTC 2026-11-03T18:11:43Z)
- Base `main`, head branch `cursor/clip-store-rights-t273-bfd2`
- Merge commit `5fa00771f21dfd75097a7796f24e23623c7cc691`, head `db5f8917ba807fb6e45977c01241778a3126d6cc`
- GitHub pull diff: +1253 / −49 across 20 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #24 Run Chatterbox Nano out of process, gated on consented refs

- URL: https://github.com/galion-studio/galion-one-voice/pull/24
- Merged: 2026-11-03T19:11:43+01:00 (UTC 2026-11-03T18:11:43Z)
- Base `main`, head branch `cursor/chatterbox-nano-worker-9932`
- Merge commit `d3604147f0ef89f1d59d94a76a17f970a5bb0539`, head `7f8e107448e35dbc3003a3cf231942dc0bd4a6dd`
- GitHub pull diff: +1310 / −129 across 19 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #23 Wire Hermes into the live reply path

- URL: https://github.com/galion-studio/galion-one-voice/pull/23
- Merged: 2026-11-03T19:11:43+01:00 (UTC 2026-11-03T18:11:43Z)
- Base `main`, head branch `cursor/hermes-live-llm-716b`
- Merge commit `3f39560de36b2a3cf0483a7d1abee3011b6d5bc2`, head `df24be24697241692dd210213d5afe0d265dc3ba`
- GitHub pull diff: +1340 / −234 across 14 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #56 Make Chatterbox Nano the V2 clone backend, Piper fallback

- URL: https://github.com/galion-studio/galion-one-voice/pull/56
- Merged: 2026-11-03T19:11:43+01:00 (UTC 2026-11-03T18:11:43Z)
- Base `main`, head branch `cursor/nano-v2-backend-2808`
- Merge commit `21932c77137d026ef8ca357d126f5280d49c72f5`, head `ced36aef622f8bbfbab1625c108177c0fb0c5775`
- GitHub pull diff: +1209 / −122 across 17 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #53 Wire Chatterbox Nano generate in the clone worker

- URL: https://github.com/galion-studio/galion-one-voice/pull/53
- Merged: 2026-11-03T19:11:44+01:00 (UTC 2026-11-03T18:11:44Z)
- Base `main`, head branch `cursor/chatterbox-nano-worker-90a2`
- Merge commit `dda7a60106d4bf88e5e3ad6d9311e7d3d1c56639`, head `c6e440e805d173629e449366812dc465758d20a4`
- GitHub pull diff: +1222 / −124 across 15 files. That is the pull's own diff, not always the first-parent delta on `main`.
- Not a first-parent merge of `main`. The commit exists, and the files reached `main` through a later rollup merge. The pull diff above is the change set; the first-parent delta is on the rollup.
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: no Actions check-runs stored. Legacy status `pending`. Green-at-merge: **unknown**.
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #63 Consent check at clone time only (Rust, in process); no per-turn consent subprocess

- URL: https://github.com/galion-studio/galion-one-voice/pull/63
- Merged: 2026-11-04T03:01:27+01:00 (UTC 2026-11-04T02:01:27Z)
- Base `main`, head branch `swarm/consent-at-clone`
- Merge commit `3b1acc9e9c2c44d76f125bed1c84727bac57579c`, head `fdf06482faf61f6ad3339d8099b1d0ccd8e75465`
- GitHub pull diff: +1919 / −69 across 27 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 27 files changed, 1919 insertions(+), 69 deletions(-) (27 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #62 latency-240/01: galion-piper-rt (ORT 1.31.1) default Piper runtime + streaming/pre-warm (owner t613s2992)

- URL: https://github.com/galion-studio/galion-one-voice/pull/62
- Merged: 2026-11-04T07:46:27+01:00 (UTC 2026-11-04T06:46:27Z)
- Base `main`, head branch `swarm/lat240-streaming-tts`
- Merge commit `5fe846b628f34a23744c9d6d174aa0d2517d63df`, head `d74d847ed3ebf013cd4b24352ca72310614a4373`
- GitHub pull diff: +1435 / −37 across 22 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 22 files changed, 1435 insertions(+), 37 deletions(-) (22 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #64 Hard-test harness: apply the six REPORT §5b fixes (local LLM run: HARD-FAIL, gaps named)

- URL: https://github.com/galion-studio/galion-one-voice/pull/64
- Merged: 2026-11-04T17:51:38+01:00 (UTC 2026-11-04T16:51:38Z)
- Base `main`, head branch `swarm/harness-v2-fixes`
- Merge commit `508cfe64d001fc1ec11419b9f7828815e5c3d2e5`, head `d51a08c7a6d8eb8995e081bd31f940e4f0eb1e77`
- GitHub pull diff: +279 / −38 across 2 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 2 files changed, 279 insertions(+), 38 deletions(-) (2 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #72 a21: build-piper-rt.sh builds and copies from the same target dir

- URL: https://github.com/galion-studio/galion-one-voice/pull/72
- Merged: 2026-11-05T03:36:34+01:00 (UTC 2026-11-05T02:36:34Z)
- Base `main`, head branch `swarm/piper-rt-target-fix`
- Merge commit `4b87ed5902be6c40a4ed008d26508c43a79015e9`, head `c263bf6d69f04318c52654c235c5fc5d1b760e49`
- GitHub pull diff: +6 / −1 across 1 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1 file changed, 6 insertions(+), 1 deletion(-) (1 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #66 latency-240 lane 02: V2 core map + ClonePlan CPU pinning (clone off during live calls)

- URL: https://github.com/galion-studio/galion-one-voice/pull/66
- Merged: 2026-11-05T03:36:41+01:00 (UTC 2026-11-05T02:36:41Z)
- Base `main`, head branch `swarm/lat240-core-pinning`
- Merge commit `9b6ffeed1830b188e8d4e6157155749ba67e8240`, head `d7832d6dd4166861ff92b5377254cef6f89abb1d`
- GitHub pull diff: +608 / −21 across 5 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 5 files changed, 608 insertions(+), 21 deletions(-) (5 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #68 Core map v3 (owner map): bus+retrieval 0, Vosk 1, in-house reasoning 2-5, Piper Samantha 6 / Jarvis 7; no Qwen

- URL: https://github.com/galion-studio/galion-one-voice/pull/68
- Merged: 2026-11-05T03:36:54+01:00 (UTC 2026-11-05T02:36:54Z)
- Base `main`, head branch `swarm/core-map-v3`
- Merge commit `fc4d72d7d66484b3853c93f668648820b6062c21`, head `6d5a4c583f5081876da18431887630e4939d141c`
- GitHub pull diff: +888 / −275 across 17 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 17 files changed, 888 insertions(+), 275 deletions(-) (17 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #65 latency-240 lane 04: pipeline (LLM at ASR final, first-clause cap fix, two-stage endpointer, warm client)

- URL: https://github.com/galion-studio/galion-one-voice/pull/65
- Merged: 2026-11-05T03:37:01+01:00 (UTC 2026-11-05T02:37:01Z)
- Base `main`, head branch `swarm/lat240-pipeline`
- Merge commit `86c5d4466f83b1687d4265a015497f645c5687b5`, head `44563f71da6c251837d1a31df64d0875b230b781`
- GitHub pull diff: +2154 / −0 across 18 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 18 files changed, 2154 insertions(+) (18 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #75 lat240 wt-14: GALION_LLM=canned local reply + in-tree Piper runtime default (eos->first audio 178.9/221.9 ms canned, n=24)

- URL: https://github.com/galion-studio/galion-one-voice/pull/75
- Merged: 2026-11-05T03:37:08+01:00 (UTC 2026-11-05T02:37:08Z)
- Base `main`, head branch `swarm/wt14-latency-sweep`
- Merge commit `3e6583a641c76a2d7bf89aa6413446640fbc1f54`, head `c036d929f10d4717153a4f8204e7a1205160b2a8`
- GitHub pull diff: +2158 / −0 across 18 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1 file changed, 8 insertions(+), 1 deletion(-) (1 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #67 latency-240 lane 03: Chatterbox Nano clone efficiency (bench-only; q8t3+s1 RTF 0.553→0.358)

- URL: https://github.com/galion-studio/galion-one-voice/pull/67
- Merged: 2026-11-05T03:37:16+01:00 (UTC 2026-11-05T02:37:16Z)
- Base `main`, head branch `swarm/lat240-clone-efficiency`
- Merge commit `d12f9cc09706bf44ede068401d6565200bfde1c4`, head `722553e577803ebbf74f4c8a6745a235206b0c71`
- GitHub pull diff: +568 / −0 across 8 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 8 files changed, 568 insertions(+) (8 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #70 Clone: CC0 donors ON (owner t628u) + CC0 licence checkbox enforced per session; re-cut tool, TOKENIZER.sha256 fix, clone_probe

- URL: https://github.com/galion-studio/galion-one-voice/pull/70
- Merged: 2026-11-05T03:37:23+01:00 (UTC 2026-11-05T02:37:23Z)
- Base `main`, head branch `swarm/clone-unblock`
- Merge commit `6c706ff9353d11b54b5abf346d70d0bd3c72e313`, head `1cfb8bdaad6cc230630b9a5ba3bca148a9112962`
- GitHub pull diff: +1241 / −67 across 29 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 29 files changed, 1241 insertions(+), 67 deletions(-) (29 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #71 Clone: cache Chatterbox conditioning as safetensors in the clip store (consent-gated, sha+pin keyed)

- URL: https://github.com/galion-studio/galion-one-voice/pull/71
- Merged: 2026-11-05T03:39:45+01:00 (UTC 2026-11-05T02:39:45Z)
- Base `main`, head branch `swarm/clone-cache`
- Merge commit `382e2e45f9e13e658b4d0291c1e308f7b23c8ee1`, head `90fc1a8efc8ed59370be1ab00f230fd7c1f47ad4`
- GitHub pull diff: +1269 / −3 across 5 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 5 files changed, 1269 insertions(+), 3 deletions(-) (5 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #73 Plan: re-anchor V2 on a PersonaPlex-style integrated dual-band model with RL (doc only, owner item 9)

- URL: https://github.com/galion-studio/galion-one-voice/pull/73
- Merged: 2026-11-05T03:39:51+01:00 (UTC 2026-11-05T02:39:51Z)
- Base `main`, head branch `swarm/personaplex-reanchor`
- Merge commit `d729f0000855a3886e78404db7301f96f1a65fcb`, head `d88a40c4da431e74da264d428f3d0c6f0b345c7a`
- GitHub pull diff: +171 / −0 across 1 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1 file changed, 171 insertions(+) (1 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #74 docs: add one-line G.A.L.I.O.N. footer to README and Markdown docs

- URL: https://github.com/galion-studio/galion-one-voice/pull/74
- Merged: 2026-11-05T03:42:00+01:00 (UTC 2026-11-05T02:42:00Z)
- Base `main`, head branch `swarm/doc-footer`
- Merge commit `bbb34376526e46d338b14b4a30b96cfa5bcc196f`, head `bc94582f64b812abb8a8f2e622f4acd58219ae87`
- GitHub pull diff: +600 / −0 across 150 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 150 files changed, 600 insertions(+) (150 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #76 hardtest-sweep: ban 'so sorry' class, in-house reflect floor, Jarvis hub on playback, nerve thought trace; pinned persona drives the mouth

- URL: https://github.com/galion-studio/galion-one-voice/pull/76
- Merged: 2026-11-05T03:43:58+01:00 (UTC 2026-11-05T02:43:58Z)
- Base `main`, head branch `swarm/hardtest-sweep`
- Merge commit `4245a80553e20eec862ae04d24ccba5510af09cf`, head `bfdfe61413037d9ce2de76258aa9c225aa72eba4`
- GitHub pull diff: +742 / −21 across 11 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 11 files changed, 742 insertions(+), 21 deletions(-) (11 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #77 V2: persona mood (PersonaPlex-style) + persona-conditioned Piper voice; premium Piper voice eval + listening-set results

- URL: https://github.com/galion-studio/galion-one-voice/pull/77
- Merged: 2026-11-05T03:49:09+01:00 (UTC 2026-11-05T02:49:09Z)
- Base `main`, head branch `swarm/wt16-premium-emotion`
- Merge commit `8bcbf85ba285c93c46b1fe56c2f7033985142b63`, head `8a27b02929e956bbc9eddc0933449520bce79be0`
- GitHub pull diff: +2827 / −18 across 27 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 27 files changed, 2827 insertions(+), 18 deletions(-) (27 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #79 V2 voice screen: pure voice-to-voice (one dot + one full-screen object, no text/UI) + integration of the V2 PR stack (deployed build)

- URL: https://github.com/galion-studio/galion-one-voice/pull/79
- Merged: 2026-11-05T03:49:33+01:00 (UTC 2026-11-05T02:49:33Z)
- Base `main`, head branch `swarm/v2-integration`
- Merge commit `3f9ae80e4d3271586f709b2c3e6490e885cf4a6a`, head `a0c4db000dc525dada3abbd0067b1c144566137f`
- GitHub pull diff: +10806 / −205 across 256 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 5 files changed, 29 insertions(+), 103 deletions(-) (5 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #80 docs: V2 sprint results, bake-off t648 report + scoring tools, research notes, listening set

- URL: https://github.com/galion-studio/galion-one-voice/pull/80
- Merged: 2026-11-05T03:49:45+01:00 (UTC 2026-11-05T02:49:45Z)
- Base `main`, head branch `swarm/v2-results-docs`
- Merge commit `2302be1fd943d398135507bf1064a6b0bec6061f`, head `d676e1b42f1c39f0d77414b9a1faf67bab151e51`
- GitHub pull diff: +19513 / −0 across 259 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 259 files changed, 19513 insertions(+) (259 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #81 galion-clone-cbx: Chatterbox Turbo ONNX fp32 clone worker (separate unit, loopback, consent-gated, off during live calls)

- URL: https://github.com/galion-studio/galion-one-voice/pull/81
- Merged: 2026-11-05T03:57:12+01:00 (UTC 2026-11-05T02:57:12Z)
- Base `main`, head branch `swarm/cbx-clone-worker`
- Merge commit `b72e6a5da3d8646a0b0d1cf53fe63fa658c27b55`, head `0708257ccbea0a39318479b02fa7d0adebe638ad`
- GitHub pull diff: +509 / −1 across 6 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 6 files changed, 509 insertions(+), 1 deletion(-) (6 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #82 galion-clone-cbx: consent gate without Nano weight check; CC0 clip store in unit

- URL: https://github.com/galion-studio/galion-one-voice/pull/82
- Merged: 2026-11-05T04:23:27+01:00 (UTC 2026-11-05T03:23:27Z)
- Base `main`, head branch `swarm/cbx-consent-gate`
- Merge commit `3479f23fa4e3063f73408252f11d6e9554e9b850`, head `a02e78fcd8dc73011e8be0a52725b23ee44d06f7`
- GitHub pull diff: +19 / −3 across 2 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 2 files changed, 19 insertions(+), 3 deletions(-) (2 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #84 cloning-v2: emotion-blend tooling (Samantha coherent-rotation)

- URL: https://github.com/galion-studio/galion-one-voice/pull/84
- Merged: 2026-11-06T07:12:46+01:00 (UTC 2026-11-06T06:12:46Z)
- Base `main`, head branch `swarm/cloning-v2-emotion-blend`
- Merge commit `7e8819dcd619b76d7989fa0be840a1923bdfd442`, head `a4be847c717f5377dcaa07f4f43ca58edf44a369`
- GitHub pull diff: +12457 / −0 across 80 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 80 files changed, 12457 insertions(+) (80 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #89 Emotion V2 A: Samantha 12-emotion taxonomy

- URL: https://github.com/galion-studio/galion-one-voice/pull/89
- Merged: 2026-11-07T13:32:34+01:00 (UTC 2026-11-07T12:32:34Z)
- Base `main`, head branch `cursor/emotion-v2-taxonomy`
- Merge commit `4e19b4b4052d1a9ac864547080fe443332891c4b`, head `d035e1c8c33de52fa73d1983b6487ba56292dfcd`
- GitHub pull diff: +1360 / −0 across 4 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 4 files changed, 1360 insertions(+) (4 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #90 Emotion V2 C: per-emotion CI test harness

- URL: https://github.com/galion-studio/galion-one-voice/pull/90
- Merged: 2026-11-07T13:35:46+01:00 (UTC 2026-11-07T12:35:46Z)
- Base `main`, head branch `cursor/emotion-v2-ci`
- Merge commit `d07ab70b663faef53f51d6d4fed21dccdc204aec`, head `d75950488aa06580d4641f84cba791d6573ebf60`
- GitHub pull diff: +2343 / −0 across 42 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 42 files changed, 2343 insertions(+) (42 paths; list capped at 80 in the JSON).
- Head: `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #91 Emotion V2 D: generic training-data pipeline

- URL: https://github.com/galion-studio/galion-one-voice/pull/91
- Merged: 2026-11-07T13:36:08+01:00 (UTC 2026-11-07T12:36:08Z)
- Base `main`, head branch `cursor/emotion-v2-dataset`
- Merge commit `5ba85c347d8f63a821dceb2a47b7dbcbad7022e1`, head `ba66a37081ba54d662ca7977df8df0c4f6833e56`
- GitHub pull diff: +3909 / −0 across 28 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 28 files changed, 3909 insertions(+) (28 paths; list capped at 80 in the JSON).
- Head: `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #94 docs: regenerated Emotion V2 foundation spec and landscape

- URL: https://github.com/galion-studio/galion-one-voice/pull/94
- Merged: 2026-11-07T14:43:55+01:00 (UTC 2026-11-07T13:43:55Z)
- Base `main`, head branch `cursor/emotion-v2-docs`
- Merge commit `f648ca7328855478991d3cf93756477116aac52b`, head `6d8182e0278de819909c6cc9ac6b81d11ce7178b`
- GitHub pull diff: +569 / −0 across 2 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 2 files changed, 569 insertions(+) (2 paths; list capped at 80 in the JSON).
- Head: `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=failure, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #93 Emotion V2 B: controlled reference capture + checker

- URL: https://github.com/galion-studio/galion-one-voice/pull/93
- Merged: 2026-11-07T14:49:48+01:00 (UTC 2026-11-07T13:49:48Z)
- Base `main`, head branch `cursor/emotion-v2-capture`
- Merge commit `bec463dcaf3b8cd31ce76a68b7fad2371b5038d9`, head `c0ad8c2b07ce1b8428066d17990764f5ca0c70c4`
- GitHub pull diff: +3509 / −4 across 16 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 16 files changed, 3509 insertions(+), 4 deletions(-) (16 paths; list capped at 80 in the JSON).
- Head: `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #95 Reap a Piper child that exits before the first audio

- URL: https://github.com/galion-studio/galion-one-voice/pull/95
- Merged: 2026-11-07T15:18:39+01:00 (UTC 2026-11-07T14:18:39Z)
- Base `main`, head branch `cursor/reap-dead-piper-runtime-01ba`
- Merge commit `1a2be8529406c05154e4676ae50f8e585f585477`, head `4ae48d18a9c5899708fdefc540ad980eeb36906d`
- GitHub pull diff: +36 / −5 across 1 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1 file changed, 36 insertions(+), 5 deletions(-) (1 paths; list capped at 80 in the JSON).
- Head: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `emotion-v2 harness (CPU stub)`=success, `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #92 Emotion V2 handoff: starting point for new account

- URL: https://github.com/galion-studio/galion-one-voice/pull/92
- Merged: 2026-11-07T15:18:43+01:00 (UTC 2026-11-07T14:18:43Z)
- Base `main`, head branch `cursor/emotion-v2-handoff`
- Merge commit `9c2b94d6a68d6fabff86397070559609ac4545dc`, head `4f249be0b86b86fb45978af92461ca807a30377f`
- GitHub pull diff: +57 / −5 across 2 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1 file changed, 21 insertions(+) (1 paths; list capped at 80 in the JSON).
- Head: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #85 save: cloning-v2 persona scorer and pipeline sources

- URL: https://github.com/galion-studio/galion-one-voice/pull/85
- Merged: 2026-11-07T15:18:48+01:00 (UTC 2026-11-07T14:18:48Z)
- Base `main`, head branch `save/2026-11-06-cloning-v2-persona-scorer`
- Merge commit `b9bd890a304656188993022d3a64e37c28d21abe`, head `2ba95c017d25637317e195da7eccccf0d4e38377`
- GitHub pull diff: +606583 / −39 across 1408 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 1407 files changed, 606547 insertions(+), 34 deletions(-) (1407 paths; list capped at 80 in the JSON).
- Head: `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #88 Symphony V2 emotion42: unified symphony stack (alpha)

- URL: https://github.com/galion-studio/galion-one-voice/pull/88
- Merged: 2026-11-07T16:00:57+01:00 (UTC 2026-11-07T15:00:57Z)
- Base `main`, head branch `cursor/symphony-v2-emotion42-d379`
- Merge commit `754643b7aab24a8c02bb65bb118aa6b05ec1e277`, head `9bcba92f40e9630bab35aec2d5d7e065401769b6`
- GitHub pull diff: +8225 / −668 across 78 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 78 files changed, 8225 insertions(+), 668 deletions(-) (78 paths; list capped at 80 in the JSON).
- Head: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #99 Fix heartbeat budget flake and add laptop runbooks

- URL: https://github.com/galion-studio/galion-one-voice/pull/99
- Merged: 2026-11-07T16:03:25+01:00 (UTC 2026-11-07T15:03:25Z)
- Base `main`, head branch `cursor/heartbeat-budget-flake-and-laptop-runbooks-d259`
- Merge commit `598e1da815176928ec1071c23a410ee8803112c4`, head `7dbe23d1307df814515937b2e15850d02a167cb2`
- GitHub pull diff: +1236 / −22 across 12 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 12 files changed, 1236 insertions(+), 22 deletions(-) (12 paths; list capped at 80 in the JSON).
- Head: `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #98 Add the Nebula live emotion feed

- URL: https://github.com/galion-studio/galion-one-voice/pull/98
- Merged: 2026-11-07T16:04:22+01:00 (UTC 2026-11-07T15:04:22Z)
- Base `main`, head branch `cursor/nebula-live-feed-7582`
- Merge commit `6e9eb85cc3987a7ec0b37a62fee6080408792820`, head `9330b1fbb1cbadcf596c66d5fbffde6052fa51fa`
- GitHub pull diff: +1528 / −0 across 12 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 12 files changed, 1528 insertions(+) (12 paths; list capped at 80 in the JSON).
- Head: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `emotion-v2 harness (CPU stub)`=success, `test`=failure. Legacy status `pending` (empty status list; ignore as a colour).
- Stored non-success on at least one SHA. That is **not** proof the button was pressed on red, and it is **not** proof the tree is green now.

### #100 Add offline nebula emotion scoring beside emotion_match

- URL: https://github.com/galion-studio/galion-one-voice/pull/100
- Merged: 2026-11-07T17:58:08+01:00 (UTC 2026-11-07T16:58:08Z)
- Base `main`, head branch `cursor/nebula-emotion-scoring-bd74`
- Merge commit `b7049d99879f2ffde0459ed14f76a71dc9e6087b`, head `323cc0e9c1280f750b73eab6fca69aef8c54f05c`
- GitHub pull diff: +889 / −4 across 6 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 6 files changed, 889 insertions(+), 4 deletions(-) (6 paths; list capped at 80 in the JSON).
- Head: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #101 Nebula Galion Voice V2 (draft)

- URL: https://github.com/galion-studio/galion-one-voice/pull/101
- Merged: 2026-11-07T17:58:17+01:00 (UTC 2026-11-07T16:58:17Z)
- Base `main`, head branch `cursor/nebula-voice-v2-ea0c`
- Merge commit `8d6e55646916badd26f2f9309b9cff45a41695ad`, head `e660e12879926b2e40929cba742aa5368b37dc3d`
- GitHub pull diff: +6508 / −1 across 36 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 36 files changed, 6508 insertions(+), 1 deletion(-) (36 paths; list capped at 80 in the JSON).
- Head: `test`=success, `emotion-v2 harness (CPU stub)`=success, `test`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success, `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

### #102 Fix phone canvas sharpness and spectrum preview scale

- URL: https://github.com/galion-studio/galion-one-voice/pull/102
- Merged: 2026-11-07T20:02:31+01:00 (UTC 2026-11-07T19:02:31Z)
- Base `main`, head branch `cursor/nebula-mobile-fixes-dpr`
- Merge commit `3e1f5a12c53cd8e0dd47d111676d61d608540559`, head `6a516c0eb9b1248000dbffc06cfb76f60d29139b`
- GitHub pull diff: +1044 / −58 across 14 files. That is the pull's own diff, not always the first-parent delta on `main`.
- First-parent tree delta on `main`: 14 files changed, 1044 insertions(+), 58 deletions(-) (14 paths; list capped at 80 in the JSON).
- Head: `test`=success, `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Merge commit: `test`=success, `test`=success, `emotion-v2 harness (CPU stub)`=success. Legacy status `pending` (empty status list; ignore as a colour).
- Every stored check-run conclusion on both SHAs is success. Still not a timed green-at-merge certificate, and the legacy status remains `pending`.

## Soft gates that are not GitHub jobs

- `docs/emotion-v2/CI.md` and `.github/workflows/emotion-v2-ci.yml`: the GPU nightly is an operator run. It is **not a job** in the workflow. Absence is not a skipped check.
- `docs/emotion-v2/NEBULA-SCORING.md`: nebula distance `affects_gate` is false. `emotion_match ≥ 0.83` is unchanged. A green unit-test job does not mean the rollout gate passed.
- Nebula workflow `nebula-voice-v2.yml` runs library tests only. It does not measure frames per second and does not deploy.

## Evening priority (7 Nov, Europe/Zagreb)

| PR | Time | What landed on main | Stored CI |
|---|---|---|---|
| #98 | 16:04 | Nebula live feed `galion-nebula-live-feed/1` | head 2 success; merge commit 1 success and 1 `test` failure |
| #99 | 16:03 | Heartbeat budget flake fix and laptop runbooks | head and merge 2 success |
| #100 | 17:58 | Offline nebula score beside `emotion_match` | head and merge 2 success |
| #101 | 17:58 | `apps/nebula-voice-v2`, `crates/nebula-elements`, gas table, browser pages | head and merge 3 success |
| #102 | 20:02 | Phone canvas, cache-bust, GPU sprite path. Does **not** edit `shade.rs` or the Demo 13 hex table | head and merge 3 success |

