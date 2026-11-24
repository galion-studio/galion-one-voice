# F — inner life

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S15, inner life. Roles: cares and wants, expectations and surprise, GEMI exchange, tests and this note. |
| Written | 2026-11-24. No voice audio, no recording, no weight, no network call. |
| Base | `main` `3269229` (PR #263 merged). |
| Crate | `crates/galion-emotion-core` module `inner`. `Brain::inner_nudge` calls it. |
| Locks | Added `LOCKS.toml` `[consent] learn = "deny"`. `scripts/check-locks.sh` asserts that string. No other lock value changed. `SOUL.md` is not in this tree; nothing was added. |

The inner life is what the speaker cares about, wants, and expects. It is a fixed block of open tokens, four slots each, not a list of emotions. A token is at most 16 ASCII bytes, the same width as a GEMI verb. Valence, arousal, dominance, and approach stay the moment's continuous point. This layer does not call `project_universal` and does not write a category pin.

## State

`InnerLife::seeded` starts from `standins/inner-life.txt`: care `heard`, want `answer`, expect `pause`. That seed is the fallback when boot is given no persona file. A loaded persona supplies the three tokens (`F-persona.md`). Memory (`memory.txt`) stays a stand-in. The intent script (`intent.txt`) stays the closed-eval stream for `EmotionCore::replay`. It is the same message type, not a second format.

`turn` reads the dialogue as ASCII bytes and the emotion point already on the moment. It does not keep the text. Care level steps a quarter of the way toward a target that rises when valence falls (`CARE_LEVEL`, `CARE_VALENCE_GAIN`). A question mark steps the want `answer` upward (`WANT_QUESTION`), and positive approach adds `WANT_APPROACH_GAIN`. These are knobs, not published loadings.

The scanner knows a short list of dialogue acts: `heard`, `answer`, `pause`, `question`, `thanks`, `wait`, `hello`, `yes`, `no`, `stop`, `again`. `wait` is the act `pause`. `thank` and `thanks` are the act `thanks`. A `?` with no act is the act `question`. Any other word is not a new care.

## Surprise

The live expectation is one token plus a belief. An inbound GEMI verb other than `hold` or `steady` is the event. Otherwise the dialogue act is the event. The same token leaves surprise at 0. A different token sets surprise to the belief (1 on the seed) and then adopts that token at belief `BELIEF_AFTER_MISS` (0.35). With no event, belief steps toward `0.5 + 0.5 * arousal`.

Surprise is a pull, not a pin. The outbound verb is the open token `surprise`. The offsets at strength 1 are `SURPRISE_VALENCE` −0.20, `SURPRISE_AROUSAL` 0.45, `SURPRISE_DOMINANCE` −0.08, and `SURPRISE_APPROACH` 0.10. `EmotionCore::step` multiplies each by `strength`, which is the surprise. `InnerLife::nudge` is the call that passes the message in. It reads `next_ms` and the current point, steps, and only then commits. A refused step leaves the inner life as it was. Nothing in this path assigns `VoiceBaseline`.

## Intent

Both directions are `IntentMsg`: 72 bytes, magic `GEMI`, version 1. `emit_gemi` encodes her own message (`IntentSource::Own`). `consume_gemi` decodes a message from the MnE side and runs one turn. A short or bad buffer is `BadIntent` and does not change the life.

Her own message copies the core's consent nibble, so a step does not clear `sense`, `adapt`, `remember`, or `share`. The pull is admitted because the source is `Own`. Inbound factor fields stay ignored by the core, as they already were.

## Learning

`[consent] learn` is `deny`. That string is `LEARN_LOCK`. `learning_allowed` is true only for `allow`. Learning would copy a novel dialogue word of 3 to 16 letters into a four-slot table. While the lock is deny, the table stays empty, `learn_refused` counts the attempts, and the seeded care and want tokens stay. Adopting the event she just saw into the live expectation is the working state of "expects". It is not that table. The lock is not a fifth EmotionWire bit.

`turn` and `nudge` do not allocate. `TURN_ALLOC_BUDGET` is 0. They sit outside `EmotionCore::step`, so `STEP_FLOP_BUDGET` is unchanged.

```shell
cargo test -p galion-emotion-core
cargo test --manifest-path horbm/Cargo.toml
bash scripts/check-locks.sh
```
