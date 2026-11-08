# Galion context, 8 November 2026

Public notes from a read-only review. Nothing here is a release, a merge, or a claim that a product is finished.

Written 2026-11-08, Europe/Zagreb (CET, UTC+1). Voice repository tip used for the review: `3e1f5a12c53cd8e0dd47d111676d61d608540559` (merge of pull request 102, 20:02 CET on 7 November).

## In this folder

| File | What it is |
|---|---|
| [08-context-review.md](08-context-review.md) | Lead context note: what was verified, what is historical, what is only a plan, what is missing. |
| [CURRENT-CONTEXT.md](CURRENT-CONTEXT.md) | Same-day picture of Nebula, emotion wiring, and the limits of that picture. |
| [MERGED-PRS-SINCE-2026-11-01.md](MERGED-PRS-SINCE-2026-11-01.md) | Every pull request merged from 1 November 2026 00:00 CET through the query, with stored CI. |
| [evidence/ci-compact.json](evidence/ci-compact.json) | Machine-readable check-run conclusions for those pulls. Legacy combined status is `pending` and is not a colour. |
| [SYMPHONY-EMOTION-AND-STORAGE.md](SYMPHONY-EMOTION-AND-STORAGE.md) | Inference, policy, and persistence, with the contradictions left open. |
| [GALION-OS-BOOT-AUDIO.md](GALION-OS-BOOT-AUDIO.md) | Specified boot versus implemented boot, and the absence of a real startup sound. |
| [GALION-OS-UNIVERSAL-TODO.md](GALION-OS-UNIVERSAL-TODO.md) | Ordered work for the operating system, Voice, and One. Includes the owner pricing names and the boot-sound priority. |
| [GALION-VOICE-V2-DATASETS-PERSONAPLEX.md](GALION-VOICE-V2-DATASETS-PERSONAPLEX.md) | What dataset code exists, and a proposed indoor PersonaPlex data model that is not a training run. |

## Already published beside this branch

These files are not in this commit. They are open documentation pull requests. This index does not merge them.

| Pull | Commit | Branch | File | How to read it |
|---|---|---|---|---|
| [103](https://github.com/galion-studio/galion-one-voice/pull/103) | `f6832719ed574cc89bcfc263ec2c9379f4e4160f` | `cursor/emotion-storage-gaps-b3fd` | `01-emotion-storage-and-unfinished-work.md` | Current tree: `respond` counterbalances; `mirror()` is only the rejected self-test. Affect store is a RAM ring. No learned detector. Unfinished Nebula files were not pushed. |
| [104](https://github.com/galion-studio/galion-one-voice/pull/104) | `9488aac93745871d930d3fe8a31af8be71399e77` | `cursor/products-map-06-c0a0` | `06-products-map.md` | Voice-tree documentation inventory only. Owner prices One $20 and Research Ultra $220, locks unchanged. Do not treat its historical “nothing deployed” or rights sentences as a fresh host check. |
| [105](https://github.com/galion-studio/galion-one-voice/pull/105) | `bf4f99d3ee7554eadc0dca40099aefc1d33724e4` | `cursor/nebula-bot-outputs-publication-b4f0` | `07-bot-outputs-publication.md` and `07-bot-outputs-evidence.json` | All twelve stopped roles listed. `fps_measured` is null. No build, test, or harness from that stop. Incomplete Nebula source was left out of the commit. |
| [106](https://github.com/galion-studio/galion-one-voice/pull/106) | `98ce3d01745fa4a745f98ddcbea8c2cd7b763ab3` | `cursor/pricing-open-source-map-c17b` | `09-pricing-and-open-source.md` | Planned Research Ultra $220 and One $20. `host_cpu` Plus/CX23 $20 and Ultra/CX43 $220 are historical host-pack fields, not payments. No billing proof. `paid_tiers` false. |
| [107](https://github.com/galion-studio/galion-one-voice/pull/107) | `71beae777b7f18ac4d36bd46024d01117082ac9d` | `cursor/galion-os-base-02-7b1e` | `02-galion-os-base.md` | Remote OS tip `e48f3d4` is README, MIT, and voice-profile docs only. No image, `galion-init`, or `galion-voiced`. Voice code is not an OS image. |
| [108](https://github.com/galion-studio/galion-one-voice/pull/108) | `c995a89824738d55429a6d9d9760ba8ef2716f22` | `cursor/galion-os-boot-sounds-f784` | `10-galion-os-boot-sounds.md` and `10-galion-os-boot-sounds.evidence.json` | No real boot asset or hook in the public trees or the local inventory. Tracked wav files are listening-set speech and presence fixtures. |
| [109](https://github.com/galion-studio/galion-one-voice/pull/109) | `2a3a49c565c7ab2cf8b996c17a5de63e55f9df38` | `cursor/voice-v2-universal-datasets-7d6b` | `05-voice-v2-universal-datasets.md` and `05-voice-v2-universal-datasets.schema.json` | Proposed joins of persona, emotion, and indoor environment. No populated corpus and no trained model. Active conditioning pins are the CC0 donors. `datasets/emotion-v2` is ignored. |

Other pair notes may still open into this folder. This branch does not wait for them and does not merge them.

## Not in git, on purpose

- `/opt/galion/SAMANTHA-V2-BRIEFING.md` and any archive beside it. The path was not readable. No SSH retry.
- Secrets, credentials, environment files, private recordings, retired film references, uncleared datasets, and model weights.
- Existing audio. No wav, mp3, or raw sample was added.
- Uncommitted Nebula V3 experiments that never reached `origin`. They are recorded as open and untested, not copied in.

## Evidence classes

**Verified** means the file or API row was read in this review. **Historical** means an older note states it and it was not remeasured. **Proposed** means an owner direction or a design that is not implemented. **Blocked** means a dependency or an access gap stops the claim.
