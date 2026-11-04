# V3 · Real-life verification

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. A checklist for a person in a room.
Every box below is open. A desk reading does not close one.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

**Docs only — no implementation.**

No V3 code until the Part A lines are witnessed in [LEDGER.md](LEDGER.md).
The gate sentence is in [STATUS.md](STATUS.md).

## How to use this list

- A box is ticked only after a person has heard or seen the thing on a real machine, and the same day a ledger line is pasted from [LEDGER.md](LEDGER.md).
- The line names the date, the host in plain words, who listened, and the backend that actually spoke (`piper`, `flite`, or a clone worker that was really in the path). It does not invent a duration, a hash, a word-error rate, or a score.
- Flite labelled as flite is an honest negative for any clone box. Piper labelled as Piper is an honest negative for any clone box.
- A headless frame, a unit test, and the 2026-11-03 persona eval are evidence of what the tree can score. They are not a tick on Part A.
- Rights: a clone box stays open while `../galion-voice-v2/RIGHTS-CLEARANCE.md` is unfilled. Fetching `clone-weights-v1` does not fill it.

Paste templates live at the bottom of [LEDGER.md](LEDGER.md). Do not
tick a box in git and leave the ledger empty. The ledger is the record.
This file's boxes are the index.

## Part A — V2 live (blocks all V3 code)

All six. Then, and only then, V3 code may be proposed.

### A1 · Samantha clone

- [ ] Rights, consent, and licence for Samantha are cleared, and the private ledger line from `RIGHTS-CLEARANCE.md` is pasted in the root `LEDGER.md`, with no field left as a placeholder.
- [ ] A person hears a reply in the Samantha register from the local clone worker, on this machine, self-offline. The log names the clone backend. Piper is not the voice they are scoring.
- [ ] The reference clip is the cleared one. It is not in git. The live room was not the reference.
- [ ] Tags, if any were spoken, are the ones her config allows, and only on that worker.
- [ ] Art. 50 and the imitation sentence are present if anyone other than the owner heard it. A public demo has its own ledger line. An internal hearing says it was internal.

### A2 · Jarvis clone

- [ ] The same rights line exists for Jarvis.
- [ ] A person hears a reply in the Jarvis register from the local clone worker. Tags are absent. The line is short. The backend is the clone, not Piper and not flite.
- [ ] Jarvis texture is stable across two replies: the second does not pick up the room, a television, or another person.
- [ ] He does not claim to be the film voice. The hearing notes that he did not.

### A3 · Memories

- [ ] Inside one live call, Galion can use something the caller established earlier in that same call, and the caller can hear that it did.
- [ ] The memory is RAM. No new file appears under the repo or under a home directory because the call remembered.
- [ ] Withdrawing consent wipes it. The next turn does not recall the wiped fact. `withdraw_wipes_ram_history` is still true. The lock file was not edited to make this pass.
- [ ] The persona does not recite a canned empathy line in order to prove it remembers.

### A4 · Emotions

- [ ] With `sense` denied, a loud or upset caller leaves `e` at 0. The reply stays neutral-warm. The wash does not swing.
- [ ] With `sense` granted on purpose, for a test the owner opened, the same `e` moves pitch, rate, loudness, warmth, and the pre-pause, and the wash moves with that same number.
- [ ] Anger is answered and not copied. The voice does not get louder to match the caller.
- [ ] A sympathy script is absent. The words stay on the matter.

### A5 · Gradients / color-in-voice

- [ ] A person can hear the colour decision: the knobs from `voice_color`, not a second voice identity.
- [ ] At rest the carried colour is the neutral mark `#F5F5F7`. At a full `e` the decision is heather `#B695CF`, with OKLab between, as `EMPATHY.md` specifies.
- [ ] The circle fill on the real surface stays `#F5F5F7`. Heather is not paint on the fill unless a later lock edit says so, and this checklist does not make that edit.
- [ ] The spoken voice and the wash come from one `e`. They do not tell two stories.

### A6 · Colorful space

- [ ] On the real surface (the phone page or the window the owner names), the ground is the living amber `#F5A524` → violet `#7C3AED` field, not a flat `#0A0A0C` with the wash missing.
- [ ] The field moves with mood and with `e`, continuously, across the time a person is looking. A single headless PNG does not close this box.
- [ ] The mark stays one circle. The wash never tints the fill.
- [ ] A latched fault, if one is raised on purpose, replaces that field with the full-bleed amber → violet pair and one `GV-` code, and clearing the fault brings this field back. `ERROR-STATE.md` is the contract. If the fault path is still undrawn, this sub-box stays open and the rest of A6 can still be witnessed on the healthy path. Say which.

## Part B — V3 scene and symphony (blocked until Part A)

Leave these open. They describe the hearing that would follow a build.
They are not a request to build.

### B1 · User-scene beside Jarvis texture

- [ ] Television speech in the room does not change the Jarvis clone's identity on the next reply.
- [ ] Another person nearby is not answered, not cloned, and not written to a transcript.
- [ ] Background noise does not become a reference clip.
- [ ] With consent denied, the session does not name a scene class.
- [ ] The mark stays one mark, with no scene caption.

### B2 · Symphony extensions

- [ ] The surface is still one dot after the extension. G4 is unchanged, or an owner re-bless is written down on purpose.
- [ ] A heavy job in flight (compile, scene class, video decode) leaves `Heartbeat::step` inside the 60 ms budget. The measured number is in the ledger.
- [ ] With the compiler absent, speech is Piper, else flite.
- [ ] A compiled artifact, if one was heard, was produced from a cleared reference and not from the live room. The artifact is not in git.
- [ ] No second HTTP server and no second tunnel showed up to carry the extension.

## Part C — Admin backlog (policy, not a runtime pass)

These close only when the decision they depend on is actually taken.
Recording the backlog does not close them. See [ADMIN-BACKLOG.md](ADMIN-BACKLOG.md).

- [ ] **C1 Video mode bar.** Image-only still matches V2 on the user surface. Video mode, if drawn, is V3, default off, off the heartbeat, and not a camera bus. The chrome question is answered in the ledger before any bar is drawn on the user surface.
- [ ] **C2 External API.** The billing shape, if an API exists, is per job, pay-as-you-go. There is no flat subscription for the API. No price is published from this folder. `[licence]` has been edited by the owner if a paid offer exists, with counsel and a ledger line.
- [ ] **C3 GPU island.** Heavy jobs can name one server of about four B200s. A default Galion One, Galion Voice, or Hermes turn completes on CPU with that island unreachable. The phone turn does not wait on it.
- [ ] **C4 Galion OS desktop.** Hermes Research and the console have a named place on a Galion OS Linux desktop. The one dot is still the voice surface. The console is still not the user UI, unless the owner has edited `console_is_user_ui` and said so.
- [ ] **C5 Voice under Galion One.** Voice-to-voice is exclusive to paid Plus and Ultra. Local, free forever, has no voice-to-voice. This box stays open while `paid_tiers = false`. Closing it requires the owner edit, counsel, and a ledger line. Until then the product people can run is the one the lock describes.

## Part D — What not to write down

- A tick with no ledger line.
- A hash you did not compute from a file you have.
- A word-error rate, a real-time factor, or a cold-start millisecond this hearing did not measure.
- "The model recognised the caller" while the ASR path is still the one the V2 plan calls unratified, or while `GALION_ASR` is anything other than the engine the owner chose (Vosk, letter A) and the model is actually on disk.
- A claim that the VPS is serving the demo. Plan V5's human steps are still the deploy. This checklist does not SSH.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
