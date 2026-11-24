# horbm-memory

HORBM's memory and the Galion knowledge cards, in one library and one CLI.

What the user told us (the RAM ring, a remembered episode, the diary) stays in `memory`. World cards stay in `knowledge`. A query returns at most four hits, split across those two lists. A miss on the cards is the answer `I don't know`.

```bash
cargo test
cargo run --release -- --knowledge /tmp/horbm-knowledge import ../data/cards
cargo run --release -- --knowledge /tmp/horbm-knowledge query "compatibilist"
cargo run --release -- --db /tmp/horbm.sqlite search "bm25"
```

Design and measured latency: [`../docs/MEMORY_STACK.md`](../docs/MEMORY_STACK.md).

MIT License. Copyright (c) 2026 Galion Labs.
