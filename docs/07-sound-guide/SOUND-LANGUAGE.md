# Sound language

Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.

Interface tones only. Not speech, not a voice, and not a recording. The pages synthesise them with WebAudio sine oscillators. There is no audio file.

The reference is **432 Hz**. The ratios used here are just intonation **2:1**, **3:2**, and **5:4**, plus the octaves those ratios produce. The soft hum is **216 Hz** and **108 Hz**.

These hertz values are the frequencies the oscillators are given. They are the arithmetic below, not a measurement of a room or a loudspeaker.

| Ratio from 432 Hz | Hz |
|---|---|
| 1:4, two octaves down (2:1 twice) | 108 |
| 1:2, one octave down | 216 |
| 1:1 | 432 |
| 5:4, major third | 540 |
| 3:2, perfect fifth | 648 |
| 2:1, one octave up | 864 |

432 × 2 = 864. 432 × 3/2 = 648. 432 × 5/4 = 540. 432 / 2 = 216. 432 / 4 = 108. Each of those products is an integer, and each factor is exact in binary floating point (2, 3/2 = 1.5, 5/4 = 1.25, 1/2, 1/4).

Anything off this list, including 288 Hz (432 × 2/3), is not a cue.

## How a tone is built

Two families. Both are sines.

**Resonance.** Notes play in order, one every 140 ms. Each note rises in 20 ms and lasts 200 ms. Under the whole figure, the hum (108 Hz and 216 Hz) rises in 30 ms and lasts `notes × 140 ms + 240 ms`. Linear gain peaks: note **0.03**, hum **0.01**. The envelope starts and ends at 0.0001.

**Bell.** The strike and its partial start together. The strike rises in 5 ms and lasts 420 ms, peak **0.03**. The partial uses the same envelope at peak **0.01**. The hum under a bell lasts 480 ms at peak **0.01**. A bell is not a stepped melody.

Peaks are linear gain on the WebAudio destination. They are the code's default levels. This note does not claim a LUFS figure.

## Galion One, `/one`

Source: `crates/galion-console/assets/v1-screen.html`. Seven cues. Each one is resonance, with the hum at 108 Hz and 216 Hz. Note peak 0.05, hum peak 0.016, step 120 ms. Those levels belong to this page.

| Cue | Notes (Hz) | Ratios from 432 Hz |
|---|---|---|
| boot | 108, 216, 432 | 1:4, 1:2, 1:1 |
| shutdown | 432, 216, 108 | 1:1, 1:2, 1:4 |
| notification | 432, 540 | 1:1, 5:4 |
| wake | 216, 432, 648 | 1:2, 1:1, 3:2 |
| done | 540, 648, 864 | 5:4, 3:2, 2:1 |
| approval | 432, 648 | 1:1, 3:2 |
| error | 216, 108 | 1:2, 1:4 |

Silence on this page is the `#silence` control. It reads `localStorage` key `galion-one-silence` (`1` on, `0` off) and the query `silence=1`. The `s` key toggles it. While silence is on, `data-silence` is `on`, `data-resonance` is `off`, and no cue is scheduled.

## Galion Research, `/research`

Source: `crates/galion-console/assets/research.html`. The desk uses the same Silence control and the same `galion-one-silence` key. On this page the `s` key toggles silence only when focus is not in a field, so typing the letter still works. A silenced desk schedules nothing, including the window bell on open.

Research events are resonance. Window events are bells. A research event never strikes a bell. A window event never plays a research melody.

### Resonance

Hum on every row: **108 Hz** and **216 Hz**. Note peak 0.03. Hum peak 0.01.

| Cue | id | When it plays | Notes (Hz) | Ratios from 432 Hz |
|---|---|---|---|---|
| Question sent | `question-sent` | Enter commits a non-empty question | 432, 648, 864 | 1:1, 3:2, 2:1 |
| Thinking | `thinking` | 500 ms later, if that ask has not returned | 540 | 5:4 |
| Answer ready | `answer-ready` | The ask returns a status other than Unreachable, Denied, Timeout, or NotFound | 432, 540, 648 | 1:1, 5:4, 3:2 |
| Source attached | `source-attached` | A source post returns the source list | 216, 432 | 1:2, 1:1 |
| Source cited | `source-cited` | 520 ms after answer ready, and only when that answer has at least one citation | 648, 540 | 3:2, 5:4 |
| Error | `error` | The ask is not ok, the status is Unreachable, Denied, Timeout, or NotFound, the source post fails, or the file reader fails | 216, 108 | 1:2, 1:4 |

The voice button and Show interface do not play a cue.

### Bells

A bell is only one of these four. Hum on every row: **108 Hz** and **216 Hz**. Strike peak 0.03. Partial peak 0.01. Hum peak 0.01.

| Cue | id | When it plays | Strike, then partial (Hz) | Ratios from 432 Hz |
|---|---|---|---|---|
| Window open | `window-open` | The first `pageshow` | 864, 432 | 2:1, 1:1 |
| Window close | `window-close` | `pagehide` | 432, 216 | 1:1, 1:2 |
| Window minimise | `window-minimise` | The document becomes hidden, unless close already ran | 540, 216 | 5:4, 1:2 |
| Window settle | `window-settle` | The document becomes visible again after a minimise | 648, 432 | 3:2, 1:1 |

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.
