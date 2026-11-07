# Rollout and consent blockers

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Status of open pull requests **#87**, **#88**, **#83**, and **#78** against `main` at `b9bd890` (2026-11-07). That tip includes #85 and #89–#95. This note is a proposal. It does not merge, deploy, retarget, or push those four branches. It does not add a measurement or a consent record.

Conflict check: `git merge-tree --write-tree --name-only --messages origin/main <head>` exited 0 for every head below and printed only a tree id. GitHub `mergeable_state` was `clean` for all four. No conflicted paths. The only path edited on both sides of each head was `crates/galion-ml-voice/src/speech/piper_worker.rs`, and the bytes already match `main` (the Piper reap from #95). That file is not in any merge result below.

Owner in the tables is the next actor: **laptop run**, **admin consent**, or **review**.

## PR #87 — reference fidelity

Draft. https://github.com/galion-studio/galion-one-voice/pull/87

Head `6f72ae14e212bafe223b331a19f09361954a56df` on `swarm/symphony-a-fidelity-20261106`. Recorded base is `save/2026-11-06-cloning-v2-persona-scorer` (`2ba95c0`), which is already an ancestor of `main` via #85. A merge into that base does not update `main`.

Merge result onto `b9bd890`: the `tools/cloning-v2/fidelity/` crate and a README link, 15 files, +3073. The reap commit on this head is already on `main` and drops out.

Checks: `gates` / `test` passed (run 42343782105). That workflow does not run `eval-laptop.sh`. There is no ECAPA, WER, or emotion job on this head.

The PR body states that the host which wrote the DSP table had no GPU, no ECAPA weights, no ASR, and no persona-scorer JSON, and that WER, ECAPA, and `emotion_match` were not measured. This note does not repeat those DSP figures and does not treat them as the rollout gate. The body also states that the reported cosine is the conditioned reference against the donor, not a Piper or clone utterance, and not ECAPA.

| Blocker | Evidence | Next action | Owner |
|---|---|---|---|
| WER, ECAPA, and `emotion_match` have no measurement | PR body; `eval-laptop.sh` leaves `wer`, `ecapa`, and `emotion_match` null unless the env vars below are set. `gates` / `test` does not set them | On the laptop, run the command in the next section and keep each clip's `*-eval.json` | laptop run |
| The hyp wav is the assembled donor, not a clone | `eval-laptop.sh` `prepare` writes `${name}-cond.wav` and scores that against the donor. PR body: the cosine does not move ECAPA of a Piper or clone utterance | If the rollout gate is a spoken clone, score that utterance with the same three fields. Do not file the DSP cosine as ECAPA | laptop run |
| One transcript is shared by every clip | `GALION_FIDELITY_REF_TEXT` is one string, applied inside `run_pair` to Amy, Alan, the long cut, and the two-half cut | Use a transcript that matches the wav being scored. Amy and Alan stay separate runs; do not average them | laptop run |
| PR base is not `main` | GitHub base ref `save/2026-11-06-cloning-v2-persona-scorer`, already merged | Retarget the PR base to `main` only when someone chooses to. Do not push the head from this note | review |

### Laptop command

From the repo root on the laptop (the script's header names an RTX 4060 8 GB). DSP only, which leaves the three gate fields null:

```bash
tools/cloning-v2/fidelity/scripts/eval-laptop.sh
```

Gate fields, using the script's own env vars. The `.../` prefixes are the script's placeholders for binaries that stay outside git:

```bash
GALION_ECAPA_BIN=.../galion-ecapa GALION_ECAPA_MODEL=.../ecapa.onnx \
GALION_WER_BIN=.../galion-wer GALION_FIDELITY_REF_TEXT='...' \
GALION_PERSONA_JSON=.../persona.json \
  tools/cloning-v2/fidelity/scripts/eval-laptop.sh
```

Optional, from the same script:

- `GALION_FIDELITY_HYP_TEXT` — with `GALION_FIDELITY_REF_TEXT`, skips `GALION_WER_BIN` and still fills `wer`.
- `GALION_FIDELITY_LONG_WAV` — one public-domain wav outside the repo. Default is the cache's decoded long cut.
- `GALION_FIDELITY_CACHE` — must be outside the repo. Default `${XDG_CACHE_HOME:-$HOME/.cache}/galion-fidelity`.

The script calls `galion-ecapa --ref <donor> <hyp> --threads 2 [--model GALION_ECAPA_MODEL] --out <name>-ecapa.json` and `galion-wer --ref "$GALION_FIDELITY_REF_TEXT" --jobs 1 --out <name>-wer.json <hyp>`. Reports land in `$CACHE/out/{amy,alan,long,long-multi}-eval.json`.

`tools/cloning-v2/fidelity/src/score.rs` on this head is the pass rule. Each report must show a number, not null, and the matching `*_pass` field true:

| Env input | JSON field | Pass |
|---|---|---|
| `GALION_WER_BIN` plus `GALION_FIDELITY_REF_TEXT`, or both `GALION_FIDELITY_REF_TEXT` and `GALION_FIDELITY_HYP_TEXT` | `wer` (fraction) and `wer_percent` | `wer` ≤ 0.018, which is `wer_percent` ≤ 1.8. `wer_pass` true |
| `GALION_ECAPA_BIN` (executable) and, when set, `GALION_ECAPA_MODEL` | `ecapa` | `ecapa` > 0.7057. `ecapa_pass` true. `embed_cosine` is the DSP vector and is not this field |
| `GALION_PERSONA_JSON` | `emotion_match` | `emotion_match` ≥ 0.83. `emotion_match_pass` true. The file may carry `emotion_match`, or `category_blend` and `emb_sim` (then `0.70 * category_blend + 0.30 * emb_sim`), or a mean of `sets.*.emotion_match` |

`mos_proxy` is a DSP proxy. It is not the gate.

## PR #88 — Symphony V2 emotion42

Draft. https://github.com/galion-studio/galion-one-voice/pull/88

Head `055cca3524f9d53776ff9f42b97ad522c4109b1c` on `cursor/symphony-v2-emotion42-d379`. Base `main`. GitHub lists 41 commits not in `main`. The PR body still says 31 ahead; that count was the earlier tip `0e1aeb2`. Local count: 41 ahead, 22 behind `b9bd890`.

Merge onto current `main` is clean. No conflicted paths. #89–#95 added `docs/emotion-v2/`, `emotion-v2/`, and `tools/emotion-v2/`; this head does not edit those paths, so the merge result keeps them. The Piper reap on this head matches `main` and is absent from the merge diff. The merge diff is 78 files, +8225 / −668, including the symphony stack, `tools/cloning-v2/emotion-match/`, `preflight/`, and `score-wrap/`.

Checks: `gates` / `test` passed on this head (run 42343776651, completed 2026-11-07 05:43 UTC). The comment posted at 05:28 UTC said `test` still failed at "Clone worker scaffold (no weight download)". That comment is older than the reap commit `055cca3` (05:41 UTC), which is the passing head. The `emotion-v2` CPU stub is not in this head's check rollup; the workflow arrived on `main` with #90. A clean merge does not delete it.

| Blocker | Evidence | Next action | Owner |
|---|---|---|---|
| `emotion_match` is under 0.83 | PR comment 2026-11-07 05:28 UTC, after `1a9c8ea` (ancestor of this head): Samantha line pick emotion 0.4912; Jarvis `v2-sim-rs-cleaned` emotion 0.7460. Same comment: WER 0.015 / 0.018 and ECAPA 0.7114 / 0.7231. `docs/emotion-v2/EMOTION-V2-FOUNDATION-SPEC.md` §7 on `main` records a different pair that also fails: Samantha 0.491 raw and 0.461 normalized, Jarvis 0.807. §7 says that file does not authorise merging #88. These are quoted records, not a new run | Produce a new laptop log that meets `emotion_match` ≥ 0.83 per voice, beside WER ≤ 1.8% and ECAPA > 0.7057. Do not average the two quoted Jarvis figures | laptop run |
| Branch docs still say the emotion rows were not run | `docs/galion-voice-v2/symphony/EMOTION-BLEND.md` on this head marks WER, ECAPA, MOS, accuracy, and emotion distance **BLOCKED** because `galion-wer` / `galion-ecapa` were absent. `GATES.md` marks the TTFA commands **BLOCKED** with `measured: false` and no `ttfa_ms` | Write the laptop log into the tree only after a real run. A PR comment is not that log | laptop run |
| No admin yes to merge | PR body: draft only, do not merge without Slavik's yes. No review on the PR. Spec §6.5: merge, deploy, or a live-voice switch needs Slavik's own explicit yes | Slavik signs in his own name after the gate log exists. Until then the PR stays draft | admin consent |
| Git conflicts with #89–#95 | None. `merge-tree` exit 0. GitHub `mergeable_state` `clean` | Do not resolve conflicts; there are none. Do not merge from this note | review |

## PR #83 — Piper fine-tune consent addendum

Open, not draft. https://github.com/galion-studio/galion-one-voice/pull/83

Head `e80ae092e73e7b241b36dda3b7312ea51ce9fd97` on `swarm/piper-cc0-finetune`. Title and body: do not merge until the corpus and the training are reviewed.

Merge result onto `b9bd890`: `LOCKS.toml` exception `formats.exceptions.v2s14_piper_cc0_finetune`, `galion_consent::piper_training_allowed`, a unit test, and two in-tree addenda. 5 files, +138. The Piper reap on this head is already on `main`.

Checks: `gates` / `test` passed (run 42379470331) and `emotion-v2` / `emotion-v2 harness (CPU stub)` passed (run 42379470333). Neither check reviews a corpus or a training run. No approving review.

The addendum text in the diff asserts that Mate Grajczyk, relaying for admin Slavik, answered YES on card `t662s3211` at 2026-11-05 10:32 CET, scope `piper-voice-training`, personas `samantha` and `jarvis` only. That card string does not appear on `main` (not in `LEDGER.md`, not in `docs/`). This note does not confirm that YES and does not write another one.

The live gate does not read the in-tree copies. `piper_training_allowed` opens `$GALION_CLIP_STORE/<persona>/consent-addendum-piper-training.txt`. `evaluate` refuses a store inside the git tree. The originals stay `scope: local-zero-shot-conditioning` (`docs/galion-voice-v2/sprint-v2/bakeoff-t648/REPORT.md`: Piper training on these donors is outside that scope). Store path on `main`: `$GALION_CLIP_STORE/<persona>/{reference.wav, reference.wav.sha256, consent.txt}` (`docs/galion-voice-v2/CLONE-CC0-DONORS.md`).

| Blocker | Evidence | Next action | Owner |
|---|---|---|---|
| Corpus and training are not reviewed | PR title and body. Diff has no corpus, no training log, and no review | Review the corpus and the training run. Do not merge on the addendum text alone | review |
| No Slavik signature for a live Piper voice | Spec §6.5. The addendum's `approved_by` line says the owner was relaying for Slavik. There is no review from Slavik on the PR | Slavik's own explicit yes, after the review above, before this merges or a live voice changes | admin consent |
| Store addendum is not in the diff | Gate path is the clip store outside git. The PR files are `docs/galion-voice-v2/clip-store-cc0/{samantha,jarvis}/consent-addendum-piper-training.txt` | Place a real addendum beside each store `consent.txt` only after the yes above. Fields the gate checks: `persona`, `scope: piper-voice-training`, `decision: t662s3211`. Do not edit the original `consent.txt` | admin consent |
| `LOCKS.toml` would change on merge | Proposed key `formats.exceptions.v2s14_piper_cc0_finetune`, `decision = "t662s3211"`, `owner = "Maciej Grajczyk"` | Leave `LOCKS.toml` unedited until the same yes exists. This note does not edit it | admin consent |

## PR #78 — Samantha reference swap

Open, not draft. https://github.com/galion-studio/galion-one-voice/pull/78

Head `153945660c0ce46517f176f6fd5b263bf29c96ef` on `swarm/samantha-exp`. Title and body: **do not merge**. The body says `5899146` needs a new real consent record which does not exist, and that consent is never fabricated.

Merge result onto `b9bd890` is clean and still contains the swap plus the experiment hook. 10 files, +149 / −19. The Piper reap matches `main` and drops out. Checks passed: `gates` / `test` (run 42379693132) and `emotion-v2` CPU stub (run 42379693151). Those checks do not create a consent record.

Proposed Samantha pin (commit `5899146` and `scripts/clone/REFS.sha256` on this head):

- sha256 `1636428272b85d4b4f6c67bb859bfe33d9983ff9439646519305ff13e6921005`
- duration 11.41 s, 44100 Hz, mono, 16-bit
- Amy Koenig, same LibriVox public-domain recording, source window 17.84–29.25 s, no denoise, no gain

Current pin on `main` (`docs/galion-voice-v2/CLONE-CC0-DONORS.md` and `scripts/clone/REFS.sha256`): sha256 `e4bc60c33f697108cf5585dead213da1065979010f5be1d513574893584a4641`, 9.60 s, cut 10.55–20.15 s. Jarvis is unchanged (`60ad5457…`).

`galion_consent::evaluate` returns `sha` when `consent.txt`, `reference.wav.sha256`, and the wav bytes disagree (`crates/galion-consent/src/lib.rs`). `crates/galion-consent/src/recut.rs` never writes `consent.txt`; the operator copies `duration_s` and `sha256` into a real signed file. Required fields are the 15 names in `REQUIRED` (`date`, `persona`, `work`, `performer`, `grantor`, `instrument`, `grantee`, `scope`, `public_demo`, `expiry`, `operator`, `withdrawal`, `counsel`, `sha256`, `duration_s`), with `scope` = `local-zero-shot-conditioning`, `persona` = `samantha`, and a `date` that carries `CET` or `Zagreb`. This head adds no `consent.txt`.

Commit `5899146` says the proposal needs an owner yes and a Slavik yes. Commit `a671031` is the `GALION_EXP=1` seeded sweep hook in `scripts/clone/chatterbox_worker.py` and is marked experiment-only, do not ship. That hook is still in the merge diff.

| Blocker | Evidence | Next action | Owner |
|---|---|---|---|
| No consent record for the new cut | PR body. No `consent.txt` in the diff. Current record is the 9.60 s / `e4bc60c3…` pin. A re-cut fails the `sha` check until the new digest is in a real store record and sidecar | Owner Maciej Grajczyk (Mate) and admin Slavik each say yes. Then the operator writes a new store `consent.txt` and `reference.wav.sha256` for sha256 `16364282…` and duration 11.41 s. Do not invent that file in git | admin consent |
| Experiment hook is still in the diff | `a671031`, `GALION_EXP=1` path in `scripts/clone/chatterbox_worker.py`. PR body: experiment-only | Keep it off `main`. A later consent record does not make this hook shippable | review |
| GitHub will merge it cleanly | `mergeable_state` `clean`. The PR is not draft | Leave it open and unmerged | review |

## Summary

| PR | Head | Merges onto `b9bd890` | Blocking item | Owner |
|---|---|---|---|---|
| #87 | `6f72ae1` | Clean. Base ref is still the already-merged persona-scorer branch | Laptop `eval-laptop.sh` with the three env groups; `wer` ≤ 0.018, `ecapa` > 0.7057, `emotion_match` ≥ 0.83 per clip. Unmeasured today | laptop run |
| #88 | `055cca3` | Clean. No conflicts with #89–#95 | Emotion gate still under 0.83 in the quoted laptop records. Slavik's own yes. Stays draft | laptop run, then admin consent |
| #83 | `e80ae09` | Clean | Corpus and training review, then Slavik's own yes. Store addendum is not the in-tree copy | review, then admin consent |
| #78 | `1539456` | Clean, and must stay unmerged | New signed `consent.txt` for sha256 `16364282…` / 11.41 s, from the owner and Slavik. `GALION_EXP=1` stays off `main` | admin consent, and review |
