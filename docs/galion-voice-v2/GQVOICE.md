# `.gqvoice` — Galion voice packet

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

This is the runtime file for a landed clone. Chatterbox worker stdout is raw
s16le at 24 kHz. That buffer is the worker's mouth. It is not the file the
Rust speech stack plays.

## Path

One file per persona. A new compile replaces it.

| Persona | Path |
|---|---|
| Samantha | `voices/gq/samantha.gqvoice` |
| Jarvis | `voices/gq/jarvis.gqvoice` |

`GALION_GQVOICE` overrides the directory. An unknown persona name lands as
Samantha, the same rule as the worker.

`voices/gq/` is gitignored. The packet holds PCM, so it stays out of git
the same way a wav would. Do not commit `.wav`, `.mp3`, or `.pt`.
Reference clips under `docs/galion-voice-v2/personas/refs/` stay out of git.
The compiler does not read them.

## Who loads it

`CloneTts::synthesize` compiles each clause in memory
(`speech::gqvoice::accept_worker_pcm`). When the utterance finishes,
`SpeechStack` writes the packet and checks that the buffer it is about to
return is the audio just read back from that file
(`handoff_at` → `open_runtime`). `open_runtime` accepts only a path ending
in `.gqvoice`. A `.s16le`, `.pcm`, `.wav`, or `.gq` path is refused, and
`CHATTERBOX_IS_RUNTIME` is false. The phone lane still receives PCM16 WAV,
because that is the existing reply route. The samples in that WAV come from
the packet, not from worker stdout.

Piper does not write a `.gqvoice`. If the clone child dies, or the compiler
returns an error, `SpeechStack::fallback_piper` still speaks through the A21
Piper pin, then flite. That exception is not edited here.
A mixed utterance (clone clause, then a Piper fallback) does not replace
the last good packet.

The heartbeat does not compile and does not touch this file.
`galion-heartbeat` `step` stays allocation-free. The packet is built on the
speech path, which already sits outside that call.

## Layout

Little-endian. 64-byte header, then payload.

| Offset | Size | Field |
|---|---|---|
| 0 | 8 | magic `GQVOICE\0` |
| 8 | 2 | version `u16 = 1` |
| 10 | 1 | persona: `1` Samantha, `2` Jarvis |
| 11 | 1 | model: `1` nano, `2` turbo |
| 12 | 4 | sample rate `u32 = 24000` |
| 16 | 4 | sector samples `u32 = 1920` |
| 20 | 4 | sample count `u32` (real samples) |
| 24 | 4 | sector count `u32` |
| 28 | 4 | flags `u32 = 1` (verbatim copy) |
| 32 | 32 | SHA-256 of the payload |
| 64 | | payload, s16le |

Magic is not `RIFF` and not the weight magic `GALIONGQ`. A `.gq` file is a
weight container. This is a voice packet. The loader refuses raw s16le,
RIFF/WAVE, a `.gq` weight, a bad hash, a non-zero pad, and any flag other
than verbatim.

The payload is whole sectors. A short tail is zero-padded out to 1920
samples so a sector is always one frame. `sample_count` is the voice.
The stack plays `sample_count` samples. The pad is clock alignment, not
audio, and it must be silence or the loader refuses the file.

One sector is 80 ms at 24 kHz: `1920` samples. That is `LOCKS.toml`
`[clock] frame_ms` and `galion-heartbeat` `FRAME_SAMPLES`. There is no
second clock. The worker today returns a whole clause before this compiler
sees it, so the compiler does not invent an earlier first sample. Once the
packet is in memory, sector 0 is one heartbeat frame. The compiler copies
bytes and hashes them. It does not resample.

Cap: 120 seconds. Longer than that is not one utterance.

## Texture

Samples are copied. There is no high-pass, click remover, or denoise.
Jarvis's hub texture — clicks, alarms, room tone — stays in the payload
if the worker put it in the PCM. The verbatim flag is required.

## What stays out of process

Chatterbox PyTorch stays in `scripts/clone/chatterbox_worker.py`. This
compiler is Rust, in `galion-ml-voice` (`speech::gqvoice`), behind the
`speech` feature. No GGML. No in-tree ORT. No new runtime crate. The
worker still writes no file of its own. Weights are still fetched only
with `GALION_CLONE_FETCH=1`, and this tool does not fetch them.

The Python stub does not call generate. With `GALION_CLONE_STUB_PCM=1` it
emits protocol PCM. A packet built from that PCM is a format proof. It is
not a film voice and not a Flash result.

## Run

From the repo root. No weight download. Protocol PCM only, until generate
is wired in the worker.

Samantha:

```text
GALION_CLONE_STUB_PCM=1 cargo run --offline -p galion-ml-voice --features speech --example gqvoice -- --persona samantha --text "Hello."
```

Jarvis:

```text
GALION_CLONE_STUB_PCM=1 cargo run --offline -p galion-ml-voice --features speech --example gqvoice -- --persona jarvis --text "Systems are ready."
```

That lands `voices/gq/samantha.gqvoice` and `voices/gq/jarvis.gqvoice`.
`--model turbo` selects the turbo id in the header. `--out DIR` writes
somewhere else. `--pcm FILE.s16le` compiles a raw worker capture and does
not spawn Python. The file must be raw s16le, not a wav and not an existing
packet.

```text
GALION_CLONE_STUB_PCM=1 python3 scripts/clone/chatterbox_worker.py > /tmp/samantha.s16le <<'EOF'
{"text":"Hello.","model":"nano","persona":"samantha","tags":true}
EOF
cargo run --offline -p galion-ml-voice --features speech --example gqvoice -- --persona samantha --pcm /tmp/samantha.s16le
```

The example prints `raw_rejected=yes` when the loader has refused the
worker bytes, then the landed path. Exit 0 on a packet, 2 on a worker or
compile failure, 64 on bad arguments.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
