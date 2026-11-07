# Persona

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

A persona is behaviour, emotion, and sound together. The clone supplies a body. The feeling band supplies the person, on the same 80 ms clock as the voice.

| Voice | Status | File |
|---|---|---|
| Galion One | Product voice | [GALION-ONE.md](../personas/GALION-ONE.md) |
| Galion Research | Product voice for the research console | [GALION-RESEARCH.md](../personas/GALION-RESEARCH.md) |
| Samantha | Test / internal | [SAMANTHA.md](../personas/SAMANTHA.md), [SAMANTHA-EMOTIONS.md](SAMANTHA-EMOTIONS.md) |
| Jarvis | Test / internal | [JARVIS.md](../personas/JARVIS.md), [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md) |

The index of those instruction files is [personas/README.md](../personas/README.md). Session mechanics and the knob tables are [SAMANTHA-V2.md](../SAMANTHA-V2.md). The bus body is [IPC.md](../IPC.md). Draft ids the screen can read are [personas/voice/README.md](../personas/voice/README.md).

Empathy is the live OKLCH weight `e` and the prosody knobs in [EMPATHY.md](../EMPATHY.md). The words stay on the matter. The feeling is in the pace, the pitch, the loudness, and the wash behind the circle. `galion-emotion` recomputes that weight from the caller frame. A high `e` quiets the voice. It does not insert a comfort script.

Situational feeling is the live weight on the feeling band. That weight is `e`, recomputed inside `Heartbeat::step` from `galion-emotion` (`empathy_weight` on `Feeling.e` and `StubExpressor::voice_color`). Consent starts denied. `DefaultConsentGate` begins at `ConsentState::Unknown`, and `check` returns `DenyNoConsent`. The `sense` bit starts off, so the first frame has no caller and `e` is 0.

Caller emotion does not steer the phone lane unless `sense` is granted. `PhoneLane` passes `Emotion::Neutral` on `reply_text`, `inject_text`, and `turn`. Callers may run `ProsodyEmotion` only when `sense` is granted. That heuristic is unwired on the phone lane. An accuracy for the read, or a claim that a granted `sense` already changed the spoken phone reply, is unmeasured and stays **BLOCKED**. Reference wavs stay out of git. This file does not add one.

## Gate

**PASS** when each persona file exists, the emotion column for that id is the one the feeling band applies, and a spoken turn carries prosody and wash with the words left on the matter.

**FAIL** when a test voice is presented as the product, or when a reply uses a stock comfort line in place of the weight.

**BLOCKED** when the feeling band has no caller frame (consent starts denied, so `e` is 0) and the row is being scored as if a live situational feeling were present. Also **BLOCKED** when the score names a phone-lane accuracy or says the lane followed the caller. That read is unmeasured.

How the screen selects a persona: [ONE-DOT.md](ONE-DOT.md). Record the gate in [SELF-EVAL.md](SELF-EVAL.md).

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
