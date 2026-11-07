# own-samantha-v2-hf-c-bwe

speaker: samantha | baseline: false | clips: 12

| metric | value | source |
|---|---|---|
| true peak dBTP | 0.19 | rust (galion-board) |
| clipped samples | 32 | rust (galion-board) |
| clicks | 0 | rust (galion-board) |
| f99 Hz (ref) | 2765.0 | rust (galion-board) |
| ΔF0 Hz | 17.99 | rust (galion-board) |
| ΔF0 sd st | 0.32 | rust (galion-board) |
| Δenergy DR dB | 4.11 | rust (galion-board) |
| rate wps | 3.47 | rust (galion-board) |
| articulation | `{"error":"scorer ran but produced no parseable JSON"}` | rust |
| similarity | `{"error":"scorer ran but produced no parseable JSON"}` | rust |

## Persona target

| target | clone | goal | distance |
|---|---|---|---|
| pitch Hz | 190.65 | 178.0 | 12.65 |
| pace w/s | 3.47 | 2.82 | 0.65 |
| emotion | `"pending"` | calm/neutral | – |
| mouth expression | `{"bin":"/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/articulation/galion-artic/target/release/galion-artic","error":"scorer ran but produced no parseable JSON","source":"rust"}` | match original | – |
| HF (f99, Samantha) | `{"baseline_f99_hz":2612,"dist_hz":-1235.0,"f99_hz":2765.0,"target_hz":4000}` | ≥4000 Hz | – |

| generation RTF / CPU | – / – | – |
