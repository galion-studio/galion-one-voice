# V2-S14 · Clone worker contract

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Nano speaks only behind the env gate in `NANO-BACKEND.md`.
Piper speaks otherwise. Not a claim that a film voice shipped.

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

Request line, written by `clone_worker::request_line_prosody`. There is no
reference field. A child that sees a non-empty `reference`, `audio_prompt`,
`audio_prompt_path`, or `clip` exits 3 and writes no PCM. The clip path is
not on this line. The child resolves it from `GALION_CLIP_STORE` when the
env gate is open.

```text
{"text":"Hello.","model":"nano","persona":"samantha","tags":true,"prosody":{"pitch_st":0.0000,"range":1.0000,"rate":1.0000,"energy":1.0000,"warmth":0.5000,"pre_pause_ms":0}}
```

`prosody` is the latest 80 ms `galion-emotion` decision (pause and loudness
are applied to the PCM; the other numbers are logged). It is not an emotion
name and not a sentence.

`model` is `nano` (primary) or `turbo` (quality option, same process shape).
`persona` is `samantha` or `jarvis`. Jarvis sets `tags` false and the worker
strips bracket tags from the text. Samantha may keep them. An unknown persona
name is treated as Samantha.

## When Nano runs

`scripts/clone/chatterbox_worker.py` calls `ChatterboxTurboTTS.from_local`
on CPU only when all of these are true:

- pinned safetensors match `scripts/clone/PINS.sha256`
- `tokenizer.json` is on disk (`GALION_CLONE_TOKENIZER`, `GALION_CLONE_CKPT`, or `weights/clone/tokenizer`)
- `GALION_CLONE_LOCAL_REFS=1`
- `$GALION_CLIP_STORE/<persona>/reference.wav` matches `reference.wav.sha256`

The child sets `HF_HUB_OFFLINE=1` and does not call `from_pretrained`.
Progress is stderr (`galion-clone-progress:`). PCM is stdout after generate.
Library prints during generate are pointed at stderr so they cannot enter
the PCM pipe.

Without `GALION_CLONE_STUB_PCM=1`, and without the weight files on disk,
the process exits 2 (`galion-clone-absent`). A sha miss, a missing
tokenizer, or a missing `chatterbox` install is `galion-clone-unwired` and
no samples. The host then speaks with Piper. Protocol PCM
(`GALION_CLONE_STUB_PCM=1`) is not a voice and not a Flash result.

`SpeechStack` selects this slot only when weights, tokenizer, and at least
one allowlisted persona ref are all present. CI has none of those.
`GALION_SPEECH=auto` therefore stays on Piper or flite. `GALION_SPEECH=clone`
with the gate closed falls through to Piper, then flite. A dead clone child
on a later speak does the same handoff without restarting the lane. A
persona whose own sidecar does not match speaks that utterance with Piper
and leaves the Nano child up.

The runbook is `NANO-BACKEND.md`. `LOCKS.toml` `reference_clips` stays false.

## Fetch

`scripts/clone/fetch-clone-weights.sh` refuses unless `GALION_CLONE_FETCH=1`,
including `--check`. It downloads only the five pinned `.safetensors` files
into `weights/clone/`, which git ignores. A hash mismatch deletes the
partial. `.pt` and audio suffixes are refused. The pins were read from the
Hugging Face tree API on 2026-11-03 (LFS oids). They are records, not blobs.

Nano: `t3_nano_v1.safetensors` plus shared `s3gen`, `s3gen_meanflow`, `ve`.
Turbo swaps in `t3_turbo_v1.safetensors`. Repo ids: `ResembleAI/chatterbox-nano`,
`ResembleAI/chatterbox-turbo`.

## Rights

Jarvis and Samantha, as film voices, stay on the hold in `VOICE-CLONING.md`.
The checklist is `RIGHTS-CLEARANCE.md`. Both gates are closed.
The persona id on this worker is the register from `SAMANTHA-V2.md` §4.
A conditioning file, when the operator already has one, lives outside git
and is opened only under the sha gate in `NANO-BACKEND.md`. This tree does
not fetch film audio. The ingest outline is `CLIP-INGEST.md`. Gates 1–6
there stay closed.

— Maciej Grajczyk
