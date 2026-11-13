# 08 — context review

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Date: 2026-11-08, Europe/Zagreb (CET). Scope: read, reason, and write notes. No build, no deploy, no merge, no training, no audio upload.

Voice `main` tip: [`3e1f5a12c53cd8e0dd47d111676d61d608540559`](https://github.com/galion-studio/galion-one-voice/commit/3e1f5a12c53cd8e0dd47d111676d61d608540559).

Operating system tip read remotely: [`e48f3d4f6b`](https://github.com/galion-studio/galion-operating-system/commit/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d) (committer 2026-11-02). Nine files, documentation only.

Open Mind tip read remotely: [`7ba85b603d`](https://github.com/galion-studio/obsidian-open-mind/commit/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac) (committer 2026-10-28). Repository `pushed_at` metadata is later than that commit; the tip commit date is the one cited.

Galion Research is a product name in the owner notes. It is not assumed to be any other repository, and no other product deployment was inspected.

## What this review is sure of

1. Nebula on `main` is a visualiser plus an offline score. It is not a live emotion recogniser and not a voice change.
2. Pull request 102 (merged 2026-11-07 20:02 CET, head `6a516c0eb9b1248000dbffc06cfb76f60d29139b`) adds a GPU sprite path and phone sharpness. It does not add the warm palette. Spectral and display hex live in `crates/nebula-elements` and `apps/nebula-voice-v2/src/shade.rs`, landed with the earlier Nebula tree (pull request 101).
3. Ninety-five pull requests merged from 1 November through the query. Stored CI is in [MERGED-PRS-SINCE-2026-11-01.md](MERGED-PRS-SINCE-2026-11-01.md). Green at the instant of the merge button is **unknown**. Several merge commits have a stored `test` failure while the head was success.
4. No real boot-sound file or playback call was found in the three repositories. A separate local inventory (2026-11-08) found no eligible boot candidate among 47 audio paths, including duplicates. Quality was not measured because nothing qualified. `/opt/galion` was absent there and was not opened here.
5. `LOCKS.toml` still has `paid_tiers = false`. The new product names are a plan, not billing code.

## Evening of 7 November versus older notes

Commits with author time from 17:00 CET on 7 November are the phone, cache, and GPU Nebula edits, merged as pull request 102 at 20:02 CET. Pull requests 100 and 101 merged at 17:58 CET and brought the visualiser, the gas table, and the offline score. Their content commits are earlier the same afternoon (about 15:56–16:22 CET). The emotion-v2 handoff file is afternoon (about 13:30 CET), not that evening.

`docs/emotion-v2/UNIVERSAL-EMOTION-GAS-TABLE-20261107.md` still says it is a research proposal and that no runtime table was written. The same landing added `crates/nebula-elements`. The status paragraph is stale relative to the commit that contains it.

`docs/emotion-v2/NEBULA-LIVE-FEED.md` says the visualiser is not in this repository. After pull request 101 it is, at `apps/nebula-voice-v2`. The feed also says the program reads that schema. The Rust parser does not. See [CURRENT-CONTEXT.md](CURRENT-CONTEXT.md).

## Publications already open

| Pull | Commit | What this review takes from it |
|---|---|---|
| [103](https://github.com/galion-studio/galion-one-voice/pull/103) `f6832719` | `cursor/emotion-storage-gaps-b3fd` | `galion_demo.respond` counterbalances. `mirror()` is only the rejected self-test, not the spoken turn. The 2 November status sentence that the Python demo still mirrors is stale. `AffectStore` is a RAM ring. `baseline_for_persist` can hand a baseline to a host when REMEMBER is set, and no host write or reload was shown. No learned detector is in the crate. |
| [104](https://github.com/galion-studio/galion-one-voice/pull/104) `9488aac9` | `cursor/products-map-06-c0a0` | Planned names Galion One $20 and Galion Research Ultra $220. `LOCKS.toml` still has `paid_tiers = false`. The map itself is a **voice-tree documentation inventory**. It quotes older README language that nothing in that tree has been deployed, and it quotes rights holds. Those sentences are not a new look at a live host and not a new rights decision. The map says it did not refetch Open Mind. |
| [105](https://github.com/galion-studio/galion-one-voice/pull/105) `bf4f99d3` | `cursor/nebula-bot-outputs-publication-b4f0` | All twelve stopped roles are listed. Evidence JSON has `fps_measured` null, and zero commits, pulls, tests, and builds from that stop. No profiling harness was written. Incomplete Nebula source was deliberately not committed. Not merged. |
| [106](https://github.com/galion-studio/galion-one-voice/pull/106) `98ce3d01` | `cursor/pricing-open-source-map-c17b` | Owner plan: Research Ultra $220 and One $20. `crates/galion-symphony/src/host_cpu.rs` notes that say Plus CX23 $20 and Ultra CX43 $220 are historical host-pack fields, not a payment system. No entitlement or billing proof. `paid_tiers` remains false. |
| [107](https://github.com/galion-studio/galion-one-voice/pull/107) `71beae77` | `cursor/galion-os-base-02-7b1e` | Operating-system repository at `e48f3d4` is the README, the MIT licence, and voice-profile docs. No image and no `galion-init` or `galion-voiced` binary. Voice code in this repository is not proof of that image. The note also records `WindowSurface::open` as unsupported. |
| [108](https://github.com/galion-studio/galion-one-voice/pull/108) `c995a898` | `cursor/galion-os-boot-sounds-f784` | No real boot asset or playback hook in the public trees or the local inventory. The tracked wav files (listening-set speech and presence fixtures) are not boot sounds. The cascade earcon is a generated demo tone ladder. A warm-up utterance is not a boot sound. No quality measurement and no listening test, because nothing eligible was found. |
| [109](https://github.com/galion-studio/galion-one-voice/pull/109) `2a3a49c5` | `cursor/voice-v2-universal-datasets-7d6b` | Proposed persona, emotion, and indoor-environment joins. No corpus filled and no model trained. Active pins remain the CC0 donors Amy Koenig and Alan Davis Drake. Movie references stay retired. `datasets/emotion-v2/` is gitignored. Rights and consent text in the tree is a gate description, not a new clearance. |
| [110](https://github.com/galion-studio/galion-one-voice/pull/110) `db35455b` | `cursor/personaplex-indoor-rl-plan-6208` | Indoor PersonaPlex reinforcement learning is a plan. No weights, no download, no training. `galion-mimi` `IS_STUB` is true. The student backend reports not placed. `exporters.py` writes one synthetic speaker on the left and silence on the right. That layout is not a duplex training set. Subject fields `samantha` and `jarvis` are registers, not a licence to train on a private recording. |
| [111](https://github.com/galion-studio/galion-one-voice/pull/111) `d6b5f1b8` | `cursor/openmind-stack-context-90ee` | Same Open Mind tip `7ba85b603d` (28 October), read as code. Desk localhost 8646. `galion-one` text 8091 and mouth 8002, not the Voice persona. Mouth stub without `GALION_MOUTH`. `omnode` pair is payload-only. Hermes v1 memory can capture, recall, search, and end a session; seed and v2-data routes return 501. That is not emotion persistence. Student refuses load. No SSH and no host probe. |
| [112](https://github.com/galion-studio/galion-one-voice/pull/112) `a628099f` | `cursor/obsidian-desktop-context-2a8d` | No public product is titled Obsidian Desktop. A Markdown desk, the Open Mind stack, and the obsidian colour are not the third-party Obsidian app. The operator surface in code is `galion-desk` plus a partial console. The OS image and its GUI are a plan. The native window open path is a stub. An October note of a live desk is historical, not a current host probe. `hermes-research` at brand commit `e204789ce73306c645dcb95b4b509cc764ca2c47` is another repository’s brand file, not an implementation of Galion Research. |
| [113](https://github.com/galion-studio/galion-one-voice/pull/113) `72d3e029` | `cursor/real-audio-quality-2942` | No fresh audio measurement. `PiperProsody::with_emotion` clones the same scales for Neutral and for every other label (`piper_worker.rs`). `noise_scale` and `noise_w` are 0 unless `GALION_A21_NOISE=natural`. Inference PCM is not a harness tone or a pitch slide. Headers of listening-set wav files (mono, 22050 Hz in that note’s count of 82) are not a listening test or a MOS. Older gate numbers stay in their own snapshots. A 7 November brief figure 480/29 is a prior run report, not a benchmark row committed as the current result; naming it does not delete that run. A heartbeat allocation subset of 13 tests passing is not all 41 tests and not product audio quality. No fresh Nano result and no boot-sound proof. |

## Nebula V3 work that did not land

Pull request 105 and pull request 103 agree, and this checkout agrees: `origin` has no `docs/nebula-v3/` and no `apps/nebula-voice-v2/src/v3/`. A local branch name `cursor/nebula-v3-integration-b3fd` was created from `3e1f5a1` and was not pushed (`git ls-remote` in the publication pass found no such head). Untracked files described there are `DESIGN.md`, `ROSTER.md`, `src/v3/mod.rs`, and `src/v3/input.rs`. `field.rs` is absent, so that module would not compile. No frame time was measured. `quality.rs` 120 is a target, not a result. This pull request does not add those sources.

## Open Mind, read directly

This review did fetch Open Mind `main` at `7ba85b603d8b0b87c4d4dede1056e1ee80c767ac` (committer date 2026-10-28). That is separate from pull request 104, which did not refetch it.

What that fetch showed, as repository text only:

- Crate `crates/galion-one` is in that repo. `deploy/boot/rendered/galion-one.service` says staged and not installed.
- `crates/galion-boot` supervises units and does not play audio.
- The README calls `persona-plex-relay` a scaffold and says the first public release was still ahead of the commit date on the page. A crate in git is not a running service.

[Pull request 111](https://github.com/galion-studio/galion-one-voice/pull/111) (`d6b5f1b8`, `04-openmind-stack.md`) reads the same Open Mind tip and goes further into the crates. Taken as code at that commit, not as a host probe:

- `galion-desk` is written for localhost port 8646.
- The `galion-one` binary binds text on 8091 and a mouth on 8002. That process is not the Voice product persona of the same name.
- The mouth path is an unavailable stub unless `GALION_MOUTH` is configured and a serve is loaded.
- `omnode` pair prints a payload and does not enroll a device.
- Hermes v1 memory in that tree implements capture, recall, search, and session end. Seed and v2-data routes answer 501. That memory is not proof that emotion baselines persist.
- The student backend refuses to load.

No deployment was claimed, and no SSH was used. Operating-system **spec** stays the documentation repository at `e48f3d4` ([pull request 107](https://github.com/galion-studio/galion-one-voice/pull/107)). Open Mind **code** is the crate tree above. Neither is a running machine.

Pull request 104’s line that Voice docs name no second Galion One repository is true of the voice tree. It is not a finding that Open Mind lacks `galion-one`. Both statements stand: the voice tree does not vendor it, and the other repository contains the crate. Neither statement is a live-host fact.

## Missing briefing

`/opt/galion/SAMANTHA-V2-BRIEFING.md` and its archive were not read. Authenticated SSH did not succeed. This review does not claim the briefing is empty, only that it is unread. Any boot sound or handoff that exists only on that disk is still unknown.

## Where the detail lives

- Merges and CI: [MERGED-PRS-SINCE-2026-11-01.md](MERGED-PRS-SINCE-2026-11-01.md)
- Emotion paths: [SYMPHONY-EMOTION-AND-STORAGE.md](SYMPHONY-EMOTION-AND-STORAGE.md)
- Boot: [GALION-OS-BOOT-AUDIO.md](GALION-OS-BOOT-AUDIO.md)
- Order of work, including pricing names: [GALION-OS-UNIVERSAL-TODO.md](GALION-OS-UNIVERSAL-TODO.md)
- Datasets and the PersonaPlex plan: [GALION-VOICE-V2-DATASETS-PERSONAPLEX.md](GALION-VOICE-V2-DATASETS-PERSONAPLEX.md)
