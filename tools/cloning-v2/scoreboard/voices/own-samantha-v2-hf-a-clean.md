# own-samantha-v2-hf-a-clean

speaker: samantha | baseline: false | clips: 12

| metric | value | source |
|---|---|---|
| true peak dBTP | 0.17 | rust (galion-board) |
| clipped samples | 13 | rust (galion-board) |
| clicks | 0 | rust (galion-board) |
| f99 Hz (ref) | 3080.0 | rust (galion-board) |
| ΔF0 Hz | 19.43 | rust (galion-board) |
| ΔF0 sd st | 0.26 | rust (galion-board) |
| Δenergy DR dB | 5.15 | rust (galion-board) |
| rate wps | 3.5 | rust (galion-board) |
| articulation | `{"error":"scorer ran but produced no parseable JSON"}` | rust |
| similarity | `{"error":"scorer ran but produced no parseable JSON"}` | rust |

## Persona target

| target | clone | goal | distance |
|---|---|---|---|
| pitch Hz | 192.1 | 178.0 | 14.1 |
| pace w/s | 3.5 | 2.82 | 0.68 |
| emotion | `"pending"` | calm/neutral | – |
| mouth expression | `{"bin":"/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/articulation/galion-artic/target/release/galion-artic","error":"scorer ran but produced no parseable JSON","source":"rust"}` | match original | – |
| HF (f99, Samantha) | `{"baseline_f99_hz":2612,"dist_hz":-920.0,"f99_hz":3080.0,"target_hz":4000}` | ≥4000 Hz | – |

| generation RTF / CPU | – / – | – |
