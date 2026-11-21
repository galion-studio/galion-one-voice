<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# HORBM layer 3 — Empathy

Package `galion-sam-empathy`. Directory `voice-stack/empathy/`. It is a member of the nested `voice-stack` workspace. The root `cargo test --workspace` does not build that workspace.

Empathy is a weight on tone and prosody. It is not a sentence. The live signals come from `voice-stack/emotion-in` (PR 230). The weight is applied by `galion_sam_prosody::apply_empathy`.

Rest matches `voice-stack/local-server/PROTOCOL.md`: arousal 0.3, valence 0.25, dominance 0.5. The tick is `LOCKS.toml` `frame_ms` (80). A mood update is compared with that 80 ms bar in `tests/budget.rs`. The test prints `MEASURE` lines. This file does not copy a duration from a run.

CPU only. No LLM, no paid API, no network, no audio file.

## Who calls it

The pipeline implements [`PipelineLayer`] and calls `tick` once per 80 ms.

The layer 5 router (core chat engine) implements against [`RouterLayer`]. Before a lookup it calls `prepare_lookup`. That rewrites pronouns with the names the state tracker stored (`NameSource`), and returns intent, the current mood, `e`, and the open / met / dropped counts. The clock does not move.

```rust
use galion_sam_empathy::{Empathy, InboundSignal, PronounClass, RouterLayer, TickIn};

let mut layer = Empathy::new();
// State tracker (layer 4) fills names. The router does not invent them.
layer.names_mut().note("Maciej", PronounClass::He);

layer.tick(TickIn {
    inbound: Some(InboundSignal {
        arousal: 0.8,
        valence: -0.6,
        dominance: 0.7,
        confidence: 0.6,
    }),
    asr: "I am angry",
    asr_final: true,
    invite_name: false,
    frame_ms: 80,
});

let prep = layer.prepare_lookup("ask him about the gate");
// prep.query == "ask Maciej about the gate"
// prep.query_intent, prep.mood, prep.e, prep.open / met / dropped
let _ = prep;
```

`prepare_lookup_with` takes a `NameSource` for one call when the tracker keeps the book itself.

`TickOut.prosody` is the weight already applied. `e == 0` matches `galion_sam_prosody::map`. There is no reply field. `named` is one word, and only when the caller invited a name (`invite_name`, or a phrase such as "how do I feel").

`InboundSignal::from_reading` takes a `galion_emotion_in::Reading`. Pass `None` when sense is denied.

## Wants

`WantState` is `Open`, `Met`, or `Dropped`. A final ASR line can open one (`I want …`, `I need …`, `I wish …`), mark the newest open want met, or drop it. The text is the caller's words, clipped. It is not spoken.

## Windows

On the owner's laptop the only drive is `G:`. `windows_galion_home` is `G:\galion`. `require_g_drive` refuses any other drive letter. `GALION_HOME` on Windows must be on `G:`.

From that checkout:

```bat
cd /d G:\galion\galion-one-voice\voice-stack\empathy
cargo test --offline
```

## Tests

`cargo test -p galion-sam-empathy --manifest-path voice-stack/Cargo.toml`

- Fixture tables for intent, query rewrite, and wants.
- Emotion-in readings feed a tick. A low-confidence pad does not move rest.
- Anger is answered (valence stays positive) and raises `e`. Joy leaves `e` at 0.
- A feeling name appears only when invited.
- Source text has no stock comfort line.
- The budget test measures mood updates and full ticks and fails if p95 or the max sample is over 80 ms. The `MEASURE` line is the measurement.
