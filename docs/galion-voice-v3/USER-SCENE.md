# V3-A · User-scene, beside Jarvis texture

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. A planning note. It specifies a boundary.
It does not specify a separator, a model, or a build.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

**Docs only — no implementation.**

No V3 code until V2 is fully live with Samantha + Jarvis clones, memories,
emotions, gradients/color-in-voice and colorful space working.
See [STATUS.md](STATUS.md).

## The split

Two things are easy to mash into one recording. V3 keeps them apart.

**Jarvis texture** is the speaking voice: the acoustic identity of the
Jarvis register, the clone when a cleared clone exists, and the prosody
and wash bias `SAMANTHA-V2.md` §4 already hangs on the id `jarvis`.
Texture is the **me** band's character. It is calm, short, and dry.
Tags stay off. It does not contain the room the caller is sitting in.

**User-scene** is the caller's space, heard as context and kept off that
texture. The owner names three classes:

| Class | What it is | What it is not |
|---|---|---|
| Background noise | The room: a fan, a street, a kettle, a bed of sound with no speaker in it. | A voice to answer, and not a reference clip. |
| Television | A set or a stream playing in the room. Speech may be in it. It is still not the caller. | A persona. A film clip to ingest. A line to quote back. |
| Other people | Someone besides the caller, present in the same space. | A second user. A clone. A transcript to keep. |

The same split holds when the speaking id is `samantha` or Galion One.
Jarvis is the named case because that texture is the one a room most
easily ruins: a television's drama is not his dryness, and another
person's pace is not his.

The you band still carries the caller's words. The scene layer is a
description beside that band. It is not a second product and not a
second dot.

## What the session may do with a scene fact

Planning only. None of this is wired.

- Know that the room is noisy, and keep the clone's reference the cleared clip, not the live mix.
- Know that a television is on, and refuse to treat the set as the person who tapped the dot.
- Know that other people are present, and avoid answering them as if they had started the call.
- Let the mark settle. A loud room is not a reason to grow a waveform. Size and brightness stay the only varies on the dot. The wash stays on the background.

The words stay on the caller's matter. The scene is not narrated
("I hear a television"). `chrome_allowed = false`. A mood report is
already forbidden in the persona files. A room report is the same kind
of mistake.

## What the session must not do

- Mix the scene into Jarvis texture, into Samantha's texture, or into the Galion One voice. A later compile ([SYMPHONY.md](SYMPHONY.md)) takes a cleared reference, not the live room.
- Clone anyone who did not consent. Other people are a class, not a voice id.
- Store the scene. V2 retention stands: no raw audio on disk by default, no transcript log, turn audio gone when the turn is gone.
- Identify a bystander. No name, no face, no "who else is there."
- Run the classification inside `Heartbeat::step`. The tick may read a coalesced slot. Reading is not classifying. The 60 ms budget is unchanged.
- Turn the layer into a recorder of a household. Default is deny. Workplace and education purposes stay prohibited even if someone tries to consent "for the meeting" or "for the class."

## Consent

Hearing a room well enough to name "television" or "other people" is a
read of the caller's space. It sits next to `sense`. The default for
`sense` is deny, and this plan does not grant a new bit.

Open question, for Maciej: is scene consent the existing `sense` bit, or
a later lock key you would add on purpose? Until you answer, the planned
default is deny, and a session with `sense` denied carries no scene class.
Noise that never becomes a class can still be a level, the way `/mic`
RMS is already a level. A level is not a label.

Withdrawal wipes RAM history, including any scene slot from this call.

## On the bus, when a bus body is allowed

V2-S1 already forbids a new broker crate. A future scene fact would be
one more body beside `Consent` and `OrbState`: a single slot, latest
replaces previous, `try_send`, a full slot does not block the heartbeat.

The slot, as a sketch and not as a type to land:

```text
scene {
  class:   noise | television | other_people | quiet
  confidence: 0..=1
}
```

`quiet` is the resting class. One class at a time. No list of people.
No quote from the television. A name that is not one of those four is
ignored, the way a persona id that is not `samantha` or `jarvis` is
ignored today.

This sketch is not a patch. It lands, if it ever lands, only after the
gate in [STATUS.md](STATUS.md) is witnessed, and only if you have
answered the consent question.

## Acceptance, later

These are the checks [VERIFICATION.md](VERIFICATION.md) will ask a person
to perform in a real room. They are not claims about today.

1. A television playing speech in the room does not change Jarvis's voice identity on the next reply.
2. A second person speaking nearby is not answered, is not cloned, and is not written into a transcript file.
3. The one mark stays one mark. No caption names the scene class.
4. With consent denied, the session does not label the room.
5. `Heartbeat::step` does not grow a scene classifier. The 80 ms frame still meets its budget.
6. No new WAV appears under the repo because a scene was noticed.

## Risks

- A clever separator that saves the television as a clip. That is an ingest. Ingest stays closed (`../galion-voice-v2/CLIP-INGEST.md`).
- Calling the mic RMS "the scene layer" and stopping. A level is not a class. The class is the part that waits on consent and on the gate.
- A scene graph, a shader, or a second shape. `../galion-voice-v2/ORB.md` already says the orb is not a scene graph and not V3. This note does not make it one. "Scene" here is the caller's acoustic space.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
