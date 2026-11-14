G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.

# Sound language

The reference is **432 Hz** just intonation. The ratios used here are **2:1**, **3:2**, and **5:4**. The warm hum under a cue is **216 Hz** (432 ÷ 2) and **108 Hz** (432 ÷ 4).

Hz below are those ratios evaluated. They are not measured pitches from a recording. Playback times and gains are the authored WebAudio parameters in the page that plays the cue.

No cue in this guide is a voice, a clone, or a wav file. Oscillators are sine.

## Galion One

Seven interface tones, in `crates/galion-console/assets/v1-screen.html`. `FUND` is 432. `HUM` is `[FUND/4, FUND/2]`, which is 108 Hz then 216 Hz, under every tone.

Each note is a sine. The page schedules notes 0.12 s apart. Each note lasts 0.16 s (the 0.12 s step plus 0.04 s) and peaks at gain 0.05. The hum peaks at gain 0.016 and lasts `notes × 0.12 + 0.36` seconds. The attack rises from gain 0.0001.

| id | when | Hz | ratio from 432 |
| --- | --- | --- | --- |
| boot | first `/dot` on `/one` | 108, 216, 432 | 1:4, 1:2, 1:1 |
| shutdown | the dot goes offline, or the page hides | 432, 216, 108 | 1:1, 1:2, 1:4 |
| notification | the halo first paints gas | 432, 540 | 1:1, 5:4 |
| wake | the tap that opens the microphone | 216, 432, 648 | 1:2, 1:1, 3:2 |
| done | speaking ends and the dot leaves speaking | 540, 648, 864 | 5:4, 3:2, 2:1 |
| approval | `/dot` `approval` becomes true | 432, 648 | 1:1, 3:2 |
| error | the page cannot open the microphone, or a turn fails | 216, 108 | 1:2, 1:4 |

540 = 432 × 5/4. 648 = 432 × 3/2. 864 = 432 × 2. 216 = 432 ÷ 2. 108 = 432 ÷ 4.

Silence on Galion One is opt-in. The key is `galion-one-silence`. `?silence=1` turns it on. While it is on, no Galion One cue starts.

These seven remain the Galion One interface tones. The Research set below does not replace them.

## Galion Research

A separate set, in `crates/galion-console/assets/research.html`, played only on `/research`. The gestures are not the seven above. The hum is the same two frequencies, held under the notes for the whole gesture.

`window.GALION_RESEARCH` lists each id, its notes, its ratios, `hum: [108, 216]`, `decay: 2.8`, and `bell: false`.

Authored playback, the same for every Research cue:

| parameter | value |
| --- | --- |
| oscillator | sine |
| attack | 0.04 s, from gain 0.0001 to the peak |
| decay | 2.8 s, exponential back to gain 0.0001 |
| note stagger | 0.28 s |
| note peak gain | 0.04 |
| hum peak gain | 0.02 |
| hum | 108 Hz and 216 Hz, from the start until the last note finishes its 2.8 s decay |

| id | when | Hz | ratio from 432 |
| --- | --- | --- | --- |
| ask | the question is committed | 432, 648, 540 | 1:1, 3:2, 5:4 |
| answer | a reply is shown | 648, 540, 432 | 3:2, 5:4, 1:1 |
| attached | a file is accepted into Sources | 288, 432 | 2:3, 1:1 |
| show | Show interface | 216, 540, 864 | 1:2, 5:4, 2:1 |
| hide | Hide interface | 864, 540, 216 | 2:1, 5:4, 1:2 |
| error | the ask fails, or a file is not attached | 648, 432, 288 | 3:2, 1:1, 2:3 |
| listen | Voice is pressed on | 432, 864 | 1:1, 2:1 |
| leave | Voice is pressed off | 864, 432 | 2:1, 1:1 |

288 = 432 × 2/3, the fifth below the reference. 2:3 is the inversion of 3:2.

No Research notes array is a Galion One notes array. Research `error` is 648, 432, 288 Hz. Galion One `error` is 216, 108 Hz.

### Silence

Silence is opt-in. The control is the Silence button on the desk. The persisted key is `galion-research-silence` (`1` on, `0` off). It is not `galion-one-silence`. `?silence=1` turns it on and stores that choice. While Silence is on, no Research cue starts. Cues already ringing are not cut.

### prefers-reduced-motion

`prefers-reduced-motion: reduce` stops new Research cues and holds the voice disc still. That preference is not written into `galion-research-silence`. The desk sets `data-motion="reduce"` and `data-resonance="off"`.

### Bells

Research cues are sines with a slow attack. They are not bells. A bell is reserved for window open, window close, window minimise, and window settle. Those four actions are not on this desk, so this guide gives them no frequency.
