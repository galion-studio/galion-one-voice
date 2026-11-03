# Persona voice configs

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Drafts. The `/v2` demo route reads these files for
prosody, wash, and the tag rule. The clone worker reads `send_reference`,
`clip_path`, and the pinned sha256 when a local wav is present. `wired`
means that route, not a live film clone. The public page is still not
that voice.

The spelling is **Galion**. The product is Galion Voice.

Two research-target registers, as ids already named in
[`SAMANTHA-V2.md`](../../SAMANTHA-V2.md) §4 and
[`VOICE-CLONING.md`](../../VOICE-CLONING.md). Samantha is the conversational
layer. Jarvis is the calm command layer. Neither file is a film voice, and
neither points at audio.

| File | Id | Tags | Reference |
|---|---|---|---|
| [samantha.toml](samantha.toml) | `samantha` | on, clone worker only | `refs/samantha.wav` (gitignored), sha256 pinned |
| [jarvis.toml](jarvis.toml) | `jarvis` | off, every backend | `refs/jarvis.wav` (gitignored), hub texture kept |

Instruction prose, if it lands beside this folder, stays prose
(`SAMANTHA.md`, `JARVIS.md`). Those files do not name a clip. The voice
binding is the TOML sibling.

## Fields

Both files use the same keys.

| Key | Meaning |
|---|---|
| `id`, `status`, `wired`, `session_default` | Draft id. `wired` is true because `/v2` reads the file. Nano is live only behind `NANO-BACKEND.md`. The session default is Samantha. |
| `[register]` | Label, line length, accent claim. Prosody at `e = 0` and `e = 1` copies the design table in `SAMANTHA-V2.md` §4 (pitch, pitch range, rate, loudness, warmth, pre-pause). Wash amber and the Luti `w`/`z` scales sit beside that table. Luti is marked `proposal`. |
| `[tags]` | Samantha: `enabled = true`, allowed `[laugh]` and `[chuckle]`, and only on the clone worker. Jarvis: `enabled = false`, `strip_brackets = true`, empty allow-list. Piper and flite never receive tags. |
| `[reference]` | `present = true`. `clip_path` is the gitignored wav. `ingest` stays false (this tree does not fetch). `denoise = false`. Rights value `owner-t273` is the local send, not a public demo. Jarvis `texture` is `preserve-hub`. |
| `[worker]` | Binding `galion.clone.worker`. Script `scripts/clone/chatterbox_worker.py`. Env `GALION_CLONE_PERSONA`, `GALION_CLONE_MODEL`, `GALION_CLONE_WORKER`, `GALION_CLIP_STORE`. Model `nano`, quality option `turbo` (not a swap). `device = cpu`, `threads = 1`. `send_reference = true`: the request carries the path only when the sha256 matches. PCM is 24 kHz s16le. Weights are not vendored. |
| `[fallback]` | Order `piper`, then `flite`. Piper voice is `en_US-ljspeech-medium` (`weights/tts/en_US-ljspeech-medium.onnx`). It speaks when the clone worker is absent, unwired, or dead. It is not the film voice. Flite speaks only when that Piper pack is missing. Tags stay off on both. |
| `[reference]` | `clip_path` is `""`. `present` and `ingest` are false. `placeholder` names a text marker under `../refs/`, not a wav. Target length is about 10 s (Turbo's public assert is longer than 5 s). Rights stay `hold`. |
| `[worker]` | Binding `galion.clone.worker`. Script `scripts/clone/chatterbox_worker.py`. Env `GALION_CLONE_PERSONA`, `GALION_CLONE_MODEL`, `GALION_CLONE_WORKER`. Model `nano`, quality option `turbo`. Request persona and tags match the id. `send_reference = false`: the landed request line has no clip field. Without `GALION_CLONE_LOCAL_REFS=1`, a non-empty reference makes the worker exit 3. With it, the child resolves an allowlisted local file by persona. PCM is 24 kHz s16le. Weights are not vendored. |
| `[fallback]` | Order `piper`, then `flite`. Piper voice is `en_US-ljspeech-medium` (`weights/tts/en_US-ljspeech-medium.onnx`). It speaks when the clone worker is absent, unwired, or dead, and while the rights hold is closed. It is not the film voice. Flite speaks only when that Piper pack is missing. Tags stay off on both. |

`../refs/` ignores audio suffixes. The wavs stay out of git. Do not commit
one. Do not denoise Jarvis. Do not download Chatterbox weights from this
draft. The runbook is [`../../symphony/CLONE-LANDING.md`](../../symphony/CLONE-LANDING.md).
| `[reference]` | `clip_path` is `""`. `present` and `ingest` are false. `placeholder` names a text marker under `../refs/`, not a wav. The page stays `hold`. The worker path is `symphony/CLIP-STORE.md`. |
| `[worker]` | Binding `galion.clone.worker`. Script `scripts/clone/chatterbox_worker.py`. Env `GALION_CLONE_PERSONA`, `GALION_CLONE_MODEL`, `GALION_CLONE_WORKER`. Model `nano`, quality option `turbo`. `send_reference = true` only after the LEDGER private-cleared line and an outside WAV. PCM is 24 kHz s16le. Weights are not vendored. |
| `[texture]` | Jarvis only. Hub bed, default on, toggle `GALION_JARVIS_TEXTURE=0`. The file is not stripped. |

`../refs/` ignores audio. A gitignored WAV may sit there. The worker reads `$GALION_CLIP_STORE`, not that directory. Do not commit the WAV. `LOCKS.toml` `reference_clips` stays false until the owner flips it.
| `[worker]` | Binding `galion.clone.worker`. Script `scripts/clone/chatterbox_worker.py`. Env `GALION_CLONE_PERSONA`, `GALION_CLONE_MODEL`, `GALION_CLONE_WORKER`, `GALION_CLONE_LOCAL_REFS`, `GALION_CLIP_STORE`. Model `nano`, quality option `turbo`. `send_reference = false`: the request line has no clip field. A non-empty reference on that line still exits 3. The child opens the store path itself when the sha sidecar matches. PCM is 24 kHz s16le. Weights are not vendored. |
| `[fallback]` | Order `piper`, then `flite`. Piper voice is `en_US-ljspeech-medium` (`weights/tts/en_US-ljspeech-medium.onnx`). It speaks when Nano is absent, unwired, the persona ref is not allowlisted, or the child dies. It is not the film voice. Flite speaks only when that Piper pack is missing. Tags stay off on both. |

`../refs/` ignores common audio and weight suffixes. Do not put a clip there.
Do not commit a wav or a weight. The local runbook is `../NANO-BACKEND.md`.

— Maciej Grajczyk
