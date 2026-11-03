# Persona voice configs

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The `/v2` demo route reads these files for prosody,
wash, and the tag rule. `wired` means that route. `clone_live` is not
a key here. It is computed by `scripts/clone/consent.py` and stays
false until the worker, the weights, and a consented reference are
all present.

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
| `id`, `status`, `wired`, `session_default` | `wired` is true because `/v2` reads the file. That is not `clone_live`. The session default is Samantha. |
| `[register]` | Label, line length, accent claim. Prosody at `e = 0` and `e = 1` copies the design table in `SAMANTHA-V2.md` §4 (pitch, pitch range, rate, loudness, warmth, pre-pause). Wash amber and the Luti `w`/`z` scales sit beside that table. Luti is marked `proposal`. |
| `[tags]` | Samantha: `enabled = true`, allowed `[laugh]` and `[chuckle]`, and only on the clone worker. Jarvis: `enabled = false`, `strip_brackets = true`, empty allow-list. Piper and flite never receive tags. |
| `[reference]` | `clip_path` is `""`. `present` and `ingest` are false. `placeholder` names a text marker under `../refs/`, not a wav. Target length is about 10 s, window 8–12 s (Turbo's public assert is longer than 5 s). `[reference.consent]` lists the fields `consent.py` requires. The store is `GALION_CLIP_STORE`, outside git. |
| `[worker]` | Binding `galion.clone.worker`. Script `scripts/clone/chatterbox_worker.py`. Env `GALION_CLONE_PERSONA`, `GALION_CLONE_MODEL`, `GALION_CLONE_WORKER`, `GALION_CLONE_GENERATE`. Model `nano`, quality option `turbo`, device CPU, no thread cap. `send_reference = false` on the speak line. `condition_op = true` is the separate stdin op. PCM is 24 kHz s16le. Weights are not vendored. |
| `[fallback]` | Order `piper`, then `flite`. Piper voice is `en_US-ljspeech-medium`. It speaks when the clone worker is absent, unwired, or dead, and while the reference gate is closed. It is not the film voice. Flite speaks only when that Piper pack is missing. Tags stay off on both. |

`../refs/` ignores common audio and weight suffixes. Do not put a clip there.
Do not run a fetch. Do not download Chatterbox weights for this draft.

— Maciej Grajczyk
