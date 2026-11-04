# Clone

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Clone landing uses the current Samantha and Jarvis models. References are the WAV files in [CLIP-STORE.md](CLIP-STORE.md). The child is one long-lived process: a JSON line on stdin, 24 kHz s16le on stdout.

```text
{"text":"Hello.","model":"nano","persona":"samantha","tags":true}
```

`model` is `nano` (primary) or `turbo` (quality option, same process). `persona` is `samantha` or `jarvis`. Jarvis sets `tags` false. The contract is [CLONE-WORKER.md](../CLONE-WORKER.md). The legal checklist is [RIGHTS-CLEARANCE.md](../RIGHTS-CLEARANCE.md). The program brief for this stack records owner clearance t273 for these two refs.

## What the tree runs today

`scripts/clone/chatterbox_worker.py` does not import torch and does not call generate. With the pinned safetensors absent it exits 2 (`galion-clone-absent`). With the files present and the stub flag off, a request is `galion-clone-unwired` and no samples are written. A line that carries `reference`, `audio_prompt`, `audio_prompt_path`, or `clip` exits 3 and writes no PCM. The reference stays in the clip store. The compiler ([COMPILER.md](COMPILER.md)) is what attaches it to the Galion voice file.

Weights, when a human opts in:

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh
```

Pins: `scripts/clone/PINS.sha256`. Nano is `t3_nano_v1.safetensors` plus shared `s3gen`, `s3gen_meanflow`, `ve`. Turbo swaps in `t3_turbo_v1.safetensors`. Repos: `ResembleAI/chatterbox-nano`, `ResembleAI/chatterbox-turbo`. The directory `weights/clone/` is gitignored.

`GALION_SPEECH=auto` stays on Piper or flite while `weights_ready` is false. `GALION_SPEECH=clone` falls through to Piper, then flite, when the pack is absent.

## Gate

**PASS** when the worker writes PCM for both personas, the reference sha256 values match [CLIP-STORE.md](CLIP-STORE.md), and a smoke speak is logged (command, machine, engine label).

**FAIL** when PCM is silent, the hash mismatches, or the Jarvis take has had the hub texture removed. See [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md).

**BLOCKED** while generate is unwired or the WAV refs are not on the machine. This landing has not logged a smoke speak.

Fetch and the flash clock: [TTFA.md](TTFA.md). Record the gate in [SELF-EVAL.md](SELF-EVAL.md).

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
