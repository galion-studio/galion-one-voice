# Swarm 10 brain-eureka clearance — t273 originals vs t623u CC0 donors

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. **Written 2026-11-15 22:06 +01:00.** Record only. Not legal advice. Not an owner signature. Not a clearance.

**Role:** Lane D worker D4 (rights clearance) plus the A0 owner brief.
**Repo:** galion-studio/galion-one-voice.
**Base read:** `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0`.
**Host of this reading:** `cursor`, Linux x86_64, 4 CPUs. This is not the NEXT-ITERATION host `grok-bot-vm-511777615`. RAM on this host was not measured.
**Status:** **BLOCKED-OWNER.** This file asks Maciej to choose. It does not choose.

`/workspace/voice-next/NEXT-ITERATION.md` (2026-11-15 21:56 +01:00, local, untracked) was read and is cited below. It is not added to git.

Orchestrator draft: [#152](https://github.com/galion-studio/galion-one-voice/pull/152) `Swarm 10 brain-eureka plan`, branch `cursor/swarm10-brain-eureka-plan-b8c3`, head `5cbb52fd570d02d72fefb555bc83179858211471`, still draft when read at 2026-11-15 22:08 +01:00. A listing of open pulls at 22:04 +01:00 had not included it yet (that listing was #149, #145, #97, #87, #86, #83, #78). `PLAN.md` on that head says workers branch from `main`, write `clearance.md` only for this role, do not edit `PLAN.md` or `SELF-EVAL.md`, and do not tick a box in `RIGHTS-CLEARANCE.md`. This pull request follows that. Merge order in the plan puts self-eval before clearance. This worker does not merge.

## The decision, for Maciej

Two writings in this tree disagree about which Samantha and Jarvis references the live clone path may use. Only Maciej can close that. This pull request does not edit `LOCKS.toml` or `scripts/clone/REFS.sha256`.

**(a) Re-open the t273 originals on the live clone path.** That is a lock-and-ledger edit Maciej makes: `ref_clip_decision`, the active rows of `REFS.sha256`, the voice TOML pins, and a ledger sentence that says what the t273 record covers. The film checklist in `RIGHTS-CLEARANCE.md` stays unchecked unless an instrument with every field filled is pasted. This tree does not contain that instrument. This file does not invent a counsel name, a grantor, a rightsholder, or a signature.

**(b) Main stays on the t623u CC0 donors.** Then symphony self-eval B (clone landing) and C (Jarvis texture), `CLONE-LANDING.md`, and the symphony index reference table have to be re-stated so they stop requiring the t273 hashes. Files that hash to the t273 digests stay refused by the worker. The product mouth stays Piper until a clone of the refs Maciej actually chose is live.

Until one of those sentences is Maciej's, A0 stays **BLOCKED-OWNER**. Generate from the t273 files stays refused by the in-tree worker. A1, the original-ref Nano clock (B3), the clone-not-Piper witness (C5), and self-eval B and C on the originals stay blocked on that decision. This pull request does not re-state B or C as PASS.

## Which line is in force

Three different sentences are in the tree. They are not the same clearance.

### 1. Live allowlist — t623u — this is what the worker enforces

Read from the tree at `8980e1a`. Not edited.

`LOCKS.toml` sha256 `b97fa057f6136d724f1283bdeb77d9d175a08a0245177ad85ae8e94ca3040f59`. That digest is the same string `SELF-EVAL.md` records as `locks_sha256` for the 2026-11-06 17:18 CET self-eval run. The lock file on that run was already this file.

```toml
reference_clips = true
ref_clip_decision = "t623u"
ref_clip_scope = "CC0 / public-domain LibriVox donors only: Amy Koenig (samantha), Alan Davis Drake (jarvis). Store outside the tree with consent.txt + sha256 sidecar. No celebrity voice, no film audio, no Armstrong."
```

The table is `[formats.exceptions.v2s14_chatterbox]`. Notes on that table: "No film audio. No celebrity clone."

`git log` on this checkout, for that key:

| Commit | When | Subject |
|---|---|---|
| `035a052` | 2026-11-04 12:20:39 +0100 | Owner t623u: `reference_clips=true` for the CC0 LibriVox donors; new REFS rows |
| `4f28c0c` | 2026-11-04 16:07:11 +0100 | Owner 16:01 CET: no CC0 donors, `reference_clips` stays false |
| `19d3685` | 2026-11-04 20:50:04 +0100 | Owner t628u 20:49 CET: CC0 donors ON again (revert of `4f28c0c`) |

`LEDGER.md` **2026-11-04 20:49 CET · CC0 donors ON again (owner t628u)** says the 16:01 "no CC0" was a wrong click, restores `reference_clips = true` for Amy Koenig (samantha) and Alan Davis Drake (jarvis), and says `consent.txt` states a public-domain dedication plus CC0 1.0, no signed release, `public_demo: no`. That is the ledger's claim about a file. This worker did not open a `consent.txt`. None is in git. `/workspace/galion-clip-store/` is absent on this host. The claim is not re-verified here.

`scripts/clone/chatterbox_worker.py` builds the allowlist from `scripts/clone/REFS.sha256` and skips `#` comments (`load_allowlist`). The module docstring says `LOCKS.toml` is not read and is not the switch. The active rows (file sha256 `e4260b9549e024a4292ec702dd135141f71de000fa630856703ce6c01f8ac03e`) are:

| Persona | Active sha256 | Pin shape | Donor named in the file comment |
|---|---|---|---|
| Samantha | `e4bc60c33f697108cf5585dead213da1065979010f5be1d513574893584a4641` | 9.60 s, 44100 Hz, mono, 16-bit | Amy Koenig, LibriVox *Love and Friendship* |
| Jarvis | `60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6` | 10.71 s, 44100 Hz, mono, 16-bit | Alan Davis Drake, LibriVox *Celebration of Dialects and Accents* Vol 1 track 30 |

The same file comments the t273 rows and says, in its own words: "The t273 wavs have no speaker clearance; do not re-enable them."

| Persona | Commented t273 sha256 | Commented shape |
|---|---|---|
| Samantha | `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` | 17.41 s, 44100 Hz, stereo, 16-bit |
| Jarvis | `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` | 15.95 s, 48000 Hz, stereo, 16-bit |

`docs/galion-voice-v2/personas/voice/samantha.toml` and `jarvis.toml` set `rights = "cc0-public-domain-t623u"` with the comment `was owner-t273 (superseded 2026-11-04)`, and they pin the active hashes above. Both say the public demo stays closed. Piper fallback sets `is_film_voice = false`.

`jarvis.toml` says the t623u clip is clean read speech and that hub clicks, alarms, and ambience are **not** baked into that donor. They come from the separate `GALION_JARVIS_TEXTURE` layer. That is a different texture fact from the t273 Jarvis clip, which the 2026-11-03 landing describes as the hub recording itself. Option (b) cannot close self-eval C "on the original Jarvis clip" by pointing at this donor. Option (a) is the only path on which that original-clip bar still means the t273 file. This briefing does not score C.

`scripts/clone/clone-landing-smoke.sh` on this main requires `reference_clips = true` and `ref_clip_decision = "t623u"`, and it checks the two active hashes above. It does not check the commented t273 hashes.

### 2. Film-voice clearance — not in force

`docs/galion-voice-v2/RIGHTS-CLEARANCE.md` leaves every Samantha and Jarvis box unchecked: performer consent, film-clip grant, licence, and public demo. The paste templates still contain `[FIELD]` placeholders. A box ticked without a filled ledger line is not clearance. This pull request ticks nothing and pastes nothing.

`LEDGER.md` **2026-11-03 10:47 CET** has two lines titled PRIVATE CLEARED, one per persona, under owner decision t273, confirmed t287. Both lines say: counsel not named; none invented; no grantor and no rightsholder copied, because no instrument printed those names into this tree; public demo still closed; checklist boxes that require a counsel name stay unchecked; `reference_clips` was still false and still needed an owner flip; the entry does not edit `LOCKS.toml`. The Samantha line says the research target remains the voice of *Her* (2014). That is the owner's research-target sentence in the ledger. It is not a film licence and not a performer signature. Both lines say the owner reported a local Nano smoke; both say the entry does not repeat that run and records no measurement from that tree.

`film_voice` on `/v2/state` comes from `gate_from` in `galion-render/src/v2_persona.rs`. An approved clone-time verdict sets `film` from the approval's `public` flag (`consent.txt` `public_demo` equal to yes). The default gate is `rights: "hold"`, `film: false`. `docs/galion-voice-v2/HARD-LOOP.md` still requires `film_voice: false` on that route. This pull request does not claim a public film voice. No new celebrity clip is added.

`crates/galion-consent` refuses a consent record whose persona, performer, work, or grantor contains `armstrong` (`BLOCKED_VOICES`). That block list is not a film clearance for any other name. The rights checklist is still the gate for a film performance. This file does not add a name to that list and does not remove one.

### 3. t273 local-send record — written, then contradicted by the pin, not the live allowlist

`LEDGER.md` **2026-11-03 10:37 CET** records owner t273 as local reference send of the two commented hashes above. No counsel name. Public demo closed. `LOCKS.toml` not edited in that entry. `reference_clips` remains false **as of that line**. Paths named there are the gitignored `docs/galion-voice-v2/personas/refs/{samantha,jarvis}.wav`.

The later pin comment in `REFS.sha256` says those wavs have no speaker clearance and must not be re-enabled. The 2026-11-04 t628u ledger line turns `reference_clips` on for the CC0 donors only, and the lock scope says no celebrity voice and no film audio.

This pull request does not delete the t273 ledger lines and does not delete the REFS comment. They disagree. The code follows the comment and the active rows, not the 2026-11-03 hashes.

The worker module docstring (lines 21–22) still says a reference is accepted only when it is one of the two owner-cleared t273 wavs. The function that runs is `load_allowlist` on `REFS.sha256`, whose active rows are t623u. The docstring is stale. This pull request does not edit the worker. That comment is Lane A (A4) if someone reconciles the sentence with the pin.

## Pages that still describe an older state

These were read. They were not given new measurements. None of them was edited. `#152` gives this role `clearance.md` only, and it tells D3 not to resolve the t273 / t623u hash disagreement. Swapping the t273 landing table for the t623u hashes would be option (b), which is Maciej's sentence.

| Page | What it still says | What this main actually has |
|---|---|---|
| `symphony/README.md` reference table | t273 hashes, 17.41 s stereo and 15.95 s stereo, "program brief records owner clearance t273" | Worker pin is t623u. Table **not edited** (plan: do not resolve the hash disagreement). |
| `symphony/CLONE-LANDING.md` | t273 table; "`reference_clips` stays `false`"; pass bullet says the key is still false | `clone-landing-smoke.sh` requires `reference_clips = true` and `ref_clip_decision = "t623u"`. Page **not edited**. |
| `symphony/SELF-EVAL.md` section B close condition | t273 hashes. Older row: lock flip BLOCKED-on-owner, `reference_clips = false` | Lock file hash matches the 2026-11-06 run, and that file has `reference_clips = true`. Matrix B and C were last recorded **OPEN** on 2026-11-06 17:18 CET (wavs absent on that run). **Not edited.** `scripts/symphony/self-eval.sh` was **not run**. |
| `symphony/EUREKA.md` row B | Git slots are placeholders; checklist leaves the personas closed; "no clip sha256 is recorded, because there is no clip" | Git slots on this host are still placeholders. Digests are recorded (t623u active, t273 commented). Film boxes stay unchecked. Page **not edited**. |
| `RIGHTS-CLEARANCE.md` "State of this writing" (2026-11-03) | `reference_clips` remains false; boxes unchecked | Boxes still unchecked. Lock on this main is the t623u exception above. Page **not edited**. No box ticked. |
| `personas/refs/.gitignore` header | "Samantha: no permission / copyright claims; Jarvis: Apache 2.0 unconfirmed with original creator — local testing only, remove later." | A third rights sentence, neither t273 PRIVATE CLEARED nor t623u CC0. **Not rewritten.** |
| `personas/refs/*.placeholder` | "clip_path stays empty until LEDGER.md records clearance" | TOMLs already set `clip_path` and `rights = "cc0-public-domain-t623u"`. Placeholders **not edited**. |

`SELF-EVAL.md` was left to Lane D worker D3. This file does not copy tonight's Piper clocks or the seven `galion-ml-voice` failures into that ledger, and it does not turn OPEN into PASS.

## Originals on disk

**This host.** `docs/galion-voice-v2/personas/refs/` contains `.gitignore`, `samantha.placeholder`, and `jarvis.placeholder`. No `samantha.wav`. No `jarvis.wav`. `ls` of `/workspace/galion-clip-store/samantha/reference.wav` and `.../jarvis/reference.wav` failed: no such file. No wav sha256 was computed here. Nothing was copied into git. Nothing was deleted.

**NEXT-ITERATION.md, other host, cited and not re-run.** On `grok-bot-vm-511777615`, commit `8980e1a`, the author of that file reports the t273 files present at the gitignored paths, hashes matching the commented rows, and an in-tree Nano generate refused before torch: both personas exit 3, 0 PCM bytes, stderr `galion-clone-refuse: reference sha256 is not allowlisted for <persona>`. That file says torch, chatterbox, and `weights/clone/tokenizer/` were absent on that box, and that the worker has no `GALION_CLONE_REF_SHA` override. This worker did not repeat that command. Those numbers are that document's measurements, not a new run.

## Parked pull requests

Read from the GitHub API on 2026-11-15. Both still open. This pull request does not merge them, does not push to their branches, and does not add a consent record.

| PR | Head | What the PR itself says | What this lane does |
|---|---|---|---|
| [#78](https://github.com/galion-studio/galion-one-voice/pull/78) | `153945660c0ce46517f176f6fd5b263bf29c96ef` on `swarm/samantha-exp` | Title begins "DO NOT MERGE". Body: an experiment hook, plus a proposed Samantha CC0 reference swap that needs a new real consent record which does not exist yet. Consent is never fabricated. | Left parked. No consent written. |
| [#83](https://github.com/galion-studio/galion-one-voice/pull/83) | `e80ae092e73e7b241b36dda3b7312ea51ce9fd97` on `swarm/piper-cc0-finetune` | Title says do not merge yet. Body says owner card t662s3211 (2026-11-05 10:32 CET) widened Piper training consent for the two CC0 donors, and asks not to merge until corpus and training are reviewed. | Left parked. This worker did not see that card. The PR body is not treated as a new consent record. Backlog item: owner decision, not a Swarm 10 merge. |

## What this pull request changes

- This briefing, and nothing else. `PLAN.md` on #152 assigns `docs/swarm-10/2026-11-15/brain-eureka/clearance.md` to this role. `SELF-EVAL.md`, `RIGHTS-CLEARANCE.md`, `LOCKS.toml`, and `REFS.sha256` are untouched.

## What this pull request does not do

- Edit `LOCKS.toml`, `SOUL.md`, `config.yaml`, or `scripts/clone/REFS.sha256`.
- Tick a rights box, paste a filled ledger template, or sign `ADMIN-UI-CHECK.md`.
- Invent consent, counsel, a grantor, a rightsholder, or a performer signature.
- Commit a WAV, a weight, or NEXT-ITERATION.md.
- Merge #78 or #83, or edit the other open draft branches (#149, #145, #97, #87, #86).
- Deploy, read or embed server credentials, or call a paid model.
- Label Piper, Flite, or protocol-stub PCM as the original Samantha or Jarvis clone.
- Close self-eval B or C.

## Worker log

**Checked:** the files in the tables above; `LOCKS.toml` lines 141–158; `scripts/clone/REFS.sha256` (full file, 12 lines); voice TOMLs `[reference]`; `LEDGER.md` t273, t287, t628u lines; `chatterbox_worker.py` allowlist and the t273 docstring; `clone-landing-smoke.sh` lock grep; `v2_persona.rs` `gate_from`; `galion-consent` `reference_clips_allowed` and `BLOCKED_VOICES`; open PRs #78 and #83. Ref directories listed. `sha256sum` of `LOCKS.toml` and `REFS.sha256` recorded above, before any edit.

**Not run:** Nano generate, `scripts/symphony/self-eval.sh`, `cargo test --workspace` (local rustc is 1.83.0; CI pins 1.85.0), any fetch of weights, any read of a `consent.txt`, any hash of a wav. Unrun stays unrun.

**Open questions for Maciej:**

1. (a) or (b) above. Agents do not flip the lock either way.
2. If (a): the t273 PRIVATE CLEARED lines say counsel was not named. Re-opening those hashes is an owner lock edit. It does not tick the film checklist and it does not open a public demo.
3. If (b): say so in the ledger, and tell D3 to re-state B and C off the t273 hashes. The t623u Jarvis note says that donor is clean read speech, so "hub texture kept in the original clip" is a different bar.
4. #78 stays parked until a real consent record exists. This swarm does not write one.
5. #83 stays parked for owner review. This swarm does not merge it.
6. Live `/one` stays current Piper plus the Samantha persona until a clone mouth is actually live. This briefing does not switch it.

## Tests

Host `cursor`, 2026-11-15 22:07 +01:00. `rustc 1.83.0` / `cargo 1.83.0` on this box. The GitHub workflow `gates` installs `1.85.0`. `cargo test --workspace` was **not run** here (toolchain is not the CI pin). No wav was hashed.

| Command | Result |
|---|---|
| `scripts/check-locks.sh` | `OK: all Eureka locks hold (LOCKS.toml)`, exit 0 |
| `scripts/check-no-forbidden.sh` | `OK: forbidden-format + forbidden-runtime gates pass`, exit 0 |
| `python3 scripts/clone/test_worker_gate.py` | 13 tests, 0.292 s, OK, exit 0 |
| `scripts/clone/clone-landing-smoke.sh` | exit 0. `lock_flip=OPEN-t623u reference_clips=true (CC0 donors only)`. Both personas `status=skip reason=ref-absent`. `generate=blocked reason=weights-not-fetched model=chatterbox-nano device=cpu`. The script also printed `protocol PCM ok (not a model voice)`. That line is the protocol stub. It is not Nano PCM, not a product TTFA, and not a clone of either original. |

`git diff` of `LOCKS.toml`, `SOUL.md`, `config.yaml`, and `scripts/clone/REFS.sha256` was empty after these edits.

## CI

Not a result until the checks on this commit finish. A green local script is not a green GitHub check.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
