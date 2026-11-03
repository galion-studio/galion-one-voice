# Compiler

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The compiler takes the clone the Chatterbox child produced and writes the Galion voice file the Rust symphony runs. That file is the `.gq` student path and the Galion clip ABI: PCM the heartbeat can frame, plus the small header the stack needs to find the persona, the rate, and the hash.

Chatterbox stays the out-of-process CPU child that knows its own weights. The file that lands in the clip store, and the file `galion-heartbeat` and `galion-ml-voice` read, is Galion's.

## Gate

**PASS** when a fixture compiles and a round-trip reads the same PCM and the same persona id back. The primary path recorded in the fixture is the Galion voice file.

**FAIL** when the round-trip drops samples, changes the persona id, or leaves the Chatterbox container as the file the Rust path opens.

**BLOCKED** when there is no compiled artifact yet. That is the state of this landing: `galion-student/` is the distillation plan, and a `.gq` voice file is not in the tree.

## Where it sits

| Piece | Path |
|---|---|
| Heartbeat frames | `crates/galion-heartbeat` (24 kHz, 80 ms, 1920 samples) |
| Speech slot | `crates/galion-ml-voice` |
| Student plan | `galion-student/` |
| Clone child | [CLONE.md](CLONE.md), [CLONE-WORKER.md](../CLONE-WORKER.md) |
| Store | [CLIP-STORE.md](CLIP-STORE.md) |

A compiled file names its persona (`samantha` or `jarvis` for the test voices) and carries the sha256 of the reference it was built from. The Jarvis file keeps the hub texture. See [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md).

Record the gate in [SELF-EVAL.md](SELF-EVAL.md).

— Maciej Grajczyk
