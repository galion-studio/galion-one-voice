# Cloning V2 Scoreboard — FINAL

Generated: 1793905551 (unix s) by galion-board (Rust, rayon 2 threads). Pass 1.

READY scorer groups: []
Rust scorer binaries used: []

Source tags: R = Rust scorer, py = Python fallback (pre-existing JSON), board = Rust built-in in galion-board.

| voice | similarity | emotion | articulation | MOS | WER | true peak dBTP (board) | clips (board) | clicks (board) | f99 Hz (board) | ΔF0 Hz (board) | ΔF0sd st (board) | ΔenergyDR dB (board) | rate wps (board) | RTF |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| own-jarvis (baseline) | 0.7327 (py) | 12/12 (py) | – | 4.1418 (py) | 0.018 (py) | 0.16 | 3 | 0 | 6044.0 | 13.25 | 0.93 | -25.35 | 3.31 | – |
| own-jarvis-v2-sim-cleaned | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-jarvis-v2-sim-multi | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha (baseline) | 0.7072 (py) | 3/12 (py) | – | 3.7285 (py) | 0.05 (py) | 0.17 | 6 | 0 | 2612.0 | 17.82 | 0.39 | 3.05 | 3.49 | – |
| own-samantha-v2-hf-a-clean | – | – | – | – | – | 0.12 | 12 | 0 | 3121.0 | 20.35 | 0.08 | 4.74 | 3.55 | – |
| own-samantha-v2-hf-b-shelf | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-hf-c-bwe | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-hf-d-orig44 | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-cleaned | – | – | – | – | – | – | – | – | – | – | – | – | – | – |
| own-samantha-v2-sim-multi | – | – | – | – | – | – | – | – | – | – | – | – | – | – |

Per-voice detail: scoreboard/voices/<voice>.{json,md}. Board wall time 5.8s.
