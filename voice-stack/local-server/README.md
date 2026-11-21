<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# S10 local server

Loopback UI process for Galion One. The directory is `voice-stack/local-server`. The socket is `galion-samantha-ws/1` on `127.0.0.1:7881`.

`GET /` and `GET /one` serve the Galion One page copied from `crates/galion-console/assets/v1-screen.html` (the `galion.app/one` front end, last updated in `8ee81f6` on 2026-11-19, and the same file at `c7179d1` on main). The canonical page keeps `PREVIEW_ONLY` true, which leaves the live mic off. The copy served here sets `PREVIEW_ONLY` false and calls `/session`, `/v1/dot`, `/v1/mic`, `/v1/turn`, `/v2/say`, and `/v2/decide`. It also loads `/nebula/vocabulary.mjs`. The old one-dot page stays at `/dot/`. Port 7878 is that one-dot screen. Port 7881 is this server.

The talk button posts a finished take to `POST /v1/turn`. A typed line uses `POST /v2/say`. The page polls `GET /v1/dot` and plays `GET /v1/reply/:id`. The WebSocket path is `galion-samantha-ws/1` on `/v1/samantha`. A mic turn ends after 640 ms of silence, then Vosk, then the card brain, then Piper. Silero VAD is the `vad-ear` process (`GALION_VAD_BIN`), kept out of this crate so `ort` stays out of the workspace. On Windows that binary loads ONNX Runtime with the `load-dynamic` feature (`ORT_DYLIB_PATH`, cache `ORT_CACHE_DIR`). Vosk recognises speech when the model and library are on disk. `Brain::open` in the orchestrator returns `horbm is not linked`. The card index is `horbm-memory` in `galion-knowledge` `v2/memory`. This server does not fetch that crate and does not create its sqlite file. It reads cards through `ReasoningRouter::turn`. A miss is `I don't know.` The router does not dial the Research gateway unless `GALION_RESEARCH_BIND` is `127.0.0.1` and a port other than 8787. This process does not bind 8787. `GET /health` is HTTP 200 with `"ok": true`. The unlinked test brain still returns `brain-unlinked`. Piper speaks the line when the LJ Speech pack is present. No model API is called.

The tracked example card is `knowledge/kettle.md`. `GALION_HORBM_KNOWLEDGE` selects another directory and is only read.

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

## Windows

The laptop keeps Galion on `G:` only. `scripts/windows/START-SAMANTHA-7881.cmd` is the start script (on the laptop, `G:\galion\work\START-SAMANTHA-7881.cmd`). `scripts/windows/Start-GalionOne.ps1` builds the binaries and fetches models when the laptop copies are missing. Both check that `G:` exists, then set:

| Variable | Path |
| --- | --- |
| repo | `G:\galion\work\galion-one-voice` |
| `CARGO_HOME` | `G:\galion\ci\toolchains\cargo` |
| `RUSTUP_HOME` | `G:\galion\ci\toolchains\rustup` |
| `CARGO_TARGET_DIR` | `G:\galion\work\target-voice` |
| models | `G:\galion\work\galion-one-voice\var` (git-ignored) |
| `TEMP` and `TMP` | `G:\galion\tmp\win` |
| npm, pip, and Hugging Face caches | `G:\galion\tmp\npm`, `G:\galion\tmp\pip`, `G:\galion\tmp\hf` |
| `APPDATA`, `LOCALAPPDATA`, `USERPROFILE` | `G:\galion\tmp\appdata`, `G:\galion\tmp\localappdata`, `G:\galion\tmp\profile` |
| logs | `G:\galion\work\logs` |
| Vosk model | `G:\galion\models\asr\vosk-model-small-en-us-0.15` |
| libvosk and its `libstdc++` / `libgcc` / `winpthread` DLLs | `G:\galion\models\asr\vosk-win64-0.3.45\` |
| Silero | `voice-stack\vad\models\silero_vad.onnx` |
| Piper | `voice-stack\tts-piper-stream\var\` |
| `ORT_DYLIB_PATH` | `G:\galion\toolchains\onnxruntime\onnxruntime-win-x64-cpu-1.31.1\` |
| `ORT_CACHE_DIR` | `G:\galion\tmp\cache\ort.pyke.io` |

`Start-GalionOne.ps1` downloads Silero VAD, Vosk small English, and Piper LJ Speech into `var\` when those laptop paths are empty, builds `vad-ear.exe` and `samantha-local.exe`, and listens on `127.0.0.1:7881`. The cmd file points at the paths above and starts the same binary. Open `http://127.0.0.1:7881/`. `-DryRun` prints the paths and does not download. A missing `G:` stops the script before it creates a directory. `GALION_VAD_HANG_MS` is `640`.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File G:\galion\work\galion-one-voice\scripts\windows\Start-GalionOne.ps1
```

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

Use `scripts/windows/Start-GalionOne.ps1` from the section above. It is the start path on the laptop. The table there is the set of directories the script creates. `GALION_SAMANTHA_REF` is still only reported on `/health` as set, missing, or unset. The file is not read and it is not downloaded.

The unit file under `deploy/` is a template. This crate does not install it and does not start it.

## Tests

`cargo test` covers the codec, the loopback check (including `::1`, refusing `0.0.0.0`), the origin check, the unlinked brain, the fake reply, and an integration test that boots the server and drives `GET /v1/samantha`. The scripted ear repeats one line. It does not recognise speech.

## Measured

`cargo test --offline --test stream -- --nocapture companion_socket`, debug profile, rustc 1.83.0, Linux. Three runs. These are not Piper, Vosk, or HORBM times. Speaker PCM is a generated 24 kHz tone. Every mood event was `source: resting`.

Run 1: `health_min_us=146 health_p50_us=181 reply_wall_us=87373 took_us=2 tts_gaps_ms=[38, 80, 81] tts_gap_median_ms=80 mood_gaps_ms=[74, 79] mood_gap_median_ms=79 typed_first_pcm_us=43261`

Run 2: `health_min_us=180 health_p50_us=226 reply_wall_us=87905 took_us=3 tts_gaps_ms=[38, 80, 81] tts_gap_median_ms=80 mood_gaps_ms=[73, 79] mood_gap_median_ms=79 typed_first_pcm_us=43338`

Run 3: `health_min_us=171 health_p50_us=208 reply_wall_us=91662 took_us=2 tts_gaps_ms=[37, 81, 80] tts_gap_median_ms=80 mood_gaps_ms=[69, 80] mood_gap_median_ms=80 typed_first_pcm_us=44440`

`/health` over 20 calls (first dropped): minimum 146–180 µs, median 181–226 µs. Fake `took_us` was 2 or 3. Last mic write to the `turn` text was 87373–91662 µs. Typed line to first speaker slice was 43261–44440 µs. Speaker slice gaps were 37–38 ms, then 80–81 ms, then 80–81 ms (median 80). Mood gaps were 69–80 ms (median 79–80).
