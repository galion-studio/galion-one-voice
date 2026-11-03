# Admin UI check — generations and LLM thoughts

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The spelling is **Galion**. The product is Galion Voice.

## Ready for Maciej

The schedule is open. This page is ready for Maciej Grajczyk to review and
sign in person. Agents must not sign. Every checklist box stays unchecked.
The date stays blank. This page does not claim HARD-PASS. A stub harness
line is not a signature. The real gates and how they are proved are in
`HARD-REAL.md`.

**Schedule.** This check is scheduled for **Maciej Grajczyk**. It was
opened 2026-11-03. It is not started, and it is not done. A cloud agent
must not tick a box and must not write the date. **Maciej Grajczyk must
sign the sign-off line himself** after he has heard the generations and
read the thoughts on a machine that produced them.

A pull request that adds this page does not complete the check.
`scripts/eval/v2-hard-loop.sh` does not see the screen. If that harness
prints `STUB-PASS`, or `HARD-PASS of the loop: refused`, the generations
line below stays open. Do not sign on a stub report.

## Where to look

The phone page is one mark. `GET /v2` redirects to `http://127.0.0.1:7878/`.
That page does not draw an LLM thought pane, a menu, or a transcript.
`LOCKS.toml` `[ui]` is one dot, and this check does not add chrome to it.

What Maciej reviews:

| Surface | What it is | What it is not |
|---|---|---|
| The one-dot at `:7878` | The mark, the wash, and the sound of a generation for Samantha and for Jarvis. | A film voice. `film_voice` stays false until a rights line exists. |
| `POST /v2/say` and `GET /dot` | Mood on the fact: grief, joy, vulnerability, quiet. The disc stays `#F5F5F7`. The wash moves. | A second graphic. Canned lines such as "I hear you" or "I'm sorry". |
| The nerve record | `galion-openmind` notes after the frame, not inside `Heartbeat::step`: `voice.frame_miss` when a frame paused, `voice.backend_up`, `voice.fallback`. Optional NDJSON when a human set a log path. See `OPENMIND-NERVE-CORTEX.md`. | A fake interim transcript. `GALION_ASR=interim` is refused on the live screen. The nerve line carries no PCM and no caller transcript. |
| `scripts/eval/v2-hard-loop.sh` on the same machine | The overall line. Sign the generations box only when that line is `HARD-PASS`. | `STUB-PASS` from this harness, from `v2-persona-selftest.sh`, or from `flash-ttfa.sh`. |

If the only words on the turn are the persona-floor table, they are the
instruction layer. They are not an LLM thought. Leave the thought box open.

Speak wins: while the mark is speaking, listening does not replace it.
Pause holds: a late heartbeat frame does not start a new me onset and does
not catch up on the next frame. The wash may sit still. It must not burst.

Failures have to be visible. A missing weight pack, a worker exit 3 on a
clip, or a hung generate is a stated failure. A silent spinner is a miss
on the last box.

## Checklist

Leave these unchecked until the signed review.

- [ ] Live generations audible (Samantha and Jarvis), from Nano, not from the protocol stub and not from Piper labelled as the film
- [ ] LLM thought / nerve pane visible and accurate (no fake interim)
- [ ] Emotion and colour stay with the turn, and the words are not canned empathy
- [ ] One-dot mark: speak wins; pause holds
- [ ] Failures surface clearly (no silent hang)
- [ ] Same machine: `scripts/eval/v2-hard-loop.sh` overall line is `HARD-PASS`
- [ ] Sign-off: Maciej Grajczyk · date ______

## Not a signature

The copyright line under this page is the licence sign on the text.
It is not the sign-off above. The date stays blank until Maciej Grajczyk
writes it.

— Maciej Grajczyk
