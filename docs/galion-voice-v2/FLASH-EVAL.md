# V2-S14 · Flash eval (CPU time to first audio)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A plan and a harness stub. Not a measurement.

The spelling is **Galion**. The product is Galion Voice.

There is no Chatterbox model named Flash. Upstream `generate()` returns
when the utterance is finished. PR 528 (open when `VOICE-CLONING.md` was
written) proposes `ChatterboxTurboTTS.stream()` so CPU playback can start
before the utterance ends. The owner's "Flash streaming eval" is that
question: how long until the first PCM sample, on CPU, with no cloud.

## What the clock is

One number, when a run happens:

- start: the worker has accepted one JSON line (`text`, model, persona)
- stop: the first PCM sample is in the parent's buffer
- machine: this CPU, not a phone, not a VPS, not PSTN
- label: `engine` from the child (`protocol-stub`, `unwired`, or a future `model`)

`product_claim` is true only for `engine=model`. This tree never sets that
engine, because generate is not wired. A protocol-stub clock is a test of
the pipe. It is not a Flash result. Do not copy a number into `LEDGER.md`
or a demo until a model run is actually recorded there.

No reference clip is sent. Film audio stays on the rights hold.

## Harness

`scripts/eval/flash-ttfa.sh` runs `examples/flash_ttfa.rs`.

Default (CI, and any shell that did not opt in):

```text
{"measured":false,"product_claim":false,"engine":null,"note":"GALION_FLASH_EVAL is not 1; no TTFA number"}
```

There is no `ttfa_ms` key. The script fails if one appears without
`GALION_FLASH_EVAL=1`.

A later opt-in, still not a product claim:

```text
GALION_FLASH_EVAL=1 GALION_CLONE_STUB_PCM=1 scripts/eval/flash-ttfa.sh
```

That arms the protocol stub. The JSON may then contain `ttfa_ms` and
`"engine":"protocol-stub"` and `"product_claim":false`.

If the flag is set and the weights are absent and the stub is not armed,
the harness still prints `measured: false` and no number. It does not
download. `scripts/clone/fetch-clone-weights.sh` is a separate step and
refuses unless `GALION_CLONE_FETCH=1`.

## What a real run would add, later

1. Pinned safetensors on disk, fetched with `GALION_CLONE_FETCH=1` from release `clone-weights-v1` (`CLONE-WEIGHTS.md`).
2. The child actually calls generate, still out of process, still with no reference clip until rights are cleared.
3. The stderr engine line says `model`.
4. Three CPU runs, cold and warm, phrase fixed, written into `LEDGER.md` with the host and the model id.
5. Still no film audio. Still no celebrity demo.

Until those lines exist, this document's only number is the absence of one.

## Stub versus hard

A run of this page, including a protocol-stub clock, is **STUB-PASS**.
It is not **HARD-PASS**. `GALION_CLONE_STUB_PCM=1` and
`GALION_CLONE_STUB_FRAMES=1` never yield **HARD-PASS**. `product_claim`
is true only for PCM whose engine is `chatterbox-nano` or `nano`.
The hard loop is `scripts/eval/v2-hard-loop.sh`, written up in
`HARD-LOOP.md` and `HARD-REAL.md`. Fetch stays
`GALION_CLONE_WEIGHTS_SOURCE=hf` or `release`. The harness does not
call it. The Admin UI check is `ADMIN-UI-CHECK.md`. Maciej Grajczyk
has to sign that page. This file does not.

— Maciej Grajczyk
