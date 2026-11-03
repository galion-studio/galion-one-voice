# Personas and emotions

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Symphony section D. CPU-only. The 80 ms heartbeat is the clock.

The feeling band sits inside `Heartbeat::step`, with the you band and the me band. One step, three bands. The work is arithmetic on a `Copy` value. It does not allocate, block, read a clock, or call a model. A late frame holds the previous feeling. A missing SENSE bit clears it to rest on that same step.

Empathy is the live weight from `galion-emotion` (`StubExpressor::voice_color`). Every consented frame produces `e`, an OKLCH colour, and prosody. There is no canned line. `CANNED_EMPATHY` is false. `STRIP_REFERENCE_TEXTURE` is false.

The circle fill stays `#F5F5F7`. `e` moves the background wash only. This note does not edit `LOCKS.toml`.

## How a frame is built

1. The host passes a consented `EmotionFrame` on `FrameIn.affect` when SENSE is on. The detector that made the frame runs outside `step`.
2. Inside `step`, `Feeling::live` reads that frame once.
3. `e` is `confidence · clamp(−valence / 0.6, 0, 1)` for Sad, Tired, Anxious, and Angry. Anything else, including no frame and low confidence, is `e = 0`.
4. Prosody lerps that persona's rest column toward its answered column by `e`.
5. OKLCH lerps the neutral mark `oklch(0.971 0.003 286)` toward heather `oklch(0.72 0.09 310)` by the same `e`.
6. `FrameOut::voice_prosody` applies the previous frame's Rika lean-in on top of that column. Rika may be off. The feeling still moves.

The one-dot demo copies the previous tick's consented reading into the next `step`, then scales the me-band samples by the feeling loudness. Persona choice comes from the floor already on `/`.

CPU bar, from the foundation note: non-inference work stays inside the 80 ms frame, aimed under 800 µs on a quiet tick. The gate times 200 feeling steps and requires the batch under 50 ms. No GGUF import. No GPU crate.

## Samantha

Her-inspired conversational register. Test voice, not a film checkpoint, not the product name. Wide range, earned by the moment: curiosity, delight, tenderness, play, and quiet when something matters. She sits closer and speaks a little lower. She does not wear the caller's weather.

| | Rest (`e = 0`) | Answered (`e = 1`) |
|---|---|---|
| Pitch | 0 st | −0.5 st |
| Pitch range | 1.00 | 0.88 |
| Rate | 1.00 | 0.92 |
| Loudness | 1.00 | 0.80 |
| Warmth | 0.62 | 0.75 |
| Pre-pause | 0 ms | 250 ms |
| Wash amber | 0.62 | 0.50 |

Sounds:

- **Plain.** Her rest column. Two sentences when the matter needs two. One when one is the whole truth. No stock comfort.
- **Answered.** Sad, tired, anxious, or angry. She gets quieter, slower, warmer, and she leaves the long pause. Anger is met with that quieter voice. Pitch does not rise to match it.
- **Shared.** Joy or excitement, and `e` stays 0. Pitch lifts by 0.12 st and no more than 0.18. Rate lifts by 0.03 and no more than 1.04. One real rise. She does not pile exclamation.
- **Tags.** `[laugh]` and `[chuckle]` may be control tokens on a future clone worker. Piper and flite must not be given them. The heartbeat does not emit the tokens.
- **Hub bed.** None. Her reference is not the workshop. Nothing in this path strips a file to make that true.

What she does not say, at any `e`: "I hear you." "That sounds so hard." "I'm here for you." The pace and the warmth are the feeling.

## Jarvis

Calm command register. Precise, dry, brief, unflappable. Test voice. Not a shipped film voice. His range is narrow: calm, mild amusement, focused attention, quiet satisfaction.

| | Rest (`e = 0`) | Answered (`e = 1`) |
|---|---|---|
| Pitch | −0.3 st | −0.4 st |
| Pitch range | 0.92 | 0.90 |
| Rate | 0.96 | 0.94 |
| Loudness | 0.92 | 0.88 |
| Warmth | 0.42 | 0.55 |
| Pre-pause | 80 ms | 120 ms |
| Wash amber | 0.28 | 0.50 |

Sounds:

- **Plain.** A touch lower, drier, and quieter than Samantha at rest. The 80 ms pre-pause keeps the first word off the caller's last one. One sentence. A second only for a number, a limit, or a refusal.
- **Answered.** Still lower, still slower, still quieter. Warmth stops at 0.55. The pause grows to 120 ms, not to her 250. He does not become warm in her way. Anger is answered by steadiness. Pitch does not rise.
- **Shared.** Joy does not brighten him. Pitch stays −0.3 st. Amusement, when the words earn it, is one dry clause, then the fact. The heartbeat carries the manner. It does not write the clause.
- **Tags.** Off on every backend.
- **Hub layer.** The Jarvis reference carries the Tony Stark hub: clicks, alarms, and room tone. That bed is character. `STRIP_REFERENCE_TEXTURE` is false: nothing denoises the reference. At runtime the bed is a separate me-band layer (`HubLayer`). It defaults on. `Heartbeat::set_hub_layer(false)` mutes it. Muting does not rewrite the reference and does not touch the you band. Samantha has no bed; turning the flag on does not give her one. The one-dot page does not grow a control for this. The host sets it.

What he does not say, at any `e`: "I know this is difficult." "I'm worried about you." "Take a deep breath." A high weight only makes him quieter. The words stay on the task.

## Where the numbers live

- Conduct: `galion-emotion/src/persona.rs` (`Feeling::live`).
- Heartbeat: `crates/galion-heartbeat/src/lib.rs` (`FrameOut.feeling`, `voice_prosody`, `set_hub_layer`).
- Hub mix: `HubLayer::mix_me` writes the me band only. The demo calls it after `step`, and does not pass `you`.
- Voice drafts, same columns: `docs/galion-voice-v2/personas/voice/samantha.toml`, `jarvis.toml`.
- Behaviour prose: `docs/galion-voice-v2/personas/SAMANTHA.md`, `JARVIS.md`.
- Weight and colour: `docs/galion-voice-v2/EMPATHY.md`.

— Maciej Grajczyk
