# Galion Voice V3 — status

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

**Docs only — no implementation.**

This folder is a plan. It is not a crate, not a feature flag, and not a
licence change. These pages do not authorize V3 code. `LOCKS.toml` is not
edited. No WAV is added. This pass does not fetch a weight and does not
call `generate`.

## Gate

No V3 code until V2 is fully live with Samantha + Jarvis clones, memories,
emotions, gradients/color-in-voice and colorful space working.

All six have to be working on a real machine, and a human has to witness
each one in [LEDGER.md](LEDGER.md). Until those lines exist, a pull request
that adds V3 code should be refused. Planning text in this folder is the
only V3 work that is allowed.

What "working" means, and what is already known to fall short, is
[VERIFICATION.md](VERIFICATION.md).

| Gate | Working means | Known on 2026-11-03 |
|---|---|---|
| Samantha clone | A cleared local clone speaks as the Samantha register. Piper is not standing in for that voice. | Rights hold stands (`../galion-voice-v2/RIGHTS-CLEARANCE.md`). The persona eval scores heard voice against the films at 1. Clone weights may be fetched (`../galion-voice-v2/CLONE-WEIGHTS.md`). Generate is not the live path. Reference clips stay off git. |
| Jarvis clone | The same, for the Jarvis register. Jarvis texture is that voice, not the room. | The same hold. The same score. Tags stay off. |
| Memories | This call can hold what it has already established, in RAM, and a withdrawal wipes it. | `withdraw_wipes_ram_history = true` is the lock. This ledger has no live-call witness. |
| Emotions | The same `e` moves the spoken knobs and the wash. `sense` deny keeps `e` at 0. | The spec is `../galion-voice-v2/EMPATHY.md` and `../galion-voice-v2/SAMANTHA-V2.md`. The persona eval moved knobs in a harness. That harness is not this witness. |
| Gradients / color-in-voice | OKLCH `voice_color` is the colour decision, and the spoken voice follows it. The fill stays `#F5F5F7`. | Designed. Not witnessed here as fully live. |
| Colorful space | The room around the one mark is the living amber → violet field, continuous, on the real surface. | A headless mood wash was measured in the persona eval. A living space on the real surface is not witnessed here. |

A Piper line is not a clone. A flite line is not a clone. A fetched
safetensors tree is not a clone. An unfilled rights checklist is not a
clearance.

## What V3 is, once the gate is witnessed

Three bodies of work, still paper:

1. A separate user-scene layer (background noise, television, other people) alongside Jarvis texture. [USER-SCENE.md](USER-SCENE.md).
2. Extensions to the V2 symphony: one dot, the 80 ms heartbeat, the Rust infra, and a Chatterbox → Galion compiler. [SYMPHONY.md](SYMPHONY.md).
3. A real-life verification checklist and this folder's LEDGER. [VERIFICATION.md](VERIFICATION.md), [LEDGER.md](LEDGER.md).

The admin backlog is recorded and is not in force. [ADMIN-BACKLOG.md](ADMIN-BACKLOG.md).

The program plan is [GALION-VOICE-V3.md](GALION-VOICE-V3.md).

The V2 program plan still has its own later line for "V3": ratified EN ASR,
a reply past the canned table, warm speech, the VPS edge, gates R0–R3, and
counsel on consent. This folder does not edit that sentence. Whether those
items sit inside this gate is an open question below.

## Open questions for Maciej

1. **Who witnesses the gate.** Is a line in this folder's LEDGER, signed by you, the only thing that opens V3 code? Or does the root `LEDGER.md` have to carry the same line?
2. **The older V3 sentence.** `../galion-voice-v2/GALION-VOICE-V2.md` still defines V3 as production readiness (real EN ASR, reply past canned, warm speech, VPS edge, R0–R3, counsel). Do those six join this gate, or do they stay a V2 exit, separate from Samantha, Jarvis, memories, emotions, and colour?
3. **Memories.** Is the gate the RAM history the consent lock already names, wiped on withdraw, with no disk and no portrait of yesterday? Or is cross-session memory (owner item E5 in the wave notes) required before any V3 code?
4. **Colorful space.** Is this the locked amber → violet wash, moved by `e` and by mood, with the fill left at `#F5F5F7`? Or a wider field that would need an edit of `gradient_scope`?
5. **Gradients / color-in-voice.** Is the witness "a person hears the knobs and sees the wash from the same `e`"? Heather on the circle fill would be a lock edit. Is that paint inside the gate, or outside it?
6. **Jarvis texture.** Does the gate mean the cloned Jarvis voice only, or also the Jarvis prosody and wash bias while Piper is still the speaker? The scene layer sits beside the texture. The texture has to be named before the layer can.
7. **Samantha and Jarvis as product.** Both ids are TEST / INTERNAL. Does "clones working" mean an internal test you can hear, or a public voice? The rights hold stays closed either way. This plan does not lift it.
8. **User-scene shape.** Is the layer a bed the caller hears under the voice, a classification the session uses (noise, television, other people), or both? Other people's voices are not a clone and not a transcript. Is scene consent the existing `sense` bit, or a new bit you would add to the lock later?
9. **Compiler container.** The Chatterbox → Galion compiler should emit a Galion-native voice artifact. Is `.gq` the container, safetensors only, or a voice-specific sibling you have not named? The PyTorch pin sunsets `2027-02-01`. Does a finished compiler replace that pin, or sit beside it until you edit the lock?
10. **Video mode bar and chrome.** A bar is chrome. `chrome_allowed = false`. Does the bar wait on a lock edit, live only in the developer console, or become a gesture with no drawn control?
11. **Image-only.** V2 is image-only. Is that the still mark and the PNG sequence, or also a still picture a person can hand the session?
12. **External API.** A "job" is unnamed. Is it one turn, one compile, one minute of audio, or something else? The local MIT binary and `free_forever` are the code grant. Does a hosted per-job API require the `[licence]` edit the brand doc already requires before any paid offer?
13. **Plus and Ultra.** Voice-to-voice under Galion One, only on paid Plus and Ultra, with Local free forever and no voice-to-voice, contradicts `paid_tiers = false` until you edit the lock, counsel signs, and a LEDGER line says so. Confirm this backlog is that future edit, and not an offer. What does Local still include?
14. **GPU island.** About one server, four B200s, for heavy jobs. Galion One, Galion Voice, and Hermes stay CPU-first. Does any heavy job ever sit on a phone turn? This plan says no. Where the server lives, and who may send it work, are yours.
15. **Galion OS desktop.** Hermes Research and the console move into a Galion OS Linux desktop. Which image, and does the one dot stay the only voice surface on that desktop? `console_is_user_ui = false` stands until you edit it.
16. **Link from the V2 plan.** This pass does not edit `GALION-VOICE-V2.md` or the README. Should the version map point here, or should this folder stay unlinked until the gate is witnessed?

## What this pass did not do

- No implementation. No new crate, script, feature, or test.
- No edit of `LOCKS.toml`. Global locks stand, including `paid_tiers = false`, `free_forever = true`, `chrome_allowed = false`, the 80 ms frame, and `reference_clips = false`.
- No WAV, no clip, no weight download, no `generate`.
- No root `LEDGER.md` line that claims a gate passed.
- No price, no SKU, and no public offer of Plus, Ultra, or an API.
- No SSH, and no claim that anything is deployed.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
