# Galion Voice

Perpetual Open MIT License (draft) · © 2026 Maciej Grajczyk · 2026-11-02 (Europe/Zagreb, UTC+1)
**LOCAL ONLY.** Nothing here has been pushed or deployed. The interface is **one dot** at `http://127.0.0.1:7878/`.

## 1. What it is

Galion Voice is a full-duplex voice-to-voice system that runs on CPU.

- **Dual-band audio.** The caller speaks on the **you** band and Galion answers on the **me** band, at the same time. A third
  **feeling** band carries an 8-byte `EmotionWire` per 80 ms frame per direction (`LOCKS.toml` `[bands]`).
- **Emotion in, emotion out.** `galion-emotion` reads affect from the caller (consent-gated, default deny) and expresses it in
  **Galion's own voice**. It counterbalances rather than mirrors: anger gets a calmer reply, and joy is the only state shared
  (slightly below the caller). See `galion-emotion/README.md` and `galion-demo/README.md`.
- **Rika.** When the caller falls silent and takes a breath, Rika leans in (≈160 ms) on the feeling band only. It never reads the transcript.
- **One clock.** Everything steps on an 80 ms heartbeat (12.5 Hz). It is not configurable.
- **CPU-friendly.** Pure Rust workspace, own tensor kernels (`galion-tensor`, SIMD/int8 GEMV), no GPU. No third-party inference
  runtime is linked into the primary crates (gates G2/G3).
- **Built from borrowed open pieces**, each kept at arm's length:
  - **A21 speech:** Piper VITS `en_US-ljspeech-medium` (MIT, LJSpeech public domain), run on CPU out of process.
  - **Fallback speech:** flite (`libflite1`).
  - **Student plan (not trained):** PersonaPlex-7B distilled into a ~1.0B student (`galion-student/`); PersonaPlex outputs as eval
    data only, never weights. Kyutai Moshi/Moshiko (CC-BY-4.0) is the logit teacher and fallback. Mimi codec is a stub.
  - **Affect reference:** SenseVoice-Small as an offline reference seam only. Nothing is downloaded or run.

Today the demo is honest about its stubs: ASR is `StubAsr`, the LLM is `MockLlm` (echo, "you said: …"), emotion is fixed in the
EN turn, Mimi is `IS_STUB`, and there is no microphone (input is a WAV or synthetic audio).

## 2. Run the V1 demo

```bash
cd /workspace/galion-voice-code
scripts/a21/fetch-a21.sh             # once: A21 Piper pack into weights/ (sha256-pinned); --check verifies offline
scripts/run-v1-demo.sh               # live demo: open http://127.0.0.1:7878/  (Ctrl-C stops)
scripts/run-v1-demo.sh --headless    # no ports/display: artifacts + soft checks → galion-demo/out/v1/V1-SUMMARY.txt
scripts/run-v1-demo.sh --check       # one live loop, curls / /dot /health on :7878, exit 0/1
FEED_PORT=17906 scripts/run-v1-demo.sh --check   # side port when :7878 is in use
```

**One UI: `http://127.0.0.1:7878/`.** One solid dot (`#F5F5F7`) on obsidian `#0A0A0C` with a slow amber → violet background wash.
No text, no buttons, no chrome. Modes: idle · rika · listening · speaking. Press `F` or double-click for fullscreen.
The server binds **loopback only** by design and serves `/`, `/dot` and `/health`. There is no public exposure and no reverse proxy.

Prerequisites:
- Rust stable + cargo (`--offline` builds), python3 (≥ 3.11 for `check-locks.sh`), curl, coreutils `timeout`.
- The A21 pack (`scripts/a21/fetch-a21.sh`). Linux x86_64 only.
- Speakers via PulseAudio/PipeWire (`paplay`) or ALSA (`aplay`). `GALION_PLAY=0` turns playback off.

Exit codes: `0` ok · `1` check FAIL · `64` bad arg · `69` port busy (`pkill -f galion-live-demo`) · `124` timeout.
Full runbook, flags and soft checks C0–C12: **[docs/V1-DEMO.md](docs/V1-DEMO.md)**.

## 3. Project structure

```
Cargo.toml              workspace (12 members below), edition 2021, license MIT
LOCKS.toml              Eureka locks: 80 ms clock, bands, gates, consent, formats, one-dot UI, licence
LEDGER.md               agent ledger + owner decisions (e.g. t123u)
crates/
  galion-tensor         SIMD/int8 GEMV, conv stream, attention, KV ring, .gq reader
  galion-heartbeat      the 80 ms Heartbeat::step
  galion-live-demo      the V1 live loop + std-only HTTP server (/, /dot, /health), play.rs
  galion-cascade        EN cascade lane: Asr / EmotionIn / Llm / Tts traits, mocks
  galion-console        developer console (not user UI); assets/v1-screen.html = the one-dot page
  galion-mimi           Mimi codec (stub)
  galion-presence       presence replay harness (specs/presence*.json)
  galion-sideband       Hermes sideband types
  galion-student-backend  student VoiceBackend slot (stub)
  galion-ml-voice       language id (EN only) + speech slots, incl. A21WeightTts (speech/weights.rs)
galion-emotion/         affect detect + express, consent gate, Rika, EmotionWire
galion-render/          one-dot render + NullSurface
galion-student/         student distillation plan (config stubs, no weights)
galion-probe/           CPU/bandwidth probe (G0/R0)
galion-demo/            legacy canned Python demo (not on the V1 path)
galion-gates/           run-gates.sh (G0–G6, PS, LK, DG, LV, V1; R0–R3 PENDING)
scripts/                run-v1-demo.sh, run-v1-screen.sh, run-live-demo.sh, run-en-cascade-demo.sh,
                        run-dot-console-demo.sh, check-locks.sh, check-no-forbidden.sh, a21/, v1/, deploy/ (dry-run)
specs/                  presence specs + fixtures
weights/                fetched A21 pack (tts/, runtime/); not vendored (.gitignore = *)
docs/                   V1-DEMO.md, LIVE-DEMO.md, agent-v1-demo/, agent-wave20/, agent-wave21/, cascade/, multilingual/
```

## 4. A21 speech path (Piper, CPU)

```
EN turn: StubAsr → MockLlm "you said: …" → A21 Piper VITS (CPU, out of process, ~0.5 s) → 24 kHz WAV → me band → dot "speaking"
```

- **Pack:** `weights/tts/en_US-ljspeech-medium.onnx` + `.onnx.json`, from HF `rhasspy/piper-voices` (MIT), runtime Piper
  2023.11.14-2 (`weights/runtime/piper/piper`, onnxruntime 1.14.1, MIT). Fetched and sha256-verified by `scripts/a21/fetch-a21.sh`.
- **Deterministic** by default (`noise_scale=0`, `noise_w=0`); `GALION_A21_NOISE=natural` opts into sampled prosody. RTF ≈ 0.03–0.05.
- **Selection:** `GALION_SPEECH=auto|a21|flite|off` (default `auto` = A21, then flite). `GALION_A21_RUNTIME=<piper>` overrides the runtime.
- **Fallback only:** flite (`apt install libflite1`) when the pack is missing.
- **Neither present:** the EN cascade **exits 3** (`SPEECH=ABSENT`). It never passes tones off as the reply.

Still open (do not bypass, do not edit LOCKS to make them go away):
1. **LOCKS ratification (owner).** `.onnx` + out-of-process onnxruntime is an owner-approved exception (t123u, LEDGER) to
   `formats.foreign_runtime_allowed=false` and `weight_formats = ["safetensors"]`. LOCKS.toml is **not** amended.
2. **espeak-ng is GPL-3 (counsel).** Piper phonemizes with it. It is unlinked and out of process, but needs sign-off.
3. **Voice choice and consent** for LJSpeech; the Galion-native `.gq` decoder is not built (`GALION_NATIVE_DECODER_WIRED=false`).

Full list: [docs/agent-v1-demo/A21-BLOCKERS.md](docs/agent-v1-demo/A21-BLOCKERS.md) · counsel memo: [docs/agent-wave20/A21-COUNSEL-MEMO.md](docs/agent-wave20/A21-COUNSEL-MEMO.md).

## 5. License

- **Code:** Perpetual Open MIT License (draft) · © 2026 Maciej Grajczyk. Free forever, no paid tiers (`LOCKS.toml` `[licence]`).
  It covers **code only**. Cargo manifests declare `license = "MIT"`.
- **Weights:** separate. They keep their own licences (e.g. `LICENSE-WEIGHTS` for a future student), and they are **not vendored**
  (`formats.weights_vendored = false`, `weights/.gitignore` = `*`). The A21 Piper pack is fetched, not shipped.
- **No GPL code is copied in.** No NC data.

## 6. Contributing

1. **Read the locks first.** [LOCKS.toml](LOCKS.toml) is the contract. Changing any value needs an owner decision and a LEDGER entry.
   Gate thresholds may only be tightened.
2. **Run the local gates before you call anything done:**
   ```bash
   scripts/check-locks.sh            # Eureka locks
   scripts/check-no-forbidden.sh     # no forbidden container formats / runtimes (galion-tensor scope)
   galion-gates/run-gates.sh         # G0–G3 hard; G4/G6/PS/LK/DG/LV/V1 soft (GALION_GATES_HARD=1 makes them count)
   scripts/run-v1-demo.sh --headless # V1 soft checks C0–C12
   ```
   No green gate, no done. Zero tests counts as a fail.
3. **No unauthorized formats or runtimes.** No GGML-family containers, no candle/ort/tract/burn/llama/ggml in manifests.
   Weights are safetensors (`.gq` reader later). The A21 `.onnx` path is the one scoped exception, still pending ratification.
4. **One dot.** No UI additions: no text, menus, buttons, orbs, faces, waveforms or logos. `galion-console` is a developer tool.
5. **Consent is deny-by-default.** Workplace and education purposes are hard-off and not overridable (EU AI Act Art. 5(1)(f)).
6. **Local first.** No pushes, deploys or weight vendoring without owner sign-off. Record status in `docs/` and `LEDGER.md`,
   dated in Europe/Zagreb time.

Further reading: [docs/V1-DEMO.md](docs/V1-DEMO.md) · [docs/agent-v1-demo/FP-SUMMARY.md](docs/agent-v1-demo/FP-SUMMARY.md) ·
[docs/LIVE-DEMO.md](docs/LIVE-DEMO.md) · [docs/cascade/EN-CASCADE-LANE.md](docs/cascade/EN-CASCADE-LANE.md) ·
[docs/multilingual/README.md](docs/multilingual/README.md) · [LEDGER.md](LEDGER.md)

— Maciej Grajczyk
