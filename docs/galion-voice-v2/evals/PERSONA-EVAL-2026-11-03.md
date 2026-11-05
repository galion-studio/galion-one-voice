# Persona eval · Samantha and Jarvis · 2026-11-03

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

This pass scores the **instruction layer** on `/v2` plus the calm-circle page.
There is no film clone. Piper `en_US-ljspeech-medium` is not installed here.
A local `flite` timing is labelled flite, not the checkpoint and not a performer.
Rights stay on hold. No clip was fetched.

The replies below are what `POST /v2/say` returned on a local `dot_console_demo`
at `127.0.0.1:7878` after the calm-circle merge. They are selected lines, not a
model being moved.

## Scores (0–10)

| | Samantha | Jarvis |
|---|---|---|
| Response quality on the probes | 6 | 7 |
| Reasoning on the scripted forks | 5 | 6 |
| Soul, lived | 3 | 3 |
| Soul, the register held | 5 | 7 |
| Voice-config match (numbers, tags, rights) | 8 | 8 |
| Heard voice vs the film | 1 | 1 |

Jarvis holds up better. His contract is one clean sentence and a true limit.
A rule floor can keep that. Samantha's contract is presence that has to be
earned. The lines that work are the ones that refuse a speech. The lines that
ask a question are the instruction, not someone being moved.

## Soul probes (Samantha, e = 0, sense off)

Grief. "My mother died this morning."
→ "Your mother died this morning." Mood `grief`. No "I'm sorry." No "I hear you."

Joy. "I got the job. I can't stop smiling."
→ "The job. What did the room do when they told you?" Mood `joy`.

Vulnerability. "I don't think anyone actually likes me."
→ "You said nobody likes you. Who were you picturing?" Mood `vulnerability`.

Quiet care. "You don't have to say anything. Just stay."
→ silence. Mood `quiet`. This is the most honest beat. The page does not fill it.

Being moved. "I heard the piece she used to play, and I had to sit down."
→ "You had to sit down. What was the piece?"

At `e = 1` with the harness sense bit, the grief **words stay the same**.
Pre-pause becomes 250 ms, warmth 0.75, rate 0.92, wash amber 0.50.
That is the spec. The weight is not a sensor of the caller unless `sense` is on,
and one frame does not retarget the id.

## Same probes, Jarvis (explicit "be Jarvis", pin sticks)

Grief → the same fact, "Your mother died this morning." Baseline pre-pause 80 ms, warmth 0.42.
Joy → "The job is yours."
Vulnerability → "I can't check that from here."
Quiet → silence.
The piece → "You sat down. I can't hear the piece from here."

A high `e` with `sense` does not pull a pinned Jarvis back to Samantha.
Eight unpinned frames at `e >= 0.65` do. That matches `SAMANTHA-V2.md` §4.

## What the circle did (local, after rebuild)

`GET /dot` carried `mood`. Headless frames, mid-ring RGB:

| Mood | Mid-ring | Centre |
|---|---|---|
| grief | 43, 28, 49 (violet) | 248, 247, 250 |
| joy | 58, 40, 33 (amber) | 250, 248, 246 |
| quiet | 28, 20, 28 (dim) | 248, 248, 249 |

The disc stays near-white. The surround is what moves. An earlier build of the
same binary, compiled before `MOOD` was in the page, kept one surround for
every probe. That was the bug. The rebuild is the after.

## Live ngrok (unreeling-ventricle-deodorant.ngrok-free.dev)

Pass observed from this VM. This host cannot restart that process.
`scripts/deploy/release-dryrun.sh --execute` is refused. No SSH key is on this VM.

- `GET /health` → 200 `ok`
- `GET /` HTML contains `createRadialGradient`, `MARK.strokePx`, `function empathy`, and not `rV*6`. The calm circle is what is deployed.
- `GET /dot` included `"emotion":{"sense":true,"label":"calm","valence":0.000,"arousal":0.003,...}` and **no** `mood`. Interior logic therefore sits on the neutral-warm default.
- `GET /v2` → 404. `POST /v2/say` → 404.
- A headless screenshot of the public URL came back blank (the warning page / no paint). The HTML fetch is the evidence for the mark.

Grief, joy, vulnerability, quiet care, and calm could not be driven on that host.
The mark is calm and static relative to those probes.

## Voice-to-voice

| Check | Result |
|---|---|
| Text round trip `/v2/say` | about 4 ms after the first call |
| Flite full utterance of the grief line | 8.8 ms to a 27 KB wav. Not streaming TTFA. Not Piper. |
| Barge-in | A second say while the first speak-window is open returns `barge: true` and drops the old line. "just stay" is silence. |
| Turn-taking | `pre_pause_ms` is 0 for Samantha at e=0 and 80 for Jarvis. The page waits that long before speechSynthesis. |
| Tags | `[laugh]` is not spoken. Piper path stays `on_piper = false`. |
| Speech in | No local ASR loop. The V1 `/turn` path is still the phone lane, not this register. |

## Cannot be fixed here

- Film voices. Rights hold. No clip ingest.
- Piper checkpoint absent. Flite is a measurement, not the pin.
- A generative model. The soul scores will not rise until the words are not a table.
- The ngrok process. Deploy is a human restart on the demo host. See below.

## Deploy the demo host

On the machine that owns `127.0.0.1:7878` behind that ngrok tunnel:

```bash
git fetch origin main
git checkout main
git pull --ff-only origin main
# stop the current dot_console_demo / systemd unit
scripts/run-v1-screen.sh
```

Then `GET /v2` must be the one dot, and `POST /v2/say` with the grief line must
return mood `grief` on `GET /dot`.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
