<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# S10 local server

Loopback UI process for Samantha. Mic PCM comes in on a WebSocket. TTS PCM,
state, and mood go out. The page is one dot. The brain, the ear, and the
mouth are traits. This crate does not link `horbm`, does not spawn Piper or
Vosk, and does not read a reference clip.

It is a standalone package. It is not a member of the repo workspace.
`cargo test` at the repo root does not build it. Dependencies are pinned
for rustc 1.83 (`tokio` 1.42.0, `hyper-util` 0.1.10). Newer releases of those
crates need a newer compiler.

## What it reuses

| Piece | Where it already lives | What this crate keeps |
| --- | --- | --- |
| 80 ms heartbeat | `crates/galion-heartbeat` `FRAME_MS` | mood tick and TTS slice |
| Vosk frame | `crates/galion-asr-vosk` 16 kHz, 320 samples | mic rate and frame size |
| Piper rate | `crates/galion-v2-live` `PIPER_HZ` 22050 | TTS PCM rate |
| Piper worker | `crates/galion-ml-voice` `piper_worker` | not spawned; mouth is `hold` |
| Chatterbox unit | `deploy/systemd/galion-clone-cbx.service` | not called; clone stays off the live path |
| One dot | `crates/galion-console` `v1-screen.html` | `#0B0F14` ground, near-white mark, tap to listen |
| Persona | `docs/galion-voice-v2/personas/SAMANTHA.md` | short plain lines; mood is a pad, not a script |
| Sandbox | `deploy/systemd/templates/galion-voice-stage.service` | `deploy/samantha-local.service` is a template, not installed |

`Pad::RESTING` is arousal `0.3`, valence `0.25`, dominance `0.5`. Each 80 ms
tick closes 25% of the gap toward the last reply. `ReplyEngine` is the trait
a real `horbm::Brain` implements through `HorbmAdapter`. `Brain::open(memory_db, knowledge_dir, cases)` is `OpenHorbm`. The `horbm` crate is not a dependency. It needs `horbm-memory` from `galion-studio/galion-knowledge` at `761a7f225304fc010379219e5b88c6669dd48f40`. This tree does not fetch that revision.

`GALION_SAMANTHA_REF` is reported on `/health` as `unset`, `set`, or `missing`.
The path is not logged and the bytes are not read.

The default mind is `FakeReply`: `Okay.` plus the heard words, at the resting
pad. The default ear is unwired: energy moves the dot, and silence does not
invent a transcript. The default mouth is `hold`: four 80 ms slices of a tone
at 22050 Hz. That tone is not Samantha's voice.

The wire format is [PROTOCOL.md](PROTOCOL.md).

## Linux

```bash
cd voice-stack/s10-local-server
cargo run --release -- --addr 127.0.0.1:8790 --ui ui
```

Open `http://127.0.0.1:8790/`. The process refuses any address outside `127.0.0.0/8`, including `0.0.0.0` and `::1`.

`GALION_S10_ADDR` and `GALION_S10_UI` override `--addr` and `--ui` when the flags are absent. The flags win when both are present.

```bash
cargo test
```

## Windows

From the crate directory, in `cmd` or PowerShell. The owner's home on that
laptop is `G:\galion`. This binary does not read `G:\galion`; it only checks
whether `GALION_SAMANTHA_REF` names a file.

```bat
cd voice-stack\s10-local-server
cargo run --release -- --addr 127.0.0.1:8790 --ui ui
```

```powershell
$env:GALION_SAMANTHA_REF = "G:\galion\voices\samantha-ref.wav"
cargo run --release -- --addr 127.0.0.1:8790 --ui ui
```

Then open `http://127.0.0.1:8790/`. Use `127.0.0.1`, not a LAN address.
Microphone capture needs that loopback origin. Playback is full gain.

The service file under `deploy/` is a unit template for a later install.
This crate does not copy it into systemd and does not start it.

## Tests

`cargo test` covers the codec, the loopback check, the origin check, the
mood ease, the fake reply, the HORBM adapter, the energy gate, and an
integration test that boots the server on `127.0.0.1:0` and drives the
WebSocket with a scripted ear. The scripted ear does not recognise speech.
It repeats one line so the socket path can be measured without Vosk.

## Measured

`cargo test --offline --test stream -- --nocapture fake_pipeline_over_websocket` on this tree, debug profile, rustc 1.83.0, Linux. Three runs, one `MEASURE` line each. These are not Piper, Vosk, or HORBM times. The mouth is the hold tone. `took_us` is the scripted reply engine. `mouth_us` is how long `hold_tone` took. TTS and mood gaps are client receive times.

Run 1: `health_min_us=184 health_p50_us=268 reply_wall_us=85705 took_us=1 mouth_us=294 tts_gaps_ms=[38, 81, 81] tts_gap_median_ms=81 mood_n=3 mood_gaps_ms=[74, 79] mood_gap_median_ms=79 typed_first_tts_us=41741`

Run 2: `health_min_us=157 health_p50_us=207 reply_wall_us=84088 took_us=1 mouth_us=284 tts_gaps_ms=[38, 80, 81] tts_gap_median_ms=80 mood_n=3 mood_gaps_ms=[77, 79] mood_gap_median_ms=79 typed_first_tts_us=42991`

Run 3: `health_min_us=145 health_p50_us=176 reply_wall_us=87405 took_us=1 mouth_us=307 tts_gaps_ms=[38, 81, 81] tts_gap_median_ms=81 mood_n=3 mood_gaps_ms=[73, 79] mood_gap_median_ms=79 typed_first_tts_us=41678`

`/health` over 20 calls (first call dropped): minimum 145–184 µs, median 176–268 µs. Scripted `took_us` was 1 on every run. Hold-tone build was 284–348 µs. Time from the last mic frame write to the reply text was 84088–87405 µs. Time from a typed line to the first TTS slice was 41678–42991 µs. The four TTS slices arrived 38 ms, then 80–81 ms, then 80–81 ms apart (median 80–81 ms). Mood events (n=3) were 73–79 ms apart (median 79 ms). The server asks for an 80 ms deadline after the first slice and an 80 ms mood tick; the test records arrival at the client.
