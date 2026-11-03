# V2 · Error state

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
The spelling is **Galion**. This is an interface requirement for the user
surface. It is not drawn on the phone page yet. It is not a lock edit, and
it is not V3.

Motion of the mark: `ORB.md`. The one screen and the Luti profile:
`ONE-UI.md`. Samantha's lean on that profile is `SAMANTHA-V2.md`.
The healthy wash, including the empathy pull, is
`EMPATHY.md` and `palette::background_at`. This file says what the surface
does when something is wrong. `LOCKS.toml` is not edited.

## What the person sees

The whole interface is the warning. Two draws change, together:

1. The background leaves the healthy vignette and becomes a full-bleed
   gradient of the locked pair, amber `#F5A524` → violet `#7C3AED`.
2. One custom code, from the `GV-` family below, is rendered in that same
   gradient.

The mark stays what `[ui]` already locks. One circle. Fill `#F5F5F7`,
solid, scaled only by brightness. Luti keeps the gain row it was already
in. A fault does not add a Luti row, does not scale `w` or `z`, and does
not become a fourth `dot_states` name. Size and brightness may still move.
The circle is not painted with the ramp, with heather `#B695CF`, or with a
persona wash.

The code sits in the field, horizontally centred, in the lower third,
clear of the circle's box, so the fill can still be seen as solid. One
line. No second shape, no icon, no button, no sentence on the glass.

## Design choice against the locked wash

`LOCKS.toml` `[ui]` stays as written:

| Key | Locked value | What a fault does with it |
|---|---|---|
| `dot_color` | `#F5F5F7` | Unchanged. |
| `dot_fill` | `solid` | Unchanged. Uniform brightness only. |
| `dot_varies` | size, brightness | Unchanged. Luti is not retuned. |
| `background` | `#0A0A0C` | The healthy ground. It drops out of the mix while a code is latched. |
| `gradient_from` | `#F5A524` | Still the start of the ramp. |
| `gradient_to` | `#7C3AED` | Still the end of the ramp. |
| `gradient_scope` | `background` | The ramp still never touches the dot. Filling the **code glyphs** with the same pair is the choice this file makes. The glyphs are not the dot. |
| `chrome_allowed` | `false` | The code is one string, and only while a fault is latched. Same class of tension as the hint line "Tap the dot to talk". The lock file is not amended to permit it. |

Healthy pixels are `palette::background_at`: a diagonal amber → violet
ramp, strength `0.38`, vignette back to `#0A0A0C` at the corners, 24 s
drift. Empathy (`EMPATHY.md`, `SAMANTHA-V2.md`) may pull that ramp toward
a half mix and desaturate it. That pull is the feeling of the room.

A latched fault suspends the empathy pull and the persona wash. Every
background pixel is `AMBER` lerped to `VIOLET` along the same diagonal,
with the same drift, and with no obsidian mix and no vignette. Corners
are amber or violet, not `#0A0A0C`. Heather is not a third stop. A red
flash, a white flash, or any other hue is a different design and would
need an owner edit of `[ui]`. This requirement does not make that edit.

The display clock eases the field from the healthy mix to the full ramp
in under 400 ms, and back on clear. `prefers-reduced-motion` cuts the
drift and the ease: one frame, then hold. The field does not blink.
Opacity of the wash is not the signal.

## The code family

The family is `GV-` plus four decimal digits. `GV` is Galion Voice. It plays the
role a 404 plays on a broken page: one short token that means this is not
the healthy surface. It is not an HTTP status, and the digits are not a
transport code copied onto the glass. A response status of 404 does not
by itself select a `GV-` code. The page today ignores HTTP 404 on
`POST /turn`; that ignore is not a fault latch.

The family is inventable. A new fault takes the next free number in its
band, a short name, a raise condition, and a clear condition, written in
the registry below. Codes are stable once listed. A retired code stays
reserved and is not reused. Bands `GV-6xxx`, `GV-7xxx`, and `GV-8xxx`
are empty on purpose; opening one is an owner line in this file. A fifth
digit is not used. A second prefix is not used unless this file grows an
owner line that names it. `LOCKS.toml` does not gain a key for any of this.

The thousands digit is the category:

| Band | Category | The voice failed at |
|---|---|---|
| `GV-1xxx` | Capture | The page cannot hear. |
| `GV-2xxx` | Turn | A turn was handed over and the lane did not answer. |
| `GV-3xxx` | Speech | There is nothing to play as speech. |
| `GV-4xxx` | Edge | The surface or the session refused the caller. |
| `GV-5xxx` | Presence | The mark is no longer being driven. |
| `GV-9xxx` | Unclassified | Something failed and no narrower code exists yet. |

Inside a band, `x403` is the mnemonic for "this part refused" and `x404`
is the mnemonic for "this part is missing". They are Galion mnemonics.
The next code in a band does not have to echo the next HTTP status. It
has to name a Galion fault.

### Registry

These are the first assignments. The list is not closed.

| Code | Short name | Raises when |
|---|---|---|
| `GV-1403` | Mic refused | Permission denied or dismissed, or no input device. |
| `GV-1404` | Capture lost | The browser cannot open a microphone, or the capture graph dies after a grant. |
| `GV-2404` | Turn lost | The surface gives up on a turn: the worker dies, or the turn fails closed and will not publish a reply. |
| `GV-2403` | Lane refused | ASR or the reply lane fails closed. |
| `GV-3404` | No speech | `NoSpeech`, reply bytes that will not play, or a published reply with `backend=tone` and `error` set. |
| `GV-4403` | Edge refused | Session rejected, foreign origin rejected, or the page is not a secure context (the mic needs HTTPS). |
| `GV-4404` | Surface lost | `/dot` is stale past the offline bound (1500 ms without a good poll), or the page cannot reach the surface at all. |
| `GV-5404` | Mark undriven | The display clock or the heartbeat has stopped stepping the mark while `/dot` is still fresh. |
| `GV-9000` | Unclassified | The surface will show a fault and this file has no narrower code for it yet. |

One code is on screen. The first match wins:

1. `GV-4404` Surface lost
2. `GV-4403` Edge refused
3. `GV-1403` Mic refused
4. `GV-1404` Capture lost
5. `GV-5404` Mark undriven
6. `GV-3404` No speech
7. `GV-2403` Lane refused
8. `GV-2404` Turn lost
9. `GV-9000` only when none of the above fit

`GV-4404` may keep the existing offline Luti row (smaller, dimmer). That
row is size and brightness. It is not the warning. The warning is the
field and the code. A dim dot on the healthy vignette, which is what the
page does today when the server is unreachable, does not meet this
requirement.

### What does not raise a code

- Idle, listening, thinking, speaking, and a Rika lean-in.
- Barge-in and supersede. Cancel drops a stale turn. It is not a fault.
- The labelled `interim-placeholder`, including `AsrUnratified` while that
  placeholder is the specified answer. The interim path is a limit the
  product already names. It is not a broken surface.
- The canned "say a bit more" line.
- Consent at default deny. Deny switches a feature off. It does not mean
  the voice failed.
- Empathy weight `e`, a persona id, or a Luti lean. Feeling is the healthy
  wash. A fault suspends that wash; the feeling did not cause the fault.
- Mic levels coalescing, or a full inbox, while the latest turn is still
  accepted.
- `voice.fallback` when another backend still speaks (flite in place of
  Piper). A fallback that produces speech is not `GV-3404`.
- A single `voice.frame_miss`. One late frame is not `GV-5404`.
- The Art. 50 disclosure, and the hint line, on their own.

Nerve notes (`voice.frame_miss`, `voice.fallback`, `voice.backend_up`)
stay logs. They do not paint the field. The surface latches from facts
it already holds: mic state, `/dot` freshness, playback, and the reply's
`error` / `backend`. `/dot` may later carry `fault` as `GV-xxxx` or
absent. It does not carry that field today. Absence means the page
decides. A future value, if it is a registered code, joins the priority
list above. Inventing a code in the JSON without listing it here is a
miss; the glass shows `GV-9000` until this file names the narrower one.

The log may keep a longer reason. The log is not the user surface
(`console_is_user_ui = false`). The glass shows the code only.

## How the gradient is applied

**Field.** Sample the locked pair at full strength, as specified above.
Do not multiply by `WASH_STRENGTH`. Do not apply `EMPATHY_WASH_AMBER` or
`EMPATHY_DESAT` while the code is latched. Draw the field first.

**Mark.** Draw the circle after the field. Every dot pixel is
`#F5F5F7` scaled by the current brightness and by nothing else. Sampling
the ramp for a dot pixel remains forbidden. Luti does not learn an error
gain.

**Code.** The glyphs are filled with the same amber → violet ramp, along
the same diagonal as the field, clipped to the letterforms. Weight 600 or
heavier. Cap height at least 24 CSS pixels, and at least 4% of the shorter
viewport edge. The string on the glass matches `GV-` and four digits.
No HTTP status beside it. No short name beside it.

A gradient sitting on the same gradient disappears, so each glyph carries
a solid `#0A0A0C` halo (stroke or shadow) thick enough to read. Measured
against the locked stops, `#0A0A0C` on `#F5A524` is about 9.7:1 and
`#0A0A0C` on `#7C3AED` is about 3.5:1, which is the large-text floor.
`#F5F5F7` on `#F5A524` is about 1.9:1, so a near-white code, and the flat
amber sentence the page uses today, both fail on this field. The halo is
obsidian because that is the pair that holds.

A surface that cannot clip a gradient to glyphs still shifts the
background to the full ramp, and still draws the code. In that degraded
draw the glyphs are `#F5F5F7` with the same obsidian halo.
`prefers-contrast: more` uses that same degraded glyph draw. Dropping the glyph gradient is
allowed. Dropping the code, or returning the background to the 0.38
vignette, is not.

While a code is latched the hint yields (it is not drawn). The Art. 50
note may stay. The code owns the lower third.

The display thread paints this. `Heartbeat::step` does not. The latched
code is a fact beside mode, latest wins, the way mic level already
coalesces.

## Recovery

The latch clears when the condition is gone, on the next honest frame.
There is no timer-only dismiss and no close control. The retry gesture is
the same tap that talks. If the condition is still true after the tap,
the code stays and the field stays.

| Code | Clears when |
|---|---|
| `GV-1403`, `GV-1404` | Capture is running again after a grant. |
| `GV-2404`, `GV-2403` | A later turn publishes a reply that is not itself a fault, or the failed turn is superseded and no new fault is raised. |
| `GV-3404` | A later reply plays as speech (Piper or flite). The two-tone signal ending does not clear it. |
| `GV-4403` | A later request is accepted, or the page is opened on a secure context and the mic is no longer refused for that reason. |
| `GV-4404` | The next successful `/dot`. |
| `GV-5404` | The next display tick that steps the mark from a fresh heartbeat frame. |
| `GV-9000` | A specific code replaces it, or no fault remains. |

Supersede clears a code that belonged only to the cancelled turn
(`GV-2404`, `GV-2403`, `GV-3404` for that turn). It does not clear
capture, edge, or presence. Clearing restores the healthy wash, including
the empathy pull if `e` is still set, and removes the code. The circle's
fill never changed, so recovery does not repaint it.

This requirement adds no clip and no spoken code. The two-tone WAV already
published for `NoSpeech` stays the audio tell it already is.

## Accessibility

Colour is not the signal. Motion is not the signal. A smaller dot is not
the signal. The code string is the signal, and it is the same string a
screen reader speaks.

The node is a live status (`role="status"`, `aria-live="polite"`). Its
accessible name is the code and the short name from the registry, for
example "GV-3404, no speech". The visual rendering may be gradient glyphs.
The accessible name is plain text. Every code uses the same ramp, so the
category is in the digits, not in a hue.

The type size and the obsidian halo above are what keep the code readable
on both ends of the ramp. A fault the surface cannot name still shows
`GV-9000`, not a blank field.

## Today, and what a later change has to satisfy

The phone page does not do this. `#err` is a top-aligned sentence in flat
`#F5A524`. Offline is a smaller, dimmer dot on the ordinary wash. Both
may stay until a change implements this file. That change replaces the
sentence with the code and replaces the healthy wash with the full ramp
for as long as the code is latched. It does not edit `LOCKS.toml`, does
not retune `dot.rs`, and does not re-bless G4.

Acceptance for that later change:

1. While a code is latched, every background pixel comes from the amber →
   violet ramp with no obsidian vignette and no empathy desaturation.
2. The visible string matches `GV-` plus four digits, and the accessible
   name includes that string and the short name.
3. Circle pixels stay a uniform brightness scale of `#F5F5F7`.
4. `interim-placeholder` does not latch a code.
5. Clearing the condition restores the healthy wash and removes the code.
6. `prefers-reduced-motion` holds a still ramp. The code does not blink.
7. `scripts/check-locks.sh` stays green because `LOCKS.toml` was not edited.

— Maciej Grajczyk
