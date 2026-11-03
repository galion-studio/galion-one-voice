# Auto-workflow

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The person speaks. The page records. The process knows the next step and takes it. That next step may be a skill, or an auto program. The mechanism can change. The process is the contract. One circle, described in [ONE-DOT.md](ONE-DOT.md), is the only place this starts.

## What already happens

On `http://127.0.0.1:7878/`:

1. The person taps the circle. The browser captures the microphone.
2. The page posts level to `POST /mic` while they talk.
3. On a pause, the page posts the WAV to `POST /turn`.
4. The turn worker, off the 80 ms tick, runs PhoneLane: Vosk when the model is present, the canned reply, Piper or flite.
5. The page plays `GET /reply/<id>.wav` and `/dot` carries the reply fact.

Typed text uses `POST /v2/say` on the same page. Speaking over the reply cuts it off. The message shapes are [IPC.md](../IPC.md).

That sequence is the recording half of the workflow. The skill / auto-program half is the hook this section adds: after the turn exists, a named next step runs without a second screen. Examples the owner named: launch as a skill, or launch an auto program. The hook is a function of the turn (transcript, persona, consent), and it returns on the same circle.

## Gate

**PASS** when a spoken turn records itself and a test double of the workflow hook fires once, with the transcript and the persona id, and no second UI appears.

**FAIL** when the person has to open another panel to start the next step, or when the hook runs inside `Heartbeat::step`.

**BLOCKED** when the hook is not in the tree yet. The recording path is the screen above. The skill dispatch is the missing piece, so this landing does not mark the row **PASS**.

```bash
scripts/run-v1-screen.sh --check
scripts/check-phone-pipeline.sh
```

Record the gate in [SELF-EVAL.md](SELF-EVAL.md).

— Maciej Grajczyk
