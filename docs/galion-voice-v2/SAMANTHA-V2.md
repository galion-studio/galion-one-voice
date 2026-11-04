# Samantha V2

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A session spec. It is not a claim that the session is built,
and it is not V3.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

Program plan, on main: `GALION-VOICE-V2.md`.
Empathy sector note: `EMPATHY.md` on the open branch
`cursor/empathy-voice-color-4169` (PR #13). It is not on main yet. The
numbers below are taken from that note and from `StubExpressor::voice_color`.
This file does not retune them. If the two ever disagree, the sector note
wins on the weight, and this file is the session that has to follow.

## 1. Concept

Samantha V2 is the Galion Voice bot session: the one a person is already in
when they talk. Voice and interface are one experience. Pitch, rate,
loudness, and warmth move with the feeling, and the orb moves with the same
feeling, in the same moment. One decision. Two places it shows up: the
speaker, and the room around the mark.

She does not catch your weather and wear it. She sits a little closer and
speaks a little lower.

This is not a second UI. It is not a skin, a chat window, a face, or a
second dot. It is the same one mark `LOCKS.toml` `[ui]` already names:
one filled circle, size and brightness only, wash on the background only,
`chrome_allowed = false`. If a screenshot shows a second shape, the session
is wrong.

Samantha, here, is the conversational register already written in
`VOICE-CLONING.md` (V2-S14). The 2014 film voice stays on the legal hold.
This spec is how the session behaves. It is not a clone, and it does not
say the Piper checkpoint is a person.

Runtime character instructions: [`personas/`](personas/README.md).

## 2. Technical mapping

The empathy sector (PR #13, `galion-emotion` `StubExpressor::voice_color`)
is the whole feeling decision. Samantha V2 calls that function. She does
not grow a ninth `Affect` label. The 3-bit wire field is full.

### The weight

`e` is continuous, one number per feeling frame, in `0..=1`.

`e` is **0** when any of these is true:

- there is no caller frame (no consent is no frame)
- confidence is under the label floor (`EmotionFrame::label` returns
  `Neutral` below 0.2, so a guess is not a feeling)
- the label is anything other than Sad, Tired, Anxious, or Angry
  (Joy, Excited, Calm, and Neutral stay at 0)

For those four labels:

```text
e = confidence · clamp(−valence / 0.6, 0, 1)
```

Anger is in the set so it can be **answered**. It is not copied. The
policy target from `StubExpressor::respond` stays positive and
lower-arousal: an angry caller is met with valence about 0.2, arousal 0.2,
dominance 0.35; anxious with a calmer 0.15 arousal; sad or tired with
gentle low arousal. Joy is shared by the old policy and does not raise `e`.
There is no reply-text table on `e`.

### Prosody at e = 1

`voice_color` blends `plan` (the VAD table) toward this profile, then
clamps. At `e = 0` the table stands. At `e = 1` the profile replaces it.
In between, a straight lerp on each knob. The table alone was raising
pitch on the consoling targets (`+0.18` to `+0.30` st). The blend pulls
that back down. `plan` by itself is still the table.

| Knob | At e = 1 |
|---|---|
| pitch | −0.5 st |
| pitch range | 0.88 |
| rate | 0.92 |
| loudness (`energy`) | 0.80 |
| warmth | 0.75 |
| pre-pause | 250 ms |

Colour at `e = 1` is heather, `oklch(0.72 0.09 310)` (`#B695CF`).
At `e = 0` the carried colour is the neutral mark,
`oklch(0.971 0.003 286)` (`#F5F5F7`). In between, OKLab. Hue 310 is a
design choice: lighter than grief, darker than calm, inside the violet
family of the wash. Heather is a value on the decision. It is not paint
on the circle.

A later spectrum extractor may nudge lightness and chroma of the current
hue. It must not move hue. Nothing in this spec computes a spectrum.

### The same e, on the interface

`/dot` carries `e` (three decimals, clamped). The page eases a shown copy
toward that target (the empathy branch uses a ~900 ms ease) so the room
does not tick on the 80 ms grid.

Background wash only. Locked stops stay amber `#F5A524` and violet
`#7C3AED` on ground `#0A0A0C`. As shown `e` rises, the ramp is pulled
toward a half mix (`EMPATHY_WASH_AMBER = 0.5`: `0` violet, `1` amber) and
saturation falls (`EMPATHY_DESAT = 0.55` on the empathy page). Heather is
that influence. It is not a third gradient stop, and it is not a fill.

Orb spring feel is **Luti** (PR #4 merged the integrator; PR #12, open,
names the gain table). Radius and brightness targets stay `DotParams`.
G4 is not retuned. What Samantha adds, as a design lean and not as a
landed constant, is the *feel* of whichever Luti motion is active:

- `e = 0`: the named table, untouched (`idle` 2.4 / 0.86, `listening`
  11 / 0.70, `thinking` 6.5 / 0.48, `speaking` 18 / 0.58, `offline`
  3.2 / 1.05).
- `e = 1`: angular frequency `w` scales by 0.85, damping `z` blends
  toward 0.92, aspect squash eases halfway back to round. The mark
  answers a little slower and settles, the way the voice does.
- Speaking still follows the playback envelope. The spring is less
  snappy. It does not stop listening to the sound.

Interpolate with the **shown** `e`, on the display clock, never inside
`Heartbeat::step`. Thinking remains a motion. It is not a fourth
`dot_states` entry.

The circle fill stays `#F5F5F7`. Recolouring it needs a later owner edit
of the lock. This spec does not make that edit.

A latched fault suspends this wash. The field goes full-bleed amber →
violet and a `GV-` code is drawn in that gradient, clear of the circle.
Luti's table is not retuned for the fault, and the fill stays `#F5F5F7`.
Contract: `ERROR-STATE.md`. When the fault clears, this session's wash
and lean return.

## 3. What it builds on

Samantha V2 extends the V2 program. It does not replace it.

| Piece | Where | What Samantha uses |
|---|---|---|
| Process bus | PR #7, merged as `5ab3d38`. Types in `crates/galion-console/src/bus.rs`. Shapes in `IPC.md`. | `BusBody::Persona` is on that bus. `try_send` only. Cancel still drops a stale turn. An explicit pin (`turn_id` 0) stays. |
| Orb engine | PR #4, merged. `ORB.md`, `ANIMATION-ENGINE.md`. | One filled mark, display-rate spring, wash behind it. |
| Luti | PR #12, open, `cursor/one-ui-luti-engine-4351`. | The named gain table. Samantha leans it with `e`. She does not rename it. |
| Empathy | PR #13, open. `EMPATHY.md` on that branch. | `voice_color`. The session behaves as that sector specifies. |

Left where they are:

- **PhoneLane.** Same lane, same worker thread, same 24 kHz out. Empathy is
  a hook on the reply, not a second speech stack.
- **`v1-screen.html`.** The phone page remains the page. No React, no second
  surface, no caption of the persona name.
- **`LOCKS.toml`.** This session spec does not edit `[ui]`, `[consent]`, the
  80 ms frame, or the Piper sunset pin. t230u added a separate out-of-process
  Chatterbox pin. It does not change those locks and it does not clear a clip.
- **Piper path.** `en_US-ljspeech-medium`, out of process, warm worker.
  Prosody may later move rate, pitch, energy, or the noise flag. It does
  not change voice identity, and it does nothing useful while `e` is 0 and
  consent is deny. Flite stays the pack-missing fallback. No PCM on disk
  "for warmth".

`sense` and `adapt` still default deny. `PhoneLane` still passes neutral
until a session has actually granted `sense`. The screen demo's grant is
the demo's, not the caller's.

## 4. Mid-session personality switching

Owner note t225u. A session may change register while it is alive. The
call does not restart. The mark does not become a menu. The voice does
not become an impression of a film.

What switches is a persona id, plus the prosody and wash bias hung on
that id. The shared weight `e` still comes from `voice_color`. Persona
and `e` are two inputs to one output. Anger is still answered, never
worn.

### On the bus

PR #7's `BusMsg` is the envelope: `id`, `turn_id`, `t_ms` (heartbeat time,
not the wall clock), `cancel`, and one body. Receivers already drop
`turn_id < cancel` before work and again before publish. `turn_id == 0`
is kept. That rule is the barge-in story, and a persona switch does not
get a special exemption from it.

This spec adds one body. It is `BusBody::Persona` in `bus.rs`, on the bus
PR #7 merged (`5ab3d38`). It is not a second broker. A name that is not
`samantha` or `jarvis` is ignored. The turn worker reads the id onto the
reply. It does not draw it, and it does not restart the lane.

```text
BusBody::Persona {
  id:      "samantha" | "jarvis" | <future id>
  source:  explicit | empathy | intent
}
```

The lane is a single slot, the same shape as `Consent` and `OrbState`:
`try_send`, latest replaces previous, a full slot does not block.
Deciding the switch happens on the worker or the consent gate, next to
the code that already knows the utterance. `Heartbeat::step` does not
classify, does not allocate a persona, and does not call the bus. The
80 ms frame may *read* the latest slot when `/dot` is built, the way it
already reads a coalesced consent bit. Reading is not deciding.

Two stamps:

- An explicit pin uses `turn_id = 0`. A barge-in does not forget that
  the person asked. It sticks until they ask again, or the session ends.
- An empathy or intent switch uses the current `turn_id`. If that turn
  is cancelled, the switch is stale and dropped. The previous persona
  remains. You interrupted; the room does not change costume for a
  sentence that will not be spoken.

No session restart. No new `PhoneLane`. No killed Piper process. The
warm worker keeps the socket it has. The next clause uses the new
baseline. A clause already in the child's stdin is not rewritten; the
cancel token drops what has not been sent, which is the supersede rule
the lane already has.

Withdrawing `sense` stops the empathy trigger and forces `e` back to 0
on the next frame. It does not restart the call. Wiping RAM history
stays the consent lock's job.

### Who the ids are

The studio table is `VOICE_PERSONAS` in `galion-studio/voice-backend`
`update_personas.py`. That script is not in this repository. Samantha V2
binds the two ids the owner named from it, and leaves the slot open.
The binding is an id plus a prosody and wash map. It is not a reference
clip.

| Id | Register, as named | What this session may do | What it must not do |
|---|---|---|---|
| `samantha` | Warm, curious, *Her*-inspired. The conversational layer. Paralinguistic tags belong to this id, later. | Baseline a little warmer and a little more amber than neutral. Empathy at `e = 1` is exactly the sector profile. | Ingest a film clip. Call Chatterbox. Claim the Piper voice is that performer. |
| `jarvis` | Calm, witty, British command layer. Short lines. Tags off. | Baseline drier, a touch lower, more violet. Commands prefer this id. High `e` still quiets and slows him. He does not become Samantha, and he does not raise his voice to match an angry caller. | A British accent on the current `en_US-ljspeech-medium` checkpoint. That accent is the intended register, not a fact about the weights. Film audio stays on the hold in `VOICE-CLONING.md`. |
| *(slot)* | Unset. | A future id declares the same fields: baseline prosody, wash-amber at `e = 0`, and whether tags are allowed. Until an owner names it, the slot is ignored. | A silent third voice snuck in beside Piper. |

Rights hold from V2-S14 still covers both names. Sky is the comparison:
a voice withdrawn because it resembled a performance. Ids, knobs, and
wash bias are not that. Fetching a clip, conditioning a clone worker,
or a public demo in either film voice still waits on rights, consent,
licence, and a LEDGER line.

Baselines below are a design choice, clamped by `ProsodyControl::clamped`.
They are not in `EMPATHY_PROSODY`. At `e = 0` the persona baseline is the
voice. As `e` rises, lerp toward that persona's empathy target. Samantha's
target at `e = 1` is the sector profile, unmodified. Jarvis's target at
`e = 1` is the drier cousin in the same row: still lower, slower, quieter,
still a pause, never a mirror of anger.

| Knob | Samantha e = 0 | Samantha e = 1 | Jarvis e = 0 | Jarvis e = 1 |
|---|---|---|---|---|
| pitch | 0 st | −0.5 st | −0.3 st | −0.4 st |
| pitch range | 1.00 | 0.88 | 0.92 | 0.90 |
| rate | 1.00 | 0.92 | 0.96 | 0.94 |
| loudness | 1.00 | 0.80 | 0.92 | 0.88 |
| warmth | 0.62 | 0.75 | 0.42 | 0.55 |
| pre-pause | 0 ms | 250 ms | 80 ms | 120 ms |
| wash amber | 0.62 | 0.50 | 0.28 | 0.50 |

Wash amber lerps toward `EMPATHY_WASH_AMBER` (0.50) with `e`, for every
id. Heather's desaturation scales with `e` only. A persona does not get
its own circle colour.

Luti follows the same pair of inputs. Samantha at `e = 0` is the named
table. Jarvis at `e = 0` is a small command snap on the active motion,
`w × 1.08`, `z` unchanged. Both then lean softer as `e` rises: Samantha's
`w` scale goes from 1.00 to 0.85, Jarvis's from 1.08 to 0.90, and `z`
blends toward 0.92. Speaking still tracks the envelope. These scales are
a proposal. PR #12's table stays the named source.

The session opens as `samantha` unless the owner pins another default.
Both ids are registers of Galion. Neither is a second product, and the
canned line is still "I'm Galion".

### What is allowed to pull the switch

Priority, top wins. A lower trigger does not override a higher one that
is still in force.

1. **Explicit request.** The person asked, in words ("be Samantha",
   "Jarvis", "switch back"). This does not need `sense`. It is an
   instruction, like any other utterance, not a reading of their mood.
   Stamped `turn_id = 0`. Sticks.
2. **Empathy weight.** Only if `sense` is granted. Otherwise this trigger
   is off, and it would be off anyway, because no frame means `e = 0`.
   Proposed gate, owner-tunable: shown `e` stays at or above **0.65** for
   **8** heartbeat frames (640 ms) and the turn is not a command. That
   prefers Samantha's warmth. Release when `e` stays at or below **0.50**
   for the same 8 frames, so one loud frame cannot flap the room.
   Sustained high `e` from anger prefers that warmth. It does not select
   a louder, faster, or harsher id. One frame is never enough.
3. **Dialogue context / intent.** A command or a task prefers Jarvis.
   A human, wandering, or feeling-shaped turn prefers Samantha when no
   explicit pin says otherwise. Word intent may come from the reply path
   that already sees text (`CannedLlm`, later the student). Intent taken
   from prosody needs `sense`, same as any other feeling read. The
   interim ASR label stays a placeholder. A switch must not be described
   as recognition.

Default for emotion-driven switches is **deny**. No `sense`, no automatic
crossing of the threshold, no "we noticed you were upset". The explicit
request and a word-level command intent still work, because they do not
classify affect.

### How the mark shows it

Same one mark. No nameplate, no second orb, no caption of the id. The
page may store the id the way it stores `data-luti`: present, not drawn.

`/dot` carries `persona` (the id string) and `e`. The wash, the Luti
lean, and the voice knobs all read that pair. The circle fill stays
`#F5F5F7` until an owner unlocks `dot_color` in `LOCKS.toml`. This spec
does not.

A person should be able to hear the switch with their eyes closed, and
see it with the sound off, and those two stories should be the same
story. That is the whole section.

## 5. Open questions for the owner

1. **Circle hue.** Heather and the persona washes stay off the fill.
   Painting either one on the circle is an owner edit of `dot_color` and
   `gradient_scope`. Yes or no, later, in the lock, by you.
2. **Thinking.** The page already pulses while a turn is in flight.
   `dot_states` is still idle / listening / speaking. Does that motion
   become a locked state before a live demo, or stay a client habit?
3. **Consent without chrome.** `sense` stays deny. A grant has to be a
   gesture, not a settings page. What is the gesture, and how does
   withdraw stay obvious when the surface cannot grow a button?
4. **When empathy meets a live demo.** PR #13 is open. Until it merges,
   `/dot` on main has no `e`. After it merges, is a public demo allowed
   to show the wash moving, or only a test that grants `sense` on purpose?
   The default demo must not classify a stranger.
5. **Rights.** The Samantha and Jarvis names are registers under the
   V2-S14 hold. This spec does not clear a clip, a clone, a tag in a
   public demo, or a claim that the session speaks in those film voices.
   The hold stands until you lift it in writing. The boxes and the
   LEDGER paste templates are `RIGHTS-CLEARANCE.md`. Both gates are closed.
6. **Luti lean.** Are the `w` and `z` scales in §2 and §4 acceptable, or
   does the named table stay fixed and only the wash move?
7. **Piper warmth.** `GALION_A21_NOISE=natural` is still open (V2-S6).
   Warmth 0.75 must not become a disk cache or a second voice. Is the
   noise flag in or out of the demo?
8. **Switch gate.** Is 0.65 / 0.50 over 8 frames the right hysteresis, or
   do you want a different pair? Emotion-driven switching stays deny
   until you say a demo may grant `sense`.
9. **Default id.** The session opens as `samantha`. Should an unnamed
   Galion register sit under both, with Samantha and Jarvis only after
   an explicit ask?
10. **Crossed orders.** An explicit Samantha pin is in force, and the
    next sentence is a command. The landed rule is the priority already
    in this section: a lower trigger does not override a higher one, so
    the pin wins. A one-turn borrow would be a later owner change.
11. **Bus landing.** Done. `Persona` is on the merged bus. `/dot` does
    not yet print the id. The page read is still open.
12. **The empty slot.** Who names the third id, and does it need its own
    rights note before it is allowed to speak?

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
