# V3-B · Extensions to the V2 symphony

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. A planning note. It names four extensions.
It does not add a crate, a flag, or a compiler.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

**Docs only — no implementation.**

No V3 code until V2 is fully live with Samantha + Jarvis clones, memories,
emotions, gradients/color-in-voice and colorful space working.
See [STATUS.md](STATUS.md).

## What the symphony is

The V2 symphony is the stack playing as one piece. The owner names four
parts:

| Part | Where it lives today | What V3 is allowed to extend, later |
|---|---|---|
| One dot | `LOCKS.toml` `[ui]`. One mark, `#F5F5F7` on `#0A0A0C`, amber `#F5A524` → violet `#7C3AED` on the background only. | Motion and wash around that same mark. Not a second mark. |
| 80 ms heartbeat | `[clock]`. 12.5 Hz, 24 kHz, 1920 samples, 60 ms budget. `Heartbeat::step`. | Nothing inside the tick. New work stays off it, and the tick may read a coalesced slot. |
| Rust infra | Heartbeat, bus, render, cascade, console, sideband, the Open Mind seam. One stack. | New bodies on the bus that already exists. No second server, no second tunnel, no broker crate. |
| Chatterbox → Galion compiler | Not built. V2 holds the out-of-process Chatterbox worker and the weight fetch. | A later compile from a cleared Chatterbox voice into a Galion-native artifact. The turn path keeps Piper, else flite, until that artifact exists and you say it may speak. |

V3 extends the symphony. It does not replace it. Image-only presentation
of the dot stays the V2 picture. Video is a V3 mode and lives in the
admin backlog ([ADMIN-BACKLOG.md](ADMIN-BACKLOG.md) item 1), not as a
quiet extra in this note.

## One dot

The rule stays `one-dot`. `dot_count = 1`. `chrome_allowed = false`.
`dot_varies` stays size and brightness. `gradient_scope` stays background.
This note does not edit those keys.

Extensions, once the gate is open and you accept them:

- The wash may answer the user-scene slot ([USER-SCENE.md](USER-SCENE.md)) inside the locked amber → violet pair. A loud room settles the mark. It does not retune `DotParams`, and it does not re-bless G4 by itself.
- Colorful space, as the gate names it, is this field being alive on the real surface: continuous, mood and `e` in the wash, fill left at `#F5F5F7`. Painting heather or a persona hue on the circle remains the lock edit `EMPATHY.md` already refuses to make.
- The dot is the mark in image-only mode and in video mode. Video does not replace it with a frame of something else, and it does not add a face.

Still forbidden on the user surface: a menu, a caption of the persona,
a waveform, a second orb, a scene label.

## 80 ms heartbeat

The clock does not grow a setting. There is no configurable alternative.
That sentence is already in the lock.

These stay off `Heartbeat::step`, the way Piper, ASR, Hermes, and the
orb integrator already stay off it:

- scene classification
- a compiler job
- video decode, if video mode ever exists
- any hop to the GPU island ([ADMIN-BACKLOG.md](ADMIN-BACKLOG.md) item 3)
- an external API job

The heartbeat may read the latest scene slot when `/dot` is built, the
way it already reads a coalesced mic level and a coalesced consent bit.
One read. No allocation beyond what `step` already does. Cancel stays an
event. It is not implemented by killing the process, and it is not
delayed until the next compile.

A full compiler queue does not block the frame. The V2 rule on the job
queue is the rule here: the tick does not wait.

## Rust infra

The infra is the crates V2 already has. V3 does not found a second
voice runtime beside them.

- Process bus: a scene body, if you approve the sketch in `USER-SCENE.md`, lives next to the worker that already exists. `try_send`. Backpressure. The barge-in token still drops a stale turn.
- Render: `OrbEngine` and Luti remain the motion. A shader, a scene graph, and wgpu stay out, as `ANIMATION-ENGINE.md` already says.
- Speech: PhoneLane stays the lane. A compiled voice, later, is another out-of-process backend with Piper behind it. That is the V2 non-goal written as a placement, not a new stack.
- Side band: Hermes stays `try_send` on `galion-sideband`, off the tick, 3 s cap. The Galion OS desktop in the admin backlog hosts Research and the console. It does not fork the heartbeat.
- Open Mind: the nerve notes stay `voice.frame_miss`, `voice.fallback`, `voice.backend_up`. A scene class is not a nerve event unless you later say it is. This plan does not add one.

No new HTTP server. No new public port. The app stays loopback. ngrok
now, Caddy later. The phone still draws.

## Chatterbox → Galion compiler

V2 chose Chatterbox as architecture to run out of process: Nano first,
Turbo as the quality option, Perth watermark left unvendored, weights
gitignored, reference clips false, generate not the live path. The pin
is `[formats.exceptions.v2s14_chatterbox]`, sunset `2027-02-01`. The
fetch path is `../galion-voice-v2/CLONE-WEIGHTS.md`. CI does not download
the blobs. This note does not widen the pin and does not run the fetch.

The compiler is the missing half of that choice. Its job, when you allow
it to be built, is to take a rights-cleared voice that Chatterbox can
condition and emit a **Galion-native artifact** the Rust speech path can
render without keeping a PyTorch process on the turn.

Boundaries, not a recipe:

| | |
|---|---|
| Input | A persona id already on the bus (`samantha` or `jarvis`, or a later id you name). A rights handle for a cleared reference. The reference stays out of git. The Chatterbox family id, `nano` or `turbo`. |
| Output | One Galion-native voice artifact for that id. Candidate containers already allowed by the lock are safetensors and `.gq`. Which one a *voice* uses is open (question 9 in [STATUS.md](STATUS.md)). No new container is named here. |
| Where it runs | Off the phone turn, off the 80 ms tick, off the default CPU path when the job is heavy. Heavy compiles are the GPU island's class of work. The default voice a person hears stays CPU. |
| What speaks until then | Piper `en_US-ljspeech-medium` when the pack is present, else flite. The clone worker stays the V2 scaffold. It does not become the compiler by being described here. |
| What is refused | A film clip as input. A live room as input. A bystander's voice as input. A `.pt` file. A weight committed to git. A public demo that calls the artifact a performer. An external TTS vendor. |

The artifact, if it is ever shipped to a machine, is fetched the way the
A21 pack is fetched: pinned, not vendored, not present in a clean tree.
This plan does not name a URL, a hash, or a tag for it. Inventing one
would be a false pin.

A finished compiler does not by itself delete the Chatterbox exception.
The sunset stays `2027-02-01` until you edit the lock. Question 9 asks
whether the compiler replaces that pin or sits beside it.

Rights, consent, and licence are still the V2 hold. Compiling a voice
the ledger has not cleared is the same act as generating one. The
checklist is `../galion-voice-v2/RIGHTS-CLEARANCE.md`. Both gates are
closed.

## Acceptance, later

1. The user surface is still one filled mark. G4 is untouched unless an owner re-blesses it on purpose.
2. A compile in flight does not move the p99 of `Heartbeat::step` over the 60 ms budget. The number is measured on a real host and written in [LEDGER.md](LEDGER.md). It is not invented here.
3. The speaking path with the compiler absent is the path V2 already ships: Piper, else flite, tones only as the explicit `backend=tone` failure signal.
4. The compiler's input reference is the cleared clip, and a television in the room is absent from that input ([USER-SCENE.md](USER-SCENE.md)).
5. No WAV, no safetensors, and no `.gq` voice blob is added to git by the work that writes the compiler down.

— Maciej Grajczyk
