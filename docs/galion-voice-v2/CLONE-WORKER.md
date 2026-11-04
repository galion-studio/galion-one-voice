# V2-S14 · Clone worker contract

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Nano generate is wired in the child. A run still needs weights and an allowlisted reference on that machine. This file is not a claim that a number was measured here.
Europe/Zagreb. Nano speaks only behind the env gate in `NANO-BACKEND.md`.
Piper speaks otherwise. Not a claim that a film voice shipped.
Europe/Zagreb. The child can call `generate` when `GALION_CLONE_GENERATE=1`.
CI does not set that flag. How to run it, and the wavs Maciej still has to
place, are `CHATTERBOX-WORKER.md`.

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
| Disk | no PCM cache | no raw PCM cache; the compiler lands `voices/gq/<persona>.gqvoice` |
| Fallback | flite if the pack is missing | Piper, then flite |

Request line, written by `clone_worker::request_line`. Device is `cpu`.
`denoise` is false. A reference field is added only for a pinned wav that
is present and matches `scripts/clone/REFS.sha256` (`symphony/CLONE-LANDING.md`).
Any other non-empty `reference`, `audio_prompt`, `audio_prompt_path`, or
`clip` exits 3 and writes no PCM. MP3 exits 3. A denoise request exits 3.
Request line, written by `clone_worker::request_line`. There is no reference
field on that line. The child resolves a local file itself when
`GALION_CLONE_LOCAL_REFS=1`. Without that variable, a non-empty
`reference`, `audio_prompt`, `audio_prompt_path`, or `clip` exits 3 and
writes no PCM.
field. A child that sees a non-empty `reference`, `audio_prompt`,
`audio_prompt_path`, or `clip` exits 3 and writes no PCM. The only
exception is `GALION_CLONE_LOCAL_REFS=1` together with an operator sha256
allowlist, below. That exception does not edit `LOCKS.toml`.
Request line, written by `clone_worker::request_line`. A `reference` path
is added only when `symphony/CLIP-STORE.md` says the gate is open: LEDGER
`PRIVATE CLEARED` for that persona (t273), `send_reference = true`, and a
WAV under `$GALION_CLIP_STORE` outside the work tree. Jarvis may also
carry `"texture":"hub"` or `"texture":"off"` (`GALION_JARVIS_TEXTURE`).
The texture flag does not rewrite the file. `audio_prompt`,
`audio_prompt_path`, and `clip` still exit 3 and write no PCM. A
`reference` that fails the store check does the same.
field. A speak line that carries a non-empty `reference`, `audio_prompt`,
`audio_prompt_path`, or `clip` exits 3 and writes no PCM. A consented clip
uses `op: condition` instead. The parent sends that line only when
`consent_ready` is true.

```text
{"text":"Hello.","model":"nano","persona":"samantha","tags":true,"device":"cpu","denoise":false}
```

`model` is `nano` (primary, not swapped) or `turbo` (quality option, same
process shape, same one-thread CPU cap). `persona` is `samantha` or
`jarvis`. Jarvis sets `tags` false and the stub strips bracket tags from
the text. Samantha may keep them for a later model. An unknown persona
name is treated as Samantha. The child is pinned to one thread
(`session_vcpu=1`). A GPU device is refused. GGUF is refused.

## Runtime packet

Worker stdout stays raw s16le. That buffer is not what the speech stack plays.
`CloneTts::synthesize` compiles each clause through `speech::gqvoice`.
When the utterance finishes, the stack writes `voices/gq/<persona>.gqvoice`
(`GALION_GQVOICE` overrides the directory) and returns the samples it read
back from that file. The format, the 80 ms sectors, and the Samantha and
Jarvis commands are [`GQVOICE.md`](GQVOICE.md). Piper still speaks when the
child or the compiler fails. The child still does not write a file.
`model` is `nano` (primary) or `turbo` (quality option, same process shape).
Generate in this worker is Nano. A turbo request is `galion-clone-unwired`
and no samples. `persona` is `samantha` or `jarvis`. An unknown persona
name is treated as Samantha. Jarvis on the Rust line sets `tags` false,
and the Nano path then strips bracket tags. A request that omits `tags`
keeps them, which is what the 2026-11-03 smoke sent. The protocol stub
still strips Jarvis tags by persona.

## Wire

Stdout is PCM only. Before torch is imported, the child dups fd 1 and
points fd 1 at stderr, so upstream `print` (`loaded PerthNet`,
`S3 Token -> Mel`) cannot shift the s16le stream. Progress is stderr.
The child flushes `galion-clone-utt: N` before it writes N PCM bytes.
The Rust reader waits until those N bytes have arrived. That is the
deadlock fix: a parent that reads stderr first no longer stalls on a
full stdout pipe, and a parent that polls both does not stop at the
marker while PCM is still in flight.

- does not import torch unless `GALION_CLONE_GENERATE=1`
- does not download weights
- does not call Chatterbox generate
- accepts only the two owner-cleared wavs, and only when the sha256 matches
- does not denoise, and does not load GGUF
`GALION_CLONE_STUB_PCM=1` is protocol PCM, not a voice and not a Flash
result. It still refuses a reference.

## When generate runs

`scripts/clone/chatterbox_worker.py` calls
`ChatterboxTurboTTS.from_local(ckpt, "cpu", nano=True)` and then
`generate`. The call is local. There is no `from_pretrained` and no
download. The reference path is passed through on the first utterance
for that file. Nothing here denoises, trims, or high-passes it.
Conditionals are reused for the next line. Perth stays inside upstream
`generate`. The engine line is `chatterbox-nano`, not `model`.

All of the following have to be true, or the child does not generate:

1. Pinned Nano safetensors are on disk (`weights/clone/`, or `GALION_CLONE_WEIGHTS`). A missing pack exits 2, `galion-clone-absent`.
2. The five tokenizer text files match `scripts/clone/TOKENIZER.sha256` under `weights/clone/tokenizer/`. They are not release assets and `publish-clone-weights.sh` does not upload them. `conds.pt` is not used. A missing tokenizer exits 2.
3. `GALION_CLONE_LOCAL_REFS=1`. Without it, weights present still means `galion-clone-unwired` and zero samples, and a reference key is still the rights-hold refusal.
4. The reference sha256 matches `scripts/clone/REFS.sha256` for that persona. Samantha is `bdcb0c29…c7a8693`. Jarvis is `c2200b59…4b532c60`. The other persona's file is refused. A sidecar `reference.wav.sha256`, when present, has to match the bytes.
5. The runtime exposes `nano=True`. PyPI `chatterbox-tts==0.1.7` does not. Upstream `resemble-ai/chatterbox` `5de7a54` does. A missing runtime exits 2 with no PCM and no traceback on the success path of the gate tests.

The file is `GALION_CLIP_STORE/<persona>/reference.wav` when that variable
is set. An empty store does not fall through. When it is unset, the child
looks at `docs/galion-voice-v2/personas/refs/<persona>.wav`. A JSON
`audio_prompt_path` (or `reference`, or `clip`) is accepted only under
the same sha rule. The Rust request line still does not send a path.

`LOCKS.toml` is not the switch. `reference_clips` stays false. This
worker does not edit it.

`Heartbeat::step` does not spawn this process and does not call
`generate`. The phone lane drains reply PCM one 1920-sample frame at a
time outside that step. Piper (`en_US-ljspeech-medium`), then flite,
still speaks when this slot is absent, unwired, or the child dies.

`GALION_CLONE_STUB_FRAMES` is not the symphony feed and not a voice.
When `GALION_SPEECH=a21`, `A21RingFeed` reads real Piper PCM, resamples it
to 24 kHz, and the heartbeat step drains one 1920-sample frame. See
`symphony/HEARTBEAT-WIRE.md`.
## What the worker does

`scripts/clone/chatterbox_worker.py` writes PCM in chunks. A chunk marker
on stderr comes before the bytes, then `galion-clone-utt:` closes the
utterance. The parent clocks the first bytes. It does not wait for that
closing line to know that audio has started.

The stub (`GALION_CLONE_STUB_PCM=1`) does not import torch and does not
call Chatterbox. Its two writes are protocol PCM, not a voice. The first
write is one sample. The second follows about 50 ms later so a gate can
see that the first bytes are not the finished utterance.

Without that flag, and without the pinned safetensors on disk, the process
exits 2 (`galion-clone-absent`). With the files present and
`GALION_CLONE_STREAM` unset, each request is `galion-clone-unwired` and no
samples are written. The unwired path does not call `generate()`.

`GALION_CLONE_STREAM=1` loads Nano from `GALION_CLONE_CKPT` and streams.
Master has no `stream()` method (that is unmerged PR 528). The worker uses
`stream()` when the install has it, otherwise a token loop plus meanflow
with `finalize=False`. A model that can only block in `generate()` is
refused, and Piper remains the fallback. The design and the gate are
`STREAM-TTFA.md`.

A reference clip still exits 3. `GALION_CLONE_LOCAL_REFS=1` plus an
operator sha256 allowlist (`GALION_CLONE_REF_SHA` or
`GALION_CLONE_REF_ALLOW`) is the only way a path is accepted. This file
does not read `LOCKS.toml`.
- calls Chatterbox `generate` only on that flag, on CPU, after a consented reference is conditioned
- refuses a reference on the speak line (exit 3, no PCM)

Without `GALION_CLONE_STUB_PCM=1`, and without the pinned files on
disk, the process exits 2 (`galion-clone-absent`). With the files present
and both the stub flag and the generate flag off, each request is
`galion-clone-unwired` and no samples are written. Protocol PCM is not a
voice and not a Flash result. `engine=model` is written only after a real
`generate`.

`SpeechStack` selects this slot only when `weights_ready` is true (size and
sha256 against `scripts/clone/PINS.sha256`). CI does not have those files
and does not set `GALION_CLONE_LOCAL_REFS`. `GALION_SPEECH=auto` therefore
stays on Piper or flite. `GALION_SPEECH=clone` with the pack absent falls
through to Piper, then flite. A dead clone child on a later speak does
the same handoff without restarting the lane.
Request line, written by `clone_worker::request_line_prosody`. There is no
reference field. A child that sees a non-empty `reference`, `audio_prompt`,
`audio_prompt_path`, or `clip` exits 3 and writes no PCM. The clip path is
not on this line. The child resolves it from `GALION_CLIP_STORE` when the
env gate is open.

{"text":"Hello.","model":"nano","persona":"samantha","tags":true,"prosody":{"pitch_st":0.0000,"range":1.0000,"rate":1.0000,"energy":1.0000,"warmth":0.5000,"pre_pause_ms":0}}

`prosody` is the latest 80 ms `galion-emotion` decision (pause and loudness
are applied to the PCM; the other numbers are logged). It is not an emotion
name and not a sentence.

`persona` is `samantha` or `jarvis`. Jarvis sets `tags` false and the worker
strips bracket tags from the text. Samantha may keep them. An unknown persona
name is treated as Samantha.

## When Nano runs

`scripts/clone/chatterbox_worker.py` calls `ChatterboxTurboTTS.from_local`
on CPU only when all of these are true:

- pinned safetensors match `scripts/clone/PINS.sha256`
- `tokenizer.json` is on disk (`GALION_CLONE_TOKENIZER`, `GALION_CLONE_CKPT`, or `weights/clone/tokenizer`)
- `GALION_CLONE_LOCAL_REFS=1`
- `$GALION_CLIP_STORE/<persona>/reference.wav` matches `reference.wav.sha256`

The child sets `HF_HUB_OFFLINE=1` and does not call `from_pretrained`.
Progress is stderr (`galion-clone-progress:`). PCM is stdout after generate.
Library prints during generate are pointed at stderr so they cannot enter
the PCM pipe.

Without `GALION_CLONE_STUB_PCM=1`, and without the weight files on disk,
the process exits 2 (`galion-clone-absent`). A sha miss, a missing
tokenizer, or a missing `chatterbox` install is `galion-clone-unwired` and
no samples. The host then speaks with Piper. Protocol PCM
(`GALION_CLONE_STUB_PCM=1`) is not a voice and not a Flash result.

`SpeechStack` selects this slot only when weights, tokenizer, and at least
one allowlisted persona ref are all present. CI has none of those.
`GALION_SPEECH=auto` therefore stays on Piper or flite. `GALION_SPEECH=clone`
with the gate closed falls through to Piper, then flite. A dead clone child
on a later speak does the same handoff without restarting the lane. A
persona whose own sidecar does not match speaks that utterance with Piper
and leaves the Nano child up.

The runbook is `NANO-BACKEND.md`. `LOCKS.toml` `reference_clips` stays false.

## Fetch

`scripts/clone/fetch-clone-weights.sh` refuses unless `GALION_CLONE_FETCH=1`,
including `--check`. It downloads the pinned `.safetensors` into
`weights/clone/`, which git ignores. The bytes are GitHub Release assets on
tag `clone-weights-v1` (`CLONE-WEIGHTS.md`). A hash mismatch deletes the
partial. `.pt`, audio suffixes, and Samantha or Jarvis filenames are refused.
The content hashes were read from the Hugging Face tree API on 2026-11-03
(LFS oids). They are records, not blobs. Tokenizer text is a separate pin
file, `scripts/clone/TOKENIZER.sha256` (nano commit `71ccd1d`). It is not a
release asset and this fetch does not upload it. WAVs are not fetched.
including `--check`. It downloads the pinned `.safetensors` files and the
tokenizer text into `weights/clone/`, which git ignores. A hash mismatch
deletes the partial. `.pt` and audio suffixes are refused. The tensor pins
were read from the Hugging Face tree API on 2026-11-03 (LFS oids). They
are records, not blobs.

Nano (`--nano`): `t3_nano_v1.safetensors` plus shared `s3gen`,
`s3gen_meanflow`, `ve`. Turbo adds `t3_turbo_v1.safetensors`. The release is
not in a clean clone. Until the admin upload lands, the script exits 1 with
`BLOCKED`. `GALION_CLONE_WEIGHTS_SOURCE=hf` still reads the same pins from
Hugging Face (`ResembleAI/chatterbox-nano`, `ResembleAI/chatterbox-turbo`).

## Rights

Owner t273 clears local send of the two current wavs. The ledger line is
in `LEDGER.md`. No counsel name is written there. The public-demo boxes in
`RIGHTS-CLEARANCE.md` stay unchecked. `LOCKS.toml` `reference_clips` stays
false; this file does not edit the lock. The persona id is still the
register from `SAMANTHA-V2.md` §4. This worker does not fetch audio.
The landing runbook is `symphony/CLONE-LANDING.md`. The ingest outline is
`CLIP-INGEST.md`.

The later build is `CHATTERBOX-WORKER.md`: architecture, IPC, lifecycle,
the clip pipeline after rights, and TTFA. It does not change this stub and
does not edit `LOCKS.toml`.
Jarvis and Samantha, as film voices, stay on the hold in `VOICE-CLONING.md`.
The checklist is `RIGHTS-CLEARANCE.md`. Both gates are closed.
The persona id on this worker is the register from `SAMANTHA-V2.md` §4.
It is not a conditioning clip. This scaffold does not fetch film audio.
The ingest outline, still closed, is `CLIP-INGEST.md`.
A Jarvis hub toggle is not a denoise flag and not a reference clip.
The ABI is `symphony/JARVIS-TEXTURE.md`. This scaffold does not mix the layer.
The checklist is `RIGHTS-CLEARANCE.md`. `reference_clips` stays false.
The env flag plus the sha allowlist is how a machine that already has
those two files can condition. It is not a clearance line and it does
not fetch film audio. The ingest outline is `CLIP-INGEST.md`. The smoke
procedure is `CLONE-SMOKE.md`.
Jarvis and Samantha, as film voices, have an owner PRIVATE CLEARED line
in `LEDGER.md` (t273, confirmed t287). Counsel is not named there. The
checklist boxes in `RIGHTS-CLEARANCE.md` stay unchecked. Public demo
stays closed. `LOCKS.toml` `reference_clips` is still false and still
needs an owner flip. The clip store is `symphony/CLIP-STORE.md`.
This scaffold does not fetch film audio and does not call generate.
The older ingest outline is `CLIP-INGEST.md`.
A conditioning file, when the operator already has one, lives outside git
and is opened only under the sha gate in `NANO-BACKEND.md`. This tree does
not fetch film audio. The ingest outline is `CLIP-INGEST.md`. Gates 1–6
there stay closed.
A film clip is a different object. The clone-time consent gate
(`crates/galion-consent`) accepts `op: condition`
only when `consent.txt` has every field from `RIGHTS-CLEARANCE.md`, the
wav is mono 16-bit PCM of about 10 seconds (8–12), the sha256 matches,
the store is outside git, and `reference_clips` is true. The lock is
still false. This tree does not fetch the wav. The supply list is
`CHATTERBOX-WORKER.md`. The ingest checker is `scripts/clone/ingest-clip.sh`.

— Maciej Grajczyk

## Merge note (#53 + #54 + #55)

Two Nano paths share this worker after the merge. Without
`GALION_CLONE_STREAM`, the #53 path calls `generate` for the whole clause
(engine `chatterbox-nano`, marker before PCM). With `GALION_CLONE_STREAM=1`,
the #54 path streams chunks through `scripts/clone/nano_stream.py`
(engine `nano`, `galion-clone-path: stream`, marker after PCM). Both use the
same reference gate (`GALION_CLONE_LOCAL_REFS=1` plus the sha256 allowlist)
and the same one-thread CPU pin. Older sentences above that say the worker
"does not call generate" describe the pre-#53 stub. Only the stream path
with real Nano PCM is a TTFA product claim (`FlashReport::from_pcm`).

### Merge note: #56 (Nano V2 backend) onto main
- The host keeps main's generate worker and gqvoice landing. #56 adds the following on top:
  - `GALION_CLONE_LOCAL_REFS=1` and a sidecar/REFS ref pre-check (`ref_allowlisted`, `CloneTts::ref_ok`);
  - a protocol-stub refusal in `try_new`;
  - a tokenizer gate that accepts either main's pinned vocab.json+merges.txt or a tokenizer.json;
  - a flat `GALION_CLONE_CKPT` weight root;
  - HF offline env;
  - per-utterance `set_persona` and `set_prosody`. The worker logs the prosody numbers as `galion-clone-prosody`.
- Frame prosody (pause and loudness) is applied to Piper and flite PCM. Clone PCM is not rewritten after the child, because the landed `.gqvoice` must equal the playback buffer.
- Fixed a pre-existing bug: a Jarvis request that carried a reference produced invalid JSON (the `reference` string was never closed), so the worker answered "bad json".
- HOLD check: the merged SpeechStack produced real local Nano speech, CPU only, with no stub and no cloud path. The test allowlist of CC0 donors was local only and was not committed. Each output sat closest to its own donor on ECAPA: Samantha 0.476 vs amy, Jarvis 0.587 vs alan.

## Conditioning cache (owner item 10, 2026-11-04)

Chatterbox Nano conditioning (`model.conds`: speaker embedding, prompt speech
tokens, S3Gen prompt features) used to be recomputed from the reference wav at
every worker start and kept only in memory. It is now saved once and reused.

- Format: **safetensors** (no pickle; `.pt` stays banned). F32 / I64 / I32
  tensors only, names `t3.*` / `gen.*` from a fixed list, plus `__metadata__`.
- Path: `$GALION_CLIP_STORE/<persona>/cond/nano-<key>.safetensors` and a
  `.sha256` sidecar. `<key>` hashes the format version, persona, model,
  reference wav sha256, a digest of `scripts/clone/PINS.sha256` +
  `TOKENIZER.sha256`, and the options (`exaggeration=0.0`,
  `norm_loudness=true`).
- Written only after the galion-consent gate: `clone_cache::plan_save` needs
  the gate's `Approval` for that persona with the same reference sha256, and
  the same approval in `galion_consent::global()`. The worker writes a temp
  file the Rust slot named; Rust verifies it (strict header reader, metadata,
  sizes) and renames it into place with the sidecar.
- Invalidation: a new reference sha256 or new pins give a new key; on save
  the old entries are renamed `*.off-<secs>` (never deleted). A file that
  fails any check on load is renamed aside and the worker recomputes from the
  wav. The worker also re-checks the sidecar and that the metadata
  `ref_sha256` equals the digest of the wav it resolved.
- Request fields: `cond_load` or `cond_save` plus `cond_meta`. Without them
  the worker behaves as before. `GALION_CLONE_COND_CACHE=0` turns it off.
- The stream path (`GALION_CLONE_STREAM=1`) now prepares or loads
  conditioning once per reference path instead of on every request.

Verified: Rust unit tests (`speech::clone_cache`), and a hand-run round trip
with the real `safetensors.torch.save_file` (torch 2.6.0, safetensors 0.5.3)
on synthetic tensors shaped like Nano's, read back by the Rust reader
(`python_written_file_verifies`, `#[ignore]`). **Not verified:** a real
clone through the gate, because `reference_clips = false` and no consented
reference exists, so `plan_save` is never reached in a live run.
