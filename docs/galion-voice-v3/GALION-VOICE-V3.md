# Galion Voice V3

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. This is the program plan. It is not a claim
that V3 has started, and it is not permission to write V3 code.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

**Docs only — no implementation.**

The gate, the open questions, and the short status are [STATUS.md](STATUS.md).

## 1. How this sits on V2

V2 is the program in
[`../galion-voice-v2/GALION-VOICE-V2.md`](../galion-voice-v2/GALION-VOICE-V2.md).
It is architecture, the orb, the typed bus, the edge, and priority-1
local cloning on the Chatterbox path. This folder does not renumber those
sectors and does not reopen a decision V2 already closed.

V2's own version map still says V3 is production-ready Galion Voice:
real EN ASR ratified, a reply model past the canned table, speech warm,
the edge on the VPS, gates R0–R3 green, and a consent story counsel has
signed. That sentence stays in the V2 plan. This pass does not edit it.

This folder is the owner's planning pass for what V3 contains once V2 is
fully live. The two descriptions are both on paper. Question 2 in
[STATUS.md](STATUS.md) asks whether the older production list joins the
gate below.

Ops numbers stay a different series. Plan V4 and Plan V5 are not Voice V4
or Voice V5. The root `LEDGER.md` titled "Galion Voice V4" is the wave
ledger. This folder keeps its own [LEDGER.md](LEDGER.md) so a planning
witness is not mistaken for a wave landing.

## 2. Gate

No V3 code until V2 is fully live with Samantha + Jarvis clones, memories,
emotions, gradients/color-in-voice and colorful space working.

The six are defined in [STATUS.md](STATUS.md) and checked, later, against
[VERIFICATION.md](VERIFICATION.md). A witness is a line in [LEDGER.md](LEDGER.md).
Until the six lines exist, the rest of this plan is reading material.

Rights stay where V2 put them. Samantha and Jarvis are owner-directed
research targets and TEST / INTERNAL personas. A clone is not "live"
while [`../galion-voice-v2/RIGHTS-CLEARANCE.md`](../galion-voice-v2/RIGHTS-CLEARANCE.md)
is unfilled and the root ledger has no clearance. Fetching Chatterbox
safetensors (`../galion-voice-v2/CLONE-WEIGHTS.md`) is a V2 weight path.
It is not a clone, not a reference clip, and not this gate.

## 3. The three bodies of V3

Once the gate is witnessed, V3 is these three, in this order. Order is
dependency, not a calendar.

### V3-A · User-scene, beside Jarvis texture

A separate layer for the caller's space: background noise, a television,
other people. It sits alongside Jarvis texture. It is not mixed into
that texture, and it is not a second voice.

Detail: [USER-SCENE.md](USER-SCENE.md).

Depends on the Jarvis clone being real, because the texture has to exist
before a room can be kept out of it. The same split applies to Samantha's
texture and to Galion One when those voices are the ones speaking. Jarvis
is the named case.

### V3-B · Extensions to the V2 symphony

The symphony is one piece:

- the one dot
- the 80 ms heartbeat
- the Rust infra
- the Chatterbox → Galion compiler

The first three are the V2 stack. The compiler is named with them and is
not built. V3 extends each part. It does not replace the dot, the clock,
or the crates.

Detail: [SYMPHONY.md](SYMPHONY.md).

Depends on V3-A for any motion that reads the room. The compiler also
depends on a cleared clone worth compiling. Until then the speaking
fallback stays the one V2 already named: Piper, else flite.

### V3-C · Real-life verification and the LEDGER

Every claim that something works is a real room, a real ear, and a line
in the ledger. Desk prose does not tick a box.

Detail: [VERIFICATION.md](VERIFICATION.md), [LEDGER.md](LEDGER.md).

Depends on nothing except honesty. The checklist may be read now. It may
not be marked passed now.

## 4. Admin backlog

Five items are written down so they are not lost. None of them is in
force. None of them edits `LOCKS.toml`. None of them is a public offer.

1. Video mode bar. Image-only is V2. Video is V3.
2. An external paid Galion API, billed per job (pay-as-you-go). Not a flat subscription for the API.
3. A GPU island of about one server with four B200s, for heavy jobs. Galion One, Galion Voice, and Hermes stay CPU-first on the default path.
4. Hermes Research and the console, integrated into Galion OS as a Linux desktop.
5. Voice under Galion One, exclusive to paid Plus and Ultra. Local, free forever, has no voice-to-voice.

Detail: [ADMIN-BACKLOG.md](ADMIN-BACKLOG.md).

`LOCKS.toml` `[licence]` still says `free_forever = true` and
`paid_tiers = false`. The brand doc still says a Local / Plus / Ultra
split is not the brand. A later owner edit, counsel, and a ledger line
are the path the brand doc already requires. This plan does not take
that path.

## 5. What stays V2

- One mark. Fill `#F5F5F7`. Ground `#0A0A0C`. Amber → violet on the background only. `chrome_allowed = false`. `dot_states` stays idle, listening, speaking until an owner adds a name.
- Heartbeat 80 ms. 12.5 Hz, 24 kHz, 1920 samples, 60 ms inside the frame. Heavy work stays off `Heartbeat::step`.
- English only on the phone path, until the V2 multilingual hold is lifted by its own decision.
- Consent default deny. `sense` and `adapt` off. Withdrawal wipes RAM history. Workplace and education purposes stay prohibited.
- Local cloning, self-offline, out of process, no film audio in git, no cloud TTS vendor. ElevenLabs, Cartesia, Play.ht, hosted Resemble cloning, Inworld, Fish Audio, and Azure Personal Voice stay dropped.
- Hermes on the side band, not on the tick. The 3 s retrieval cap stands.
- Loopback app. ngrok now, Caddy later. No second tunnel. No SSH from this program.
- `paid_tiers = false` until an owner changes it.

## 6. Non-goals

- Implementing any section of this folder before the gate is witnessed.
- Editing `LOCKS.toml` from a planning pass.
- Committing a WAV, a reference clip, or a weight.
- A second voice stack beside PhoneLane.
- A second mark, a face, a waveform, or a caption of a persona id on the user surface.
- Folding the television, the room, or another person into Jarvis texture.
- Cloning a bystander.
- A flat subscription for the external API.
- Putting the GPU island on the default path of Galion One, Galion Voice, or Hermes.
- Treating the developer console as the user interface.
- A price list, a SKU, or a public Plus / Ultra offer.
- Calling this folder V3-complete, or calling the V2 spring page V3.

— Maciej Grajczyk
