# V3 · Admin backlog

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Five items, written so they stay distinct.
None of them is in force. None of them is an offer. None of them edits
`LOCKS.toml`.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

**Docs only — no implementation.**

No V3 code until V2 is fully live with Samantha + Jarvis clones, memories,
emotions, gradients/color-in-voice and colorful space working.
See [STATUS.md](STATUS.md). Even after that gate, each item below still
waits on its own decision. The checklist is Part C of
[VERIFICATION.md](VERIFICATION.md).

The brand doc
([`../brand/GALION-VOICE-BRAND.md`](../brand/GALION-VOICE-BRAND.md))
still says a Local / Plus / Ultra split is not the brand, and that a
paid tier starts as an owner change to `[licence]`, with counsel, and a
line in the root ledger. `free_forever = true`. `paid_tiers = false`.
This file records intent beside that rule. It does not replace it.

## 1. Video mode bar

**Statement.** Image-only is the V2 user experience. Video is V3. The
V3 control is a video mode bar.

**Image-only, V2.** The user surface is the one mark: the phone canvas,
the PNG sequence at the heartbeat, the still picture of that circle on
its ground. `docs/galion-voice-v2/ANIMATION-ENGINE.md` already says the
orb is not a movie, full-HD decode at 240 Hz is not the path, and
`VIDEO=1` on the dot demo is an offline archive of heartbeat-rate
frames. That archive is not a video mode. There is no video feature
flag, and this note does not add one.

**Video, V3.** A mode, entered on purpose, default off. When it exists
it stays optional and CPU on the machine in front of the person, off
`Heartbeat::step`, and it is not a camera bus and not a second ngrok.
Those limits are already written in `ANIMATION-ENGINE.md`. This backlog
adopts them. It does not loosen them.

**The bar.** A mode bar is chrome. `chrome_allowed = false` forbids
menus, buttons, and text on the user surface. So the bar cannot be
drawn on that surface until you decide one of these, in a ledger line:

- an owner edit of the lock that names the bar and nothing else, or
- the bar lives in the developer console, which is already not the user UI (`console_is_user_ui = false`), or
- the mode changes by a gesture, and nothing is drawn.

Until that line exists, image-only is the only user-facing picture, and
it remains available after video exists. Video does not replace the dot.
The dot is the mark inside either mode.

Open questions are 10 and 11 in [STATUS.md](STATUS.md): where the bar
lives, and whether "image-only" also means a still a person can hand
the session. This item does not answer them.

## 2. External paid Galion API, per job

**Statement.** An external Galion API, for callers outside the local
product, is paid per job. The shape is pay-as-you-go. It is not a flat
subscription for the API.

**What a job is** is open (question 12). This note does not pick "one
turn" or "one minute" in order to look finished. You name the unit
before anyone meters it.

**What this is not.**

- Not a subscription that unlocks unlimited API use.
- Not Plus or Ultra. Those names belong to item 5, the product, and they do not meter the API. A person on a future Plus tier does not, by that fact, hold an API plan. Whether they may call the API at all is yours to answer.
- Not ElevenLabs, Cartesia, Play.ht, hosted Resemble, Inworld, Fish Audio, or Azure Personal Voice. V2 dropped those. They stay dropped. This item is Galion's own API, if you later run one.
- Not a price. No number is set here. Publishing a number is a later ledger line, after the licence question below.
- Not a key in the repo, and not a client in this tree.

**Licence.** The code grant stays Perpetual Open MIT, `free_forever` for
the code, weights separate. A hosted meter is a paid offer. The brand
doc already says a cloud plan starts as an owner edit of `[licence]`,
with counsel, and a root ledger line. Until that edit exists, public
text does not offer the API. This file is not that offer, and it is not
that edit.

The local voice path does not call this API to speak. V2's reply slot
stays local. The compiler in [SYMPHONY.md](SYMPHONY.md) is not this API.

## 3. GPU island

**Statement.** Heavy jobs may use a GPU island of about one server with
four NVIDIA B200 GPUs. The default path of Galion One, Galion Voice, and
Hermes stays CPU-first.

**Heavy** means the work V2 already keeps off the phone: student
distillation (blocked on GPU budget, A21 legal, and R0), a Chatterbox →
Galion compile when you class it as heavy, and batch work of that size.
It does not mean a phone turn, a heartbeat frame, a Piper reply, or a
Hermes lookup inside the 3 s side-band cap.

**CPU-first, named products.**

| Product | Default path |
|---|---|
| Galion One | CPU, on the machine in front of the person. The product voice in `../galion-voice-v2/personas/GALION-ONE.md`. |
| Galion Voice | CPU. The one-dot session, PhoneLane, Piper or flite, the local clone worker when a cleared one exists. |
| Hermes | CPU. The side band in V2-S15. Galion Research is the voice; Hermes is the access layer. A lookup does not move onto the island to seem faster. |

The island is unreachable from `Heartbeat::step`. A phone turn does not
wait on it. If the island is down, the default path still speaks. There
is no fallback that "tries the GPU anyway" inside the 80 ms frame.

This note does not claim the server exists, does not name a host, an
account, or a price, and does not place an order. Sizing is the owner's:
about one server, four B200s. Question 14 asks where it lives and who
may send it work.

t204u and t205u still stand for the CPU workers: full threading, SIMD,
maximum memory, no artificial caps, self-offline. The island does not
revoke that. It is a different machine for a different class of job.

## 4. Hermes Research and the console, on Galion OS

**Statement.** Hermes Research and the console integrate into Galion OS
as a Linux desktop.

Names, so they do not collapse:

- **Galion Research** is the product voice (previously called Hermes Research). The instruction file is `../galion-voice-v2/personas/GALION-RESEARCH.md`. If a person says "Hermes", they mean this path, and the voice introduces itself as Galion Research.
- **Hermes** is the access layer behind that voice: `galion-sideband`, V2-S15, off the 80 ms tick.
- **The console** is `galion-console`. `console_is_user_ui = false`. It is the developer tool. It is not the orb.
- **Galion OS** is the Linux desktop this item aims at. The voice-side nerve already uses the group name `galion-os` (`../galion-voice-v2/OPENMIND-NERVE-CORTEX.md`). That group is not, by itself, a desktop install.

**What the integration is for.** One desktop where Research and the
console have a home: the research path and the developer surface live
on Galion OS, next to each other, instead of as two unnamed hosts.
The voice a person is in, when voice is in the session, is still the
one dot. The desktop does not become a second voice UI, a menu around
the mark, or a place the heartbeat is rewritten.

**What this item does not do.**

- It does not vendor Obsidian Open Mind. The intended VPS path remains `/opt/galion/openmind` after a human deploy. This plan does not SSH.
- It does not install a package, a systemd unit, or a framebuffer. The older `/dev/fb0` note in the wave status files stays a spec. This item does not promote it to a build.
- It does not flip `console_is_user_ui`. A desktop can show a developer console and still keep the product surface as the dot.
- It does not put Hermes on the tick, and it does not move Research's lookup onto the GPU island.

Question 15 asks which Galion OS image, and confirms the dot stays the
only voice surface on that desktop. Until you name the image, "Galion OS
Linux desktop" is the target, not a downloaded artifact.

## 5. Voice under Galion One

**Statement.** Voice-to-voice sits under Galion One. It is exclusive to
paid Plus and Ultra. Local, free forever, has no voice-to-voice.

**Galion One** is the product name and the product voice
(`../galion-voice-v2/personas/GALION-ONE.md`). Samantha and Jarvis stay
TEST / INTERNAL. They are not the voices this tier sells. This item
does not rewrite the persona file.

**The three names.**

| Name | This backlog's intent | In force today |
|---|---|---|
| Local | Free forever. No voice-to-voice. | The lock's `free_forever = true` covers the code. It does not yet define a Local product that lacks voice. The running tree is a voice program. This row does not remove voice from it. |
| Plus | Paid. Includes voice-to-voice under Galion One. | Not a product. `paid_tiers = false`. |
| Ultra | Paid. Includes voice-to-voice under Galion One. | Not a product. `paid_tiers = false`. |

No price is attached to Plus or Ultra. Memory profiles in the research
notes that use these words are deployment profiles. They are not these
tiers. The research brief already says so. This item does not convert
them.

**What has to happen before the statement is true.**

1. You edit `[licence]` yourself. `paid_tiers` becomes true only in that edit, and `free_forever` is rewritten only as far as you explicitly write. This planning pass does not touch the file.
2. Counsel signs, the way the brand doc requires.
3. A line in the root `LEDGER.md` records the edit.
4. The brand doc is updated in a later pass so public text matches the lock. Until that pass, public text keeps the current brand: no paid tier, no trial, no cloud plan.
5. You say what Local still includes. This file refuses to invent a Local feature list (the mark, typed turns, silence, or something else). Question 13 asks.

**Voice-to-voice** means the caller speaks and Galion speaks back. Local,
under this intent, does not have that loop. The external API in item 2
is a different meter and does not become Local's way around the limit.
Research voice-to-voice is not silently included. If Galion Research
follows the same exclusivity, you say so. Until you do, this item covers
Galion One only.

The master slogan stays the reason the voice exists. It is not a
justification for putting voice-to-voice on a tier before the lock says
the tier exists.

## Order

These five do not depend on each other to be *written*. They do depend
on the V3 code gate before any of them is *built*, and items 2 and 5
also depend on a licence edit before they are *offered*. Item 1 depends
on the chrome decision before a bar is drawn. Item 3 depends on a host
you name. Item 4 depends on an image you name.

A pull request that implements one of them, while Part A of the
verification list is open, or while the decision that item needs is
missing, should be refused. The refusal is the point of writing them
down here instead of inside a crate.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
