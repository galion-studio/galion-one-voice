# Galion Voice V2 · Chatterbox Nano backend

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

The spelling is **Galion**. The product is Galion Voice.

Nano is the clone backend on the V2 speech stack. Piper A21 is the fallback.
Flite remains the fallback after Piper. This note is the runbook for a local
box that already has the weights and a reference clip. It does not fetch
either, and it does not flip `LOCKS.toml`.

`reference_clips` in `[formats.exceptions.v2s14_chatterbox]` stays `false`.
A local clip is an operator file outside git, opened only when the env gate
below is on and the sha sidecar matches. That is not a rights grant. Film
audio stays on the hold in `VOICE-CLONING.md`. Do not commit a wav, a
safetensors file, or a tokenizer.

## What speaks

`GALION_SPEECH=auto` (the default) picks the first slot that can speak:

| Order | Slot | When |
|---|---|---|
| 0 | Chatterbox Nano (`chatterbox-nano`) | Pinned safetensors match `scripts/clone/PINS.sha256`, `tokenizer.json` is on disk, `GALION_CLONE_LOCAL_REFS=1`, and at least one of Samantha or Jarvis has a matching sha sidecar |
| 1 | Piper A21 | The clone slot is closed, the child dies, or that persona's ref is not allowlisted |
| 2 | flite | The A21 pack or the Piper runtime is missing |

`GALION_SPEECH=a21` never opens Nano. `GALION_SPEECH=clone` with the gate
closed falls through to Piper, then flite. `GALION_SPEECH=off` speaks nothing.

Turbo is the same child with `GALION_CLONE_MODEL=turbo`. Nano is the default.
The request line is still `text`, `model`, `persona`, `tags`, and numeric
`prosody`. It has no `reference`, `audio_prompt`, `audio_prompt_path`, or
`clip`. A client that sends one of those keys makes the child exit 3 and
write no PCM. The child opens the store path itself.

Generate runs in `scripts/clone/chatterbox_worker.py`, off `Heartbeat::step`.
The 80 ms loop only stores the caller frame (`last_caller_frame`) and, on the
phone screen, copies prosody into a slot the turn worker reads when it
speaks. Step stays zero-alloc. The worker does not add a sentence.

## Emotion

Prosody is `galion-emotion` on the 80 ms frame: `StubExpressor::voice_color`
on that frame, then Rika `apply_prosody`. No consent means no caller frame,
so the weight `e` is 0. Pause and loudness are applied to the PCM. Pitch,
range, rate, and warmth are logged on the clone child's stderr
(`galion-clone-prosody:`). They are not a second emotion name and not a
line of empathy.

## Env

| Variable | Role |
|---|---|
| `GALION_SPEECH` | `auto` (default), `clone`, `a21`, `flite`, `off` |
| `GALION_CLONE_MODEL` | `nano` (default) or `turbo` |
| `GALION_CLONE_PERSONA` | `samantha` (default) or `jarvis`. The live bus can change this per turn |
| `GALION_CLONE_LOCAL_REFS` | `1` opens the local ref gate. Any other value leaves it closed |
| `GALION_CLIP_STORE` | Absolute directory outside the git work tree. Unset in CI |
| `GALION_CLONE_WEIGHTS` | Split tree. Default `<repo>/weights/clone` |
| `GALION_CLONE_CKPT` | Optional flat directory `from_local` can open (`ve.safetensors`, `s3gen_meanflow.safetensors`, `t3_nano_v1.safetensors`, `tokenizer.json`) |
| `GALION_CLONE_TOKENIZER` | Directory that contains `tokenizer.json` if it is not under the weights tree or the flat checkpoint |
| `GALION_CLONE_WORKER` | Override for `scripts/clone/chatterbox_worker.py` |
| `GALION_CLONE_STUB_PCM` | `1` is protocol PCM for the harness. Not Nano. Not a Flash result |
| `GALION_CLONE_FETCH` | `1` allows `scripts/clone/fetch-clone-weights.sh`. CI does not set it |
| `HF_HUB_OFFLINE`, `TRANSFORMERS_OFFLINE` | Forced to `1` on the child so a missing file cannot download |

Piper is unchanged: `GALION_A21_RUNTIME`, `GALION_A21_NOISE`, `GALION_FLITE_VOICE`.

## Local box

The weights and the clips are already on the machine. This repo does not
contain them.

```text
$GALION_CLONE_WEIGHTS/                 # or the default weights/clone/
  shared/s3gen.safetensors
  shared/s3gen_meanflow.safetensors
  shared/ve.safetensors
  nano/t3_nano_v1.safetensors
  tokenizer/tokenizer.json             # plus the tokenizer sidecars from the same snapshot
$GALION_CLIP_STORE/
  samantha/reference.wav               # longer than 5 s; about 10 s
  samantha/reference.wav.sha256        # one sha256 hex, nothing else required
  jarvis/reference.wav
  jarvis/reference.wav.sha256
```

A flat Hugging Face snapshot can be `GALION_CLONE_CKPT` instead of the split
tree. The four pinned tensors are checked by basename against
`scripts/clone/PINS.sha256`. A size or sha miss does not load the model.

`chatterbox-tts` has to be installed in the `python3` that the host spawns
(`from chatterbox.tts_turbo import ChatterboxTurboTTS`). The child calls
`ChatterboxTurboTTS.from_local(..., device="cpu", nano=True)`. It does not
call `from_pretrained`.

```bash
# Confirm pins. This does not speak.
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --check

# Samantha and Jarvis through Nano when both refs match. Otherwise Piper.
export GALION_SPEECH=auto
export GALION_CLONE_MODEL=nano
export GALION_CLONE_LOCAL_REFS=1
export GALION_CLONE_WEIGHTS=/path/to/weights/clone
export GALION_CLONE_TOKENIZER=/path/to/weights/clone/tokenizer
export GALION_CLIP_STORE=/path/outside/git/clips

# Phone screen. Loopback only. The 80 ms loop is unchanged.
cargo run -p galion-console --features dot-viz-web --example dot_console_demo -- \
  --web 127.0.0.1:7878 --realtime
```

stderr from the child, when Nano actually ran:

```text
galion-clone-ref: samantha <12 hex chars>
galion-clone-progress: pins
galion-clone-progress: import
galion-clone-progress: load
galion-clone-progress: generate
galion-clone-engine: chatterbox-nano
galion-clone-utt: <pcm bytes>
```

`galion-clone-unwired` means this utterance did not come from Nano. The host
then uses Piper if the pack is on disk. A missing `chatterbox` install, a
sha miss, or a missing tokenizer is that path. Piper without Nano is the
`:7878` headless path: `scripts/run-v1-demo.sh --headless` does not set
`GALION_CLONE_LOCAL_REFS`, so the clone slot stays closed.

Jarvis strips bracket tags before generate. Samantha may keep them. Neither
persona is given an empathy line.

## What this does not do

- It does not edit `LOCKS.toml`.
- It does not put a clip on `POST /turn` or on the bus.
- It does not commit wavs, weights, or tokenizer files.
- It does not run generate inside `Heartbeat::step`.
- It does not publish a Flash number. A local clock is a local clock.

— Maciej Grajczyk
