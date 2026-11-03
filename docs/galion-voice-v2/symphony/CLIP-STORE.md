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

— Maciej Grajczyk
