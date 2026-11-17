# Swarm 10 — persona-v2v one-dot (C2)

Perpetual Open MIT License. (c) 2026 Galion Labs.
Written 2026-11-15 22:10 +01:00 (Europe/Zagreb). Persona edit removed 2026-11-15 22:12 +01:00.

The spelling is **Galion**. The product is Galion Voice. Owner: Maciej Grajczyk.

Role: one-dot. Item C2. Lane plan: pull request [#150](https://github.com/galion-studio/galion-one-voice/pull/150) (`cursor/persona-v2v-plan-6a84`). This seat did not edit that plan, `LOCKS.toml`, `SOUL.md`, `config.yaml`, or any other open draft.

Base after the plan merge: `origin/main` `107a11f3a5af7e9adb4b3213e458e1c68cc6b2aa` (pull request #150). The first commit on this branch was `8980e1a`.
Host of the commands below: this cloud VM, Linux 6.12.113+, 4 CPUs, `rustc 1.85.0 (4d91de4e4 2025-02-17)`. RAM was not measured. This is not the NEXT-ITERATION host named in the backlog. Loopback only (`127.0.0.1`). No deploy.

## What changed

`scripts/run-v1-screen.sh --check` runs the existing synthetic room script in that same process, checks that `/dot` `t_ms` moves, that a speaking sample has `state` speaking and that a non-speaking sample is in the same poll, and waits for `workflow proceed source=mic program=phone-turn` after `POST /turn` with `X-Galion-Proceed: auto`. It prints the first `phone reply` log line so the mouth backend is visible. The hour-long serve without `--check` stays idle until a mic or typed turn. `PhoneHost` prints that proceed line. No second panel.

## `/one` persona is not in this pull request

The first commit on this branch also changed `v1-screen.html`, `dotviz.rs`, and `e2e/one/one.spec.js` so `/one` kept `data-persona="samantha"`. The swarm lead said that change lives in one place only: [#156](https://github.com/galion-studio/galion-one-voice/pull/156). Those three files are restored to `main` here. The `--check` lines that failed on `dataset.persona="galion"` and required `shownPersona` are removed. [#156](https://github.com/galion-studio/galion-one-voice/pull/156) stays the owner-flagged candidate. It does not merge without Maciej's explicit yes, and neither does any other copy of that edit.

On the 22:10 +01:00 host the Piper pack was **absent**. The `phone reply` line in that run was Flite (`backend=system-flite`). That is not Piper, and it is not a clone. The markup `data-mouth="piper"` on `/` is the page default. It is not a claim that the self-test audio was Piper.

## Checks on this host

`bash scripts/symphony/check-e-one-dot.sh` (2026-11-15 22:09 +01:00): `E PASS`. That script says the live browser was not run. It is the contract test only.

`scripts/check-locks.sh`: `OK: all Eureka locks hold (LOCKS.toml)`.
`scripts/check-no-forbidden.sh`: `OK: forbidden-format + forbidden-runtime gates pass`.

`cargo test -p galion-console --features phone-turn --lib` (Rust 1.85.0): **88 passed, 0 failed**, exit 0.

`cargo test -p galion-render --features web --lib page_shares_the_lattice_and_the_hold`: **1 passed**, exit 0.

`npx playwright test` in `e2e/one` (Chromium, Playwright 1.56.1): **12 passed**, exit 0, on the head that still expected `data-persona="samantha"` after the persona edit. That assertion is no longer in this pull request. The re-run of `one.spec.js` against the restored page is quoted with the second `--check` below.

`scripts/run-v1-screen.sh --check` (2026-11-15 22:10 +01:00), exit 0, on the head that still contained the `/one` persona check. Vosk model absent at `weights/asr/vosk-model-small-en-us-0.15`. Quoted output from that earlier head:

```text
mic ASR: Vosk model missing at weights/asr/vosk-model-small-en-us-0.15
  typed text still works. Mic turns return an error and do not invent a transcript.
Galion Voice · one dot → http://127.0.0.1:7878/
  / : one canvas page OK (39015 bytes)
  / : persona, mic page, orb OK
  /one : default persona samantha, mouth piper (not a clone)
  /v2 : redirect OK
  /v2/state : rights hold OK
  POST /v2/say : no gateway, plain unreachable, no canned empathy
  /dot samples: 150 modes: idle listening rika speaking
  /dot : t_ms 2160 -> 13760
  /dot : speak wins (state speaking, and a non-speaking sample)
  POST /mic 204 OK
  OPTIONS /turn 204 OK
  POST /turn 202 OK
  workflow hook: proceed source=mic program=phone-turn
  mouth log: phone reply id=1 asr=text-inject backend=system-flite audio_ms=1680 in_ms=0 took_ms=23 err=-
  security headers OK
```

The absolute model path in the real stdout was the repo `weights/asr/...` path. No server address other than loopback was printed.

After those three files were restored to `main`, the same script was run again on this VM (host `cursor`, rustc 1.85.0, started 2026-11-15 22:12 +01:00, wall 16 s), exit 0:

```text
mic ASR: Vosk model missing at /agent/repos/galion-one-voice/weights/asr/vosk-model-small-en-us-0.15
  typed text still works. Mic turns return an error and do not invent a transcript.
Galion Voice · one dot → http://127.0.0.1:7878/
  / : one canvas page OK (38661 bytes)
  / : persona, mic page, orb OK
  /v2 : redirect OK
  /v2/state : rights hold OK
  POST /v2/say : no gateway, plain unreachable, no canned empathy
  /dot samples: 150 modes: idle listening rika speaking
  /dot : t_ms 2080 -> 13760
  /dot : speak wins (state speaking, and a non-speaking sample)
  POST /mic 204 OK
  OPTIONS /turn 204 OK
  POST /turn 202 OK
  workflow hook: proceed source=mic program=phone-turn
  mouth log: phone reply id=1 asr=text-inject backend=system-flite audio_ms=1680 in_ms=0 took_ms=38 err=-
  security headers OK
```

There is no `/one` persona line in that output. The page still flips `data-persona` to `galion` on `/one`. That flip is only in [#156](https://github.com/galion-studio/galion-one-voice/pull/156).

Same command, `cargo +1.85.0 test -p galion-console --features phone-turn --lib screen_page_is_phone_path_and_v2_orb`: 1 passed, exit 0. `npx playwright test one.spec.js` in `e2e/one`: 3 passed, exit 0, including idle `/one` with the restored `data-persona="galion"` expectation. The other nine Playwright specs were not re-run in this pass.

Same run, from `target/v1-screen/dot-samples.jsonl` (not committed): 150 samples, `t_ms` 2160 to 13760, `engine.frames` 28 to 173, every `persona` was `samantha`, modes `idle` `listening` `rika` `speaking`, every speaking sample had `state` `speaking`. One speaking sample: `t_ms` 2320, mode speaking, state speaking, persona samantha, `engine.frames` 30. One other sample: `t_ms` 2160, mode listening, state listening, persona samantha, `engine.frames` 28.

The heartbeat log for that process also contained `synthetic room/you/me script on` and `workflow proceed source=mic program=phone-turn`.

## What this number is not

Speak-wins here is the synthetic room script (and, on this host, a Flite text-inject reply that the log names). It is not a Nano generate, not a clone of the t273 originals, not a product TTFA, and not a Piper measurement. TTFA was **not measured**. `GALION_HARD_HTTP=1 scripts/eval/v2-hard-loop.sh` was **not run**. That full report is C4. C5 stays BLOCKED on A0; this seat did not re-run the clone worker.

## CI

At the time this note was written, checks on the pull-request head had not been started. The pull-request body is updated after they finish. `nebula-voice-v2` does not run for these paths (path filter). A skip is not a pass and not a failure.

## Open questions for Maciej

1. [#156](https://github.com/galion-studio/galion-one-voice/pull/156) is the only open copy of the `/one` Samantha default. PERSONA.md still calls Samantha a test voice. It does not merge without an explicit yes.
2. The 22:10 self-test reply was Flite because the Piper pack was absent on that VM. The Titan default (Piper plus Samantha) was not re-measured here.
3. A0 (t273 versus t623u) is unchanged. This seat did not edit locks or `REFS.sha256`.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
