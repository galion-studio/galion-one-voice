# TTFA

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Time to first audio is the wait until sound exists. Three clocks stay separate. A header, a JSON byte, or a TTFB is not audio.

| Clock | Start | Stop | What a number means |
|---|---|---|---|
| Flash, this CPU | the clone child accepted one JSON line | the first PCM sample is in the parent buffer | product claim only when `engine` is `model` |
| Virtual user | end of inject | a reply WAV is in hand, plus leading silence | `ttfa_kind` = `loopback-full-utterance` |
| Web / PSTN targets | user speech offset | first non-silent agent audio | desk targets in [80MS.md](80MS.md), not yet this machine |

There is no Chatterbox model named Flash. Upstream `generate()` returns when the utterance is finished. The owner's flash question is the first clock: CPU, this machine, no cloud. The plan page is [FLASH-EVAL.md](../FLASH-EVAL.md).

## Run

Default. CI uses this. The line has `measured: false` and no `ttfa_ms`.

```bash
scripts/eval/flash-ttfa.sh
```

Opt-in pipe clock. `product_claim` stays false. `engine` is `protocol-stub`.

```bash
GALION_FLASH_EVAL=1 GALION_CLONE_STUB_PCM=1 scripts/eval/flash-ttfa.sh
```

Loopback utterance clock, filled only when Piper or flite wrote a WAV:

```bash
scripts/eval/virtual-user.sh
```

The report is `galion-gates/results/presence-eval/latest.json`. With no speech backend, `ttfa_ms` is null and `ttfa_status` is `unmeasured`. Doc: [PRESENCE-EVAL.md](../../eval/PRESENCE-EVAL.md).

A product number is copied into [SELF-EVAL.md](SELF-EVAL.md) when `engine` is `model` and the command, the machine, and the persona are in the evidence. A stub clock can be filed as evidence of the pipe. It is labeled stub.

## Gate

**PASS** when the default harness prints `measured: false` with no `ttfa_ms`, and any number that is filed carries its engine label.

**FAIL** when a stub or an unmeasured run is written up as a product TTFA.

**BLOCKED** when a model engine has not spoken, so the product clock has no sample. The harness itself can still **PASS** on the default line.

— Maciej Grajczyk
