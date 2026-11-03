# V2-S14 · Clone worker contract

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A scaffold. Not a claim that Chatterbox has spoken.

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

Request line, written by `clone_worker::request_line`. Device is `cpu`.
`denoise` is false. A reference field is added only for a pinned wav that
is present and matches `scripts/clone/REFS.sha256` (`symphony/CLONE-LANDING.md`).
Any other non-empty `reference`, `audio_prompt`, `audio_prompt_path`, or
`clip` exits 3 and writes no PCM. MP3 exits 3. A denoise request exits 3.

```text
{"text":"Hello.","model":"nano","persona":"samantha","tags":true,"device":"cpu","denoise":false}
```

`model` is `nano` (primary, not swapped) or `turbo` (quality option, same
process shape, same one-thread CPU cap). `persona` is `samantha` or
`jarvis`. Jarvis sets `tags` false and the stub strips bracket tags from
the text. Samantha may keep them for a later model. An unknown persona
name is treated as Samantha. The child is pinned to one thread
(`session_vcpu=1`). A GPU device is refused. GGUF is refused.

## What the stub does not do

`scripts/clone/chatterbox_worker.py`:

- does not import torch
- does not download weights
- does not call Chatterbox generate
- accepts only the two owner-cleared wavs, and only when the sha256 matches
- does not denoise, and does not load GGUF

Without `GALION_CLONE_STUB_PCM=1`, and without the pinned safetensors on
disk, the process exits 2 (`galion-clone-absent`). With the files present
and the stub flag off, each request is `galion-clone-unwired` and no
samples are written. Protocol PCM is not a voice and not a Flash result.

`SpeechStack` selects this slot only when `weights_ready` is true (size and
sha256 against `scripts/clone/PINS.sha256`). CI does not have those files.
`GALION_SPEECH=auto` therefore stays on Piper or flite. `GALION_SPEECH=clone`
with the pack absent falls through to Piper, then flite. A dead clone child
on a later speak does the same handoff without restarting the lane.

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

Owner t273 clears local send of the two current wavs. The ledger line is
in `LEDGER.md`. No counsel name is written there. The public-demo boxes in
`RIGHTS-CLEARANCE.md` stay unchecked. `LOCKS.toml` `reference_clips` stays
false; this file does not edit the lock. The persona id is still the
register from `SAMANTHA-V2.md` §4. This worker does not fetch audio.
The landing runbook is `symphony/CLONE-LANDING.md`. The ingest outline is
`CLIP-INGEST.md`.

— Maciej Grajczyk
