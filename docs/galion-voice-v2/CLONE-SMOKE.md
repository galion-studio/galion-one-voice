# V2-S14 · Clone smoke

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. How to check the Nano worker. Not a latency claim.

The spelling is **Galion**. The product is Galion Voice.

`generate` runs in `scripts/clone/chatterbox_worker.py`, out of process.
`Heartbeat::step` does not call it. Piper remains the fallback. `LOCKS.toml`
is not edited. `reference_clips` stays false. No WAV and no weight file
belongs in git.

## What CI runs

`scripts/clone/test_worker_gate.py`, also launched from the `galion-ml-voice`
speech tests. It does not import torch and does not download.

It checks the sha allowlist (Samantha `bdcb0c29…c7a8693`, Jarvis
`c2200b59…4b532c60`), the rights-hold refusal when `GALION_CLONE_LOCAL_REFS`
is unset, exit 2 when the pack is missing, protocol PCM on stdout with the
logs on stderr, and that a large PCM write is framed with the stderr marker
first.

```text
python3 scripts/clone/test_worker_gate.py
scripts/clone/clone-smoke.sh
```

`clone-smoke.sh` stops there unless `GALION_CLONE_SMOKE=1`.

## What a box with weights does

The 2026-11-03 smoke on the studio machine loaded Chatterbox Nano from
upstream `resemble-ai/chatterbox` `5de7a54` (`nano=True`). PyPI
`chatterbox-tts==0.1.7` has no such argument. Safetensors come from
release `clone-weights-v1` (`CLONE-WEIGHTS.md`). Tokenizer text is not a
release asset. Place the five files from `scripts/clone/TOKENIZER.sha256`
under `weights/clone/tokenizer/`. `conds.pt` stays out.
`publish-clone-weights.sh --upload` is not part of this smoke.

```text
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --nano
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --check --nano
```

References stay on the machine. One file per persona, gitignored:

- `$GALION_CLIP_STORE/samantha/reference.wav`
- `$GALION_CLIP_STORE/jarvis/reference.wav`

or, when the store is unset, `docs/galion-voice-v2/personas/refs/<persona>.wav`.
A sidecar `reference.wav.sha256` is checked when it exists. The bytes still
have to match the allowlist for that persona.

```text
GALION_CLONE_SMOKE=1 GALION_CLONE_LOCAL_REFS=1 scripts/clone/clone-smoke.sh
```

That sends one short line per persona through the worker and checks that
stdout is even-length s16le, stderr carries `galion-clone-engine: chatterbox-nano`,
and stdout does not contain the framing text. It does not write a WAV.
Whole-utterance `generate()` is still the call. This document does not
record a time-to-first-audio number. Streaming is not this path.

The runtime the smoke used, for a machine that does not already have it:

```text
uv venv -p 3.11 .venv
VIRTUAL_ENV=$PWD/.venv uv pip install --index-strategy unsafe-best-match \
  --extra-index-url https://download.pytorch.org/whl/cpu "torch==2.6.0+cpu" "torchaudio==2.6.0+cpu"
git clone https://github.com/resemble-ai/chatterbox.git src
git -C src checkout 5de7a54
VIRTUAL_ENV=$PWD/.venv uv pip install --no-deps -e ./src
VIRTUAL_ENV=$PWD/.venv uv pip install "setuptools<81"
```

`resemble-perth` 1.0.1 imports `pkg_resources`. Without `setuptools<81` the
load fails with `TypeError: 'NoneType' object is not callable`. Point
`GALION_CLONE_PYTHON` at that venv's python when it is not `python3` on
`PATH`.

## What this smoke does not decide

Hub texture in the Jarvis reference does not come back out of Nano
zero-shot. That wants a render-side bed, not a different checkpoint.
The 80 ms first-audio bar is a streaming question. `generate` returns
the utterance in one buffer. The heartbeat still drains one frame of
already-produced PCM. It does not wait on this child.

— Maciej Grajczyk
