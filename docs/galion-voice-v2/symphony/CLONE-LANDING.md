# Symphony B · Clone landing

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. The harness is the spec.

Master slogan: "The seamless connection with the human being, on the voice only."

## Policy

Cloning is Rust-first. This landing's clip emit is Rust (`speech::galion_clip`). The Python child is the protocol stub. Piper training on own-voice clips is forbidden. Reference wavs stay outside git and are read-only. The deployed Jarvis voice must not change. Hub texture stays in the reference file. WER, ECAPA, MOS, and timings are not measured on this page. Those gates stay **BLOCKED**. No audio file is added. `LOCKS.toml` is not edited.

This is the landing for the **current** Samantha and Jarvis references on the
current Chatterbox Nano worker. It does not swap models. Turbo stays the
quality option already named on the worker line. It does not fetch weights.
It does not edit `LOCKS.toml`.

## What lands

| Piece | Value |
|---|---|
| Model | `chatterbox-nano` (TOML `model = "nano"`). Not swapped. |
| Process | `scripts/clone/chatterbox_worker.py`, one child, PyTorch only if a later generate is wired inside that process. This landing does not import torch. |
| Device | CPU only. `GALION_CLONE_DEVICE=cpu`. `CUDA_VISIBLE_DEVICES` empty. A `cuda` / `gpu` / `mps` request exits 3 and writes no PCM. |
| Session size | **1 vCPU** for this child (`threads=1`). `GALION_CLONE_THREADS` above 1 is clamped to 1. |
| Voice budget | Planning band from the CPU foundation: ~1.0 vCPU average per voice session (ASR 0.4, TTS 0.3, VAD/SER 0.15, plus reserve), burst ~4 vCPU across that cascade, ~0.65–1.15 GB RAM pooled. The burst is not given to this process. RAM is not a measured RSS and is not an enforced ulimit. |
| Frame | `LOCKS.toml` `frame_ms = 80`. Read, not edited. `Heartbeat::step` does not run this child. |
| GGUF | Refused. Not a voice weight. The symphony path stays PCM here and `.gq` when that compiler exists. |
| Public demo | Closed. `/v2/state` still says `rights: hold` and `film_voice: false`. The mouth on that page is still Piper or flite until generate is actually on. |

## References

Owner t273 clears **local** send of the two current wavs. No counsel name is
recorded. The wavs stay outside git and are read-only. MP3 is never a reference. This tree does
not fetch them, does not commit them, and does not add an audio file.

| Persona | Repo path (gitignored) | Store path | sha256 | Shape | Texture |
|---|---|---|---|---|---|
| Samantha | `docs/galion-voice-v2/personas/refs/samantha.wav` | `$GALION_CLIP_STORE/samantha/reference.wav` | `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` | 17.41 s, 44100 Hz, stereo, 16-bit | `preserve` |
| Jarvis | `docs/galion-voice-v2/personas/refs/jarvis.wav` | `$GALION_CLIP_STORE/jarvis/reference.wav` | `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` | 15.95 s, 48000 Hz, stereo, 16-bit | `preserve-hub` |

On the owner machine the store is:

```bash
export GALION_CLIP_STORE=/workspace/galion-clip-store
```

Unset is valid. The worker then uses the repo path when that file exists.
CI has neither. A missing file is a skip, not a failure: the request line
omits `audio_prompt_path` and the protocol stub still answers.

The worker sends a path only when all of these are true:

- persona TOML has `present = true`, `send_reference = true`, `denoise = false`, and `rights` is not `hold`
- the path is exactly the repo ref or `$GALION_CLIP_STORE/<persona>/reference.wav`
- the basename is `<persona>.wav` or `reference.wav`
- the sha256 matches `scripts/clone/REFS.sha256`

A mismatch, an MP3, a foreign path, or disagreeing reference fields exits 3
with no PCM. Piper still speaks when the clone slot is absent.

`reference_clips` in `LOCKS.toml` stays `false`. This landing does not flip
that key. Local send is the t273 worker path recorded in `LEDGER.md`.

## Jarvis texture layer

The Jarvis reference is not clean dialogue. It is the Tony Stark hub:
clicks, alarms, ambience. That texture is a **separate layer**, not a
filter step and not something the worker bakes into every sample.

| | |
|---|---|
| Layer id | `jarvis-hub` |
| Policy | `preserve-hub` |
| Toggle | `GALION_JARVIS_TEXTURE` or the JSON field `texture_layer` |
| Default | on (`1`) |
| Off | the layer stays in the Galion clip with `enabled=0` and `stripped=0`. It is not mixed. The wav is not cleaned. |
| On | `enabled=1`. A consumer may mix the layer. This landing does not denoise a stem out of the wav. |
| Strip | refused. `strip_texture` exits 3 and writes no PCM. |

Samantha has no hub layer (`present=0`).

The reference file is never rewritten. Off does not mean "remove the clicks
from the wav". It means the layer is not applied.

## Galion clip

Chatterbox safetensors stay the weight files in the other process. They are
not the runtime clip. `.gq` (`GALIONGQ`) stays the tensor reader; its writer
is still deferred. The landing emit is a Galion Voice Clip, magic `GALIONVC`,
built by `speech::galion_clip::emit_landing`.

The clip holds the utterance text, 24 kHz s16le PCM, the reference sha256,
and, for Jarvis, a 64-byte hub layer record. No GGUF. No filtered texture
stem. Round-trip is `read_clip`.

## Lock flip

`LOCKS.toml` `[formats.exceptions.v2s14_chatterbox]` `reference_clips` is
still `false`. This change does not edit that file. The lock flip is
**BLOCKED-on-owner**. Wiring, the clip emit, and the texture toggle land
anyway. A public demo stays closed.

## Credits

Chatterbox is MIT, copyright 2025 Resemble AI. It is called out of process
and is not vendored. No Apache-licensed source is copied into this landing.
The Galion Voice Clip and this worker are Perpetual Open MIT, copyright
2026 Maciej Grajczyk. Piper, when the clone slot is absent, remains the
existing A21 path.

## Run the harness

From the repo root. No network. No weight fetch.

```bash
scripts/clone/clone-landing-smoke.sh
```

Pass means:

- `frame_ms` is 80 and `reference_clips` is still false
- the worker source does not import torch, GGUF, or a denoise tool
- a stub utterance returns PCM and logs `threads=1 device=cpu model=nano gguf=false`
- denoise, GPU, MP3, and a bad sha256 produce no PCM
- each pinned wav that exists matches its sha256 and is accepted
- each pinned wav that does not exist prints `status=skip reason=ref-absent`
- the last lines say `generate=blocked reason=weights-not-fetched` until the safetensors are on disk, then `generate=unwired` because this process still does not call Chatterbox `generate`

Weights, when an owner wants them later, are a separate decision:

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh
```

That script still refuses audio suffixes. It is not part of this harness.

Protocol PCM (`GALION_CLONE_STUB_PCM=1`) is the pipe. It is not Samantha's
voice and not Jarvis's voice. `product_claim` stays false until an engine
labelled `model` is written in `LEDGER.md`. This file does not write that.

## Request line

With no wav on disk:

```text
{"text":"Hello.","model":"nano","persona":"jarvis","tags":false,"device":"cpu","denoise":false}
```

With the pinned Jarvis wav present, the same line also carries
`audio_prompt_path`, `reference` (the same path), and
`"texture":"preserve-hub"`. Jarvis also sends `texture_layer` (`true` or
`false`). The bytes stay in the file. They are not in the JSON and not in
the log. Stdout stays raw 24 kHz s16le. The parent emits a `GALIONVC` clip.
No reply wav is written.

## Blocked, honestly

Chatterbox generate is not called. The Nano safetensors are not in git and
are not downloaded by this change. Until both exist, `SpeechStack` does not
select the clone slot (`weights_ready` is false) and Piper or flite speaks.
A CPU time-to-first-audio number is not claimed. WER, ECAPA, MOS, and timings stay **BLOCKED**. The 0.65–1.15 GB RAM band
is the planning figure already written above, not a new measurement from this machine. The deployed Jarvis voice must not change.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
