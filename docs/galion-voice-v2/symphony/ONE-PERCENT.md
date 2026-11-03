# One-percent

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Non-inference work on the host stays at or under 1% of the CPU. The tick itself, inside the 80 ms frame, has p99 well under 800 µs. A late frame pauses. It does not glitch forward to catch up.

Inference (the clone child, Piper, Vosk, a future `.gq` student) is outside that 1%. It runs off `Heartbeat::step`. The 1% is the engine around it: the frame step, the circle, the accounts.

## What the tree already counts

`OrbEngine::step` keeps an `OrbAccount` ([ONE-UI.md](../ONE-UI.md), [ANIMATION-ENGINE.md](../ANIMATION-ENGINE.md)):

| Field | Meaning |
|---|---|
| `frames` | accepted steps |
| `dt_s` | seconds integrated on the last call, `0` if the call was refused |
| `settle_err` | larger channel error against the last accepted target |
| `cost_hint` | `48` after an accepted step, else `0` |

`48` is a fixed hint for a few dozen flops. It is not a measured time. `GET /dot` adds `"engine"` from one Luti step. The heartbeat hot path allocates nothing and does not read the clock. See [HEARTBEAT.md](HEARTBEAT.md).

## Gate

**PASS** when a host sample shows non-inference CPU at or under 1%, and a tick histogram shows p99 well under 800 µs, on the machine named in the evidence.

**FAIL** when the tick blocks, allocates, or a late frame is spliced into the next one to hide the miss.

**BLOCKED** when the only number on hand is `cost_hint`. That hint is not the host sample. This landing has the account and the budget. It does not yet have the host sample.

The frame the budget sits inside is [80MS.md](80MS.md). Record the sample in [SELF-EVAL.md](SELF-EVAL.md).

— Maciej Grajczyk
