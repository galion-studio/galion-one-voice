# Eureka

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Champion: a conversation is one heartbeat, not a pipeline. One 80 ms clock. Three bands — you, me, feeling. Cheap CPU. The heartbeat tests itself.

## Standards

1. Demo, spec, test, and teacher are one artifact. The same run proves the section on every change.
2. The harness exists before the weight download.
3. The heartbeat tests itself. Presence replay and the nightly ledger decide what is deployed. The ledger for this stack is [SELF-EVAL.md](SELF-EVAL.md).
4. `Heartbeat::step` returns on the frame it was given. A late frame is a pause. The next frame does not catch up. See [HEARTBEAT.md](HEARTBEAT.md).
5. The frame is 80 ms. See [80MS.md](80MS.md).
6. Question the part, delete what the voice does not need, simplify what remains, speed the hot path, then automate the check.
7. Three eval loops: inner (the turn), middle (the session), outer (the release). The model does not grade itself without a human-calibrated anchor. An honest gap is a success.
8. A score is a number in a log.

## Where the champion already lives

`crates/galion-heartbeat` is the one step. `galion-emotion` is the feeling band. `scripts/run-live-demo.sh` and `galion-heartbeat-presence` are the harness. Rika, when consent bits `sense` and `adapt` are open, leans in on the feeling band. Both bits default deny. With consent closed, the you-band PCM is not read for affect.

## Gate

**PASS** when presence replay and the heartbeat harness have been run for the commit being scored, and the summary line is pasted into [SELF-EVAL.md](SELF-EVAL.md).

**FAIL** when the harness is skipped and a section is still marked complete, or when a score is a sentence with no number.

**BLOCKED** when the presence specs file cannot be read. The default path is the one `galion-heartbeat-presence` prints.

```bash
cargo run --offline -q -p galion-heartbeat --bin galion-heartbeat-presence
cargo test --offline -p galion-heartbeat
```

— Maciej Grajczyk
