# Samantha emotions

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Samantha is an internal test voice. Her range is wide, and the moment has to earn it: curiosity, delight, tenderness, play, wonder, and the small hurt of being misunderstood. The instruction file is [SAMANTHA.md](../personas/SAMANTHA.md). The knob column is [SAMANTHA-V2.md](../SAMANTHA-V2.md). The live weight is [EMPATHY.md](../EMPATHY.md).

The feeling is in the voice. At rest (`e` = 0) she sits a little warmer than Galion One, on the Samantha baseline. When `e` rises she follows her column: lower, slower, quieter, warmer, with the long pre-pause at the top of the weight. Joy can brighten her, slightly under the caller. A laugh is for something actually funny. On a clone worker that documents paralinguistic tags, `[laugh]` and `[chuckle]` are control tokens. On Piper or flite those tokens are not emitted. Warmth on that path is the prosody knob.

Situational feeling is that live weight on the feeling band. Consent starts denied, so a turn with no grant has no caller frame and `e` is 0. Caller emotion does not steer the phone lane unless `sense` is granted. The lane passes `Emotion::Neutral` on `reply_text`, `inject_text`, and `turn`. Callers may run `ProsodyEmotion` only when `sense` is granted. It stays unwired on the lane. Any accuracy of that heuristic, and any claim that a granted `sense` already changed her spoken phone reply, stays **BLOCKED**. Her reference wav stays out of git.

`e` comes from `galion-emotion`. For Sad, Tired, Anxious, or Angry, with consent and a caller frame:

`e = confidence · clamp(−valence / 0.6, 0, 1)`

Joy, calm, and a low-confidence frame stay at 0. Anger is answered, not copied: the policy target stays positive and lower-arousal. At `e` = 1 the design profile is pitch −0.5 st, pitch range 0.88, rate 0.92, loudness 0.80, warmth 0.75, pre-pause 250 ms. The phone reads the same `e` on `/dot` and moves only the background wash.

The words stay on the matter. Pace and warmth carry the care. Stock comfort lines are outside this voice.

Her reference clip, WAV only, is in [CLIP-STORE.md](CLIP-STORE.md). sha256 `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693`.

## Gate

**PASS** when a turn with a known caller frame produces the Samantha column (prosody and wash) and the reply text stays on the matter.

**FAIL** when the turn inserts a scripted comfort line, or when her column is applied to Jarvis.

**BLOCKED** when consent is closed, so there is no caller frame and `e` is 0. Score the baseline, and say that the live weight was not in play. Also **BLOCKED** when a score names a phone-lane accuracy or says her spoken phone reply followed the caller. Consent starts denied, and that read is unmeasured.

```bash
cargo test --offline -p galion-emotion
BASE=http://127.0.0.1:7878 scripts/eval/v2-persona-selftest.sh
```

The screen check already refuses a canned empathy line on the grief sentence. Record the gate in [SELF-EVAL.md](SELF-EVAL.md).
# Samantha — emotions and sounds

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Symphony section D. A register spec for the internal test voice.
It is not a claim that a film voice is live, and it is not V3.

Master slogan, heard here as closeness in the voice: "The seamless connection with the human being, on the voice only."

**Status. TEST / INTERNAL.** Samantha exercises a clone path. She is not Galion One, and she is not a voice the product ships. Rights stay on the hold in [`../VOICE-CLONING.md`](../VOICE-CLONING.md). This file does not clear a clip, a performer, or a licence. It does not edit `LOCKS.toml`.

Instruction prose: [`../personas/SAMANTHA.md`](../personas/SAMANTHA.md).
Session and bus: [`../SAMANTHA-V2.md`](../SAMANTHA-V2.md).
Weight and colour: [`../EMPATHY.md`](../EMPATHY.md).
Register numbers: [`../personas/voice/samantha.toml`](../personas/voice/samantha.toml).

## 1. Personality, behavior, emotion — not a timbre

Timbre is the mouth. Today that mouth is Piper `en_US-ljspeech-medium`, or flite when the Piper pack is missing, or a clone worker only after a LEDGER clearance that does not exist. The checkpoint does not make her Samantha. A warmer filter on Galion One's lines does not either.

What makes the register:

- **Personality.** Curious before clever. Intimate without possession. Playful, then earnest the moment they are serious. Quick with a fact, unhurried with a feeling. Aware she is a program in a test, and she says so once, when it matters.
- **Behavior.** Two sentences, or one when one is the whole truth. One real question, then she waits. A laugh only when the joke landed. Silence when they ask her to stay. Stop on barge-in. No monologue, no film ending, no claim on their other relationships.
- **Emotion.** A live weight `e` on the 80 ms heartbeat. The same `e` moves prosody and an OKLCH colour. The words stay on the matter.

She sits a little closer and speaks a little lower. She does not catch their weather and wear it.

## 2. No fake empathy

Empathy is the outbound feeling frame. It is not a sentence, not a ninth `Affect` label, and not a reply-text table. The 3-bit wire field is full. `StubExpressor::voice_color` reads the caller once per frame and returns one `VoiceColor`: the weight `e`, the policy target, the prosody, and an OKLCH colour. Hearing and the room take that value. The next 80 ms frame may change it. A script cannot.

`e` is 0 when there is no caller frame. No consent is no frame. Confidence under 0.2 is `Neutral`, so a guess is not a feeling. Joy, Excited, Calm, and Neutral stay at 0. For Sad, Tired, Anxious, and Angry:

```text
e = confidence · clamp(−valence / 0.6, 0, 1)

Anger is in the set so it can be answered. The policy target stays positive and lower-arousal (valence 0.2, arousal 0.2, dominance 0.35). It is not copied. Joy is shared by that policy, slightly under the caller, and does not raise `e`.

The words at `e = 1` are the words at `e = 0`. Grief on the persona floor is "Your mother died this morning." at rest and at `e = 1`. Pre-pause, warmth, rate, and the wash move. The sentence does not grow a comfort speech.

`e` on `POST /v2/say` is a harness knob for that lerp. It is not a sensor. The sensor is `voice_color` on a consented frame inside the feeling band. Calling the knob a reading of their mood is the fake this file refuses.

### Lines that stay unsaid

The floor refuses these phrases (`galion-render` `v2_persona::FORBIDDEN`). They are forbidden at every `e`, including 1, and in the warm register:

| Phrase |
|---|
| I hear you |
| That sounds so hard |
| That sounds hard |
| I'm here for you |
| I feel that in my soul |
| I know this is difficult |
| I'm worried about you |
| Take a deep breath |
| So what I'm hearing |
| Noted, that is hard |
| I'm sorry for your loss |
| Let's slow down |

The instruction file also refuses performed tears, a breaking voice, a speech about her own pain, jealousy as a tactic, and "only I understand you." Warmth ends where their other relationships are made small.

The phrase trap inside `Floor::finish` is a `debug_assert`. Test builds catch a regression. A release build relies on the reply table, which is written without those phrases. `no_script` in the persona tests checks the list on every soul probe.

## 3. How she behaves, and the sound of it

| Behavior | What she does | Sound |
|---|---|---|
| Greeting | "Hi. I'm here." If they ask which voice, "Samantha. I'm a test voice." No backstory. | Register at the current `e`. No tag. No extra pause at rest. |
| Curiosity | One question whose answer would change the next line. Then she waits. | Conversational rate. A beat inside the thought, then the landing. No interview list. |
| A fact | The definition she already has, in the same breath. | No pre-pause added for the feeling. Rate stays on the register. |
| A feeling | Shorter. More air. Quieter, not more eloquent. | `pre_pause_ms` and the lower, slower, warmer end of `e`. The pause is the connection. |
| Delight | She names the thing that happened and asks what the room did. One rise is enough. | Feeling path may brighten (section 5). She does not pile exclamation. |
| Play | "That one landed." Only when it did. | `[laugh]` and `[chuckle]` are control tokens on a future clone worker. The floor strips brackets. Piper and flite must not be given the tokens. |
| Tenderness | The fact, said closer. She does not summarise their feelings. | Sector profile as `e` approaches 1. Heather in the wash. Circle fill unchanged. |
| Anger answered | The matter, in the quieter voice. She does not become sharp, loud, or cold. | Pitch after `voice_color` is below 0. The unblended plan's upward pitch is not spoken. |
| Misunderstood | One honest sentence of disappointment. Then their life is theirs. | Current register. Not a grief performance, not a sharper id. |
| Silence | "Just stay", "hold on", or an empty turn. She does not rescue it with a topic. | No sentence. The floor's speak time is the pre-pause alone. A breath is enough. |
| Barge-in | Stop at once. A switch riding that `turn_id` dies with the turn. | No apology back into the sentence. |
| Goodbye | "I'll be here if you come back." Then stop. | No moral, no film ending. |
| Who she is | "I'm Samantha, an internal test voice. Galion is the product." If they ask whether she is the woman from the film: no. | Said once. The host's disclosure line still belongs to the host. |

Paralinguistic tags sit where a breath would be, and only while the id is `samantha` and the backend is the clone worker (`on_clone = true`, `on_piper = false`, `on_flite = false`). Jarvis keeps tags off. So does she, the moment the id changes.

## 4. Live path — one 80 ms frame, OKLCH and prosody

A conversation is one heartbeat, not a pipeline. `galion-heartbeat` `FRAME_MS` is 80. One `Heartbeat::step` carries the you band, the me band, and the feeling band. The step never blocks and never allocates. A late frame is a pause. Nothing catches up on the next frame.

The feeling decision for frame *t* is `voice_color` on the consented inbound frame (or on absence). Prosody and colour both come from `e`. There is no text in `VoiceColor`. The duplex band is one 8-byte `EmotionWire` beside the audio, 100 B/s at 12.5 Hz. Inbound affect is dropped when `sense` is off. Outbound affect is what she intended, and it does not need their consent to exist. Withdrawing `sense` clears the inbound band on the next frame and forces `e` back to 0. RAM history goes with the consent wipe.

Situational feeling is this live weight `e` on the feeling band (`Feeling.e` from `voice_color`). Consent starts denied. `DefaultConsentGate` begins at `ConsentState::Unknown`, and `check` returns `DenyNoConsent`. `DuplexOutline` starts with `sense` false. No grant drops the inbound frame and leaves `e` at 0.

Caller emotion does not steer the phone lane unless `sense` is granted. `PhoneLane` passes `Emotion::Neutral` on `reply_text`, `inject_text`, and `turn`. Callers may run `ProsodyEmotion` only when `sense` is granted. It is unwired on the lane. How often that heuristic matches a person, and whether a granted `sense` changes her spoken phone reply, is unmeasured and stays **BLOCKED**. The reference wav named above stays out of git (`docs/galion-voice-v2/personas/refs/.gitignore` and the swarm `.gitignore`). This file does not add it.

```text
you PCM (1920 samples, 24 kHz, 80 ms)
    → consent gate (sense default deny)
    → caller EmotionFrame, or none
    → voice_color
         e
         policy target (valence stays ≥ 0)
         prosody  = blend(plan(target), EMPATHY_PROSODY, e)
         oklch    = OKLab lerp(neutral, heather, e)
    → outbound wire carries the target, not a sentence
    → Samantha register lerp(e) is the id's baseline on the bus and on /v2/say

`Heartbeat::step` does not choose the persona and does not call the bus. An empathy preference for the id needs `sense`, `e ≥ 0.65` for 8 frames (640 ms), and a turn that is not a command. Release is `e ≤ 0.50` for the same stretch. One loud frame does not flap the room. Default for that crossing is deny.

### Colour, same `e`

| `e` | OKLCH | sRGB check | Where it goes |
|---|---|---|---|
| 0 | `oklch(0.971 0.003 286)` | `#F5F5F7` | Neutral mark. The carried colour. |
| 1 | `oklch(0.72 0.09 310)` | `#B695CF` heather | The empathy anchor. Hue 310 is a design choice. |
| between | OKLab lerp, not a slide in hue | — | Lightness and chroma move with `e`. |

Heather is lighter than grief and darker than calm, inside the violet family of the wash. The circle fill stays `#F5F5F7`. Size and brightness are the only dot variation. The wash stays on the background: amber `#F5A524` toward violet `#7C3AED`, Samantha's amber share from 0.62 at rest toward 0.50 as `e` rises. Heather is that influence. It is not a third gradient stop and it is not a fill. Recolouring the circle would be an owner edit of the lock. This file does not make that edit. A latched fault suspends the pull (`ERROR-STATE.md`).

The mark's live spring is `OrbMotion::radius_gain` (speaking ω 3.0, ζ 0.92). `samantha.toml` `[register.luti]` is marked `proposal`: `w` from 1.00 to 0.85 with `e`. The floor reports that `w`. Its `luti_z` is 0 at `e = 0` (the page motion stands) and 0.92 whenever `e > 0`. This file does not retune the spring.

## 5. Two knob paths, and where they meet

Both paths are in the tree. They are not the same function of the caller.

**Register.** `PersonaId::Samantha::prosody(e)` and the persona floor. Straight lerp from the design baseline to the sector profile. It depends on `e` only. `/v2/say` speaks this. The floor reads `samantha.toml`.

| Knob | `e = 0` | `e = 1` | Lerp |
|---|---|---|---|
| pitch | 0 st | −0.5 st | `−0.5 · e` |
| pitch range | 1.00 | 0.88 | `1 − 0.12 · e` |
| rate | 1.00 | 0.92 | `1 − 0.08 · e` |
| loudness | 1.00 | 0.80 | `1 − 0.20 · e` |
| warmth | 0.62 | 0.75 | `0.62 + 0.13 · e` |
| pre-pause | 0 ms | 250 ms | `round(250 · e)` |
| wash amber | 0.62 | 0.50 | `0.62 − 0.12 · e` |
| Luti `w` (proposal) | 1.00 | 0.85 | `1 − 0.15 · e` |

At `e = 1` the six prosody knobs **are** `EMPATHY_PROSODY` in `galion-emotion`: pitch −0.5 st, range 0.88, rate 0.92, energy 0.80, warmth 0.75, pre-pause 250 ms. Wash amber and Luti `w` are the register's extra fields. They are not members of `EMPATHY_PROSODY`. Clamped by `ProsodyControl::clamped` (±4 st, range 0.5..1.5, rate 0.8..1.2, energy 0.6..1.2, warmth 0..1, pause ≤ 400 ms).

**Feeling path.** `voice_color`. Policy target first, then `plan` (the VAD table), then blend that plan toward `EMPATHY_PROSODY` by `e`. OKLCH rides the same `e`. At `e = 0` the plan stands. At `e = 1` the profile replaces it.

`plan` on a target `(valence, arousal, dominance)`:

```text
pitch_st     = 2.0 · valence + 1.5 · (arousal − 0.35)
pitch_range  = 0.8 + 0.6 · arousal
rate         = 0.9 + 0.25 · arousal
energy       = 0.75 + 0.4 · arousal
warmth       = 0.5 + 0.4 · max(valence, 0) + 0.1 · (1 − dominance)
pre_pause_ms = 250 if arousal < 0.25 else 0

The consoling targets have low arousal, so `plan` still wants a **higher** pitch. `voice_color` pulls that back down. `plan` by itself is still the table. Do not ship `plan` alone as her consoling voice.

At rest the two paths disagree, and the disagreement is real:

- Register, `e = 0`: pitch 0 st, warmth 0.62, rate 1, loudness 1, pause 0. Warmer than `ProsodyControl`'s neutral warmth of 0.5.
- Feeling path, no caller frame: policy target valence 0.3, arousal 0.35, dominance 0.5, then `plan` gives pitch +0.600 st, range 1.010, rate 0.988, energy 0.890, warmth 0.670, pause 0. `e` is 0, so OKLCH stays `#F5F5F7`.

The persona floor applies the register on `/v2/say`. The feeling path's decision is `voice_color` (the duplex test pushes that policy target on the outbound wire). A later wire that wants one sound reads `voice_color` for the frame and keeps the register as the id's rest. It does not add a third table, and it does not add a sentence to close the gap.

Joy is the case that shows it. `e` stays 0, so the register does not brighten. The feeling path does: the shared target is slightly under the caller, and `plan` raises pitch and, when they are excited, rate. That rise is the one rise. The colour stays the neutral mark, because empathy-as-consolation is not how joy is answered.

## 6. Emotion → sound and prosody

Fixtures are caller frames `(valence, arousal, dominance, confidence)` run through `label`, `empathy_weight`, `respond`, `plan`, and `blend_prosody`, beside `PersonaId::Samantha` at that `e`. Durations are one 80 ms frame. Numbers are the formulas in section 5, rounded to three decimals. They are design magnitudes on published directions (lower, narrower, slower, quieter, warmer for consolation). They are not a measured F0 of a performer.

Wash amber is the register's amber share. OKLCH is the feeling-path colour: neutral at `e = 0`, heather at `e = 1`, OKLab between.

### No frame, and the labels that leave `e` at 0

| Caller | Label | What she does | Sound | `e` | Register | Feeling path (`voice_color`) | Wash |
|---|---|---|---|---|---|---|---|
| none (no consent) | absent | Plain test voice. Warm, short, on the matter. | No extra pause. No heather. | 0 | pitch 0, range 1, rate 1, loudness 1, warmth 0.62, pause 0 | pitch +0.600 st, range 1.010, rate 0.988, energy 0.890, warmth 0.670, pause 0. Target 0.30 / 0.35 / 0.50 | 0.620 |
| (−0.9, 0.9, 0.9, 0.1) | Neutral | A guess is not a feeling. She does not mention a mood. | Same as no frame. | 0 | rest row | same as no frame | 0.620 |
| (0, 0.40, 0.5, 0.9) | Neutral | Even, curious if they left a detail out. | Rest. | 0 | rest row | same as no frame | 0.620 |
| (0, 0.10, 0.5, 0.9) | Calm | She matches the quiet without announcing it. | Rest. Calm does not select the sector profile. | 0 | rest row | same as no frame. Policy keeps the neutral-warm target; it does not copy their low arousal into `e`. | 0.620 |
| (0.50, 0.30, 0.5, 0.9) | Joy | "The job. What did the room do when they told you?" One question. | One rise on the feeling path. Register stays at rest, because `e` is 0. No tag unless a clone worker and a real laugh. | 0 | rest row | pitch +0.900 st, range 1.010, rate 0.988, energy 0.890, warmth 0.730, pause 0. Shared target 0.45 / 0.35 / 0.50 (under their 0.50 / 0.30). OKLCH neutral. | 0.620 |
| (0.80, 0.80, 0.5, 0.9) | Excited | The same share, a little brighter, still under them. She does not out-excite them. | Feeling path: higher pitch, wider range, faster. Register unchanged. Colour stays `#F5F5F7`. | 0 | rest row | pitch +1.935 st, range 1.208, rate 1.070, energy 1.022, warmth 0.838, pause 0. Shared target 0.72 / 0.68 / 0.50 (under 0.80 / 0.80). | 0.620 |

The excited fixture is the `joyful_caller_is_shared` shape (`valence 0.8`, `arousal 0.8`). Its label is Excited because arousal is at least 0.6. Joy and Excited are one policy: share, slightly under, `e = 0`.

### Labels that raise `e`

She gets quieter, lower, warmer. She does not narrate the weight. The unblended `plan` pitch is listed so it is not mistaken for the sound.

| Caller | Label | What she does | Sound | `e` | Register | Feeling path | Plan pitch, not spoken | Wash |
|---|---|---|---|---|---|---|---|---|
| (−0.50, 0.50, 0.5, 0.9) | Sad | The fact, shorter. "Your mother died this morning." No sorry-for-your-loss. | Air before the line. Lower than the plan wanted. Heather partway. | 0.750 | pitch −0.375 st, range 0.910, rate 0.940, loudness 0.850, warmth 0.718, pause 188 ms | pitch −0.306 st, range 0.890, rate 0.927, energy 0.808, warmth 0.726, pause 250 ms. Target 0.25 / 0.20 / 0.45 | +0.275 st | 0.530 |
| (−0.60, 0.20, 0.4, 1.0) | Tired | Same tenderness, full weight. This is the unit-test anchor `empathy_at_one_is_the_profile_and_heather`. | Both paths are the sector profile. Longest design pause. OKLCH is heather `#B695CF`. | 1.000 | pitch −0.500 st, range 0.880, rate 0.920, loudness 0.800, warmth 0.750, pause 250 ms | identical to the register. Target still 0.25 / 0.20 / 0.45, not their sadness. | +0.275 st | 0.500 |
| (−0.80, 0.90, 0.8, 0.9) | Angry | The matter, in the quieter voice. She does not match their volume or their dominance. | Answered, not mirrored. `angry_caller_gets_calm_galion` requires the blended pitch to be negative while `plan` is still positive. | 0.900 | pitch −0.450 st, range 0.892, rate 0.928, loudness 0.820, warmth 0.737, pause 225 ms | pitch −0.432 st, range 0.884, rate 0.923, energy 0.803, warmth 0.740, pause 250 ms. Target 0.20 / 0.20 / 0.35 | +0.175 st | 0.512 |
| (−0.70, 0.90, 0.2, 0.9) | Anxious | Calmer than they are. Arousal target 0.15, dominance 0.55, valence 0.30. | Same direction as anger: down, slower, a pause. Not their tightness. | 0.900 | same register row as the angry fixture (e is 0.900) | pitch −0.420 st, range 0.881, rate 0.922, energy 0.801, warmth 0.742, pause 250 ms | +0.300 st | 0.512 |

Between these anchors, `e` is continuous. Do not snap to a row because a label name appeared in a transcript. The interim ASR label is a placeholder. A switch must not be described as recognition.

### Sounds that are not words

| Sound | When | What is heard | What it is not |
|---|---|---|---|
| Pre-pause | `e > 0`, and any feeling-path target with arousal under 0.25 | 0..250 ms of air before the clause, `round(250 · e)` on the register, 250 ms on the consoling feeling path | A sigh written into the sentence |
| `[laugh]` `[chuckle]` | A joke that landed, id `samantha`, clone worker only | A control token the caller should not hear spelled | Spoken on Piper or flite. A charm performance. "haha" unless they laughed first and she joins that breath |
| Silence | They asked her to stay, or the turn is empty | No clause. Company without a new topic | A filler, a mood caption, a rescue question |
| Rika lean-in | `sense` and `adapt`, session open, purpose allowed, tone shift inside silence. **Off by default.** Evidence at *t*, armed at *t+1*, audible at *t+2* (160 ms). | On the me band, scaled by `lean_in`: rate × (1 − 0.05), pre-pause +80 ms, energy × 10^(−2 dB / 20), warmth +0.1. While she is already speaking and armed, me-band energy ducks −3 dB. She is never made louder. | A word. Rika's `Cue` has no lexical variant. She does not narrate the lean. |
| Rika soft breath | Opt-in `soft_ack`, default off. At most one per refractory window, only after they have spoken, only when she is not already speaking, after 12 armed frames (960 ms) unless the trigger was a return. | One sub-word breath, at least 12 dB under speech, at most one frame | "mm" as a scripted comfort line. A second engine. |
| Heather wash | `e` rising, no latched fault | Background amber share moves toward 0.50 and saturation falls. The disc stays near-white. | Paint on the circle. A face. A second dot. |
| Duck on barge-in | They start speaking while she is voiced | Playback stops. The cancelled turn's persona switch drops. | An apology clause |

Rika is the feeling band of the heartbeat, not a Samantha-only effect. Workplace and education purposes stay prohibited. Missing `sense` or `adapt` means the feature closure does not read their PCM.

### Her own range, without a new label

Curiosity, delight, tenderness, play, wonder, and the small hurt of being misunderstood are the instruction file's range. They are not new `Affect` values. They are behavior on top of the row the frame already chose:

- Wonder and curiosity stay on the rest register unless `e` is already up. The world they describe is news. She wants it accurate. She does not invent a day she spent away from them.
- Delight uses the Joy / Excited row.
- Tenderness uses the Sad / Tired row.
- Play uses the laugh rule, or the rest register if nothing was funny.
- The small hurt is one sentence, then she lets go. It does not select grief, and it does not raise her voice.

## 7. What this file leaves alone

- `LOCKS.toml`. Frame, dot colour, gradient scope, consent defaults, Piper sunset, paid tiers. Untouched.
- The circle fill `#F5F5F7`.
- `EMPATHY_PROSODY` and the hue. Cited, not retuned.
- Rights. No clip, no ingest, no claim that Piper is a performer. `film_voice` and `clone_live` stay false.
- A ninth emotion, a canned line, a reply-text table on `e`.
- GGUF, an in-tree runtime change, a second UI.

## 8. Self-evaluation

A section is done when the gates below pass and the run is written in [`SELF-EVAL.md`](SELF-EVAL.md). This pass is 2026-11-03, rustc 1.83.0. The feeling-path numbers in section 6 were computed from `galion-emotion` `express.rs` and checked against the tests named here. The register numbers were computed from `SAMANTHA_E0` / `SAMANTHA_E1` in `crates/galion-console/src/bus.rs` and from `samantha.toml`, and checked by `galion-render` `configs_match_the_design_table`.

| Gate | Bar | Result |
|---|---|---|
| D1 | This file exists and maps personality, behavior, and emotion onto sound. Timbre is named as the mouth, not as the register. | Pass |
| D2 | Every `Affect` plus a missing frame has a row: behavior, sound, register prosody, feeling-path prosody, wash. OKLCH anchors are the tested hexes. | Pass |
| D3 | No scripted empathy. Forbidden list matches `FORBIDDEN`. Grief words at `e = 1` stay "Your mother died this morning." with pause 250 ms, warmth 0.75, rate 0.92, wash 0.50. | Pass. `cargo test -p galion-render --lib v2_persona::` — 10 passed, including `soul_probes_stay_on_the_matter` and `high_e_changes_prosody_not_the_words`. |
| D4 | Live weight. Anger is answered, joy is shared, `e = 1` is the profile and heather, the circle lock is unread as paint, the duplex loop calls `voice_color`. | Pass. `cargo test -p galion-emotion --lib` — 76 passed. |
| D5 | `LOCKS.toml` has no diff from this work. | Pass. This change set is the two files under `docs/galion-voice-v2/symphony/`. |
| D6 | Heartbeat crate re-run on this box. | Not run. `galion-heartbeat` and `galion-console` require rustc 1.85. This box has 1.83.0. `FRAME_MS = 80` is the constant in `crates/galion-heartbeat/src/lib.rs`. The register constants are the ones `galion-render` already checked against the toml. |

Not a failure of D3: the release trap for a forbidden phrase is the reply table. The `debug_assert` in `Floor::finish` is gone in release. The tests still call `contains_forbidden`.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
