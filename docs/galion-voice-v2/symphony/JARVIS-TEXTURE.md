# Jarvis texture

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The Jarvis reference carries the Tony Stark hub: clicks, alarms, and room ambience. That texture is the character. The clone keeps it. A denoise pass that strips the room leaves a different voice, and the gate fails.

The file and its hash are in [CLIP-STORE.md](CLIP-STORE.md):

| | |
|---|---|
| Path | `docs/galion-voice-v2/personas/refs/jarvis.wav` |
| Also | `/workspace/galion-clip-store/jarvis/reference.wav` |
| Shape | 15.95 s, 48000 Hz, stereo, 16-bit |
| sha256 | `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` |

WAV only. The git slot is `jarvis.placeholder`. The audio is gitignored.

## Behaviour that travels with the texture

Jarvis is an internal test voice. Calm, precise, dry, brief. The range is narrow: calm, mild amusement, focused attention, quiet satisfaction. Tags stay off. The words stay on the matter. When the empathy weight rises he goes lower, slower, and quieter, on the Jarvis column in [SAMANTHA-V2.md](../SAMANTHA-V2.md). The full instruction file is [JARVIS.md](../personas/JARVIS.md).

The hub is part of the reference, not a line he speaks about. He does not narrate the clicks.

## Gate

**PASS** when a spectral or energy check shows the texture band still present beside the speech, compared with a clean-speech-only baseline of the same take, and the sha256 of the ref still matches.

**FAIL** when a prep step high-passes, denoises, or otherwise removes the hub before the clone.

**BLOCKED** when `jarvis.wav` is not on the machine, so the check has no input. This landing does not commit the WAV.

Record the curves and the hash in [SELF-EVAL.md](SELF-EVAL.md).

— Maciej Grajczyk
