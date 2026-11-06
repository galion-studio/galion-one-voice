# own-samantha-v2-hf-e-mild

speaker: samantha | baseline: false | clips: 12

| metric | value | source |
|---|---|---|
| true peak dBTP | 0.12 | rust (galion-board) |
| clipped samples | 3 | rust (galion-board) |
| clicks | 0 | rust (galion-board) |
| f99 Hz (ref) | 3695.0 | rust (galion-board) |
| ΔF0 Hz | 24.06 | rust (galion-board) |
| ΔF0 sd st | 0.24 | rust (galion-board) |
| Δenergy DR dB | 3.84 | rust (galion-board) |
| rate wps | 3.48 | rust (galion-board) |
| articulation | `{"error":"scorer ran but produced no parseable JSON"}` | rust |
| similarity | `{"error":"scorer ran but produced no parseable JSON"}` | rust |

## Persona target

| target | clone | goal | distance |
|---|---|---|---|
| pitch Hz | 196.72 | 178.0 | 18.72 |
| pace w/s | 3.48 | 2.82 | 0.66 |
| emotion | `"pending"` | calm/neutral | – |
| mouth expression | `{"bin":"/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/articulation/galion-artic/target/release/galion-artic","error":"scorer ran but produced no parseable JSON","source":"rust"}` | match original | – |
| HF (f99, Samantha) | `{"baseline_f99_hz":2612,"dist_hz":-305.0,"f99_hz":3695.0,"target_hz":4000}` | ≥4000 Hz | – |

| generation RTF / CPU | – / – | – |
