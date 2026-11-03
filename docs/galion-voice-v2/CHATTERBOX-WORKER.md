# V2-S14 · Chatterbox Nano worker

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The worker is runnable. A clone is live only when the gate below is open.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

One OS process, PyTorch CPU, not linked into any Galion crate. Nano is the primary. Turbo is the same process with `GALION_CLONE_MODEL=turbo`. Piper speaks when this child is absent, unwired, or dead. Flite speaks when the Piper pack is missing. No GGML-family port. No in-tree ONNX Runtime. No candle. The child does not set a thread cap (t204u, t205u: full cores, full memory, self-offline).

`LOCKS.toml` is not edited by this change. `[formats.exceptions.v2s14_chatterbox]` still has `reference_clips = false`. That value blocks conditioning. `/v2/state` reports `"clone_live":false` until the gate is actually open.

## Run

Weights stay out of git. CI does not set the fetch flag and does not install PyTorch.

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh
```

That fetches the pinned safetensors and the tokenizer text into `weights/clone/`. A hash mismatch deletes the partial. `.pt` and audio suffixes are refused.

On the machine that will speak, in a venv that is not committed:

```bash
python3 -m venv .venv-clone && . .venv-clone/bin/activate
pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu
pip install 'chatterbox-tts @ git+https://github.com/resemble-ai/chatterbox.git'
```

The Chatterbox tree has to be one that loads Nano through `ChatterboxTurboTTS.from_local(..., device="cpu", nano=True)`. That is upstream master as read on 2026-11-03. Perth is imported by `generate()`. It is not copied into this repo. If the import fails, the child writes no PCM and the lane uses Piper.

```bash
GALION_CLONE_GENERATE=1 GALION_CLONE_MODEL=nano GALION_SPEECH=clone \
  python3 scripts/clone/chatterbox_worker.py
```

The phone lane uses the same flags. `GALION_SPEECH=auto` selects this slot only when `weights_ready` matches `scripts/clone/PINS.sha256`. Without `GALION_CLONE_GENERATE=1` the child stays unwired and Piper speaks. `GALION_CLONE_STUB_PCM=1` is a pipe test. It is not a voice and not a Flash result.

Speak line, still with no path:

```text
{"text":"Hello.","model":"nano","persona":"samantha","tags":true}
```

Stdout is raw 24 kHz s16le. A non-empty `reference`, `audio_prompt`, `audio_prompt_path`, or `clip` on that line exits 3 and writes no PCM. Jarvis forces tags off. Samantha may keep `[laugh]` and `[chuckle]`.

Conditioning is a different line, sent by the parent only after the gate passes. The path is not written to stderr.

```text
{"op":"condition","persona":"samantha","audio_prompt_path":"/absolute/outside/reference.wav"}
```

Success is `galion-clone-conditioned: samantha 10.000` and `galion-clone-utt: 0`. Later speak lines for that persona use the RAM embedding. `{"op":"drop","persona":"samantha"}` drops it. Nothing is written on the way down.

`generate` returns when the utterance is finished. There is no model named Flash. A clock with `engine=model` is a real generate. CI does not arm one. See `FLASH-EVAL.md`.

## What Maciej supplies

Two files per persona, outside the git tree. This repository does not fetch them, record them, trim them, or invent a sha256.

```text
$GALION_CLIP_STORE/samantha/reference.wav
$GALION_CLIP_STORE/samantha/reference.wav.sha256
$GALION_CLIP_STORE/samantha/consent.txt
$GALION_CLIP_STORE/jarvis/reference.wav
$GALION_CLIP_STORE/jarvis/reference.wav.sha256
$GALION_CLIP_STORE/jarvis/consent.txt
```

`GALION_CLIP_STORE` is an absolute path outside the work tree. Unset means the store does not exist. There is no default under the repo, under `weights/`, or under `/tmp`.

Each wav:

- mono, 16-bit PCM
- about 10 seconds, inside 8–12 seconds (Turbo's public assert is longer than 5 seconds; the same file serves Nano and Turbo)
- one clip per persona
- Samantha: the performance in *Her* (2014)
- Jarvis: one Iron Man film performance, title and year copied from the instrument

A longer file is the wrong object. This tree does not cut it down.

`consent.txt` is text. Every field from `RIGHTS-CLEARANCE.md` / `CLIP-INGEST.md` has to be filled. No brackets, no empty value, no placeholder word:

`date` (include `CET` or `Zagreb`), `persona`, `work`, `performer`, `grantor`, `instrument`, `grantee`, `scope` (`local-zero-shot-conditioning`), `public_demo` (`no` until a public line exists), `expiry`, `operator`, `withdrawal`, `counsel`, `sha256`, `duration_s`.

```bash
GALION_CLIP_STORE=/absolute/outside scripts/clone/ingest-clip.sh samantha
GALION_CLIP_STORE=/absolute/outside scripts/clone/ingest-clip.sh jarvis
```

The first run prints `duration_s` and `sha256` measured from the wav. Copy those two into `consent.txt` and into `reference.wav.sha256`. The script does not write them for you. A mismatch leaves the wav in place.

## When `/v2/state` says clone_live

`clone_live` is true only when all of these are true for the current persona:

1. `scripts/clone/chatterbox_worker.py` is present.
2. The pinned Nano (or `GALION_CLONE_MODEL`) files match the sizes in `PINS.sha256`.
3. The wav and `consent.txt` pass the checker.
4. `reference_clips` is true in `[formats.exceptions.v2s14_chatterbox]`.

Otherwise the field is false and `clone_block` names the first miss (`store`, `wav`, `mono`, `duration`, `consent`, `sha`, `lock`, `weights`, `worker`). `rights` stays `hold` until that gate is open, then `cleared`. `film_voice` stays false unless `public_demo` is `yes`. The page mouth stays the browser. The flag is readiness, not a claim that the dot is speaking the film.

Today the lock is false, so a placed wav still reports `clone_block` `lock` and `clone_live` false. Opening the lock is an owner edit of `LOCKS.toml`. This change does not make it.

Restart the demo after the files and the lock change. The floor reads the gate once at start.

## What this change does not do

- Edit `LOCKS.toml`, including `reference_clips`.
- Tick a box in `RIGHTS-CLEARANCE.md`, or paste a LEDGER line that still has a blank field.
- Place, fetch, or hash a film wav.
- Vendor Perth, PyTorch, or the Chatterbox tree.
- Run generate in CI, or publish a TTFA number.
- Open a public demo. `public_demo: no` is the default. Art. 50 stays on. No nameplate on the mark.

— Maciej Grajczyk
