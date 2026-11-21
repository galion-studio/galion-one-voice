<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# S10 local server

Loopback UI process for Samantha. The directory is `voice-stack/local-server`, matching `voice-stack/ARCHITECTURE.md`. The socket is `galion-samantha-ws/1` on `127.0.0.1:7881`.

Mic PCM comes in on the WebSocket. Speaker PCM, dot, and mood go out. The page is one dot, `#F5F5F7` on `#0A0A0C`. The brain is not linked. A turn is an error code, and the mood tick stays `source: resting`. This crate does not spawn Piper or Vosk and does not read a reference clip.

It is a standalone package. It is not a member of the repo workspace. `cargo test` at the repo root does not build it. Dependencies are pinned for rustc 1.83 (`tokio` 1.42.0, `hyper-util` 0.1.10).

## What it reuses

| Piece | Where it already lives | What this crate keeps |
| --- | --- | --- |
| 80 ms heartbeat | `crates/galion-heartbeat` `FRAME_MS` | mood tick and speaker slice |
| Vosk frame | `crates/galion-asr-vosk` 16 kHz, 320 samples | mic rate code `1` |
| Speaker rate | architecture, 24 kHz | binary rate code `0` |
| Piper worker | `crates/galion-ml-voice` `piper_worker` | not spawned |
| Chatterbox unit | `deploy/systemd/galion-clone-cbx.service` | not called |
| One dot | `LOCKS.toml` `[ui]` | `#F5F5F7` on `#0A0A0C`, no second widget |
| Persona | `docs/galion-voice-v2/personas/SAMANTHA.md` | stock comfort and bracket tags stay unspoken |
| Sandbox | `deploy/systemd/templates/galion-voice-stage.service` | `deploy/samantha-local.service` is a template, not installed |

`Pad::RESTING` is arousal `0.3`, valence `0.25`, dominance `0.5`. `HorbmAdapter` and `OpenHorbm` match `Brain::open(memory_db, knowledge_dir, cases)` and `turn` / `tick`. The `horbm` crate is not a dependency. It needs `horbm-memory` from `galion-studio/galion-knowledge` at `761a7f225304fc010379219e5b88c6669dd48f40`. This tree does not fetch that revision.

`GALION_SAMANTHA_REF` is reported on `/health` as `unset`, `set`, or `missing`. The path is not logged and the bytes are not read.

The wire format is [PROTOCOL.md](PROTOCOL.md).

## Linux

```bash
cd voice-stack/local-server
cargo run --release -- --ui ui
```

Open `http://127.0.0.1:7881/`. Another loopback port, including `[::1]`:

```bash
GALION_SAMANTHA_WS='[::1]:7881' cargo run --release -- --ui ui
```

`0.0.0.0` and any non-loopback address refuse to start. `--addr` overrides `GALION_SAMANTHA_WS`.

```bash
cargo test
```

## Windows

From the crate directory. The owner's home on that laptop is `G:\galion`. This binary does not read `G:\galion`. It only checks whether `GALION_SAMANTHA_REF` names a file.

```bat
cd voice-stack\local-server
cargo run --release -- --ui ui
```

```powershell
$env:GALION_SAMANTHA_WS = "127.0.0.1:7881"
$env:GALION_SAMANTHA_REF = "G:\galion\voices\samantha-ref.wav"
cargo run --release -- --ui ui
```

Then open `http://127.0.0.1:7881/`. Microphone capture needs that loopback origin. Playback is full gain.

The unit file under `deploy/` is a template. This crate does not install it and does not start it.

## Tests

`cargo test` covers the codec, the loopback check (including `::1`, refusing `0.0.0.0`), the origin check, the unlinked brain, the fake reply, and an integration test that boots the server and drives `GET /v1/samantha`. The scripted ear repeats one line. It does not recognise speech.

## Measured

`cargo test --offline --test stream -- --nocapture companion_socket`, debug profile, rustc 1.83.0, Linux. Three runs. These are not Piper, Vosk, or HORBM times. Speaker PCM is a generated 24 kHz tone. Every mood event was `source: resting`.

Run 1: `health_min_us=146 health_p50_us=181 reply_wall_us=87373 took_us=2 tts_gaps_ms=[38, 80, 81] tts_gap_median_ms=80 mood_gaps_ms=[74, 79] mood_gap_median_ms=79 typed_first_pcm_us=43261`

Run 2: `health_min_us=180 health_p50_us=226 reply_wall_us=87905 took_us=3 tts_gaps_ms=[38, 80, 81] tts_gap_median_ms=80 mood_gaps_ms=[73, 79] mood_gap_median_ms=79 typed_first_pcm_us=43338`

Run 3: `health_min_us=171 health_p50_us=208 reply_wall_us=91662 took_us=2 tts_gaps_ms=[37, 81, 80] tts_gap_median_ms=80 mood_gaps_ms=[69, 80] mood_gap_median_ms=80 typed_first_pcm_us=44440`

`/health` over 20 calls (first dropped): minimum 146–180 µs, median 181–226 µs. Fake `took_us` was 2 or 3. Last mic write to the `turn` text was 87373–91662 µs. Typed line to first speaker slice was 43261–44440 µs. Speaker slice gaps were 37–38 ms, then 80–81 ms, then 80–81 ms (median 80). Mood gaps were 69–80 ms (median 79–80).
