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
# One-dot auto-record → workflow

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Master slogan: "The seamless connection with the human being, on the voice only."
Europe/Zagreb. 2026-11-03.

The person speaks. The dot records. The utterance proceeds.
There is no second AI screen.

## The path

`/` is the page (`crates/galion-console/assets/v1-screen.html`).
`/v2` redirects there. The microphone on that page is the workflow.

1. Tap the dot once. That grants the microphone and unlocks playback.
   After that the dot listens. There is no second tap to start a recording.
2. Speech above the VAD floor for 80 ms starts the take (200 ms while a
   reply is playing, so echo does not cut the reply).
3. Quiet for 640 ms, or 15 s, ends the take. That end is the auto-record.
4. `proceedTurn` posts the WAV to `POST /turn` with `X-Galion-Proceed: auto`.
   Same route the phone path already used. No `/workflow`, no `/skill`, no chat.
5. `PhoneHost::submit_turn` calls `proceed(Mic, None)` once for that take.
   No skill token is on the wire, so the live proceed is the auto program:
   the phone reply lane already queued for this turn.
6. The reply returns on `/dot` and `/reply/<id>.wav`. The dot plays it.
   The mark stays one circle.

`POST /mic` is still only the level. The utterance is `POST /turn`.

## Skill launch and the auto program

| Hint | Proceed | What runs |
|---|---|---|
| none, empty, `auto`, `program`, or not a token | Program `phone-turn` | PhoneLane on this turn |
| a skill token (`open-mind`, `jarvis`, `samantha`) | Skill | That name is the program slot. The same voice lane still speaks |

A skill token is one label: 1–32 characters, ASCII lowercase, digits, and
single hyphens. It is not a path, not a shell line, and not a URL.
`proceed` does not exec, does not spawn a process, and does not open a window.
Binding a token to a Galion program binary is a later sector. The launch
recorded here is the commitment: this utterance belongs to that skill, and
the voice program continues.

The page has no skill field. It sends `X-Galion-Proceed: auto` and no name.
`auto` is the mark that this POST is the proceed. It is not a skill name.
`proceed(Mic, None)` is the live `/` path, so the live proceed is the auto program.
A later owner program may call `proceed(Mic, Some(token))`. Junk falls back
to the auto program.

The typed box on `/` still posts `POST /v2/say` to the persona floor.
It does not launch a skill. The mic path is the workflow.

## What you see

One dot. The hint, the Art. 50 line, and the existing text box stay what
`ONE-UI.md` already describes. Nothing new is drawn for the workflow.
`LOCKS.toml` is not edited. `[ui]` stays one dot. `chrome_allowed` stays false.

## Hook

| Piece | Where |
|---|---|
| page | `proceedTurn` in `v1-screen.html` |
| header | `X-Galion-Proceed: auto` on `POST /turn` |
| allow | `CORS_ALLOW_HEADERS` includes that name. It is not a new route |
| decision | `galion_console::auto_workflow::proceed` |
| call | `PhoneHost::submit_turn`, once per accepted utterance |

The 80 ms heartbeat does not call `proceed`. Quiet frames do not count.
The call is one token scan when a take has already ended. It does not load
a model and it does not draw.

`galion-render` stores the WAV as it did. It does not choose the skill.
The choice sits in `galion-console`, next to the turn that was just accepted.

## Run

scripts/run-v1-screen.sh
scripts/symphony/auto-workflow-eval.sh

Open `http://127.0.0.1:7878/`. Tap the dot. Speak. Stop. The take posts
itself. The dot answers. There is no second page to open.


`scripts/symphony/auto-workflow-eval.sh` exits 0 only when this doc, the
page hook, the decision function, and an untouched `LOCKS.toml` all hold.
The result is recorded in `docs/galion-voice-v2/symphony/SELF-EVAL.md`.

## What this is not

Not a chatbot. Not an agent panel. Not a skill picker. Not a change to the
80 ms frame. Not a new model. Not a `LOCKS.toml` edit.

— Maciej Grajczyk
