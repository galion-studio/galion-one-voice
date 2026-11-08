# Galion OS universal todo

Status of the whole product: **not finished**. This list is an order of work with acceptance checks. It does not mark a row done unless the evidence column says verified.

Dates are Europe/Zagreb. Voice tip `3e1f5a12` (2026-11-07 20:02). OS tip `e48f3d4f6b` (2026-11-02). Open Mind tip `7ba85b603d` (committer 2026-10-28).

`LOCKS.toml` `[licence] paid_tiers = false` stays as it is. This document does not change locks or runtime.

## Pricing names (owner direction, 2026-11-08)

**Proposed, not implemented.**

| Name | Price named by the owner | What it is |
|---|---|---|
| GALION ONE | $20 | The $20 product name going forward. |
| GALION RESEARCH ULTRA | $220 | The $220 product name going forward. |

This supersedes the 2026-11-03 mapping that called the same dollar amounts generic Voice Plus and Voice Ultra. The same names and the same lock lines are written up as a voice-tree inventory in [pull request 104](https://github.com/galion-studio/galion-one-voice/pull/104) (`9488aac9`, `06-products-map.md`). That page did not refetch Open Mind. Its quotations of an older “nothing deployed” README line and of rights holds are historical document text, not a new inspection of a live host and not a new rights decision. Direct Open Mind evidence is in [08-context-review.md](08-context-review.md): the `galion-one` crate is in that other repository, and the rendered unit file says it is not installed.

Historical pages, left unchanged:

- `docs/galion-voice-v2/symphony/PLAN-PLUS.md` (2026-11-03) prices **Galion Voice Plus** at $20 per month, CX23-class, voice-to-voice included, no GPU.
- `docs/galion-voice-v2/symphony/PLAN-ULTRA.md` (2026-11-03) prices **Ultra** at $220 per month as one CX43-class computer, and says Galion Research is a separate console that Ultra does not give a second machine.

Those pages are sizing notes. They do not charge anyone. The same dollar figures also sit as notes on host-pack rows in `crates/galion-symphony/src/host_cpu.rs` (Plus, CX23-class, $20/mo; Ultra, CX43-class, $220/mo). [Pull request 106](https://github.com/galion-studio/galion-one-voice/pull/106) (`98ce3d01`, `09-pricing-and-open-source.md`) records those fields as historical host packs, not payments, and records no billing or entitlement proof. `paid_tiers = false` means the program still has no paid-tier switch. No billing service was found in the OS repository (docs only) or in the Open Mind boot units. Galion Research is not assumed to be another git repository.

Acceptance for a future billing change, not this pull request: an owner-approved price table uses these two names, a test proves `paid_tiers` is still false until that owner says otherwise, and no charge path exists before that yes.

## Order

Dependencies point at earlier ids. “Blocked” means the row cannot be called done.

### Base

| Id | Work | Status | Depends on | Acceptance |
|---|---|---|---|---|
| OS-00 | Keep one public story: Galion OS is MIT, Voice profile first, Hermes GUI later, Galion One is a separate product. | Specified in OS README and `docs/voice-profile/README.md` (2026-11-02). Image not published. Same tip `e48f3d4` is recorded in [pull request 107](https://github.com/galion-studio/galion-one-voice/pull/107) (`71beae77`, `02-galion-os-base.md`): README, MIT, and voice-profile docs only. | — | Root README still says downloads come when an image exists. No image URL is invented. Voice crates are not treated as the OS image. |
| OS-01 | `galion-init` and `galion-voiced` as real static binaries, boot order kernel → init → ALSA → voiced. | **Proposed.** Named in `voice-first.md` and `PACKAGING.md`. Binaries absent. | OS-00 | A build manifest lists the two binaries and their hashes. A boot log shows that order. Not done. |
| OS-02 | Real boot sounds: start, finish, notice, question. Owner asked for real recordings, not a synth, earcon, warm-up sentence, or clone reference. Sits beside the voice-quality fix (V-01). | **Blocked.** No asset and no playback hook. [Pull request 108](https://github.com/galion-studio/galion-one-voice/pull/108) (`c995a898`, `10-galion-os-boot-sounds.md`) records the same absence in the public trees and the local inventory. Tracked wav files in this repo are listening-set speech and presence fixtures, not boot cues. The cascade earcon is a generated demo tone ladder. No eligible file was measured or listened to. `/opt/galion` absent. | OS-01 | See [GALION-OS-BOOT-AUDIO.md](GALION-OS-BOOT-AUDIO.md). Hash, format, one-shot playback after the device is ready, mute and night silence, log line. Speech PCM is not accepted as this row. |
| OS-03 | Staged Open Mind supervisor (`galion-boot`, `deploy/staged/boot.toml`) stays labelled not installed until an owner installs it. | Verified staged text. Not a running host. | — | Units still say not installed. No sound unit is added by accident. |
| OS-04 | Read `/opt/galion/SAMANTHA-V2-BRIEFING.md` and its archive when access exists. | **Blocked.** Unread. SSH was not retried. | — | A later note quotes the briefing with a date, or records that the file is confirmed absent on the machine. |

### Voice (first product priority)

| Id | Work | Status | Depends on | Acceptance |
|---|---|---|---|---|
| V-01 | Voice audio quality fix. Live path is still Piper (and Flite as a narrow backend). Emotion recognition is the RMS / zero-crossing stub. `emotion_match` 0.83 was not remeasured; historical notes put Samantha below it. Demo earcon is not speech (`V1-DEMO.md` C7). | **Open.** Code verified. Scores not rerun. | — | A published score sheet from the current tip, with the gate unchanged, and a speech reply that is not an earcon. Separate from OS-02. |
| V-02 | One emotion policy. `film.rs` forbids negative target valence. `persona_mood.rs` can produce one. | **Open contradiction.** | V-01 | One function is the runtime policy. A test runs the persona mood through `check_counterbalance` or the policy is explicitly split and documented. |
| V-03 | Persistence only with REMEMBER, and withdrawal wipes it. API exists. No host writes the baseline. | **Not shown.** | — | Restart-surviving baseline in a test, gone when REMEMBER clears. |
| V-04 | Nebula picture reads `galion-nebula-live-feed/1` or the feed gains a writer the picture already parses. Gas ids match. Warmth either reaches shade or the README stops claiming it. | **Open.** Contracts verified side by side. | V-01 | A recorded side-stream frame changes particles, and a mismatched element id is still rejected. |
| V-05 | Frame-rate targets stay targets until a device log stores achieved fps. Do not cite 120 as a result. | Verified as constants only. | — | A profile names the device, the tier, and the measured rate. |

### One (second product priority)

| Id | Work | Status | Depends on | Acceptance |
|---|---|---|---|---|
| ONE-01 | Galion One remains the voice surface in Open Mind (`galion-one`), distinct from the OS image and from the $20 name above. | Crate exists on the Open Mind tip. Service file is staged, not installed. | OS-03 | README and the staged unit agree on bind addresses. No claim the unit is running. |
| ONE-02 | PersonaPlex relay stays a scaffold until a GPU host and a licence review exist. NVIDIA weights are not in git. | Open Mind README: scaffold, no socket, no GPU. Voice `PERSONAPLEX-REANCHOR.md` (2026-11-04): plan only. | V-01 | Relay log shows a real session or the README still says scaffold. |
| ONE-03 | GALION ONE at $20 is the name for that price. It does not flip `paid_tiers`. | **Proposed 2026-11-08.** | OS-00 | Price table uses this name. Locks unchanged until a separate owner yes. |

### Research

| Id | Work | Status | Depends on | Acceptance |
|---|---|---|---|---|
| R-01 | GALION RESEARCH ULTRA at $220 is the name for that price. It supersedes calling $220 a generic Voice Ultra seat. | **Proposed 2026-11-08.** Historical Ultra page is 2026-11-03. | ONE-03 | Same as ONE-03 for the $220 row. Research is not given a second machine by assumption. |
| R-02 | Indoor PersonaPlex data model: two subject personas, environments, consent, disjoint splits. | **Proposed.** [Pull request 109](https://github.com/galion-studio/galion-one-voice/pull/109) (`2a3a49c5`) publishes the join schema and states no corpus is populated and no model is trained. `.gitignore` ignores `datasets/emotion-v2/`. Active conditioning notes name CC0 Amy Koenig and Alan Davis Drake. Movie refs stay retired. Pipeline code is a stub synth. Rights and consent gates in the public docs are not a new clearance. | V-01, ONE-02 | No training job is implied by the schema landing. A later corpus is subject-disjoint and does not commit audio until a clearance says so. |

## Explicitly not in this order as done

- A published OS image.
- A boot sound.
- Billing.
- A measured 120 fps Nebula.
- A green emotion gate.
- A disk-backed mood.
- PersonaPlex training.
