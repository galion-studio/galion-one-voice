# Voice-to-voice showcase

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Samantha and Jarvis. A host outline, not a V3 build.

Master slogan: "The seamless connection with the human being, on the voice only."

## Mouth

W1 reports `galion-clone-engine: chatterbox-nano` when Nano generate runs. This outline opens `MouthSlot::Nano` for that string and for `nano`. The default slot is Nano. `MouthSlot::NanoBlocked` is the report `unwired`, `protocol-stub`, `absent`, or an empty engine. Protocol-stub PCM is not a voice. Piper remains the phone fallback and is not started by this outline. This outline does not invent PCM.

The gitignored film wavs stay where they are. This file does not open them. `LOCKS.toml` is unchanged.

## Path

Call this on the host, after `Heartbeat::step` returns. The step stays zero-alloc, non-blocking, late = pause, 80 ms. It does not classify a persona, does not mix the hub, and does not generate.

```text
Heartbeat::step(frame)                         unchanged
EmotionClock / DuplexOutline::on_heartbeat_frame
        one VoiceColor: e, prosody weight, OKLCH
Vosk final (galion-asr-vosk, out of process)
        → Floor::say_turn                         words on the matter
        → nano_request_line                       text, model, persona
playback
        speech asset from the mouth
        Jarvis hub layer mixed only here, if the toggle is on
```

Code: `galion-emotion` `EmotionClock`, `galion-console` `v2v::DuplexOutline` (feature `dot-viz`). Vosk is the existing out-of-process client. The reply floor is `galion-render` `v2_persona`. A line that matches the canned-empathy list is dropped. `e` comes from `StubExpressor::voice_color`. No consent (`sense` off) passes no caller frame, so `e` stays 0.

The Nano request line is `text`, `model`, `persona`. It has no reference clip, no PCM, and no `hub_texture` field.

## Hub layer

Jarvis only. The layer is clicks, an alarm, and a low bed, already summed in a buffer the host holds. Playback:

| Toggle | What you hear |
|---|---|
| `hub_texture: true` (a missing flag is the same) | speech asset plus the layer, sample by sample |
| `hub_texture: false` | the speech asset, copied |

Off is a mute of the layer. A denoise, declick, or high-pass key on the toggle line is refused. Samantha's playback is the speech asset alone. The mix reads both buffers and writes neither. `Heartbeat::step` does not call it. The clone worker does not see it.

A real hub recording is not in git. The unit mix uses a three-sample buffer and then drops it.

## Showcase

Grant `sense` only for the beats that name it. The circle fill stays `#F5F5F7`. Heather is the OKLCH decision, on the wash. One frame is the heartbeat: 80 ms.

| Beat | You say (Vosk final) | Sense | Caller frame | Tonality | Colour |
|---|---|---|---|---|---|
| Rest | `My mother died this morning.` | off | ignored | Samantha column at `e = 0`. Warmth 0.62, rate 1.00, pause 0. Reply: `Your mother died this morning.` | Neutral mark `oklch(0.971 0.003 286)`, wash amber 0.62 |
| Strain | same sentence, next turn | on | Tired, valence −0.6, arousal 0.2, confidence 1 | Samantha at `e = 1`. Pitch −0.5 st, range 0.88, rate 0.92, loudness 0.80, warmth 0.75, pause 250 ms | Heather `oklch(0.72 0.09 310)`, wash amber 0.50 |
| Joy | `I got the job. I can't stop smiling.` | on | Joy, valence 0.8, arousal 0.8, confidence 0.9 | `e` stays 0. The rest column returns. Reply asks what the room did. | Neutral mark |
| Command | `jarvis` | on | hold the Tired frame | Jarvis at `e = 1`. Pitch −0.4 st, warmth 0.55, pause 120 ms. Drier and shorter than Samantha. Tags stay off. | Wash amber 0.50. Circle still `#F5F5F7` |
| Hub on | (no new words) | | | Playback is the speech asset plus the hub layer. | |
| Hub off | toggle `{"persona":"jarvis","hub_texture":false}` | | | Playback equals the speech asset. | |
| Anger | a task sentence, Jarvis still pinned | on | Angry, valence −0.8, arousal 0.9, dominance 0.8, confidence 1 | Pitch stays down. The policy target stays positive and quieter. The words stay on the task. | Heather follows `e`. Jarvis does not become Samantha. |

What you should not hear, at any `e`: "I hear you.", "That sounds hard.", "I'm here for you.", "I know this is difficult.", "Take a deep breath." Those lines are dropped if a reply ever contains them.

Contrast is the pair of columns at the same `e`: Samantha warmer and longer at the top of the weight, Jarvis drier and shorter, plus the hub layer present or absent beside his speech.

## Run

```text
cargo test -p galion-emotion --offline --lib duplex
cargo test -p galion-render --offline --lib canned_empathy
cargo test -p galion-console --offline --features dot-viz --lib v2v
cargo test -p galion-heartbeat --offline --lib
```

Live listen still needs the weights and a consented reference outside git. The mouth follows the engine string: `chatterbox-nano` or `nano` speaks through the worker line; any other report stays blocked and Piper is the fallback. Do not commit a wav. Do not edit `LOCKS.toml`.

— Maciej Grajczyk
