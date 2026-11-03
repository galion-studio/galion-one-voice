# Clone worker

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. From the repository root.

The contract is `docs/galion-voice-v2/CLONE-WORKER.md`. The child is `scripts/clone/chatterbox_worker.py`. The Rust side is `crates/galion-ml-voice/src/speech/clone_worker.rs`. One long-lived process, one JSON line in, raw 24 kHz s16le on stdout, `galion-clone-utt:` on stderr. CPU only, out of process, no GPU. PyTorch is not imported by the stub.

The test `two_turns_reuse_one_clone_process_and_the_clock_is_not_a_product_claim` says the worker clock is not a product claim. Do not publish a clone latency from this page.

## Protocol smoke

This PCM is `protocol-stub`. It is not Samantha, not Jarvis, and not a Flash result.

```bash
printf '%s\n' '{"text":"Hello.","model":"nano","persona":"jarvis","tags":true}' \
  | GALION_CLONE_STUB_PCM=1 python3 scripts/clone/chatterbox_worker.py >/tmp/galion-clone.pcm
```

Stderr should contain `galion-clone-engine: protocol-stub`, `galion-clone-persona: jarvis`, and `galion-clone-utt:`. The byte count of the file is the measurement. There is no timing claim beside it.

A reference clip is refused. The child exits 3 and writes no PCM:

```bash
printf '%s\n' '{"text":"Hello.","reference":"/tmp/galion-not-a-clip.wav"}' \
  | GALION_CLONE_STUB_PCM=1 python3 scripts/clone/chatterbox_worker.py
```

Without `GALION_CLONE_STUB_PCM=1`, and without the pinned safetensors on disk, the process exits 2 (`galion-clone-absent`). With the files present and the stub flag off, a request is `galion-clone-unwired` and no samples are written. Weights are fetched only when you set `GALION_CLONE_FETCH=1` on `scripts/clone/fetch-clone-weights.sh`. CI does not set that flag. This runbook does not run the fetch.

## What is not landed

The owner pins for the reference wavs are in [CLIP-STORE.md](CLIP-STORE.md). They are published pins, not a hash this tree computed, until the files are on disk and section B says `ref_sha256=measured`. The worker does not read those files. A matching hash would still not be a landed voice while the engine line says `protocol-stub`.

— Maciej Grajczyk
