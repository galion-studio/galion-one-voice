# Swarm 10 — persona-v2v plan (Lane C)

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Written 2026-11-15 22:02 +01:00 (Europe/Zagreb).

The spelling is **Galion**. The product is Galion Voice. Owner: Maciej Grajczyk.

This file is the Lane C plan. It is the only file on the plan pull request. Workers stay in this lane. They do not merge.

## Baseline

| | |
|---|---|
| Repo | `galion-studio/galion-one-voice` |
| Base | `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` (merge of #144) |
| Lane | `persona-v2v` |
| Orchestrator branch | `cursor/persona-v2v-plan-6a84` |
| Plan PR title | `Swarm 10 persona-v2v plan` |
| Live mouth today | Piper `en_US-ljspeech-medium`. Say so. Do not call it the original Samantha or Jarvis clone. |
| CI on this tip | `gates`, `emotion-v2`, and `one-desktop` completed **success** on `8980e1a` (runs 43660665688, 43660665722, 43660665724, 2026-11-15). `nebula-voice-v2` did not run (path filter). |

`/workspace/voice-next/NEXT-ITERATION.md` (2026-11-15 21:56 +01:00, same tip, host `grok-bot-vm-511777615`) was read and is cited below. Do not commit that file.

Branch from current `origin/main`. Do not use a dirty checkout on `cursor/piper-stream-live-t301`. Do not edit open pull requests #145, #149, #97, #86, #87, #78, or #83.

## Workers

Open a **draft** pull request from a **new** branch off `main`. One concern. Title must start with the string in this table so the orchestrator can find it. Put the notes file in that same pull request. Mark the pull request ready for review only after the local commands in your row have been run and every required check on your head is green or still pending for a reason you name. Workers do not merge.

| Role | Item | PR title | Notes file |
|---|---|---|---|
| persona-live | C1 | `Swarm 10 persona-v2v persona-live: <short goal>` | `docs/swarm-10/2026-11-15/persona-v2v/persona-live.md` |
| one-dot | C2 | `Swarm 10 persona-v2v one-dot: <short goal>` | `docs/swarm-10/2026-11-15/persona-v2v/one-dot.md` |
| v2v | C3 and C5 | `Swarm 10 persona-v2v v2v: <short goal>` | `docs/swarm-10/2026-11-15/persona-v2v/v2v.md` |
| admin-brief | C4 | `Swarm 10 persona-v2v admin-brief: <short goal>` | `docs/swarm-10/2026-11-15/persona-v2v/admin-brief.md` |

Each notes file records what was checked, what changed, tests with results, CI, and open questions. Timestamps are Europe/Zagreb with offset `+01:00`. Copyright holder is Maciej Grajczyk. No tokens, keys, passwords, or server addresses. Redact IPv4. Unrun stays "not run". Unmeasured stays "not measured".

## Merge order

From backlog §8, applied to this lane. The orchestrator merges **one pull request at a time**, with a normal merge (no squash unless the PR is already a single commit and a merge commit is refused for a non-bypass reason — prefer a merge commit). No force-push, no history rewrite, no branch-protection bypass, no skipped or disabled checks.

Before each merge, every **required and non-skipped** check on the **current head** must be green. A pending check is not green. A check skipped by a path filter is skipped and does not block. After a merge, the next pull request updates from `main` and its checks run again on the new head before that next merge.

1. **C2 one-dot** first. Piper is enough for the screen hook. It does not wait on emotion or on the clone.
2. **C1 persona-live** next. Mouth evidence is about Piper until a clone mouth exists.
3. **C3 v2v** after C1, so the session log can cite the mouth C1 actually measured. **C5 stays BLOCKED** in that same pull request. There is no fifth merge for C5.
4. **C4 admin-brief** last, so the unsigned brief can name C1, C2, C3, and C5.

If a later pull request is green while an earlier one is still open, it waits. If an earlier one is honestly BLOCKED with no code left to merge, say so and continue with the next item that is green. Do not merge a red head to "keep the order".

## What each worker ships

### C1 — persona-live

Read `docs/galion-voice-v2/symphony/PERSONA.md`, `galion-emotion` `StubExpressor`, and `PiperProsody::with_emotion` in `crates/galion-ml-voice/src/speech/piper_worker.rs`.

On `8980e1a`, `with_emotion` returns `self` for every label, including non-neutral. Feeling is `StubExpressor` (RMS/ZCR-class stub). `v2-persona-selftest.sh` without a live server is STUB-PASS and is not this item's pass.

Done when there is one real spoken or `POST /v2/say` turn for Samantha and one for Jarvis, with `sense` on, and the notes show whether feeling-band `e` / the OKLCH wash moved and whether prosody was applied to the mouth that actually spoke. Words stay on the matter. The canned empathy list is absent. The engine name in the log is the engine that spoke.

The brief's done line is wash **and/or** mouth, or an honest FAIL or BLOCKED. The backlog acceptance asks for both. Report the split. If the mouth is Piper and emotion still cannot move its scales, the mouth half is FAIL or BLOCKED, not a hidden PASS. A wash movement from `StubExpressor` does not become a Piper-prosody pass.

A code change is allowed only when it applies scales that already exist on the Piper path, does not swap the voice pack, does not touch locked files, and a test shows a non-neutral label changes a scale that `Neutral` does not. Do not take on Emotion V2 training, `emotion_match`, or pull requests #86, #87, or #97.

Local commands before ready: the tests you add, plus `cargo test -p galion-emotion --lib` and, if `piper_worker.rs` changes, `cargo test -p galion-ml-voice --lib speech::piper_worker`. On a machine that has the t273 originals or clone weights on disk, `cargo test -p galion-ml-voice --features speech --lib` can fail the seven tests in backlog §5b. Those failures are Lane A item A5. Do not delete refs or weights, and do not flip pins, to make them pass.

### C2 — one-dot

Read `docs/galion-voice-v2/symphony/ONE-DOT.md` and run `scripts/run-v1-screen.sh --check` on `127.0.0.1`.

Done when the notes quote: `GET /` is the screen, `GET /v2` redirects to `/`, `/dot` `t_ms` or `engine.frames` moves, one poll shows speaking versus not speaking (speak wins), and the workflow hook fires in that same run. Loopback only. No second panel. Do not edit `LOCKS.toml` and do not add chrome to the dot.

Do not change the `/one` `data-persona` assignment. That candidate belongs to the orchestrator (see below). `productIsOne()` may stay; it also gates the Galion OS tones. Leave those tones alone unless a check you ran is red because of them.

Local command before ready: `scripts/run-v1-screen.sh --check`. If the console build fails, paste the error. Do not claim the gate from a contract test that never started the server.

### C3 — v2v, including C5

Read `docs/galion-voice-v2/V2V-SHOWCASE.md` and `docs/galion-voice-v3/VERIFICATION.md` Part A1–A2. No V3 code.

Showcase beats are the script: Rest, Strain, Joy, Command, hub on, hub off, Anger. One session record for Samantha and one for Jarvis. A wav stays **outside git**. A written listen log is acceptable when recording is withheld. Emotion or colour moves with the turn. No scripted empathy. Engine and LLM names are the ones the process printed.

Vosk weights are absent on the baseline host. A mic turn that errors is not a transcript. If you use text inject, label it. That row cannot claim full voice-to-voice. Brain may still be canned or `galion-reflect`. Write the name you observed. Tonight's virtual-user (2026-11-15 21:55 +01:00) was `llm=canned`, `speech_backend=a21-weights` (Piper). Do not recycle that file as your session.

**C5 stays BLOCKED.** A0 is BLOCKED-OWNER: `LOCKS.toml` `ref_clip_decision = "t623u"` and `scripts/clone/REFS.sha256` pin the CC0 donors, while symphony landing still names the t273 originals. NEXT-ITERATION generate from those originals was refused before torch (`reference sha256 is not allowlisted`, exit 3, 0 PCM, both personas). Do not flip `LOCKS.toml` or `REFS.sha256`. Do not label Piper or Flite as the clone. Piper labelled honestly is a negative for A1 and A2, not a pass. Agents do not sign V3-start. If you re-run the worker, quote the new stderr. If the refs are not on your machine, write "not run" and leave C5 BLOCKED on A0.

Unit commands from the showcase doc, when you touch that code:

```text
cargo test -p galion-emotion --offline --lib duplex
cargo test -p galion-render --offline --lib canned_empathy
cargo test -p galion-console --offline --features dot-viz --lib v2v
cargo test -p galion-heartbeat --offline --lib
```

`--offline` fails if the local index lacks a crate. Say so and retry the same targets online. Do not invent a listen.

### C4 — admin-brief

Read `docs/galion-voice-v2/ADMIN-UI-CHECK.md`. Run `scripts/eval/v2-hard-loop.sh` on CPU. The audio engine for a HARD-PASS of that loop is Piper, not a stub and not Nano labelled as HARD-PASS. Paste the overall line. `STUB-PASS` is not `HARD-PASS`. A missing live server is a stated gap, not a signature.

Ship a short brief for Maciej in the notes file. Include C5 as BLOCKED, with the A0 reason, even if C3 has not merged yet — then update the notes after C1–C3 land if their results change the sentence. **Do not tick a checklist box. Do not write the sign-off date. Do not sign `ADMIN-UI-CHECK.md`.** The thought box stays open while the only words are the instruction floor. Review the nerve record. Do not add a second pane to the one-dot.

## Checks the orchestrator will require

On each head, from the workflows that trigger on `pull_request`:

| Workflow | When it runs | Job |
|---|---|---|
| `gates` | every pull request | `test` |
| `emotion-v2` | every pull request | `emotion-v2 harness (CPU stub)` |
| `one-desktop` | every pull request | `playwright` |
| `nebula-voice-v2` | only if nebula paths change | `test` |

A docs-only pull request still runs `gates`, `emotion-v2`, and `one-desktop`. `nebula-voice-v2` staying absent is a skip.

The seven `galion-ml-voice` failures in NEXT-ITERATION §4.2 are local, on a tree that has t273 wavs and pinned safetensors. They are not a red `gates` run on `8980e1a`. Do not "fix" them in this lane.

## Orchestrator candidate — Samantha stays the default on `/one`

Not a worker task. Workers do not edit this.

On `8980e1a`, `crates/galion-console/assets/v1-screen.html` `mountSession` does this when `productIsOne()` (`pathname` is `/one` or `/one/`):

```text
document.title = "Galion One"
cv.dataset.persona = "galion"
return
```

The canvas markup default is `data-persona="samantha"` and `data-mouth="piper"`. `e2e/one/one.spec.js` posts `persona: "galion"` through `/__state` and expects `#s` `data-persona="galion"`. The Rust `/dot` floor still annotates `"persona":"samantha"` unless a turn asked for Jarvis (`galion-render` `v2_persona`).

Owner instruction for this lane: Voice V2 live default is **Piper + the Samantha persona** until a clone mouth is actually live. The orchestrator will open a **separate** pull request that stops the `/one` path from forcing `data-persona="galion"`, keeps the HTML default `samantha` and `data-mouth="piper"`, and updates the Playwright expectation that currently locks the flip. Title will start with `Swarm 10 persona-v2v`. It is a **candidate for Maciej**. It will not be merged by this lane as if the product-voice question were settled.

Why it stays flagged: `docs/galion-voice-v2/symphony/PERSONA.md` says Galion One is the product voice, Samantha is test/internal, and presenting a test voice as the product is FAIL. Backlog §7 item 4 still asks when, if ever, `/one` leaves Piper + Samantha for the Nano clone. Those two sentences are for Maciej. This lane does not edit `LOCKS.toml`, does not rename the page title away from Galion One, and does not claim the Piper pack is the original performer.

`productIsOne()` also starts the 432 Hz Galion OS tones and hides the desktop decide box. The candidate does not remove that behaviour.

## Locked and forbidden

Do not edit `SOUL.md`, `config.yaml`, or `LOCKS.toml`. Do not edit `voices/` weights, `weights/`, or `runtimes/` binaries. Do not flip `reference_clips`, `ref_clip_decision`, `paid_tiers`, `chrome_allowed`, or the 80 ms clock. Do not rewrite `scripts/clone/REFS.sha256`. Do not commit Samantha or Jarvis wavs, CC0 donor wavs, or clone weights. Do not delete a wav because a hash mismatches.

No deploys, no Titan, no server credentials, no paid model calls, no Qwen, no Groq, no cloud LLM, no DNS, no Cloudflare, no HADES, no firewall changes. CPU only. No GPU-mandatory path. Chatterbox and PyTorch stay out of process. No GGUF in Voice crates.

Out of scope: Emotion V2 pipeline, One-app and Research-desk feature work, payments, V3 implementation.

Protocol-stub PCM, `GALION_CLONE_STUB_*`, and a 15 ms stub first-PCM are FAIL for a product TTFA or HARD-PASS. Do not file them as a pass.

## Blockers (record, do not clear)

| Item | State | Why |
|---|---|---|
| C5 | BLOCKED | A0 BLOCKED-OWNER and A1 has not produced original-ref Nano PCM. Lane A owns the allowlist. |
| A0 | BLOCKED-OWNER | t273 originals versus t623u pins. No lock edit in this lane. |
| Vosk | absent on the baseline host | Text inject must be labelled. Full mic voice-to-voice is not claimed. |
| Piper emotion scales | no-op in `with_emotion` | C1 reports FAIL or BLOCKED for the mouth half if that is still true after the worker's run. |
| Admin sign-off | unsigned | Maciej signs `ADMIN-UI-CHECK.md` after he has heard generations. Agents do not. |
| `/one` persona | candidate only | Samantha-default pull request is flagged for Maciej and is not in the C2–C4 merge list. |

## Decisions left for Maciej

1. A0 — re-open the t273 originals on the live clone path, or state that main stays on t623u and symphony B/C must be re-stated. This lane does not choose.
2. Whether `/one` should keep `data-persona="samantha"` (Piper mouth) as the live default, against the PERSONA.md rule that a test voice is not the product voice. The candidate pull request is the proposal. Merging it is his.
3. Sign-off of `ADMIN-UI-CHECK.md`, after a HARD-PASS he has heard. The date stays blank here.
4. When the live mouth may leave Piper + Samantha for a clone. Not while C5 is BLOCKED.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
