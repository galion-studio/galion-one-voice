# Empathy

Galion Voice — Perpetual Open MIT. (c) 2026 Maciej Grajczyk.

Empathy is a weight on the outbound feeling frame, not a sentence and not a
ninth emotion label. `StubExpressor::voice_color` in `galion-emotion` reads
the caller frame once and returns prosody and an OKLCH colour from the same
weight `e`. The feeling path (`voice_color` inside the duplex loop) is that
call. The phone reads the same `e` on `/dot` and moves only the background wash.

## Weight

`e` is 0 when there is no caller frame. No consent means no frame, so `e` is 0.
Otherwise, when the label is Sad, Tired, Anxious or Angry:

`e = confidence · clamp(−valence / 0.6, 0, 1)`

Joy, calm and a low-confidence frame stay at 0. Anger is answered, not copied:
the policy target stays positive and lower-arousal. There is no reply-text
table attached to `e`.

The 3-bit `Affect` field on `EmotionWire` is full, and `feeling_wire` is
locked, so empathy is not a new label.

## Prosody at e = 1

Blended from the existing VAD table toward this profile, then clamped.
The table alone raises pitch on the consoling targets (`+0.18` to `+0.30 st`).
`voice_color` overrides that. `plan` by itself is still the table.

| Knob | At e = 1 | Tag |
|---|---|---|
| pitch | −0.5 st | direction: lower mean F0 (Chen et al. 2024; Hernández Melo 2026). Size is a design choice. Kept above grief (−1.2 st). |
| pitch range | 0.88 | direction: lower pitch variability (Chen et al. 2024). Size is a design choice. |
| rate | 0.92 | direction: slower (Chen et al. 2024; Juslin & Laukka 2003, tenderness). Size is a design choice. |
| loudness (`energy`) | 0.80 | direction: quieter (Chen et al. 2024; Hernández Melo 2026). Size is a design choice. |
| warmth | 0.75 | design. Chen et al. read higher jitter and shimmer as breathiness; HNR was not significant. |
| pre-pause | 250 ms | the pause the consoling targets already take. No acoustic source claimed. |

Calm’s table pitch is 0 st and its warmth is lower. Grief’s table pitch is
−1.2 st. Empathy sits between them on pitch and above both on warmth.

## Colour

At `e = 1` the colour is `oklch(0.72 0.09 310)` → `#B695CF` (heather).
At `e = 0` it is the neutral mark `oklch(0.971 0.003 286)` → `#F5F5F7`.
In between, OKLab interpolation. Hue 310 and the lightness are a design
choice: lighter than grief (0.45), darker than calm (0.86), in a gap of the
palette, inside the violet family of the wash. No colour-empathy study was
found for this hue.

`LOCKS.toml` `[ui]` still has `dot_color = "#F5F5F7"` and
`dot_varies = ["size", "brightness"]`, and `gradient_scope = "background"`.
This change does not edit the lock. The circle fill stays the locked colour,
scaled only by brightness. `e` eases the existing amber `#F5A524` ↔ violet
`#7C3AED` wash toward a half mix (`EMPATHY_WASH_AMBER = 0.5`) and lowers its
saturation. Recolouring the circle needs an owner edit of the lock.
A latched fault suspends this pull: the field becomes the locked amber →
violet pair at full strength, and the circle stays `#F5F5F7`
(`ERROR-STATE.md`). When the fault clears, this pull returns.

## Spectrum

Not wired. A later extractor may nudge lightness and chroma of the current
hue. It must not move hue. No spectral feature is computed here.

## What this is not

Not a chromagram, not a second emotion engine, not a canned line, not a
change to `DotParams` radius or brightness.

— Maciej Grajczyk
