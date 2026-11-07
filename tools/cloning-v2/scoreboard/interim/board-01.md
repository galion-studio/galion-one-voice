# Cloning V2 Scoreboard — interim

Generated: 1793905754 (unix s) by galion-board (Rust, rayon 2 threads). Pass 1.

READY scorer groups: []
Rust scorer binaries used: []

Source tags: R = Rust scorer, py = Python fallback (pre-existing JSON), board = Rust built-in in galion-board.

| voice | similarity | emotion | articulation | MOS | WER | true peak dBTP (board) | clips (board) | clicks (board) | f99 Hz (board) | ΔF0 Hz (board) | ΔF0sd st (board) | ΔenergyDR dB (board) | rate wps (board) | RTF |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| own-jarvis (baseline) | 0.7327 (py) | 12/12 (py) | – | 4.1418 (py) | 0.018 (py) | 0.16 | 3 | 0 | 6044.0 | 13.25 | 0.93 | -25.35 | 3.31 | – |
| own-jarvis-v2-sim-cleaned | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-sim-multi | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-sim-rs-cleaned | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-sim-rs-multi | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha (baseline) | 0.7072 (py) | 3/12 (py) | – | 3.7285 (py) | 0.05 (py) | 0.17 | 6 | 0 | 2612.0 | 17.82 | 0.39 | 3.05 | 3.49 | – |
| own-samantha-v2-hf-a-clean | – | – | – | – | – | 0.17 | 13 | 0 | 3080.0 | 19.43 | 0.26 | 5.15 | 3.5 | – |
| own-samantha-v2-hf-b-shelf | – | – | – | – | – | -3.21 | 0 | 0 | 4379.0 | 17.43 | -0.34 | 1.09 | 4.09 | – |
| own-samantha-v2-hf-c-bwe | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-hf-d-orig44 | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-cleaned | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-multi | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-rs-cleaned | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-rs-multi | – | – | – | – | – | – | – | – | – | – | – | – | – | – |

## Persona target distances (Sam: 178 Hz, 2.82 w/s, HF; Jar: 129 Hz, 2.63 w/s; both calm/neutral, mouth match)

| voice | pitch Hz (Δ) | pace w/s (Δ) | emotion vs calm | mouth match | HF f99 Hz (Δ to 4 kHz) |
|---|---|---|---|---|---|
| own-jarvis | 138.9 (9.9) | 3.31 (0.68) | 12/12 (py) | pending | n/a |
| own-jarvis-v2-sim-cleaned | – (–) | – (–) | – | pending | n/a |
| own-jarvis-v2-sim-multi | – (–) | – (–) | – | pending | n/a |
| own-jarvis-v2-sim-rs-cleaned | – (–) | – (–) | – | pending | n/a |
| own-jarvis-v2-sim-rs-multi | – (–) | – (–) | – | pending | n/a |
| own-samantha | 190.48 (12.48) | 3.49 (0.67) | 3/12 (py) | pending | 2612.0 (-1388.0) |
| own-samantha-v2-hf-a-clean | 192.1 (14.1) | 3.5 (0.68) | – | pending | 3080.0 (-920.0) |
| own-samantha-v2-hf-b-shelf | 190.09 (12.09) | 4.09 (1.27) | – | pending | 4379.0 (379.0) |
| own-samantha-v2-hf-c-bwe | – (–) | – (–) | – | pending | – (–) |
| own-samantha-v2-hf-d-orig44 | – (–) | – (–) | – | pending | – (–) |
| own-samantha-v2-sim-cleaned | – (–) | – (–) | – | pending | – (–) |
| own-samantha-v2-sim-multi | – (–) | – (–) | – | pending | – (–) |
| own-samantha-v2-sim-rs-cleaned | – (–) | – (–) | – | pending | – (–) |
| own-samantha-v2-sim-rs-multi | – (–) | – (–) | – | pending | – (–) |

Per-voice detail: scoreboard/voices/<voice>.{json,md}. Board wall time 6.7s.
