# Galion Voice V3 — symphony plan

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03.

**Status. PLAN ONLY. Gate closed.** This file authorises no crate, no worker,
no format byte, no fetch, and no edit of `LOCKS.toml`. V3 implementation
begins only after the real-life gate in §4 is entirely true. Until then the
program stays on Galion Voice V2.

Master slogan: "The seamless connection with the human being, on the voice only."

## Where this file lives

| Copy | Path | Who writes it |
|---|---|---|
| Canonical | `docs/galion-voice-v2/symphony/V3-PLAN.md` in `galion-studio/galion-one-voice` | This repository. This is the file a PR reviews. |
| Local mirror | `/workspace/galion-symphony-v2/V3-PLAN.md` | The coordinator, on the machine that owns `/workspace`. |

The two copies are the same bytes. A cloud agent cannot create
`/workspace/galion-symphony-v2/` from this repository. After this file
lands, the coordinator copies the canonical file to the mirror path.
The repository stays the source of truth. A mirror that drifts is replaced
from this path. It is not edited in place.

## 1. What has to be running before this plan may be built

V2 is the symphony. V3 is the next layer on that symphony, and only after
the symphony is programmed, tested, and running in real life with working
personalities and AI. The briefs this plan follows are the 2026-11-03
symphony notes: the stack brief (one compiler, clone landing, Jarvis
texture, persona, one dot, 80 ms), the CPU foundation (CPU-first, no GPU
assumption, own format, ≤1% non-inference on the tick), and Eureka applied
(one heartbeat, three bands, the heartbeat tests itself).

What that stack already is, in this tree:

| Piece | Where it lives | V2 duty this plan inherits |
|---|---|---|
| One dot | `crates/galion-console/assets/v1-screen.html`, `ONE-UI.md`, `ORB.md` | One mark. Speak, auto-record, the workflow proceeds. No second AI UI. |
| 80 ms heartbeat | `crates/galion-heartbeat`, `LOCKS.toml` `[frame] frame_ms = 80` | One `Heartbeat::step` per frame. You band, me band, feeling band. Never blocks, never allocates. Late frame is a pause. |
| Rust infrastructure | `galion-heartbeat`, `galion-emotion`, `galion-presence`, `galion-console` (bus, phone, turn worker), `galion-render`, `galion-cascade`, `galion-tensor` | The conversation runs on this stack. The turn worker stays off `Heartbeat::step`. |
| Compiler | V2 symphony section A. Output is Galion's own voice file format and path (`.gq` / Galion clip ABI) that the Rust stack runs. | Chatterbox is the out-of-process clone source. Chatterbox format is not the runtime the symphony keeps. |
| Clone landing | `VOICE-CLONING.md`, `CLONE-WORKER.md`, `personas/voice/{samantha,jarvis}.toml` | Current Samantha and Jarvis models. Nano is the CPU primary. Turbo is the quality option. Piper, then flite, remain the fallbacks. Models are not swapped. |
| Jarvis texture | V2 symphony section C, Jarvis reference | Hub clicks, alarms, and ambience stay in the character. They are not stripped. |
| Persona and emotion | `personas/SAMANTHA.md`, `personas/JARVIS.md`, `EMPATHY.md`, `SAMANTHA-V2.md` | Personality and feeling are in the voice. `e` is a live weight. No canned empathy line. |
| Colour | `EMPATHY.md`, `SAMANTHA-V2.md` §2 | The same `e` is OKLCH on the voice decision and the amber → violet wash behind the one dot. |
| CPU | CPU foundation note, t204u / t205u in `LEDGER.md` | CPU only. Chatterbox stays out of process. No GGUF in Voice crates. `paid_tiers` stays false. |
| Self-eval | Eureka sections A–G | A section is complete only when its gate PASSes and the result is written in `docs/galion-voice-v2/symphony/SELF-EVAL.md`. |

`docs/galion-voice-v2/GALION-VOICE-V2.md` §1 already uses the name V3 for a
later production bar (ratified EN ASR, a reply model past the canned table,
warm speech, the VPS edge, gates R0–R3, counsel signed). This file does not
rewrite that row. Symphony V3 does not open that bar. Nothing in §1 starts
because this plan exists.

Rights stay where the tree puts them. `LEDGER.md` still records that neither
Samantha nor Jarvis clearance box is ticked, and
`[formats.exceptions.v2s14_chatterbox]` keeps `reference_clips = false`.
The symphony brief's t273 line is a V2 instruction to land the clones. It
is not a lock edit, and this plan does not perform one. Clone-live in §4
is a fact about a running voice, under whatever rights record that run
actually enforces.

## 2. Two layers

The room a person is sitting in and the character of Jarvis are different
signals. V3 exists to keep them that way. V2 already decided that the hub
texture stays. V3 adds the caller's non-clean audio as its own layer beside
that texture. The two are never one knob.

### 2.1 Jarvis texture layer

This layer is character. It belongs to the Jarvis reference and to the
Galion-format asset the compiler emits for the id `jarvis`.

What it holds: the Tony Stark hub virtual texture in the Jarvis reference.
Clicks. Alarms. Ambience. That bed is part of how Jarvis sounds. V2 section
C forbids stripping it. A spectral or energy check against a clean-speech-only
baseline has to show the texture band still present. That check is a V2 gate.
V3 must not weaken it.

Where it plays: the me band, inside the compiled voice, while the id is
`jarvis`.

What it is made of: the reference WAV, compiled. It is an authored asset.
It is not a microphone.

Samantha does not wear this layer. Her reference is her own character asset.
Her paralinguistic tags stay hers (`SAMANTHA-V2.md`). The hub bed is not
copied onto her to make the two clones "match".

### 2.2 User-room layer

This layer is the human's space on the way in. It is non-clean user audio.

What it holds:

- Background noise in the room.
- A television, or any other playback device, sounding while the person talks.
- Other people in the chat. A second voice in the same room, or on the same
  open microphone, who is not the persona.

Where it lives: on the you band, as its own label beside the caller's
speech. Eureka's champion stays: a conversation is one heartbeat, and the
heartbeat has three bands (you, me, feeling). The room is a slice of the
you band. It is not a fourth band, and it is not a second clock.

What it is made of: the live microphone, for the length of the frame, in
RAM. `SECURITY-EDGE.md` already says utterance bytes are not written to
disk. The room layer uses that rule. A television is not ingested as a
reference clip. Another person in the room is not enrolled, not cloned,
and not given a persona id.

### 2.3 How they sit together

```text
you band (inbound, one 80 ms frame)
  caller speech
  user-room layer          background, TV, other people
       │
       │   off the hot path: label only, fixed size, try_send
       ▼
heartbeat step             you · me · feeling
  feeling band             e, OKLCH, prosody, wash
  me band                  compiled Galion voice
       └── jarvis texture  clicks, alarms, ambience
                            (authored asset, id = jarvis)
```

The feeling band is the third thing in the frame. It is neither room mess
nor hub texture. Colour in the voice and colour in the space come from `e`.
A loud television does not raise `e`. Another person laughing does not
become the caller's joy. Other talkers do not select `samantha` or `jarvis`.

Rules for the pair:

1. Jarvis texture is compiled into the Galion-format asset and plays on the
   me band when the id is `jarvis`.
2. User-room audio stays on the you band under its own layer label.
3. The compiler's inputs are the clone and the character reference. User
   audio, television, and bystander speech never enter the compiler and
   never land in the Galion voice file.
4. A microphone cleanup decision applies to the user-room layer only. It
   does not rewrite the compiled Jarvis asset, and it does not remove hub
   clicks, alarms, or ambience.
5. The choice to keep character texture applies to the Jarvis asset only.
   Television bleed and room noise stay out of that asset. They are not
   "kept because texture is character."
6. `Heartbeat::step` does not denoise, diarise, or classify a television.
   That work, when a later change is allowed to build it, sits off the
   tick, the way the turn worker already does. The step still never blocks
   and never allocates. A late frame is still a pause.
7. Another person, and a television, are room. They are not, by themselves,
   a barge-in and not an instruction. Barge-in stays the primary speaker's
   onset on the control path `IPC.md` already names. The 80 ms tick stays
   off the cancel path.
8. Withdrawal still wipes RAM history on the next frame. The room layer
   keeps no private copy of bystanders.

The conflation this plan is written to prevent is one switch that both
"cleans the user" and "cleans Jarvis", or one bed that mixes the hub into
the television. Those are the same mistake in opposite directions.

## 3. Extension points

Each row is a hook on the V2 stack. None of them is a build. No type in
this section is added to a crate by this change.

### 3.1 One-dot interface

The page stays `v1-screen.html`. One filled circle, `#F5F5F7`, size and
brightness only. Ground `#0A0A0C`. Wash amber `#F5A524` → violet `#7C3AED`
on the background only. `chrome_allowed` stays false. `gradient_scope`
stays `background`. Luti stays the spring. Thinking stays a motion, not
a fourth locked state.

The listening radius already follows microphone energy. That energy
includes the room. A later room layer may tell speech energy from room
energy only by a value the page already knows how to read: `/dot`, or one
bus body, consumed as the listening drive already consumes `level`. The
mark stays one mark.

The colorful space is that wash, driven by the same `e` as the voice
(`SAMANTHA-V2.md`: half-mix `EMPATHY_WASH_AMBER = 0.5`, desaturation
`EMPATHY_DESAT = 0.55`, heather as an influence, not a third stop). Room
noise does not recolor the field. A fault still uses `ERROR-STATE.md`:
full-bleed wash, one `GV-` code, fill unchanged.

V3 adds no meter, no second orb, no waveform, no "room" panel, and no
caption of the layer name. Recolouring the circle fill needs an owner edit
of `LOCKS.toml`. This plan does not make that edit.

### 3.2 80 ms heartbeat

`FRAME_MS` is 80. `SAMPLE_RATE` is 24 kHz. `FRAME_SAMPLES` is 1920.
`FrameIn.you_pcm` remains the you band. `FrameOut` remains the me-band
onset, the pause bit, and the feeling decision applied next frame.

The extension point is a fixed-size room descriptor computed off the tick
and handed in beside `you_pcm`, the way `affect` is already an optional
inbound frame. It names speech level, room level, and whether other talk
is present. It is borrowed or copied into the struct the step already
owns. The step does not allocate it.

The non-inference budget from the CPU foundation still applies: tick work
stays inside the 60 ms compute budget, p99 well under 800 µs, and
non-inference engine work stays at or under 1% of the host. A room
classifier that misses the frame is a failed design. Late stays pause.
Nothing catches up on the next frame.

Display stays on its own clock (the panel's `dt`, Luti in `OrbEngine` and
on the page). The heartbeat does not speed up for a 240 Hz panel. The room
label does not resample the spring onto 80 ms.

### 3.3 Rust infrastructure

No new crate. The future room label, when the gate is open, is a body
beside the ones in `IPC.md` (`MicLevel`, `Utterance`, `Consent`,
`Persona`). Same envelope. `try_send` only. A full queue coalesces or
drops. It does not block `Heartbeat::step`. It is not a broker.

`PhoneLane`, the turn worker, and `ReplyBox` stay the speech path. Room
labeling does not become a second TTS and does not open a socket. Cancel
still drops a stale turn before publish. `BusBody::Persona` still carries
`samantha` or `jarvis` only. The room layer does not grow a third id.

CPU-first stands. Chatterbox Nano stays out of process on CPU. Any future
room analysis is CPU, and either off the tick or inside the 1% budget if
it must share the frame. The compiler's output stays runnable on the
CPU hosts the foundation names. GGUF stays out of Voice crates. `.gq` /
PCM remains the symphony path. `paid_tiers` stays false. Plus and Ultra
figures in the CPU note are infra notes for the owner. They are not a
product change in this plan.

### 3.4 Compiler — Chatterbox into Galion's own format

V2 section A deletes keeping Chatterbox format as the runtime. The worker
may emit clone PCM out of process. The asset the Rust stack loads is
Galion's own file format and path. A round-trip fixture has to refuse a
tree where raw Chatterbox output is the primary path. That fixture is a
V2 gate. This plan does not write the format.

Jarvis texture is an input to that compile and has to survive it. The
energy-band check in section C is the proof.

The V3 extension point on the compiler is a marker, in that same Galion
format, that the texture layer is present and is the character layer.
Downstream code can then show that a dirty you band did not replace the
hub bed. This plan names the marker. It does not define magic numbers,
chunk layout, or a parser.

The compiler grows no input slot for user audio, television, or a
bystander. Samantha's asset and Jarvis's asset stay separate compiles.
The hub texture is compiled into Jarvis only.

Watermark and Art. 50 stay as `VOICE-CLONING.md` and `CLIP-INGEST.md`
already state them. Perth is not vendored by this plan.

### 3.5 Feeling, memory, and the room

Emotion stays `StubExpressor::voice_color` in `galion-emotion`: one weight
`e`, prosody knobs, OKLCH. The 3-bit `Affect` field stays full. Empathy is
not a new label. Words stay on the matter. The feeling is how they are said.

Session memory stays the V2 behaviour the personas already require. Inside
one live call, a fact the person stated can be answered on a later turn
(the recall path around the stored fact). Samantha answers as Samantha.
Jarvis answers as Jarvis. Withdrawal wipes RAM history on the next frame,
and the following turn does not recall it. There is no transcript log and
no audio on disk. Yesterday's call is not in anyone's pocket
(`personas/GALION-ONE.md`).

The room layer does not write memories. Another person's sentence is not
a stored fact. A television line is not a stored fact. V3 does not extend
recall into the room.

### 3.6 What a later self-eval would have to prove

When, and only when, §4 has passed and an owner has allowed an
implementation, that implementation is complete only when its own gate
PASSes and the line is written in
`docs/galion-voice-v2/symphony/SELF-EVAL.md`. The proof a future gate
needs, stated now so it does not get lost:

- A you-band fixture with room noise, television, and a second talker.
- A me-band Jarvis asset whose texture energy matches the asset compiled
  before that fixture was played.
- A feeling frame that did not move because the television got loud.
- No byte of the you-band fixture inside the Galion voice file.
- `Heartbeat::step` still allocation-free on that path.
- The one dot still the only graphic, wash still the only gradient, fill
  still `#F5F5F7`.

This document does not add that gate, and it does not write a PASS.

## 4. Real-life gate

V3 work starts when every row below is true on a machine a person is
actually using: a real microphone, a real speaker, Samantha and Jarvis
both heard, in one running program. A unit test, a stub ASR, a canned
reply, or a plan file does not tick a row. A missing proof is a closed
gate.

The live session is CPU-only. The Rust heartbeat is in the loop. The
reply path is a real personality and a real AI answer for that call.
`CannedLlm` alone does not tick the gate. Hermes is not required. The
model on the wire is the Galion Open Mind / student `.gq` path once V2
has landed it. Until that path is what the live call is using, this gate
stays closed. Current Samantha and Jarvis clone models stay the ones
that speak. They are not swapped for a newer zoo.

### 4.1 Checklist

Each box is mandatory.

- [ ] **Clone live.** A person hears Samantha from the current Samantha
      model and Jarvis from the current Jarvis model, in one live session.
      The PCM the Rust stack plays was loaded from Galion's own format,
      produced by the compiler from the Chatterbox clone. Chatterbox
      format is not the file the symphony executes. Piper may still be
      the documented fallback, and a fallback turn does not count as this
      box. The rights record the running tree enforces allows the run.
      This plan does not flip `reference_clips`.
- [ ] **Jarvis texture still in that live clone.** Hub clicks, alarms, and
      ambience are audible in Jarvis and confirmed by the section C
      energy check against a clean-speech baseline. Samantha is heard
      without that hub bed.
- [ ] **Memories.** In that same call, the person states a fact and a
      later turn recalls it in the register of whoever is speaking.
      Withdrawal wipes RAM history on the next frame, and the turn after
      that does not recall the fact. No audio file of the call is left
      on disk. No transcript log is left on disk.
- [ ] **Emotions.** `galion-emotion` runs on the live 80 ms frames. `e`
      moves pitch, pitch range, rate, loudness, warmth, and pre-pause as
      `EMPATHY.md` and the persona column in `SAMANTHA-V2.md` specify.
      At `e = 0` the voice stays plain. No scripted empathy line is
      spoken. Anger is answered, not copied.
- [ ] **Gradients / colour in the voice.** The same `e` is an OKLCH colour
      on the voice decision: neutral mark `oklch(0.971 0.003 286)` at
      `e = 0`, heather `oklch(0.72 0.09 310)` at `e = 1`, OKLab in
      between. That colour is carried as prosody. The persona does not
      describe the colour, and the circle fill is not painted with it.
- [ ] **Colorful space.** In that same call the one-dot page shows the
      amber `#F5A524` → violet `#7C3AED` wash on `#0A0A0C` moving with
      the same `e` (half-mix and desaturation as specified). One circle.
      Fill stays `#F5F5F7`. No second graphic. `LOCKS.toml` `[ui]` is
      unchanged.
- [ ] **Together.** Clone, memory, emotion, colour-in-voice, and the wash
      are one conversation, not five demos. Both personas are heard in
      that conversation. The space and the voice change together because
      they share `e`.
- [ ] **One dot, live.** One screen. The person speaks, the mic records
      without a second panel, and the workflow proceeds. `chrome_allowed`
      is still false.
- [ ] **80 ms, live.** The running clock is `frame_ms = 80`. A late frame
      pauses. `Heartbeat::step` does not block and does not allocate.
      The turn worker is not inside the step.
- [ ] **Tests green.** On the tree that is running:
      `scripts/check-locks.sh`,
      `cargo test --workspace`,
      `cargo test -p galion-ml-voice --features speech`,
      `cargo test -p galion-render --features web --lib`,
      `cargo test -p galion-cascade --features cascade --lib`,
      `cargo test -p galion-console --features phone-turn --lib`.
      The heartbeat zero-alloc test is among the workspace tests and it
      passes. `LOCKS.toml` is unmodified by the V3 effort.
- [ ] **Self-eval PASS.** `docs/galion-voice-v2/symphony/SELF-EVAL.md`
      exists and records PASS for Eureka sections A–G: compiler,
      clone landing, Jarvis texture retained, persona and emotion,
      one dot, runbooks, 80 ms latency. Each PASS cites the command or
      the live check that produced it. A missing file is not a pass.
      An incomplete section is blocked. This V3 plan does not write
      those lines. V2 writes them after the live run.

### 4.2 Who may open the gate

All eleven boxes true, recorded by Maciej Grajczyk against the commit that
was actually running. A branch that only adds this plan leaves every box
open. Opening the gate is a later decision. It is not implied by merging
the plan.

## 5. Out of scope for this change

- Any Rust, Python, HTML, or format implementation of the room layer.
- A denoiser, a diariser, a television detector, or a new bus type.
- Editing `LOCKS.toml`, including `reference_clips`, `frame_ms`,
  `gradient_scope`, `chrome_allowed`, and `paid_tiers`.
- Fetching weights, fetching clips, or hashing a reference.
- Rewriting `GALION-VOICE-V2.md` §1.
- Writing `SELF-EVAL.md`, or writing a PASS for a section that has not run.
- A second UI, a noise meter, or a colour on the circle fill.
- Treating bystander audio as a clone source or as a memory.
- A GPU path, a GGUF import, or a cloud TTS API.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
