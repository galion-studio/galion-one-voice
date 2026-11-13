# Emotion V2 — prompt lines for Samantha and Jarvis

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Lines the owner can speak during the controlled capture.
They are original. They are not film lines. They are not a reply table.

The protocol still wins: the owner may replace any suggested sentence with
their own words, and **the same carrier is used for every official emotion
on that line** so a later score compares the emotion. Write the sentence you
actually used on the session card.

A machine-readable copy is [prompts.json](prompts.json).

Forbidden comfort phrases from the persona floor are not used here (`I hear
you`, `I'm sorry for your loss`, `take a deep breath`, and the rest of that
list in `emotion-v2/taxonomy.py`).

## Shared carrier (use this for the five official takes)

| Line | Suggested carrier |
|---|---|
| Samantha | The note is on the table. I will read it at four. |
| Jarvis | The report is on the desk. I will open it at four. |

Speak it once in Neutral as a level check, then keep the wording for Calm,
Happy, Amused, Excited, Tender, Sad, Disappointed, Angry, Fearful, Surprised,
and Disgusted. Only the delivery changes.

## Delivery notes, by sector

These are performance notes, not words to read aloud.

### Grounded — Neutral, Calm

- **Neutral (both):** even pace, no smile in the voice, no weight, no extra
  air. This is the rest reference, not a flat computer.
- **Calm (Samantha):** unhurried, slightly lower energy, still present.
- **Calm (Jarvis):** measured, quiet authority, no softness that turns into
  Tender.

### Bright — Happy, Amused, Excited, Tender

- **Happy (Samantha):** warmth you can hear, not a laugh and not a shout.
- **Happy (Jarvis):** clear satisfaction, still precise. No giggle.
- **Amused (Samantha):** a short private smile in the voice. Not a performed
  laugh track.
- **Amused (Jarvis):** dry recognition that something landed. Not teasing.
- **Excited (Samantha):** forward pace and lift, still on the mark, no clip.
- **Excited (Jarvis):** urgency without shout. The news is good; the diction
  stays clean.
- **Tender (Samantha):** close and low, not breathy enough to hide the words.
- **Tender (Jarvis):** care without intimacy. Stay formal. Do not lean into
  the optional Samantha slot.

### Heavy — Sad, Disappointed

- **Sad (both):** slower, heavier, no sob and no whisper that kills the level.
- **Disappointed (Samantha):** the air goes out of the line. Not anger.
- **Disappointed (Jarvis):** clipped expectation that was not met. Not a
  reprimand (that is Angry).

### Sharp — Angry, Fearful, Surprised, Disgusted

- **Angry (Samantha):** firm and held. Not a scream. The policy of the live
  session still answers a caller's anger rather than wearing it; this take is
  only a scorer reference.
- **Angry (Jarvis):** controlled steel. Consonants stay clean. No shout that
  clips.
- **Fearful (both):** tighter and smaller, not a theatrical gasp.
- **Surprised (both):** the turn happens on the first stress. Recover enough
  that the rest of the sentence is still audible.
- **Disgusted (Samantha):** withdraw, don't perform a cartoon ugh.
- **Disgusted (Jarvis):** rejection of the fact, still articulated.

## Optional per-emotion color lines

These are the three fixed taxonomy carriers from
[`../TAXONOMY.md`](../TAXONOMY.md) §7. They are for warm-up or a later
full-intensity color pass. They are **not** a substitute for the shared
carrier on the five official takes, because changing the sentence between
emotions confuses the score.

The wording is the same for both lines. Only delivery changes. Jarvis keeps
diction tighter and does not add intimacy on Tender.

| Emotion | Sector | Lines |
|---|---|---|
| Neutral | Grounded | "The notes are in the folder on the desk." / "Thursday at four still works." / "I can read that back if you want." |
| Calm | Grounded | "We can leave it there for now." / "Nothing else has to move today." / "I'll wait while you find the page." |
| Happy | Bright | "They kept the Thursday slot for you." / "That came out right." / "The room went quiet, and then they clapped." |
| Amused | Bright | "You did not just name the printer." / "That one landed." / "I keep hearing the way you said spreadsheet." |
| Excited | Bright | "It is Thursday, and they said yes." / "The build finished. All of it." / "Tell me the part where it actually worked." |
| Tender | Bright | "You do not have to explain the whole thing." / "I'm glad you told me that piece." / "That part matters. We can stay with the fact." |
| Sad | Heavy | "Your mother died this morning." / "The house is going to feel different tonight." / "I don't have a better sentence for that." |
| Disappointed | Heavy | "I thought we were going to finish it today." / "That is not the version I was hoping you would hear." / "I wanted that to go the other way." |
| Angry | Sharp | "That was not yours to decide." / "Say it again, plainly, without the excuse." / "I am not raising my voice. I am not letting it pass." |
| Fearful | Sharp | "Something in the hallway just changed, and I don't know what." / "If that alarm is real, we stop and check." / "I need a second. That did not sound right." |
| Surprised | Sharp | "Wait. That is not the number I had." / "They moved it to Thursday. I did not see that coming." / "You finished it already?" |
| Disgusted | Sharp | "That smell does not belong in a kitchen." / "Leave the rest of it. I don't want it described." / "We are not putting that back on the table." |

Sad's first taxonomy line is the grief carrier already used by the persona
floor. It is heavy. You do not have to use it in this session. The shared
carrier above is enough for the official takes.

## Optional Samantha-only slot: seductive

Not one of the twelve. Not required for a sealed session. Not recorded for
Jarvis. If Mate consents to include it, record it after the official twelve,
same room, microphone, gain, and 30 cm, still the owner's own voice.

Keep the wording adult and specific, not explicit, and not a film line.
Suggested carrier, Samantha only:

> Stay a moment. I wanted you to hear this from me.

Delivery: closer and lower than Tender, still on the mark, still intelligible,
no whisper that fails the noise or loudness gate. Five takes, same as the
others, if you record it at all.

Do not write a Jarvis prompt for this slot.
