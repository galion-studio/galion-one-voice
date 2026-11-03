# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Produced by `cargo run -p galion-symphony -- eval` on Rust 1.85.1.
`cargo test -p galion-symphony` passed (5 unit tests, 1 pack test).

Section: Rust translation of the clone interchange into the Galion voice pack and clip ABI.
A missing owner file is BLOCKED. A broken gate is FAIL. Infra is PASS only when Fails is 0 and the `.gq` and clip gates passed.

| Gate | Status | Detail |
|---|---|---|
| gq-roundtrip | PASS | GqFile::verify_all and payload bytes match the fixture |
| namespace | PASS | every .gq name is in sym.; the interchange name stays in the manifest |
| output-clean | PASS | pack directory holds .gq, SYMPHONY.txt, SHA256SUMS, and clip.gclip |
| clip-abi | PASS | GALIONCL payload is the WAV; stereo samples stay distinct; hub chunk kept |
| texture-keep | PASS | jarvis policy keep; clicks, alarms, ambience declared; strip flag 0 |
| clip-not-lane | PASS | clip stays 48 kHz stereo; the heartbeat lane stays 24 kHz |
| presence | PASS | schema galion-presence-report/v1, bands you/me/feeling, no engagement field |
| empathy | PASS | live OKLCH weight; scripted empathy false |
| clock | PASS | 80 ms, 24 kHz, 1920 samples, same constants as galion-heartbeat and galion-ml-voice |
| foreign-refused | PASS | foreign weight magic and refused extensions do not enter the pack |
| cpu-budgets | PASS | EPYC 9655P, CX23, CX43; session about 1 vCPU, burst 4, 0.65–1.15 GB, non-inference ceiling 1%, tick ceiling 800 µs |
| pins-table | PASS | five interchange pins match scripts/clone/PINS.sha256 |
| heartbeat-pause | PASS | on-time 1920-sample frame runs; a late frame and a short buffer pause |
| landing-samantha-wav | BLOCKED | docs/galion-voice-v2/personas/refs/samantha.wav is not on this machine |
| landing-jarvis-wav | BLOCKED | docs/galion-voice-v2/personas/refs/jarvis.wav is not on this machine |
| landing-clone-weights | BLOCKED | weights/clone/shared/ve.safetensors is not on this machine |
| clone-generate | BLOCKED | this crate does not spawn the clone child and does not call generate; weights are not in the tree |

Infra: PASS

Fails: 0

Fixture translate: 4347 µs. Fixture load: 626 µs. The 80 ms bar is the heartbeat frame (`late` pauses). It is not the wall clock of packing a weight file.

`LOCKS.toml` was not edited.

Signed: Maciej Grajczyk
