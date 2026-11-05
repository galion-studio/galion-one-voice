# Galion Voice — brand

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
The name is Galion. The product is Galion Voice.

## Slogan

"The seamless connection with the human being, on the voice only."

That sentence is the public description. It is one line, not a stack of slogans.
The mark stays one dot. The slogan is not printed on the user surface.

## Name and voice

Galion Voice is a full-duplex voice that runs on the machine in front of you.
It speaks in short, spoken sentences. It does not announce itself as a studio,
a platform, or a model brand. In the canned replies it says "I'm Galion" and
stops. It does not claim to be a person.

The voice on the neural path is Piper VITS `en_US-ljspeech-medium` (the A21
slot): calm, English, CPU. Flite is the fallback and must not be described as
that voice. Until ASR is ratified, the demo must not be described as
understanding the caller's words. The interim path is a placeholder and is
labelled.

Tone of the writing around it: plain and specific. The master slogan above
is the only slogan. The product can be quiet. The dot is the interface.

## The mark

The mark is one orb. It is the V1 dot, given mass.

- Fill `#F5F5F7` (near-white). The live mark's body stops are that token.
  A soft inner highlight and a hairline edge stay in the same near-white
  family. Brightness is glow and a little size. It does not multiply the fill.
  The PNG golden still scales the solid token by brightness (G4). That path
  is not the phone.
- Ground `#0A0A0C` (near-black).
- Background wash only: amber `#F5A524` → violet `#7C3AED`. The wash never
  tints the mark. `LOCKS.toml` `[ui]` `gradient_scope = background` — the
  amber→violet wash stays on the ground. `LOCKS.toml` was not edited.
- Motion: idle breath, listen (opens with the microphone), think (a quiet
  pulse while a turn is in flight), speak (radius follows the reply you hear).
  The mark stays a circle. See `docs/galion-voice-v2/ORB.md`.
- Interior: a quiet warmth from the live `/dot` `emotion` object (heartbeat
  `AffectFeed`), read through the empathy table. Not a hue change of the mark.

There is no wordmark on the user surface, no second shape, no face, no
waveform bar. `chrome_allowed = false`. A one-line hint, "Tap the dot to talk",
exists so a phone can grant the microphone. It is the only text, it fades,
and an owner may still decide it violates the lock. The lock was not edited
to permit it.

Developer consoles (`galion-console` panels, ANSI bars) are not the product
surface. `console_is_user_ui = false`.

## Colour use outside the product surface

Docs, the README, a slide: near-black ground, the off-white mark, amber and
violet only as a thin wash or a single accent, never as a rainbow. Emotion
does not recolour the mark into another hue. On the phone it is a quiet
shift inside the near-white highlight, plus the breath. `dot_color` stays
one value.

## Licence, and what is not a price list

`LOCKS.toml` `[licence]`:

- Code: Perpetual Open MIT. (c) 2026 Maciej Grajczyk.
- `free_forever = true`.
- `paid_tiers = false`.

Weights are not part of that grant. They stay out of git and keep their own
licences (the A21 Piper pack is MIT and fetched, not vendored; a future
student would ship with `LICENSE-WEIGHTS`).

The Ultra host plan is `docs/galion-voice-v2/symphony/PLAN-ULTRA.md`
($220/month, CX43-class / Titanaxe Max 3XL). It is not the brand. The
one-dot page does not show a price, a trial, or a checkout. Putting that
plan into `[licence]` starts as an owner change to the lock file, with
counsel, and a LEDGER line. Until that edit exists, `paid_tiers` stays false.

## Writing rules

- Galion, never a misspelling and never a different studio name.
- V1 is the foundation, V2 is this architecture program, V3 is production later.
- Do not call the interim ASR recognition.
- Do not say the service is deployed on the VPS. It is not, until Plan V5's
  human steps are done and a LEDGER line says so.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
