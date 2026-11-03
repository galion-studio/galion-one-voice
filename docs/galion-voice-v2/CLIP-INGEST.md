# V2-S14 · Film-clip ingest (outline)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Outline only. The gate is closed.

Master slogan: "The seamless connection with the human being, on the voice only."

This file is the process for two reference clips, about ten seconds each:
Samantha (*Her*, 2014) and Jarvis (an Iron Man film voice). It records the
gates, the store, the consent line, the hash, the retention term, the
worker handoff, the Art. 50 and watermark notes, and deletion.

Nothing in this change fetches, places, hashes, or commits film audio.
`[formats.exceptions.v2s14_chatterbox]` keeps `reference_clips = false`.
Publishing this outline grants no rights.

The legal hold it follows is `VOICE-CLONING.md`. The worker it must not
loosen is `CLONE-WORKER.md`. Caller audio stays where `SECURITY-EDGE.md`
already puts it: RAM, then gone.

## The two clips

Zero-shot conditioning. No fine-tune. One clip per persona. Not a scene,
not a chapter, not a third voice.

| Id | Research target | Length | Register |
|---|---|---|---|
| `samantha` | Voice of *Her* (2014) | about 10 s | Conversational. Paralinguistic tags may stay. |
| `jarvis` | Iron Man film voice | about 10 s | Calm command. Tags off. Hub layer stays separate. |

The Jarvis hub (clicks, alarms, virtual ambience) is a separate layer.
The toggle mixes it. Turning it off does not denoise the asset. The ABI
and the forbid-list are `symphony/JARVIS-TEXTURE.md`.

About ten seconds is the owner target in `VOICE-CLONING.md`. Chatterbox
Turbo's public code asserts a reference longer than five seconds, so the
same clip fits the Nano primary and the Turbo option. This outline names
no source URL, no timestamp, no scene, and no converter. A human who is
later allowed to place a file places one that is already that short
reference. A longer recording is the wrong object and is deleted. It is
not cut down inside this tree.

Persona ids on the bus are registers (`SAMANTHA-V2.md` §4, `IPC.md`).
They are not these clips.

## Gates before any fetch

Every row has to be true before anyone copies, downloads, or records a
clip. A miss is a stop. The stop is the whole action: no search for a
source, no screen capture, no rip, no stream grab, no torrent, no paste
into a fetch script, no call to a dropped clone API.

| # | Gate | Where it lives | State in this change |
|---|---|---|---|
| 1 | Owner line that names the persona and says fetch is allowed | `LEDGER.md` | Absent. This file does not write one. |
| 2 | Performer or rightsholder consent, and a licence for the film performance. Both. Scope is local zero-shot conditioning. | The same LEDGER line, citing the instrument | Absent. |
| 3 | The licence leaves out redistribution, a cloud upload, and fine-tune | The same line | Absent. |
| 4 | `public_demo` explicit. Default is no. A demo that speaks the voice needs its own yes. | The same line | Absent. |
| 5 | Counsel when the paper is not a direct grant from the rightsholder. The operator does not self-clear. | LEDGER, or a counsel memo the line cites | Not requested here. |
| 6 | `reference_clips = false` has been changed by a later owner decision | `LOCKS.toml` `[formats.exceptions.v2s14_chatterbox]` | Still false. A LEDGER line alone does not flip it. |
| 7 | The worker contract has been changed to accept a path | `CLONE-WORKER.md`, `scripts/clone/chatterbox_worker.py` | Still a refusal. Exit 3, no PCM. |

The comparison `VOICE-CLONING.md` requires stays in force. In May 2024
OpenAI withdrew the Sky voice after Scarlett Johansson said it resembled
her performance in *Her*. OpenAI said Sky was not an imitation of her.
That dispute is the warning. "Research target" is not a licence.

Order, when a later change is allowed to run it:

1. Read `LEDGER.md` and the Chatterbox pin. Both have to allow the fetch. Either one closed means stop.
2. A human who already holds a file they are entitled to place copies it into the store below. The worker is not the process that fetches.
3. Hash the bytes. Write the hex into the LEDGER line. A mismatch deletes the file.
4. Only then may a future worker open the path.

`scripts/clone/fetch-clone-weights.sh` is the weight fetch. It refuses
unless `GALION_CLONE_FETCH=1`, and it refuses `.pt` and audio suffixes
(`.wav`, `.mp3`, `.flac`, `.mp4`, `.mkv`, `.mov`). CI does not set that
flag. Clip bytes do not get a row in `scripts/clone/PINS.sha256`. That
file is five safetensors pins. Dropped vendors stay dropped: ElevenLabs,
Cartesia, Play.ht, hosted Resemble, Inworld, Fish Audio, Azure Personal
Voice.

## Storage layout

Film audio stays out of git. That covers the work tree, Git LFS, a
submodule, a fixture, a CI artifact, a commit on a side branch, and a
GitHub Release asset. `clone-weights-v1` carries safetensors only
(`CLONE-WEIGHTS.md`).

`weights/clone/` is the Chatterbox safetensors store. `weights/.gitignore`
ignores everything under `weights/` except itself. Audio does not go in
that directory. The weight fetch already refuses those suffixes.

The store this outline names is not created, and no program here reads it:

```text
$GALION_CLIP_STORE/                 # absolute path outside the git work tree
  samantha/reference.wav            # the only audio object for that persona
  samantha/reference.wav.sha256
  samantha/consent.txt              # text; the LEDGER id; no samples
  jarvis/reference.wav
  jarvis/reference.wav.sha256
  jarvis/consent.txt
```

`GALION_CLIP_STORE` is unset in this repository and stays unset in CI.
Unset means the store does not exist. There is no default under the
clone, under `weights/`, or under `/tmp`.

Reply audio is a different object. The worker contract is text in, raw
24 kHz PCM out, no file of samples. `SECURITY-EDGE.md` keeps phone-path
PCM in RAM and out of the demo log. These clips do not add a raw-audio
folder on that path.

## Consent logging

Two consents. They stay separate.

Caller consent is `LOCKS.toml` `[consent]`: `sense`, `adapt`, `remember`,
`share`, default deny. Those bits travel on the emotion wire and the bus
`Consent` body. They gate the caller's affect. They do not authorize a
film voice. `prohibited_purposes` still includes workplace and education.
`withdraw_wipes_ram_history` stays true. An explicit "be Samantha" or
"Jarvis" on the session (`SAMANTHA-V2.md`) selects a register. It is not
performer consent.

Clip consent is an owner line in `LEDGER.md`, after an instrument exists.
The line is text. It carries:

- date and clock, Europe/Zagreb
- persona, `samantha` or `jarvis`
- work title, and the performer as the instrument names them
- grantor, instrument id
- scope `local-zero-shot-conditioning`
- `public_demo`, default `no`
- expiry
- operator
- sha256, filled only after the file exists and the hash matches

The line carries no samples, no transcript of the performed line, and no
URL. A future `consent.txt` beside the wav repeats the LEDGER id. The
copy that counts is the ledger line. No such line exists. This outline
does not append a rights grant.

## Hashing

When a file exists — none does, and this change computes no digest — the
check is the same one the weight fetch uses:

1. `sha256sum` of the wav bytes.
2. Write `reference.wav.sha256` next to the wav, outside git.
3. Copy the hex into the LEDGER line for that persona.
4. On mismatch, delete the wav and the sidecar. Do not hand the path to the worker. The ledger records the mismatch and the time. It does not keep the bytes.

The clip hash does not go in `scripts/clone/PINS.sha256` and does not go
in `LOCKS.toml`. This file prints no placeholder digest.

## Retention

The term is the grant expiry, or withdrawal, whichever comes first.

The file lives on the one machine that would run the clone worker. It
does not live in CI, on the git remote, in an object bucket, or at a
dropped vendor. A copy on the VPS waits on a later LEDGER line with
`public_demo=yes`. That demo still follows `SECURITY-EDGE.md`: login at
the edge, exact `https://` origins, Art. 50 left on, request inspector
off so a body is not replayed.

The clip is a conditioning prompt for one local worker. It is not
training data. Generated speech stays RAM-only, then dropped, as the
phone path already does. The phone log does not grow a field for the
reference.

## Clone-worker handoff

Owner t273 opens local send. The runbook is `symphony/CLONE-LANDING.md`.
`LOCKS.toml` is not edited here, so `reference_clips` stays false. The
worker accepts a path anyway, and only the pinned one.

The stdin line is `{"text","model","persona","tags","device","denoise"}`.
`device` is `cpu`. `denoise` is false. When the pinned wav is on disk and
the sha256 matches, the line also carries `audio_prompt_path` and
`reference` set to that same path, plus `texture`. A missing file omits
those keys (CI skip). A foreign path, an MP3, a hash mismatch, a GPU
device, or a denoise flag exits 3 with no PCM. Jarvis still forces `tags`
false. Hub texture is not cleaned. An unknown persona name is still
treated as Samantha.

`BusBody::Persona` carries `id` and `source`. It does not carry samples
or a filesystem path. `Utterance` is the caller's turn. A film clip does
not ride either body, and it does not ride `POST /turn`.

If the clone slot is absent, Piper (`en_US-ljspeech-medium`) speaks, then
flite. A refused clip does not become a cloned voice on the way down.

The child still does not fetch. It does not rewrite the wav. Stdout stays
raw 24 kHz s16le. No wav of the reply is written. The public demo is
still closed. Generate is still not called.

## Art. 50 and the watermark

`SECURITY-EDGE.md` already shows, and speaks on the first tap, both of:

- HR: `Razgovarate s umjetnom inteligencijom. Galion je AI asistent, a ne osoba.`
- EN: `You are talking to an AI. Galion is an AI assistant, not a person.`

The first accepted `POST /turn` repeats those strings in `ai_disclosure`
(`article` 50). Later turns do not. `GALION_AI_DISCLOSURE=0` is for
benches. A public demo leaves the disclosure on.

A voice a listener could take for a performer keeps that spoken line.
It does not move the disclosure behind a flag. The research note in
`docs/research/galion-voice-v2/domains/07-security-privacy.md` (synthesis
dated 2026-11-03, not a LOCKS change) asks for a machine-readable mark
beside the audible label. The product note for that mark is the Perth
watermark upstream Chatterbox `generate()` applies, as `VOICE-CLONING.md`
already records. Perth stays unvendored. Its licence was not read in
that note. Generate is not wired on the stub. A watermark is not stripped,
and this outline names no switch that strips one.

The disclosure is not rights clearance. The watermark is not rights
clearance. With `reference_clips = false`, this tree is not on the
film-voice path.

## Deletion

Delete on grant expiry, withdrawal, an owner instruction, a hash
mismatch, a file that is the wrong length or the wrong persona, or a
clip that has landed in the work tree.

1. Leave the worker's clip keys unused. Do not send the path.
2. If a future worker has loaded the clip, stop that process so the embedding leaves RAM. No grace period. That matches `withdraw_wipes_ram_history`.
3. Unlink `reference.wav`, `reference.wav.sha256`, and `consent.txt` in the outside store.
4. Confirm the path is gone.
5. Append a LEDGER line: persona, the hash that was destroyed, time, reason. The line has no audio.

Film audio is not committed, so git history has no blob to scrub. A
staged wav is dropped before the commit is published, and it is not
pushed. A blob that did reach the remote is an owner incident. A second
copy does not stay in the repo as evidence.

## Closed, as of this file

| Check | State |
|---|---|
| LEDGER local send, owner t273 | Written. Not a counsel opinion. Not a public demo. |
| `reference_clips` | `false`. Lock file not edited. |
| Worker accepts a clip key | Only the two pinned wavs, and only when the sha256 matches. Anything else exits 3 with no PCM. |
| `GALION_CLIP_STORE` | Read when set. Unset falls through to the gitignored repo path, then skips if that is absent too. |
| Clip bytes in git | None. |
| sha256 of a clip | Pinned in `scripts/clone/REFS.sha256` from the owner's digests. Recomputed when a file is present. |

## What the landing still does not do

- No fetch of the wavs. No commit of the wavs. No LOCKS edit.
- No Perth package. No model download. No paid API. No GGUF. No GPU.
- No public demo in either film voice. No denoise of the Jarvis hub.

— Maciej Grajczyk
