# Clip store

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

## Policy

Reference wavs stay outside git and are read-only. This tree does not add a wav, an mp3, or any other audio file. A worker opens a store file to read it. It does not rewrite, trim, denoise, or replace the bytes. A stage copy, when an owner path exists, writes a new file outside the work tree and leaves the source bytes alone. Piper training on own-voice clips is forbidden. Cloning is Rust-first: the store is a path the Rust gate checks. It is not a Piper training corpus. The deployed Jarvis voice must not change, and the Jarvis hub texture stays in the file. A hash, a duration, or a rate that this checkout did not compute is unmeasured. Unmeasured gates stay **BLOCKED**. Pins already printed below are owner-published records. They are not a new measurement from this tree.

Two reference clips. WAV only. One per test persona. They condition the clone. They are not in git. They are read-only.

| Id | In-repo path (gitignored) | Outside the repo |
|---|---|---|
| Samantha | `docs/galion-voice-v2/personas/refs/samantha.wav` | `/workspace/galion-clip-store/samantha/reference.wav` |
| Jarvis | `docs/galion-voice-v2/personas/refs/jarvis.wav` | `/workspace/galion-clip-store/jarvis/reference.wav` |

Each outside file has a sibling `.sha256`.

| Id | Shape | sha256 |
|---|---|---|
| Samantha | 17.41 s, 44100 Hz, stereo, 16-bit | `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` |
| Jarvis | 15.95 s, 48000 Hz, stereo, 16-bit | `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` |

Git keeps `samantha.placeholder` and `jarvis.placeholder`. `docs/galion-voice-v2/personas/refs/.gitignore` ignores wav, mp3, and the other audio and weight suffixes. Clone references are WAV.

The Jarvis WAV keeps the hub texture. See [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md). The ingest outline, including retention and deletion, is [CLIP-INGEST.md](../CLIP-INGEST.md). The worker does not take a clip path on the JSON line. It exits 3 if one appears. The compiler ([COMPILER.md](COMPILER.md)) is what binds a stored ref to the Galion voice file, by hash.

Check a file that is present:

```bash
sha256sum docs/galion-voice-v2/personas/refs/samantha.wav \
          docs/galion-voice-v2/personas/refs/jarvis.wav
sha256sum /workspace/galion-clip-store/samantha/reference.wav \
          /workspace/galion-clip-store/jarvis/reference.wav
```

## Gate

**PASS** when both WAV files exist at the store path, both hashes match the table, and `git status` shows no reference audio.

**FAIL** when a hash mismatches, when an mp3 is used as the ref, or when a WAV is committed.

**BLOCKED** when the files are absent on this machine. The placeholders are the slots. Absence is not a hash. WER, ECAPA, MOS, and timings are not measured from these files on this page. Those gates stay **BLOCKED**.
Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. From the repository root.

Film audio stays out of git. The outline is `docs/galion-voice-v2/CLIP-INGEST.md`. `GALION_CLIP_STORE` is unset in this repository and stays unset in CI. Unset means the store does not exist. There is no default under the work tree, under `weights/`, or under `/tmp`.

Jarvis hub texture (clicks, alarms, ambience) is character. Do not strip it. The clone worker has no denoise step, and it still refuses a reference key. This runbook does not add one.

## Layout, when an owner path exists

# store stays unset until that path is real and outside git
test -z "${GALION_CLIP_STORE:-}" && echo "clip store unset"
# when it is set, the only audio objects are:
#   $GALION_CLIP_STORE/samantha/reference.wav
#   $GALION_CLIP_STORE/jarvis/reference.wav
sha256sum "${GALION_CLIP_STORE}/samantha/reference.wav" "${GALION_CLIP_STORE}/jarvis/reference.wav"

Slots inside the tree are placeholders only: `docs/galion-voice-v2/personas/refs/`. Wav and mp3 are gitignored. MP3 is never a clone ref.

## Owner-published pins

These hex strings are the owner's published pins. This repository did not hash the bytes. Section B prints `ref_sha256=not-measured` until a file is on disk, and `ref_sha256=measured` only after `sha256sum` matches.

| Persona | Published sha256 | Owner-published length |
| samantha | `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` | 17.41 s, 44100 Hz stereo 16-bit, not re-measured here |
| jarvis | `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` | 15.95 s, 48000 Hz stereo 16-bit, not re-measured here |

On a mismatch the ingest outline says to delete the wav. The self-eval check fails the section and does not delete anything.

A clean-speech baseline for the texture check is optional and separate: `GALION_TEXTURE_BASELINE`, or `$GALION_CLIP_STORE/jarvis/clean-speech.wav`. Without both files the spectral compare is `not measured`. No ratio is filled in.

`LOCKS.toml` still has `reference_clips = false` for the Chatterbox exception. This page does not change that.
# V2-S14 · Clip store (owner clearance t273)

Europe/Zagreb. 2026-11-03. The worker gate for one reference WAV per
research-target persona.

Master slogan: "The seamless connection with the human being, on the voice only."


Owner decision **t273** records clearance for local zero-shot conditioning
of the Samantha and Jarvis references. The machine record is
`clearance-t273.rec` in this directory. The human line is in `LEDGER.md`.
This file is not a counsel opinion. **No counsel name is written here,**
because none was supplied. Do not invent one. The checklist boxes in
`RIGHTS-CLEARANCE.md` stay unchecked. A public demo stays closed.

`LOCKS.toml` is not edited. `[formats.exceptions.v2s14_chatterbox]`
`reference_clips` stays `false`. `scripts/check-locks.sh` still requires
that value. The worker does not read the lock boolean. It reads the
clearance record and `send_reference` in the persona voice file.

Nothing in this change downloads weights, calls Chatterbox `generate`,
or commits audio.

## Where a WAV may live

Two places. Both are WAV only. An mp3, a renamed mp3, or any other
audio suffix is not a reference.

| Role | Path | In git |
| Landing slot | `docs/galion-voice-v2/personas/refs/samantha.wav` | No. `personas/refs/.gitignore` ignores `*.wav` and `*.mp3`. |
| Landing slot | `docs/galion-voice-v2/personas/refs/jarvis.wav` | No. Same ignore. |
| Runtime store | `$GALION_CLIP_STORE/samantha/reference.wav` | No. The variable is an absolute path outside the work tree. |
| Runtime store | `$GALION_CLIP_STORE/jarvis/reference.wav` | No. |
| Hash | `$GALION_CLIP_STORE/<persona>/reference.wav.sha256` | No. Written from the file by `sha256sum`. Not copied into `LEDGER.md` unless a human computed it and pasted it. |

`GALION_CLIP_STORE` is unset in CI. Unset means the worker sends no
`reference` field. There is no default under the clone, under
`weights/`, or under `/tmp`.

The landing slots are the owner's machine copies. They are gitignored.
This tree does not contain them, so this file records no duration and
no sha256. A hash that is not computed from the file is not written.

The runtime store is the path the worker may send. It is outside git.
`scripts/clone/stage-clip-store.sh` copies a landing WAV into that
store and writes the sidecar. It refuses a store inside the work tree,
a missing clearance record, and any non-WAV landing file (including
`.mp3`). It does not fetch, and it does not commit.

Jarvis may carry the hub bed (clicks, alarms, room). That bed is a
separate layer from the voice, not something the stage script removes.
The stage script copies the file as bytes. It does not trim. The older
ten-second outline in `CLIP-INGEST.md` is not a cutter.

The layer toggle is `GALION_JARVIS_TEXTURE`. Unset, or any value other
than `0` or `off`, sends `"texture":"hub"` with a Jarvis reference.
`0` or `off` sends `"texture":"off"`. Samantha has no texture field.
The child logs `galion-clone-texture:` and does not rewrite the WAV.
Turning the layer off is a request to a later mixer. This wiring does
not filter samples. Default is on, so the hub bed stays unless the
operator asks for it off.

## When `send_reference` is on

Both voice files set `send_reference = true`:

- `docs/galion-voice-v2/personas/voice/samantha.toml`
- `docs/galion-voice-v2/personas/voice/jarvis.toml`

That flag is necessary and not sufficient. The worker adds one
`reference` field to the stdin JSON only when every row below is true.
Otherwise the line stays `text`, `model`, `persona`, `tags`, and the
child is not given a path.

| # | Check |
|---|---|
| 1 | `clearance-t273.rec` is readable (`GALION_CLIP_CLEARANCE` overrides the path). |
| 2 | `decision=t273`, `owner=Maciej Grajczyk`, `recorded` is non-empty, `scope=local-zero-shot-conditioning`. |
| 3 | `public_demo=no` and `private_cleared=yes`. This record does not open a demo. |
| 4 | `wav_only=yes`, `lock_edited=no`, `send_reference=yes`. |
| 5 | The `counsel` key is present. An empty value is the recorded state. A name is accepted only if a human wrote it into the file. Code does not fill one. |
| 6 | The persona is `samantha` or `jarvis`, and it is listed in `personas`. |
| 7 | That persona's voice file has `send_reference = true` under `[worker]`. |
| 8 | `GALION_CLIP_STORE` is absolute, and its canonical path is not inside the git work tree. |
| 9 | The file is `$GALION_CLIP_STORE/<persona>/reference.wav`, canonical path still under that persona directory, and not inside the work tree. |
| 10 | The first twelve bytes are `RIFF` and `WAVE`. A `.wav` name with an mp3 header is refused. |
| 11 | `reference.wav.sha256` exists beside it. The first field is 64 hex digits and matches the file. A missing sidecar is a refusal. No hash is invented. |
| 12 | `LEDGER.md` has a `PRIVATE CLEARED` line for this persona that also names `t273` and Maciej Grajczyk. Both personas are required for a copy. The checklist boxes in `RIGHTS-CLEARANCE.md` stay unchecked, because no counsel name was supplied. |

`clip_path` in the voice files stays `""`. The `/` page still reports
`rights` `hold` and `film_voice` false. The public surface is not this
gate. Piper is still not a film voice.

The child accepts that one `reference` path when the same checks pass.
It still does not import torch and does not call `generate`. A protocol
stub may emit its pipe PCM. The WAV bytes are not the reply.

These keys still exit 3 with no PCM: `audio_prompt`, `audio_prompt_path`,
`clip`. A `reference` that fails the table exits 3 with no PCM. The
reason is `rights hold` when the record or the voice flag says no,
`wav only` when the object is not a WAV, `clip store` when the path is
not the outside store, and `sha256` when the sidecar is missing or wrong.

## What t273 does not do

- It does not tick a box in `RIGHTS-CLEARANCE.md`.
- It does not name counsel, a grantor, or a rightsholder.
- It does not open a public demo, a tunnel, or a login gate.
- It does not set `GALION_CLONE_FETCH`.
- It does not flip `reference_clips`. That boolean is still false. An owner flip of `LOCKS.toml` is still required. Until that commit, `scripts/check-locks.sh` keeps requiring false. The worker gate is the ledger, not the lock file.
- It does not put a WAV, an mp3, or a hash of a missing file into git.
- It does not claim the page is speaking either performer.

Withdrawal is the refused template in `RIGHTS-CLEARANCE.md`, plus
deletion of the outside store files as `CLIP-INGEST.md` already describes.
Clearing `send_reference` in the record, or removing the record, stops
the worker from sending a path on the next line. No grace period.

## Run

Landing files, if the owner has them, sit beside the placeholders and
stay untracked:

```text
docs/galion-voice-v2/personas/refs/samantha.wav
docs/galion-voice-v2/personas/refs/jarvis.wav

Stage them only into an outside store:

```text
GALION_CLIP_STORE=/absolute/path/outside/this/repo scripts/clone/stage-clip-store.sh

Then a clone child may be started with that same variable. CI does not
set it. Absent landing files print `absent` and exit 0. They are not an
error, because the WAV is not part of the clone.

## PASS / FAIL / BLOCKED (t287, this tree)

| Check | Result |
|---|---|
| Owner t273 recorded as LEDGER `PRIVATE CLEARED` for Samantha and Jarvis, counsel name not invented | PASS |
| Worker `send_reference` when that ledger line, the record, and an outside WAV all pass | PASS, covered by the crate tests. No path is sent when the store is unset. |
| Jarvis texture is its own toggle and does not rewrite the WAV | PASS, covered by the worker test (`hub` default, `off` leaves the bytes). |
| `LOCKS.toml` `reference_clips` flipped to true | BLOCKED. The value is still false. This change does not edit the lock. The owner flips it in a later commit. |
| Repeat the Nano smoke from `personas/refs/*.wav` here | BLOCKED. Those WAVs are not in this checkout. Clone weights are not in git. t287 reports a local smoke already generated Nano clones from those files. This tree does not re-measure that run and does not invent a duration or a sha256. |
| WER, ECAPA, MOS, timings | BLOCKED. Not measured in this tree. |
| Public demo | Not opened. |
| `generate()` inside this process | Not wired. The stub still does not import torch. |

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
