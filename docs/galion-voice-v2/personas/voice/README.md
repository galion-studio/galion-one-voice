# Persona voice configs

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Drafts. The `/v2` demo route reads these files for
prosody, wash, the tag rule, and the rights hold. The clone worker
does not. `wired` means that route, not a live film clone.

The spelling is **Galion**. The product is Galion Voice.

Two research-target registers, as ids already named in
[`SAMANTHA-V2.md`](../../SAMANTHA-V2.md) §4 and
[`VOICE-CLONING.md`](../../VOICE-CLONING.md). Samantha is the conversational
layer. Jarvis is the calm command layer. Neither file is a film voice, and
neither points at audio.

| File | Id | Tags | Reference |
|---|---|---|---|
| [samantha.toml](samantha.toml) | `samantha` | on, clone worker only | empty placeholder |
| [jarvis.toml](jarvis.toml) | `jarvis` | off, every backend | empty placeholder |

Instruction prose, if it lands beside this folder, stays prose
(`SAMANTHA.md`, `JARVIS.md`). Those files do not name a clip. The voice
binding is the TOML sibling.

## Fields

Both files use the same keys.

| Key | Meaning |
|---|---|
| `id`, `status`, `wired`, `session_default` | Draft id. `wired` is true because `/v2` reads the file. The clone worker is still not live. The session default in the spec is Samantha. |
| `[register]` | Label, line length, accent claim. Prosody at `e = 0` and `e = 1` copies the design table in `SAMANTHA-V2.md` §4 (pitch, pitch range, rate, loudness, warmth, pre-pause). Wash amber and the Luti `w`/`z` scales sit beside that table. Luti is marked `proposal`. |
| `[tags]` | Samantha: `enabled = true`, allowed `[laugh]` and `[chuckle]`, and only on the clone worker. Jarvis: `enabled = false`, `strip_brackets = true`, empty allow-list. Piper and flite never receive tags. |
| `[reference]` | `clip_path` is `""`. `present` and `ingest` are false. `placeholder` names a text marker under `../refs/`, not a wav. The page stays `hold`. The worker path is `symphony/CLIP-STORE.md`. |
| `[worker]` | Binding `galion.clone.worker`. Script `scripts/clone/chatterbox_worker.py`. Env `GALION_CLONE_PERSONA`, `GALION_CLONE_MODEL`, `GALION_CLONE_WORKER`. Model `nano`, quality option `turbo`. `send_reference = true` only after the LEDGER private-cleared line and an outside WAV. PCM is 24 kHz s16le. Weights are not vendored. |
| `[texture]` | Jarvis only. Hub bed, default on, toggle `GALION_JARVIS_TEXTURE=0`. The file is not stripped. |
| `[fallback]` | Order `piper`, then `flite`. Piper voice is `en_US-ljspeech-medium` (`weights/tts/en_US-ljspeech-medium.onnx`). It speaks when the clone worker is absent, unwired, or dead. It is not the film voice. Flite speaks only when that Piper pack is missing. Tags stay off on both. |

`../refs/` ignores audio. A gitignored WAV may sit there. The worker reads `$GALION_CLIP_STORE`, not that directory. Do not commit the WAV. `LOCKS.toml` `reference_clips` stays false until the owner flips it.

— Maciej Grajczyk
