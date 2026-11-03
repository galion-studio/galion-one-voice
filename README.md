# Galion Voice

Perpetual Open MIT License · © 2026 Maciej Grajczyk

"The seamless connection with the human being, on the voice only."

One voice. One mark on a near-black ground. An 80 ms heartbeat under everything.

You speak on the **you** band. Galion answers on the **me** band. A feeling
band carries a few bytes of emotion each frame, and only when consent allows
it. Consent starts denied. The clock does not move: 12.5 Hz, 24 kHz, from
`LOCKS.toml`.

The mark is a single orb (`#F5F5F7`). It rests, it listens, it thinks while a
turn is in flight, and it moves with the reply you actually hear. Amber
(`#F5A524`) and violet (`#7C3AED`) are a slow wash behind it. They never tint
the mark.

```text
phone  --https-->  ngrok now, Caddy later
                      |
                      v
               127.0.0.1:7878  WebSurface (loopback only)
                      |
        +-------------+-------------+
        | 80 ms heartbeat           |  turn worker (never inside the tick)
        | you / me / Rika           |  PhoneLane: ASR → canned reply → Piper
        +-------------+-------------+
                      |
                      v
               one orb  (spring, mic energy, playback envelope)
```

## Where the work stands

| | |
|---|---|
| **V1** | Foundation. One-dot console, heartbeat, Rika, English cascade, Piper A21, phone interim path. |
| **V2** | This program: process bus, orb engine, harder edge. Plan: [docs/galion-voice-v2/GALION-VOICE-V2.md](docs/galion-voice-v2/GALION-VOICE-V2.md). |
| **V3** | Production-ready Galion Voice. Later. |
| **Plan V5** | How this stack would reach the VPS, and why it has not. [docs/ops/PLAN-V5.md](docs/ops/PLAN-V5.md). |

English speech recognition on the phone: the owner chose **letter A** (Vosk,
out of process). The sidecar is not in the tree. `GALION_ASR=interim`
is still a labelled placeholder (a sentence chosen by how long you talked).
It does not know the words, and it stays until a WER bar passes.
[docs/policy/RATIFY-EN-ASR-PHONE-PATH.md](docs/policy/RATIFY-EN-ASR-PHONE-PATH.md).
whisper.cpp is rejected.

The reply you hear is a short canned line, spoken by Piper
(`en_US-ljspeech-medium`, CPU, out of process) when that pack is on disk, else
flite. That Piper pin is `[formats.exceptions.a21_piper]` in `LOCKS.toml`,
sunset `2027-02-01`. Other lock values are unchanged. Tones are not passed
off as a reply. The first student size on the backlog is S0. Canned stays
the fallback until readiness tests. The `.gq` student does not exist yet.

Obsidian Open Mind is a separate Rust agent stack,
[galion-studio/obsidian-open-mind](https://github.com/galion-studio/obsidian-open-mind).
This repository keeps only the voice-side seam: nerve notes
(`voice.frame_miss`, `voice.fallback`, `voice.backend_up`) and a Hermes
side-band handoff for problem-solving. Hermes is not on the 80 ms tick.
The Open Mind tree is not vendored. On a VPS it belongs at
`/opt/galion/openmind` after a human deploy. It is not on this Cursor host,
and nothing here connects.
[OPENMIND-NERVE-CORTEX.md](docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md).

Nothing here has been deployed to the VPS. Access is approved-pending-setup:
per-user anonymized codes, installed by a human. No shared key. Priority 1
for V2 voice work is local Chatterbox cloning, not a cloud API
([VOICE-CLONING.md](docs/galion-voice-v2/VOICE-CLONING.md)). No film audio
is in this repo.

## Run the demo

Loopback only. Rust 1.85 or newer.

```bash
scripts/a21/fetch-a21.sh          # once, optional: Piper pack into weights/ (sha256-pinned)
scripts/run-v1-screen.sh          # http://127.0.0.1:7878/  for an hour
scripts/run-v1-screen.sh --check  # page, /dot, /mic, /turn, security headers
```

On a phone the microphone needs HTTPS. In another terminal, on the same machine:

```bash
ngrok http 127.0.0.1:7878
```

Open the `https://` name, tap the orb, talk. The page posts a WAV to `/turn`
when you pause, then plays `/reply/<id>.wav`. Speaking over the reply cuts it
off. Notes: [docs/agent-v1-demo/EDGE-NGROK.md](docs/agent-v1-demo/EDGE-NGROK.md).

The screen script sets `GALION_ASR=interim` and `GALION_LLM=canned` for that
process. Export `GALION_ASR=off` beforehand if you want real speech to be
refused instead of the placeholder.

Headless V1 checks (no phone):

```bash
scripts/run-v1-demo.sh --headless
```

Virtual user (no network, no phone). Text or a fixture WAV goes into PhoneLane. The report lands in `galion-gates/results/presence-eval/`. Interim is not called recognition. See [docs/eval/PRESENCE-EVAL.md](docs/eval/PRESENCE-EVAL.md).

```bash
scripts/eval/virtual-user.sh
```

Full runbook: [docs/V1-DEMO.md](docs/V1-DEMO.md).

## Layout

```text
LOCKS.toml            the contract (clock, bands, consent, formats, the one mark)
LEDGER.md             what landed, and what an owner decided
crates/galion-heartbeat     the 80 ms step
crates/galion-openmind      nerve notes + Hermes handoff (no daemon required)
crates/galion-cascade       EN lane + PhoneLane (feature cascade)
crates/galion-console       screen demo, v1-screen.html, turn worker
galion-render/              the mark, and the loopback HTTP surface
galion-emotion/             affect, consent, Rika
crates/galion-ml-voice      Piper slot + flite fallback (feature speech)
galion-student/             distillation plan only; no weights
galion-gates/               local gates; R0–R3 pending SSH
crates/galion-presence-eval  virtual user; loopback T0–T2
scripts/eval/virtual-user.sh
scripts/run-v1-screen.sh
scripts/check-phone-pipeline.sh
docs/galion-voice-v2/       V2 sector plan
docs/ops/PLAN-V5.md
docs/brand/GALION-VOICE-BRAND.md
```

`weights/` is gitignored. Fetch the A21 pack; do not commit it.

## Licence

Code in this repository is Perpetual Open MIT, (c) 2026 Maciej Grajczyk.
`LOCKS.toml` says free forever, and no paid tiers. That is the licence.

Weights are separate. They are not in git. The Piper voice pack keeps its own
MIT grant and is downloaded by `scripts/a21/fetch-a21.sh`. A future student
model would ship with its own `LICENSE-WEIGHTS`. No GPL code is copied in.

## Contributing

1. Read [LOCKS.toml](LOCKS.toml). Changing a value is an owner decision and a
   LEDGER line. Do not "fix" a lock to make a demo easier.
2. No GGML-family containers, and no candle / ort / tract / burn / llama / ggml
   in manifests. The A21 Piper path is the sunset pin
   `[formats.exceptions.a21_piper]`. Do not widen it. Chatterbox, if it is
   ever fetched, stays out of process until an owner writes a separate exception.
3. The user surface is one mark. No menu, face, or second widget. The phone
   hint is the gesture prompt; do not grow it into chrome.
4. Consent stays deny by default. Do not wire caller-emotion into the phone
   lane unless `sense` is granted in that session.
5. Before you call it done:

```bash
scripts/check-locks.sh
scripts/check-no-forbidden.sh
cargo test --workspace
cargo test -p galion-render --features web
cargo test -p galion-cascade --features cascade
cargo test -p galion-console --features phone-turn
scripts/eval/virtual-user.sh
scripts/run-v1-screen.sh --check
```

6. Deploy is [Plan V5](docs/ops/PLAN-V5.md). Dry-run scripts refuse `--execute`.
   Do not add a key or an access code, and do not point the app at a public address.
   `WebSurface` will reject the bind.

Further: [V2 plan](docs/galion-voice-v2/GALION-VOICE-V2.md) ·
[Open Mind nerve and cortex](docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md) ·
[animation engine](docs/galion-voice-v2/ANIMATION-ENGINE.md) ·
[Plan V5](docs/ops/PLAN-V5.md) ·
[brand](docs/brand/GALION-VOICE-BRAND.md) ·
[phone API](docs/cascade/PHONE-TURN-API.md) ·
[A21 blockers](docs/agent-v1-demo/A21-BLOCKERS.md) ·
[LEDGER](LEDGER.md)

— Maciej Grajczyk
