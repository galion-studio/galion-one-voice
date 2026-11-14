# Sound language

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.

The spelling is **Galion**. These are interface tones. Each one is a sine synthesized in the browser with Web Audio. No audio file. No voice.

MIT License

Copyright (c) 2026 Maciej Grajczyk

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Lattice

The reference is 432 Hz, just intonation. The ratios on this desk are 1:1, 5:4, 3:2, and 2:1. The warm hum is 216 Hz (1:2) and 108 Hz (1:4). The hertz below are those products. They are the tuning standard, not a measured spectrum.

| Ratio | Interval | Hz |
|---|---|---:|
| 1:4 | hum, two octaves down | 108 |
| 1:2 | hum, octave down | 216 |
| 1:1 | unison | 432 |
| 5:4 | major third | 540 |
| 3:2 | perfect fifth | 648 |
| 2:1 | octave | 864 |

432 × 5/4 = 540. 432 × 3/2 = 648. 432 × 2 = 864. 432 / 2 = 216. 432 / 4 = 108.

## Galion One

Seven tones, in `crates/galion-console/assets/v1-screen.html`. Each note is one sine. The hum, 108 Hz then 216 Hz, holds under the figure. `FUND` in that page is 432.

| Cue | id | Notes (Hz) |
|---|---|---|
| Boot | boot | 108, 216, 432 |
| Shutdown | shutdown | 432, 216, 108 |
| Notification | notification | 432, 540 |
| Wake | wake | 216, 432, 648 |
| Task done | done | 540, 648, 864 |
| Approval needed | approval | 432, 648 |
| Error | error | 216, 108 |

Silence starts no oscillator. It is on when the query is `silence=1`, when `localStorage` key `galion-one-silence` is `1`, or when the Silence control (or the S key, away from a text field) is pressed. The page sets `data-silence="on"` and `data-resonance="off"`.

## Galion Research

Five dedicated cues on the `/research` desk, in `crates/galion-console/assets/research.html`. One sine each. The page uses these hertz. Ask sent, thinking, and answer ready keep the pitches the desk already used for the ask, the helper, and the return. Error sits on the hum at 216 Hz, the same pitch the Galion One error starts on. Source attached uses the octave.

| Cue | id | Ratio | Hz |
|---|---|---|---:|
| Ask sent | ask-sent | 1:1 | 432 |
| Thinking | thinking | 5:4 | 540 |
| Answer ready | answer-ready | 3:2 | 648 |
| Source attached | source-attached | 2:1 | 864 |
| Error | error | 1:2 | 216 |

Ask sent and thinking play when the question is posted. Thinking starts 0.2 s after ask sent, so the two sines do not stack. Answer ready plays when a reply is in hand, including an answer whose standing is Unknown. Error plays when the ask fails, when the standing is Unreachable, Denied, Timeout, or NotFound, and when a file does not attach. Source attached plays once each time a file lands in Sources.

The same Silence mode stops all five. The hide-interface toggle (`#ui-toggle`) only hides the rail and the work piles (`data-ui="minimal"`). It does not mute these five. Hiding the interface plays hide at 216 Hz. Showing it plays show at 864 Hz. Those two, and the voice-button cues below, also stop when Silence is on.

| Cue | id | Ratio | Hz |
|---|---|---|---:|
| Listen | listen | 1:1 | 432 |
| Hide interface | hide | 1:2 | 216 |
| Show interface | show | 2:1 | 864 |
| Leave | leave | 1:4 | 108 |

Hide shares 216 Hz with error. Show shares 864 Hz with source attached. Listen shares 432 Hz with ask sent. They are separate events on a six-note lattice.
