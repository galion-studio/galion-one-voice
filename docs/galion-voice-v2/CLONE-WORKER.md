# V2-S14 · Clone worker contract

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Nano generate is wired in the child. A run still needs weights and an allowlisted reference on that machine. This file is not a claim that a number was measured here.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

## Shape

One long-lived child, the same idea as the warm Piper worker
(`crates/galion-ml-voice/src/speech/piper_worker.rs`).

| | Piper A21 | Clone worker |
|---|---|---|
| Process | one child for the life of the slot | one child for the life of the slot |
| Stdin | one JSON line `{"text":"..."}` | one JSON line, below |
| Stdout | raw s16le | raw s16le, 24 kHz |
| Boundary | stderr `Real-time factor:` | stderr `galion-clone-utt:` |
| Disk | no PCM cache | no PCM cache |
| Fallback | flite if the pack is missing | Piper, then flite |

Request line, written by `clone_worker::request_line`. There is no reference
field on that line. The child resolves a local file itself when
`GALION_CLONE_LOCAL_REFS=1`. Without that variable, a non-empty
`reference`, `audio_prompt`, `audio_prompt_path`, or `clip` exits 3 and
writes no PCM.

```text
{"text":"Hello.","model":"nano","persona":"samantha","tags":true}
```

`model` is `nano` (primary) or `turbo` (quality option, same process shape).
Generate in this worker is Nano. A turbo request is `galion-clone-unwired`
and no samples. `persona` is `samantha` or `jarvis`. An unknown persona
name is treated as Samantha. Jarvis on the Rust line sets `tags` false,
and the Nano path then strips bracket tags. A request that omits `tags`
keeps them, which is what the 2026-11-03 smoke sent. The protocol stub
still strips Jarvis tags by persona.

## Wire

Stdout is PCM only. Before torch is imported, the child dups fd 1 and
points fd 1 at stderr, so upstream `print` (`loaded PerthNet`,
`S3 Token -> Mel`) cannot shift the s16le stream. Progress is stderr.
The child flushes `galion-clone-utt: N` before it writes N PCM bytes.
The Rust reader waits until those N bytes have arrived. That is the
deadlock fix: a parent that reads stderr first no longer stalls on a
full stdout pipe, and a parent that polls both does not stop at the
marker while PCM is still in flight.

`GALION_CLONE_STUB_PCM=1` is protocol PCM, not a voice and not a Flash
result. It still refuses a reference.

## When generate runs

`scripts/clone/chatterbox_worker.py` calls
`ChatterboxTurboTTS.from_local(ckpt, "cpu", nano=True)` and then
`generate`. The call is local. There is no `from_pretrained` and no
download. The reference path is passed through on the first utterance
for that file. Nothing here denoises, trims, or high-passes it.
Conditionals are reused for the next line. Perth stays inside upstream
`generate`. The engine line is `chatterbox-nano`, not `model`.

All of the following have to be true, or the child does not generate:

1. Pinned Nano safetensors are on disk (`weights/clone/`, or `GALION_CLONE_WEIGHTS`). A missing pack exits 2, `galion-clone-absent`.
2. The five tokenizer text files match `scripts/clone/TOKENIZER.sha256` under `weights/clone/tokenizer/`. They are not release assets and `publish-clone-weights.sh` does not upload them. `conds.pt` is not used. A missing tokenizer exits 2.
3. `GALION_CLONE_LOCAL_REFS=1`. Without it, weights present still means `galion-clone-unwired` and zero samples, and a reference key is still the rights-hold refusal.
4. The reference sha256 matches `scripts/clone/REFS.sha256` for that persona. Samantha is `bdcb0c29…c7a8693`. Jarvis is `c2200b59…4b532c60`. The other persona's file is refused. A sidecar `reference.wav.sha256`, when present, has to match the bytes.
5. The runtime exposes `nano=True`. PyPI `chatterbox-tts==0.1.7` does not. Upstream `resemble-ai/chatterbox` `5de7a54` does. A missing runtime exits 2 with no PCM and no traceback on the success path of the gate tests.

The file is `GALION_CLIP_STORE/<persona>/reference.wav` when that variable
is set. An empty store does not fall through. When it is unset, the child
looks at `docs/galion-voice-v2/personas/refs/<persona>.wav`. A JSON
`audio_prompt_path` (or `reference`, or `clip`) is accepted only under
the same sha rule. The Rust request line still does not send a path.

`LOCKS.toml` is not the switch. `reference_clips` stays false. This
worker does not edit it.

`Heartbeat::step` does not spawn this process and does not call
`generate`. The phone lane drains reply PCM one 1920-sample frame at a
time outside that step. Piper (`en_US-ljspeech-medium`), then flite,
still speaks when this slot is absent, unwired, or the child dies.

`SpeechStack` selects this slot only when `weights_ready` is true (size and
sha256 against `scripts/clone/PINS.sha256`). CI does not have those files
and does not set `GALION_CLONE_LOCAL_REFS`. `GALION_SPEECH=auto` therefore
stays on Piper or flite. `GALION_SPEECH=clone` with the pack absent falls
through to Piper, then flite. A dead clone child on a later speak does
the same handoff without restarting the lane.

## Fetch

`scripts/clone/fetch-clone-weights.sh` refuses unless `GALION_CLONE_FETCH=1`,
including `--check`. It downloads the pinned `.safetensors` into
`weights/clone/`, which git ignores. The bytes are GitHub Release assets on
tag `clone-weights-v1` (`CLONE-WEIGHTS.md`). A hash mismatch deletes the
partial. `.pt`, audio suffixes, and Samantha or Jarvis filenames are refused.
The content hashes were read from the Hugging Face tree API on 2026-11-03
(LFS oids). They are records, not blobs. Tokenizer text is a separate pin
file, `scripts/clone/TOKENIZER.sha256` (nano commit `71ccd1d`). It is not a
release asset and this fetch does not upload it. WAVs are not fetched.

Nano (`--nano`): `t3_nano_v1.safetensors` plus shared `s3gen`,
`s3gen_meanflow`, `ve`. Turbo adds `t3_turbo_v1.safetensors`. The release is
not in a clean clone. Until the admin upload lands, the script exits 1 with
`BLOCKED`. `GALION_CLONE_WEIGHTS_SOURCE=hf` still reads the same pins from
Hugging Face (`ResembleAI/chatterbox-nano`, `ResembleAI/chatterbox-turbo`).

## Rights

Jarvis and Samantha, as film voices, stay on the hold in `VOICE-CLONING.md`.
The checklist is `RIGHTS-CLEARANCE.md`. `reference_clips` stays false.
The env flag plus the sha allowlist is how a machine that already has
those two files can condition. It is not a clearance line and it does
not fetch film audio. The ingest outline is `CLIP-INGEST.md`. The smoke
procedure is `CLONE-SMOKE.md`.

— Maciej Grajczyk
