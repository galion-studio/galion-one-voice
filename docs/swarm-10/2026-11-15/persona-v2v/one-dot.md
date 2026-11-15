# Swarm 10 — persona-v2v one-dot (C2)

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Written 2026-11-15 22:10 +01:00 (Europe/Zagreb).

The spelling is **Galion**. The product is Galion Voice. Owner: Maciej Grajczyk.

Role: one-dot. Item C2. Lane plan: pull request [#150](https://github.com/galion-studio/galion-one-voice/pull/150) (`cursor/persona-v2v-plan-6a84`). This seat did not edit that plan, `LOCKS.toml`, `SOUL.md`, `config.yaml`, or any other open draft.

Base: `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0`.
Host of the commands below: this cloud VM, Linux 6.12.113+, 4 CPUs, `rustc 1.85.0 (4d91de4e4 2025-02-17)`. RAM was not measured. This is not the NEXT-ITERATION host named in the backlog. Loopback only (`127.0.0.1`). No deploy.

## What changed

`/one` used to set `data-persona` to the product page name inside `productIsOne()`. The canvas markup was already `data-persona="samantha"` and `data-mouth="piper"`.

`shownPersona` keeps Jarvis when the floor says Jarvis, and keeps Samantha for every other label, including the product page name. On `/one` the title stays **Galion One**, the canvas persona is Samantha, and `data-mouth` stays `piper`. No new element was added. The 432 Hz tones and the decide box are unchanged. `productIsOne()` still gates those tones.

`scripts/run-v1-screen.sh --check` now runs the existing synthetic room script in that same process, checks that `/dot` `t_ms` moves, that a speaking sample has `state` speaking and that a non-speaking sample is in the same poll, checks the served `/one` HTML, and waits for `workflow proceed source=mic program=phone-turn` after `POST /turn` with `X-Galion-Proceed: auto`. It prints the first `phone reply` log line so the mouth backend is visible. The hour-long serve without `--check` stays idle until a mic or typed turn.

## Flag for Maciej — not a settled product-voice ruling

[#150](https://github.com/galion-studio/galion-one-voice/pull/150) says this persona edit is a candidate and that workers do not own it. The launch instruction for this seat said to include the small tested change and to flag it. It is in this pull request for that reason.

`docs/galion-voice-v2/symphony/PERSONA.md` says Galion One is the product voice, Samantha is test/internal, and presenting a test voice as the product is FAIL. This change does not rename the page title. It does set the canvas persona attribute on `/one` to Samantha, with the mouth attribute `piper`, because the owner instruction for Voice V2 is Piper plus the Samantha persona until a clone mouth is actually live.

Merging that attribute is Maciej's decision. This seat does not merge.

On this VM the Piper pack is **absent** (`weights/tts/en_US-ljspeech-medium.onnx` absent, `weights/runtime/piper/piper` absent). The one `phone reply` line in the self-test was Flite. That is not Piper, and it is not a clone of the original Samantha. The canvas attribute `data-mouth="piper"` is the labelled default. It is not a claim that this run's audio was Piper.

## Checks on this host

`bash scripts/symphony/check-e-one-dot.sh` (2026-11-15 22:09 +01:00): `E PASS`. That script says the live browser was not run. It is the contract test only.

`scripts/check-locks.sh`: `OK: all Eureka locks hold (LOCKS.toml)`.
`scripts/check-no-forbidden.sh`: `OK: forbidden-format + forbidden-runtime gates pass`.

`cargo test -p galion-console --features phone-turn --lib` (Rust 1.85.0): **88 passed, 0 failed**, exit 0.

`cargo test -p galion-render --features web --lib page_shares_the_lattice_and_the_hold`: **1 passed**, exit 0.

`npx playwright test` in `e2e/one` (Chromium, Playwright 1.56.1): **12 passed**, exit 0. The desktop `/one` test expects `data-persona="samantha"` and `data-mouth="piper"`, then Jarvis from `/__state`, then the product-page label falls back to Samantha.

`scripts/run-v1-screen.sh --check` (2026-11-15 22:10 +01:00), exit 0. Vosk model absent at `weights/asr/vosk-model-small-en-us-0.15`. Quoted output:

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

Same run, from `target/v1-screen/dot-samples.jsonl` (not committed): 150 samples, `t_ms` 2160 to 13760, `engine.frames` 28 to 173, every `persona` was `samantha`, modes `idle` `listening` `rika` `speaking`, every speaking sample had `state` `speaking`. One speaking sample: `t_ms` 2320, mode speaking, state speaking, persona samantha, `engine.frames` 30. One other sample: `t_ms` 2160, mode listening, state listening, persona samantha, `engine.frames` 28.

The heartbeat log for that process also contained `synthetic room/you/me script on` and `workflow proceed source=mic program=phone-turn`.

## What this number is not

Speak-wins here is the synthetic room script (and, on this host, a Flite text-inject reply that the log names). It is not a Nano generate, not a clone of the t273 originals, not a product TTFA, and not a Piper measurement. TTFA was **not measured**. `GALION_HARD_HTTP=1 scripts/eval/v2-hard-loop.sh` was **not run**. That full report is C4. C5 stays BLOCKED on A0; this seat did not re-run the clone worker.

## CI

At the time this note was written, checks on the pull-request head had not been started. The pull-request body is updated after they finish. `nebula-voice-v2` does not run for these paths (path filter). A skip is not a pass and not a failure.

## Open questions for Maciej

1. Keep `data-persona="samantha"` and `data-mouth="piper"` on `/one`, or put the product-page name back on the canvas. PERSONA.md still calls Samantha a test voice.
2. This VM had no Piper pack, so the spoken reply in the self-test was Flite. The Titan default (Piper plus Samantha) was not re-measured here.
3. A0 (t273 versus t623u) is unchanged. This seat did not edit locks or `REFS.sha256`.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
