# Galion sound language

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Developed and distributed by Galion Labs.

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network

The standard is **432 Hz just intonation**. The generators are the octave **2:1**, the fifth **3:2**, and the major third **5:4**. A frequency belongs on the lattice when it is `432 × 2^a × (3/2)^b × (5/4)^c` for integers a, b, and c. The warm hum is **216 Hz** (`432 × 1/2`) and **108 Hz** (`432 × 1/4`).

These are the frequencies the pages synthesize with Web Audio oscillators. They are not a measured spectrum of a recording. No voice audio, no sample, and no wave file is used. Peak gain below is the oscillator coefficient in the page, not a measured loudness.

## Lattice used by the cues

| Hz | Ratio to 432 | How it is built |
|---:|---|---|
| 108 | 1/4 | octave down, twice (2:1) |
| 216 | 1/2 | octave down (2:1) |
| 288 | 2/3 | fifth down (3:2 inverted) |
| 324 | 3/4 | fifth up, then octave down: (3/2) × (1/2) |
| 432 | 1/1 | the standard |
| 540 | 5/4 | major third |
| 648 | 3/2 | fifth |
| 864 | 2/1 | octave |

440 Hz and 880 Hz are not on this lattice.

## Galion OS interface tones

Exactly seven. They live on `/one` in `crates/galion-console/assets/v1-screen.html` (`OS_SOUNDS`). Each one is a sine line, and the page also lays the 216 Hz and 108 Hz hum under it (hum gain 0.016, note gain 0.05). Research cues below are a different desk. They do not add an eighth OS tone.

| Id | Notes, in order (Hz) |
|---|---|
| boot | 108, 216, 432 |
| shutdown | 432, 216, 108 |
| notification | 432, 540 |
| wake | 216, 432, 648 |
| done | 540, 648, 864 |
| approval | 432, 648 |
| error | 216, 108 |

## Galion Research

The desk is `crates/galion-console/assets/research.html`, served at `/research`. The cue table is the JSON element `research-cues`. `window.GALION_RESEARCH` is that same table.

Resonance is on by default. Silence is opt-in. The control is the Silence button. `localStorage` key `galion-research-silence` is `"1"` when silenced and `"0"` when resonance is on. A visit with `?silence=1` opts in and is remembered. Nothing is synthesized before a pointer or key gesture. `prefers-reduced-motion: reduce` holds the voice mark still and plays no cue. The Silence preference is not rewritten because of that setting.

Resonance cues are sines with a 28 ms attack. Bell cues are sines with a 4 ms attack and a faster decay, so the strike is the envelope. Every partial is still on the lattice. Bells are used only for window open, window close, window minimise, and window settle. A hum partial that is already one of the notes is not started a second time. Under a resonance cue the hum gain is the cue gain capped at 0.012. In a bell with more than one partial, partial *i* (counting from 0) is scaled by `1 - i × 0.18`.

An answer plays one cue, from the standing status. Found, Inferred, and Unknown are different lines. Unreachable and Denied use the Unknown line.

| Cue | Kind | Partials, in order (Hz) | Hum (Hz) | Peak gain | Timing |
|---|---|---|---:|---:|---|
| Question sent | resonance | 432, then 540 | 108, 216 | 0.04 | step 0.09 s, each note held 0.20 s |
| Thinking | resonance | 108 and 216 together | the notes are the hum, not stacked again | 0.008 | held 0.70 s. This is the quietest cue |
| Answer, Found | resonance | 540, then 864 | 108, 216 | 0.042 | step 0.10 s, held 0.24 s |
| Answer, Inferred | resonance | 648, then 540 | 108, 216 | 0.036 | step 0.10 s, held 0.22 s |
| Answer, Unknown | resonance | 288, then 216 | 108, 216 | 0.034 | step 0.11 s, held 0.26 s. 216 is the note, so the hum adds 108 only |
| File attached | resonance | 540 | 108, 216 | 0.038 | held 0.16 s |
| Interface hidden | resonance | 216 | 108, 216 | 0.03 | held 0.18 s. The hum adds 108 only |
| Interface shown | resonance | 432, then 864 | 108, 216 | 0.04 | step 0.08 s, held 0.18 s |
| Window open | bell | 540, 648, 864 together | none | 0.05 | decays 0.42 s, 0.28 s, 0.16 s |
| Window close | bell | 432, 288, 216 together | none | 0.046 | decays 0.40 s, 0.30 s, 0.22 s |
| Window minimise | bell | 432, 324, 216 together | none | 0.04 | decays 0.26 s, 0.20 s, 0.16 s |
| Window settle | bell | 432 | 108, 216 | 0.028 | strike decays 0.55 s. Hum gain 0.012. Plays 180 ms after the window action when motion is allowed |

Ids in the table are `question`, `thinking`, `found`, `inferred`, `unknown`, `file`, `hide`, `show`, `window-open`, `window-close`, `window-minimise`, and `window-settle`.
