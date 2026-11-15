# Clone

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

## Policy

Cloning is Rust-first. The request line, the clip compiler, and the cbx slot are Rust. `scripts/clone/chatterbox_worker.py` stays an out-of-process child. Piper training on own-voice clips is forbidden. Reference wavs stay outside git and are read-only. The deployed Jarvis voice must not change. This page does not record WER, ECAPA, MOS, or a timing. Those gates stay **BLOCKED** until a run on this machine writes the measurement beside the command. No audio file is added.

Clone landing uses the current Samantha and Jarvis models. References are the WAV files in [CLIP-STORE.md](CLIP-STORE.md). The child is one long-lived process: a JSON line on stdin, 24 kHz s16le on stdout.

```text
{"text":"Hello.","model":"nano","persona":"samantha","tags":true}
```

`model` is `nano` (primary) or `turbo` (quality option, same process). `persona` is `samantha` or `jarvis`. Jarvis sets `tags` false. The contract is [CLONE-WORKER.md](../CLONE-WORKER.md). The legal checklist is [RIGHTS-CLEARANCE.md](../RIGHTS-CLEARANCE.md). The program brief for this stack records owner clearance t273 for these two refs.

## What the tree runs today

Updated 2026-11-15. `scripts/clone/chatterbox_worker.py` is the out-of-process child. Torch is not imported at module level. It is imported inside the Nano `generate` path, and only then.

| Condition | What the child does |
|---|---|
| Pinned safetensors absent | Exit 2, `galion-clone-absent`, no PCM. |
| `GALION_CLONE_STUB_PCM=1` | A supplied wav whose sha256 is not the uncommented pin exits 3 (`reference sha256 mismatch`) and writes no PCM. Otherwise protocol PCM, engine `protocol-stub`. Not a voice and not a product claim. |
| Weights present, `GALION_CLONE_LOCAL_REFS` unset, stream unset | Same pin check. A non-pin digest exits 3 (`reference sha256 mismatch`). With no reference, or with the pinned wav, `galion-clone-unwired` and no samples. `generate` is not called. |
| `GALION_CLONE_LOCAL_REFS=1` and the wav sha256 is an uncommented row in `scripts/clone/REFS.sha256` | Nano `generate` when torch, chatterbox, and the pinned tokenizer are on that machine. Engine `chatterbox-nano`. |
| Wav present, sha256 is not an uncommented pin (a commented t273 original, or any other digest) | Exit 3, `galion-clone-refuse`. The generate path says `reference sha256 is not allowlisted`. The stub and unwired pin check says `reference sha256 mismatch`. The Rust request path names a t273 file inside `reference sha256 mismatch` and does not attach it. The file is not deleted. |
| `model` `turbo` on the Nano generate path | `galion-clone-unwired`. Turbo is not this generate path. |

A reference the allowlist rejects is not attached and writes no PCM. Piper remains the fallback. Piper is not the original clone. The compiler ([COMPILER.md](COMPILER.md)) turns accepted worker PCM into the Galion voice file.

The live allowlist on this main is the uncommented t623u rows in `scripts/clone/REFS.sha256` (CC0 donors). `LOCKS.toml` `ref_clip_decision = "t623u"`. The commented t273 hashes are the recorded originals, not the pin. This page does not edit `LOCKS.toml` or `REFS.sha256`. Re-opening t273 is an owner decision.

Weights, when a human opts in:

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh
```

Pins: `scripts/clone/PINS.sha256`. Nano is `t3_nano_v1.safetensors` plus shared `s3gen`, `s3gen_meanflow`, `ve`. Turbo swaps in `t3_turbo_v1.safetensors`. Repos: `ResembleAI/chatterbox-nano`, `ResembleAI/chatterbox-turbo`. The directory `weights/clone/` is gitignored.

`GALION_SPEECH=auto` stays on Piper or flite while `weights_ready` is false. `GALION_SPEECH=clone` falls through to Piper, then flite, when the pack is absent.

## Gate

**PASS** when the worker writes PCM for both personas from the active allowlist, the reference sha256 values are the uncommented `REFS.sha256` rows, and a smoke speak is logged (command, machine, engine label). Engine `chatterbox-nano` or `nano`. Protocol-stub PCM is not this pass.

**FAIL** when PCM is silent, the hash is not the active pin, or the Jarvis take has had the hub texture removed. See [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md). A t273 file at the gitignored path is a named refuse, not a pass.

**BLOCKED** on this page until that smoke speak is logged. Generate is wired behind the gates in the table above. This landing has not logged a smoke speak. WER, ECAPA, MOS, and timings are not measured here. Each of those gates stays **BLOCKED**. An unlogged smoke speak does not become a timing. A protocol stub does not close the gate.

Fetch and the flash clock: [TTFA.md](TTFA.md). Record the gate in [SELF-EVAL.md](SELF-EVAL.md).

## Emotion blend (opt-in)

Turbo ONNX voice-profile conditioning can miss late expressive moments because
the encoder graph crops LM features to the first 6 s. The opt-in prep in
[EMOTION-BLEND.md](EMOTION-BLEND.md) (`tools/cloning-v2/`) rotates or montages
the reference before `voice add`. Default clone landing and Piper stay unchanged.
Samantha production on 2026-11-05 uses the coherent-rotation candidate. Jarvis does not.
Piper training on own-voice clips is forbidden. The reference wav stays outside git and is read-only.
The deployed Jarvis voice must not change.
This section does not copy WER, ECAPA, MOS, or a timing from the blend report. Those gates stay **BLOCKED** here.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
