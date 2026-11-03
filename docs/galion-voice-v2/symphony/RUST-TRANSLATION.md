# Rust translation

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The symphony runs on the Galion Rust stack that already speaks. Translation means the compiler's Galion voice file is what those crates open. Chatterbox remains a CPU child, PyTorch, one process, for the clone step. The hot path after that is Rust.

Minimum Rust is 1.85.

| Crate | Role on the symphony |
|---|---|
| `galion-heartbeat` | One `step` per 80 ms frame. You, me, feeling. |
| `galion-emotion` | OKLCH weight `e`, prosody, consent. Default deny. |
| `galion-cascade` | English lane and PhoneLane. Off the tick. |
| `galion-asr-vosk` | Vosk child for the microphone. |
| `galion-ml-voice` | Piper child, flite fallback, clone-worker slot. |
| `galion-console` | The one circle, the turn worker, `/turn` and `/reply`. |
| `galion-render` | The mark and the loopback HTTP surface. |
| `galion-live-demo` | Headless 80 ms demo. |
| `galion-presence`, `galion-presence-eval` | Replay and the virtual user. |
| `galion-openmind` | Nerve notes. The Open Mind tree is not vendored. |
| `galion-student` | Distillation plan. A `.gq` voice file is not in the tree yet. |

The student path, when the weights exist, is `.gq`. The nerve notes the voice side already emits are `voice.frame_miss`, `voice.fallback`, and `voice.backend_up`. Hermes is a side band for problem-solving. It is off the 80 ms tick. See [OPENMIND-NERVE-CORTEX.md](../OPENMIND-NERVE-CORTEX.md).

```bash
cargo test --offline -p galion-heartbeat
cargo test --offline -p galion-emotion
cargo test --offline --workspace
```

## Gate

**PASS** when a Galion voice file from [COMPILER.md](COMPILER.md) loads through the Rust path and the PyTorch process is still a child, with no torch linked into these crates.

**FAIL** when the runtime path opens a Chatterbox container as its primary voice file, or when a Python interpreter sits inside `Heartbeat::step`.

**BLOCKED** while the compiler has not emitted a file. The crates build and test today. The translated artifact is the missing piece.

— Maciej Grajczyk
