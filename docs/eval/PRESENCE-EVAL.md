# Presence eval — virtual user (T0–T2)

Galion Voice · owner Maciej Grajczyk · Perpetual Open MIT · © 2026

Offline harness for the phone path. The virtual user and PhoneLane share one machine. Nothing is sent off the box, and nothing is a PSTN call.

```bash
scripts/eval/virtual-user.sh
```

Exit codes: `0` green, `1` a scored case failed, `2` usage or IO. The script passes `--offline` to Cargo. The binary binds `127.0.0.1` and refuses any other address.

## Report

| Path | What |
|---|---|
| `galion-gates/results/presence-eval/<UTC stamp>.json` | One run |
| `galion-gates/results/presence-eval/latest.json` | Copy of the newest run |
| `galion-gates/results/presence-eval/<UTC stamp>/*.wav` | Captured reply audio, when Piper or flite spoke |

`galion-gates/results/` is gitignored. CI runs the script and reads the exit code. Schema: `galion-presence-eval/v1`.

Direct binary, same behaviour:

```bash
cargo run --offline -p galion-presence-eval -- --out report.json --wav-dir wavs --commit "$(git rev-parse --short HEAD)"
```

## What one run does

1. **Text inject.** `PhoneLane::inject_text` / `turn_text`. The label is `text-inject`. The words were supplied. `claims_recognition` is false.
2. **Fixture WAV.** A stub-codec WAV of a known phrase. PhoneLane decodes it exactly (`stub-fixture`). That decode is not reported as recognition.
3. **Reply.** CannedLlm, then Piper (`a21-weights`) or flite (`system-flite`) when `GALION_SPEECH` can speak. The harness writes the 24 kHz reply WAV and reads it back (RIFF, rate, peak).
4. **Score.**
   - **TTFA** (`ttfa_ms`) only when a reply WAV was captured. It is wall time from the end of inject until that file is in hand, plus leading silence in the file. `ttfa_kind` is `loopback-full-utterance`. PhoneLane returns the whole utterance, so this is not a streaming first-frame time and not PSTN mouth-to-ear. With no speech backend the field is null and `ttfa_status` is `unmeasured`. The draft web and PSTN budgets are not a CI failure.
   - **Task facts.** Canned intents (`hello`, `how are you`, `hello how are you`, `can you hear me`, `thanks`, and an unknown line) must match the table in `CannedLlm`.
   - **Barge-in.** `TurnWorker::supersede` must drop the older reply and still accept the next turn. `barge_in_kind` is `worker-supersede`. `barge_in_stop_ms` is how long that drop took to observe. It is not an acoustic stop on a speaker.
   - **ASR label.** A synthetic tone under `GALION_ASR=interim` must come back as `interim-placeholder`, with `claims_recognition: false` and `fixture_decoded: false`. The transcript is a duration placeholder. The tone was not heard as words.

## Tiers in this harness

| Tier | Where | What this run actually does |
|---|---|---|
| T0 | Every PR | Task facts, interim label, metric denylist, supersede cancel. No socket. |
| T1 | Every PR | Text, fixture WAV, and the interim tone, posted to `127.0.0.1`. |
| T2 | Every PR, loopback only | One spoken capture when a backend exists. `pstn` is `not_run`. |

The research brief's larger T1 pack (about 50 scenarios, WER, false barge-in) and the external PSTN probe are not this script. This is the offline loop the phone path can run before any of that.

## Receipt

Each report records commit, `asr_mode` (`interim`), `llm` (`canned`), speech backend and label, `endpointing` (`scripted-inject`), `vad` (`not-on-this-path`), `transport` (`loopback`), `bind` (`127.0.0.1`), `network` (`false`).

## Not in the report

The same names `galion-presence` rejects are rejected as JSON keys here: engagement, retention, session length, time in session, dwell, speech elicited. A poisoned key fails the run before the file is written.

Interim is a placeholder. The report must not say it recognised the caller. `claims_recognition` on a green run is false for every case, including stub fixtures.

No GGML. No change to `LOCKS.toml`. No SSH.

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
