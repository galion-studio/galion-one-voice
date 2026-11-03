# Clip store

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Two reference clips. WAV only. One per test persona. They condition the clone. They are not in git.

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

**BLOCKED** when the files are absent on this machine. The placeholders are the slots. Absence is not a hash.
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

— Maciej Grajczyk
