# Clip store

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. From the repository root.

Film audio stays out of git. The outline is `docs/galion-voice-v2/CLIP-INGEST.md`. `GALION_CLIP_STORE` is unset in this repository and stays unset in CI. Unset means the store does not exist. There is no default under the work tree, under `weights/`, or under `/tmp`.

Jarvis hub texture (clicks, alarms, ambience) is character. Do not strip it. The clone worker has no denoise step, and it still refuses a reference key. This runbook does not add one.

## Layout, when an owner path exists

```bash
# store stays unset until that path is real and outside git
test -z "${GALION_CLIP_STORE:-}" && echo "clip store unset"
# when it is set, the only audio objects are:
#   $GALION_CLIP_STORE/samantha/reference.wav
#   $GALION_CLIP_STORE/jarvis/reference.wav
sha256sum "${GALION_CLIP_STORE}/samantha/reference.wav" "${GALION_CLIP_STORE}/jarvis/reference.wav"
```

Slots inside the tree are placeholders only: `docs/galion-voice-v2/personas/refs/`. Wav and mp3 are gitignored. MP3 is never a clone ref.

## Owner-published pins

These hex strings are the owner's published pins. This repository did not hash the bytes. Section B prints `ref_sha256=not-measured` until a file is on disk, and `ref_sha256=measured` only after `sha256sum` matches.

| Persona | Published sha256 | Owner-published length |
|---|---|---|
| samantha | `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` | 17.41 s, 44100 Hz stereo 16-bit, not re-measured here |
| jarvis | `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` | 15.95 s, 48000 Hz stereo 16-bit, not re-measured here |

On a mismatch the ingest outline says to delete the wav. The self-eval check fails the section and does not delete anything.

A clean-speech baseline for the texture check is optional and separate: `GALION_TEXTURE_BASELINE`, or `$GALION_CLIP_STORE/jarvis/clean-speech.wav`. Without both files the spectral compare is `not measured`. No ratio is filled in.

`LOCKS.toml` still has `reference_clips = false` for the Chatterbox exception. This page does not change that.

— Maciej Grajczyk
