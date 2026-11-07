# Cloning V2 Scoreboard — interim

Generated: 1793907805 (unix s) by galion-board (Rust, rayon 2 threads). Pass 3.

READY scorer groups: ["articulation", "ecapa"]
Rust scorer binaries used: ["galion-ecapa=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/ecapa/target/release/galion-ecapa", "galion-artic=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/articulation/galion-artic/target/release/galion-artic"]

Source tags: R = Rust scorer, py = Python fallback (pre-existing JSON), board = Rust built-in in galion-board.

| voice | similarity | emotion | articulation | MOS | WER | true peak dBTP (board) | clips (board) | clicks (board) | f99 Hz (board) | ΔF0 Hz (board) | ΔF0sd st (board) | ΔenergyDR dB (board) | rate wps (board) | RTF |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| own-jarvis (baseline) | err(R) | 12/12 (py) | err(R) | 4.1418 (py) | 0.018 (py) | 0.16 | 3 | 0 | 6044.0 | 13.25 | 0.93 | -25.35 | 3.31 | – |
| own-jarvis-v2-emo-ln | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-emo-r10 | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-emo-r6 | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-pipe | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-sim-cleaned | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-sim-multi | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-sim-rs-cleaned | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-sim-rs-multi | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha (baseline) | err(R) | 3/12 (py) | err(R) | 3.7285 (py) | 0.05 (py) | 0.17 | 6 | 0 | 2612.0 | 17.82 | 0.39 | 3.05 | 3.49 | – |
| own-samantha-v2-emo-ln | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-emo-r2 | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-emo-r9 | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-hf-a-clean | err(R) | – | err(R) | – | – | 0.17 | 13 | 0 | 3080.0 | 19.43 | 0.26 | 5.15 | 3.5 | – |
| own-samantha-v2-hf-b-shelf | err(R) | – | err(R) | – | – | 0.26 | 3 | 0 | 4572.0 | 27.39 | 0.22 | 1.11 | 3.74 | – |
| own-samantha-v2-hf-c-bwe | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-hf-d-orig44 | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-hf-e-mild | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-pipe | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-cleaned | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-multi | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-rs-cleaned | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-rs-multi | err(py) | – | err(py) | – | – | – | – | – | – | – | – | – | – | – |

## Persona target distances (Sam: 178 Hz, 2.82 w/s, HF; Jar: 129 Hz, 2.63 w/s; both calm/neutral, mouth match)

| voice | pitch Hz (Δ) | pace w/s (Δ) | emotion vs calm | mouth match | HF f99 Hz (Δ to 4 kHz) |
|---|---|---|---|---|---|
| own-jarvis | 138.9 (9.9) | 3.31 (0.68) | 12/12 (py) | err(R) | n/a |
| own-jarvis-v2-emo-ln | – (–) | – (–) | – | err(py) | n/a |
| own-jarvis-v2-emo-r10 | – (–) | – (–) | – | err(py) | n/a |
| own-jarvis-v2-emo-r6 | – (–) | – (–) | – | err(py) | n/a |
| own-jarvis-v2-pipe | – (–) | – (–) | – | err(py) | n/a |
| own-jarvis-v2-sim-cleaned | – (–) | – (–) | – | err(py) | n/a |
| own-jarvis-v2-sim-multi | – (–) | – (–) | – | err(py) | n/a |
| own-jarvis-v2-sim-rs-cleaned | – (–) | – (–) | – | err(py) | n/a |
| own-jarvis-v2-sim-rs-multi | – (–) | – (–) | – | err(py) | n/a |
| own-samantha | 190.48 (12.48) | 3.49 (0.67) | 3/12 (py) | err(R) | 2612.0 (-1388.0) |
| own-samantha-v2-emo-ln | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-emo-r2 | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-emo-r9 | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-hf-a-clean | 192.1 (14.1) | 3.5 (0.68) | – | err(R) | 3080.0 (-920.0) |
| own-samantha-v2-hf-b-shelf | 200.05 (22.05) | 3.74 (0.92) | – | err(R) | 4572.0 (572.0) |
| own-samantha-v2-hf-c-bwe | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-hf-d-orig44 | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-hf-e-mild | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-pipe | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-sim-cleaned | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-sim-multi | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-sim-rs-cleaned | – (–) | – (–) | – | err(py) | – (–) |
| own-samantha-v2-sim-rs-multi | – (–) | – (–) | – | err(py) | – (–) |

Per-voice detail: scoreboard/voices/<voice>.{json,md}. Board wall time 2057.5s.
