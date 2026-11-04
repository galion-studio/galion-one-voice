# One-dot workflow

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Symphony section E. CPU-only. One mark. No second AI interface.

The surface is the page already served at `/`: the circle, the mic, the persona floor, and the text box (`crates/galion-console/assets/v1-screen.html`). `GET /v2` redirects to `/`. It does not record on its own.

## What happens when someone speaks

1. The first tap unlocks the microphone on that same page.
2. An energy VAD on the device cuts the utterance (about 80 ms to start, about 640 ms of quiet to end) and posts it to `POST /turn` as WAV. That is the recording. Nothing is written to disk.
3. `PhoneHost::drive_me`, outside `Heartbeat::step`, takes the newest utterance and sets the workflow to `Recorded`. The turn id is kept. The samples are not.
4. The existing turn worker runs ASR, the reply, and speech off the 80 ms thread. When words come back, `next_step` replaces `Recorded` with the next action. The transcript is not stored on the hook.
5. The reply audio returns on `/dot` as `reply.url`. The same page plays it. The circle is still the only mark.

Typed lines use the text box already on `/` and `POST /v2/say`. They do not open a chat panel.

## The hook

`crates/galion-console/src/workflow.rs`. Four outcomes, all in-process:

| `Next` | When | What proceeds |
|---|---|---|
| `Recorded` | Audio accepted, words not in yet | Wait. Do not invent a tool. |
| `Speak` | Anything that is not a known program or skill, including empty words | The persona answers on the dot. |
| `Program("heartbeat")` | "start the heartbeat" or "run the heartbeat" | The heartbeat program already in the process. |
| `Program("live-demo")` | "start the demo" or "run the demo" | The live demo already in the process. |
| `Skill("presence")` | "skill presence" or "run presence" | The presence skill already in the process. |
| `Skill("locks")` | "check the locks" or "skill locks" | The locks check already in the process. |

An unknown "skill …" stays `Speak`. The mechanism is this table. A later automatic program can register another static name. It must not add a screen.

`PhoneHost::workflow()` returns `(turn_id, Next)` for the host. `PhoneHost::voice_persona()` is what the demo copies onto the heartbeat before each `step`, so Jarvis or Samantha is the feeling column for that frame.

CPU bar: the hook runs after the utterance is queued, not inside `Heartbeat::step`. The step itself stays non-allocating and non-blocking. No GGUF import. No GPU.

## What this refuses

- A second panel, a chat transcript view, or a skill picker.
- A scripted empathy sentence as the thing the workflow "knows." Care, when it is real, is the feeling band on the heartbeat.
- Recording to disk, or keeping the transcript on the hook.

— Maciej Grajczyk
