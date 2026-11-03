# Galion Voice V2 symphony

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

Champion: a conversation is one heartbeat, not a pipeline.
Demo = Spec = Test = Teacher. Harness before model.

One 80 ms tick carries you, me, and feeling. Non-inference work on that tick stays inside 1% of the frame (800 µs). A late frame is a pause. `Heartbeat::step` does not block and does not allocate.

The host plan is CPU only, no GPU. The flagship app node is an AMD EPYC 9655P. Plus is CX23-class. Ultra is CX43-class. Those names are a sizing plan, not a measurement of the machine you are on. See [CPU.md](CPU.md).

## How to run

| Environment | Runbook | First command |
|---|---|---|
| Local demo | [LOCAL-DEMO.md](LOCAL-DEMO.md) | `bash scripts/run-live-demo.sh` |
| Clone worker | [CLONE-WORKER.md](CLONE-WORKER.md) | `scripts/clone/chatterbox_worker.py` with `GALION_CLONE_STUB_PCM=1` |
| One dot | [ONE-DOT.md](ONE-DOT.md) | `bash scripts/run-v1-screen.sh` |
| Clip store | [CLIP-STORE.md](CLIP-STORE.md) | leave `GALION_CLIP_STORE` unset until an owner path exists |
| Self-eval | [GATES.md](GATES.md) | `bash scripts/symphony/self-eval.sh` |

The tick bar is [LATENCY.md](LATENCY.md). The ledger is [SELF-EVAL.md](SELF-EVAL.md). A section there is complete only when its check exits 0.

— Maciej Grajczyk
