# V2-S14 · Flash eval (CPU time to first audio)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A plan and a harness stub. Not a measurement.

The spelling is **Galion**. The product is Galion Voice.

There is no Chatterbox model named Flash. Upstream `generate()` returns
when the utterance is finished. PR 528 (still open on 2026-11-03) proposes
`ChatterboxTurboTTS.stream()`. Master does not have that method. The
worker streams without it. See `STREAM-TTFA.md`. The owner's "Flash
streaming eval" is still this question: how long until the first PCM
sample, on CPU, with no cloud.

## What the clock is

One number, when a run happens:

- start: the worker has accepted one JSON line (`text`, model, persona)
- stop: the first PCM sample is in the parent's buffer (the first chunk, not the `galion-clone-utt:` line)
- machine: this CPU, not a phone, not a VPS, not PSTN
- label: `engine` from the child (`protocol-stub`, `unwired`, `chatterbox-nano`, or a future `model`)
- label: `engine` from the child (`protocol-stub`, `unwired`, or `model`)
- `path`: `stream` when chunks were framed, `whole` when they were not

`product_claim` is true only for real Nano PCM on `path=stream`, or for
real Piper PCM from `GALION_SPEECH=a21` (`STREAM-TTFA.md`). A
protocol-stub clock is FAIL. It is not a Flash result. Do not copy a stub
number into `LEDGER.md`.

The default Flash harness does not require a reference. Consented local wavs are the clone-landing path in `symphony/CLONE-LANDING.md` (owner t273). A protocol-stub clock is still not a Flash result, and a public demo stays closed.
`product_claim` is true only for `engine=model`. A Nano generate reports
`chatterbox-nano`, which is not that label, so a clock from it is still
not a product claim. The default harness does not set
`GALION_CLONE_LOCAL_REFS` and does not send a reference, so CI stays on
`measured: false`. A protocol-stub clock is a test of the pipe. Do not
copy a number into `LEDGER.md` until a model run is actually recorded
there. `generate()` still returns the whole utterance.

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
2. The child calls generate, still out of process. Nano does that when `GALION_CLONE_LOCAL_REFS=1` and the sha allowlist matches. The default harness still sends no clip.
3. A product claim wants the stderr engine line `model`. The worker reports `chatterbox-nano` and writes PCM. Protocol-stub PCM stays `product_claim: false`.
1. Pinned safetensors on disk, fetched with `GALION_CLONE_FETCH=1` from release `clone-weights-v1` (`CLONE-WEIGHTS.md`), plus a flat `GALION_CLONE_CKPT`. `GALION_CLONE_STREAM=1`.
2. The child streams. It does not call `generate()`. Still out of process. Still no reference clip until the operator allowlist is armed. `LOCKS.toml` stays as it is.
3. The stderr engine line says `nano` and the path line says `stream`. `ttfa_ms` is the first chunk. `utt_ms` is the closing marker. The live CPU pass today is Piper, `GALION_SPEECH=a21`.
4. Three CPU runs, cold and warm, phrase fixed, written into `LEDGER.md` with the host and the model id.
5. Still no film audio. Still no celebrity demo.

Until those lines exist, this document's only number is the absence of one.
Conditioning stays off this clock until the rights gates in
`CHATTERBOX-WORKER.md` are open. This harness still sends no clip.

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
