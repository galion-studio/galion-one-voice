# Galion Voice V2 — Symphony index

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Program t282.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

Champion, from Eureka: a conversation is one heartbeat, not a pipeline. One 80 ms clock. Three bands (you, me, feeling). The heartbeat tests itself.

This page is the index of the symphony stack. It names every section, the file that owns it, the page in the tree that already describes the sector, and the blank gate ledger. A section is complete when its gate is **PASS** and that row has been copied into [SELF-EVAL.md](SELF-EVAL.md).

The Rust path this stack sits on is already in the tree: `galion-heartbeat`, `galion-emotion`, `galion-cascade`, `galion-console`, `galion-render`, `galion-ml-voice`, `galion-asr-vosk`. The symphony compiles a clone into the Galion voice file those crates run.

## Section index

Paths in the Doc column live in this directory. The Anchor column is the page that already describes the sector.

| Section | Doc | Owns | Anchor |
|---|---|---|---|
| Compiler | [COMPILER.md](COMPILER.md) | Chatterbox clone output compiled into the Galion voice file (`.gq` and the clip ABI) the Rust stack runs. | [GALION-VOICE-V2.md](../GALION-VOICE-V2.md) |
| Clone | [CLONE.md](CLONE.md) | Current Samantha and Jarvis models. WAV refs only. Voice lands as PCM from the out-of-process worker. | [CLONE-WORKER.md](../CLONE-WORKER.md), [VOICE-CLONING.md](../VOICE-CLONING.md) |
| Persona | [PERSONA.md](PERSONA.md) | Personality, behaviour, and emotion in the voice. Timbre is the body. The feeling band is the person. | [personas/README.md](../personas/README.md), [SAMANTHA-V2.md](../SAMANTHA-V2.md) |
| One-dot | [ONE-DOT.md](ONE-DOT.md) | One surface at `/`. Speak, the page records, the workflow proceeds. `GET /v2` redirects to `/`. | [ONE-UI.md](../ONE-UI.md), [ORB.md](../ORB.md) |
| Runbooks | [RUNBOOKS.md](RUNBOOKS.md) | How each environment is started. The commands below are the short form. | [V1-DEMO.md](../../V1-DEMO.md), [LIVE-DEMO.md](../../LIVE-DEMO.md) |
| 80 ms | [80MS.md](80MS.md) | The live-streaming bar. The frame is 80 ms (12.5 Hz, 24 kHz, 1920 samples). | [04-latency-streaming.md](../../research/galion-voice-v2/domains/04-latency-streaming.md) |
| Self-eval | [SELF-EVAL.md](SELF-EVAL.md) | The filled ledger. Demo, spec, test, and teacher are the same artifact. | [PRESENCE-EVAL.md](../../eval/PRESENCE-EVAL.md), [FLASH-EVAL.md](../FLASH-EVAL.md) |
| Eureka | [EUREKA.md](EUREKA.md) | One heartbeat, three bands, harness before weights, measurable scores. | [evals/PERSONA-EVAL-2026-11-03.md](../evals/PERSONA-EVAL-2026-11-03.md) |
| Plus / Ultra / Research | [PLUS-ULTRA-RESEARCH.md](PLUS-ULTRA-RESEARCH.md) | Hosted plan. Local stays free. Research is its own console. | [personas/GALION-RESEARCH.md](../personas/GALION-RESEARCH.md) |
| Rust translation | [RUST-TRANSLATION.md](RUST-TRANSLATION.md) | The symphony runs on the Galion Rust crates. The compiler's file is what they read. | [OPENMIND-NERVE-CORTEX.md](../OPENMIND-NERVE-CORTEX.md) |
| Jarvis texture | [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md) | The Jarvis reference keeps the hub: clicks, alarms, room ambience. That texture is the character. | [personas/JARVIS.md](../personas/JARVIS.md) |
| Samantha emotions | [SAMANTHA-EMOTIONS.md](SAMANTHA-EMOTIONS.md) | Her range, mapped onto the live weight `e` and the prosody knobs. | [personas/SAMANTHA.md](../personas/SAMANTHA.md), [EMPATHY.md](../EMPATHY.md) |
| One-percent | [ONE-PERCENT.md](ONE-PERCENT.md) | Non-inference work stays within 1% of the host. A late frame pauses. | [ONE-UI.md](../ONE-UI.md), [ANIMATION-ENGINE.md](../ANIMATION-ENGINE.md) |
| Heartbeat | [HEARTBEAT.md](HEARTBEAT.md) | `Heartbeat::step` carries you, me, and feeling for one frame. | `crates/galion-heartbeat` |
| Clip store | [CLIP-STORE.md](CLIP-STORE.md) | WAV references, hashed, outside git. | [CLIP-INGEST.md](../CLIP-INGEST.md) |
| Auto-workflow | [AUTO-WORKFLOW.md](AUTO-WORKFLOW.md) | After the recording, the process knows the next step (a skill, or an auto program). | [IPC.md](../IPC.md), [ONE-UI.md](../ONE-UI.md) |
| TTFA | [TTFA.md](TTFA.md) | Time to first audio. A product number names a model engine. | [FLASH-EVAL.md](../FLASH-EVAL.md) |
| Staging | [STAGING.md](STAGING.md) | Public HTTPS in front of loopback, with auth and an exact CORS origin. | [PLAN-V5.md](../../ops/PLAN-V5.md), [SECURITY-EDGE.md](../SECURITY-EDGE.md) |

Product voices on a normal call are Galion One and, on the research path, Galion Research. Samantha and Jarvis are internal test voices for the clone.

## Gate ledger (template)

Copy a finished row into [SELF-EVAL.md](SELF-EVAL.md). Until a gate has been run, leave Result as the three tokens. Replace them with exactly one of:

| Token | When to write it |
|---|---|
| **PASS** | The gate ran and met the bar. Evidence is a command log, a hash, or a report path. |
| **FAIL** | The gate ran and missed the bar. Evidence shows the miss. |
| **BLOCKED** | The gate cannot be run yet. Evidence names what is missing (ref, weight, host, clearance). |

| Section | Gate | Result | Evidence | When (Europe/Zagreb) |
|---|---|---|---|---|
| Compiler | Round-trip fixture. The file the Rust stack reads is the Galion voice file. | PASS / FAIL / BLOCKED | | |
| Clone | Worker PCM. Ref sha256 matches the table below. Smoke speak logged. | PASS / FAIL / BLOCKED | | |
| Persona | Emotion maps to prosody and the wash. Feeling is the live weight, refreshed every frame. | PASS / FAIL / BLOCKED | | |
| One-dot | `GET /` is the only screen. Mic records. `GET /v2` redirects to `/`. | PASS / FAIL / BLOCKED | | |
| Runbooks | Every command in this index runs from a clean checkout, or the row says why it cannot. | PASS / FAIL / BLOCKED | | |
| 80 ms | Frame is 80 ms. Streaming-sector budget is written and measured, or marked stub-measured. | PASS / FAIL / BLOCKED | | |
| Self-eval | This table's finished rows live in `SELF-EVAL.md`. A blank template is not a score. | PASS / FAIL / BLOCKED | | |
| Eureka | Presence replay and the heartbeat harness run. Scores are numbers in a log. | PASS / FAIL / BLOCKED | | |
| Plus / Ultra / Research | Local, Plus, Ultra, and the Research console are written with the figures in that section. | PASS / FAIL / BLOCKED | | |
| Rust translation | A compiled Galion voice file loads on the Rust path. The PyTorch child stays out of process. | PASS / FAIL / BLOCKED | | |
| Jarvis texture | Spectral check: hub clicks, alarms, and ambience remain against a clean-speech baseline. | PASS / FAIL / BLOCKED | | |
| Samantha emotions | Her column in the prosody table is the one the feeling band applies. Comfort is pace and warmth. | PASS / FAIL / BLOCKED | | |
| One-percent | Non-inference CPU ≤ 1% of the host. Tick work p99 is well under 800 µs. Late frame pauses. | PASS / FAIL / BLOCKED | | |
| Heartbeat | `Heartbeat::step` advances one frame and allocates nothing on that path. A late frame sets pause. | PASS / FAIL / BLOCKED | | |
| Clip store | Both WAV refs exist at the store path. sha256 matches. No reference audio is in git. | PASS / FAIL / BLOCKED | | |
| Auto-workflow | A spoken turn records itself and the workflow hook fires in the test. | PASS / FAIL / BLOCKED | | |
| TTFA | Harness prints `measured: false` by default. A product claim requires `engine` = model. | PASS / FAIL / BLOCKED | | |
| Staging | Loopback bind holds. The public name has auth. CORS origin is the exact `https://` name. | PASS / FAIL / BLOCKED | | |

## Reference clips

Clone references are WAV. The slots in git are the placeholder files. The audio stays outside git.

| Id | Path | Expected |
|---|---|---|
| Samantha | `docs/galion-voice-v2/personas/refs/samantha.wav` | 17.41 s, 44100 Hz, stereo, 16-bit. sha256 `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` |
| Jarvis | `docs/galion-voice-v2/personas/refs/jarvis.wav` | 15.95 s, 48000 Hz, stereo, 16-bit. sha256 `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` |

Outside the repo, the same bytes live at `/workspace/galion-clip-store/samantha/reference.wav` and `/workspace/galion-clip-store/jarvis/reference.wav`, each with a sibling `.sha256`. The Jarvis file keeps the hub texture.

The program brief records owner clearance t273 for cloning these two refs with the current models. The checklist page is [RIGHTS-CLEARANCE.md](../RIGHTS-CLEARANCE.md). Clearance in a brief becomes a **PASS** on the Clone row only after the smoke speak is logged.

## Hosted plan (pointer)

Full write-up: [PLUS-ULTRA-RESEARCH.md](PLUS-ULTRA-RESEARCH.md). Figures below are the owner plan from the CPU note (t284, 2026-11-03). They are capacity notes until a benchmark records them on the machine.

| Surface | What it is |
|---|---|
| Local | Free. The person's own Linux machine or a Raspberry Pi. It does not draw datacenter compute. |
| Plus | $20 / month. CX23-class (about $6 COGS). Voice-to-voice included. 100 GB SSD, 16 GB RAM. |
| Ultra | $220 / month. CX43-class, Titanaxe Max 3XL. 2 GB SSD. |
| Research | Galion Research console. Its own hosted surface, beside Plus and Ultra. |

The hosted voice plan is CPU and RAM. The flagship app, voice, and research node in that note is a 1U AMD EPYC 9655P (96 cores), 768 GB DDR5. A voice session is sized near 1.0 vCPU average and about 0.65–1.15 GB RAM. Chatterbox Nano, when it runs, runs on CPU in its own process. Voice crates read the Galion file (`.gq` and PCM).

## How to run the environments

Repo root. Rust 1.85 or newer. Builds below pass `--offline` where the script already does. Loopback only, unless the staging block says otherwise. Weights live in `weights/` and are not in git.

### One-dot screen

The phone surface. One circle, a mic, a text box, a persona. `GET /v2` redirects to `/`.

```bash
scripts/asr/fetch-vosk.sh          # once, so the mic can hear English
scripts/a21/fetch-a21.sh           # once, optional: Piper en_US-ljspeech-medium
scripts/run-v1-screen.sh           # http://127.0.0.1:7878/  for one hour
scripts/run-v1-screen.sh --check   # page, /dot, /mic, /turn, /v2 redirect
scripts/one-ui.sh                  # the page needs no Python virtualenv
```

The screen sets `GALION_LLM=canned`. It sets `GALION_ASR=vosk` when `weights/asr/vosk-model-small-en-us-0.15` is on disk, and `stub` when it is not. A missing model returns an error on a mic turn. Typed text still works. `GALION_SPEECH=auto` uses Piper when the A21 pack is present, otherwise flite. Address and lifetime: `V1_SCREEN_ADDR` (default `127.0.0.1:7878`), `V1_SCREEN_SECONDS` (default 3600).

On the same machine, open `http://127.0.0.1:7878/`, tap the circle, speak. The page posts a WAV to `/turn` and plays `/reply/<id>.wav`.

Persona instruction check, with the screen already up:

```bash
BASE=http://127.0.0.1:7878 scripts/eval/v2-persona-selftest.sh
```

### Heartbeat (80 ms, no weights)

Headless. Synthetic or WAV in, one `Heartbeat::step` per 80 ms frame, stereo out (left = you, right = me).

```bash
scripts/run-live-demo.sh
scripts/run-live-demo.sh --showcase
LIVE_REALTIME=1 scripts/run-live-demo.sh
cargo run --offline -q -p galion-heartbeat --bin galion-heartbeat-presence
cargo test --offline -p galion-heartbeat
cargo test --offline -p galion-emotion
```

`LIVE_OUT` defaults to `galion-demo/out/live`. The last line of the live demo is `GALION_LIVE_DEMO_OUT=<dir>`. `--realtime` sleeps to the wall clock, 80 ms per frame.

The full local demo, including the speech turn and the one-dot frames:

```bash
scripts/run-v1-demo.sh --headless
scripts/run-v1-demo.sh --check
```

Notes: [V1-DEMO.md](../../V1-DEMO.md), [LIVE-DEMO.md](../../LIVE-DEMO.md).

### Speech turn and presence

```bash
scripts/run-en-cascade-demo.sh --text "hello galion"
scripts/eval/virtual-user.sh
```

The cascade writes 24 kHz PCM16 and prints `GALION_EN_CASCADE_WAV=<path>`. The virtual user writes `galion-gates/results/presence-eval/latest.json`. TTFA in that report is loopback time until a reply WAV exists. It is filled only when Piper or flite spoke. It is not a phone mouth-to-ear number. Doc: [PRESENCE-EVAL.md](../../eval/PRESENCE-EVAL.md).

Phone-path smoke (starts the screen unless `--no-start`):

```bash
scripts/check-phone-pipeline.sh
```

### Clone worker and TTFA

The worker is a long-lived child: one JSON line in, 24 kHz s16le out. The script in tree does not import torch and does not call generate until a later wiring lands. Fetch of the pinned weights is explicit:

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh
```

Time to first audio, CPU, this machine:

```bash
scripts/eval/flash-ttfa.sh
GALION_FLASH_EVAL=1 GALION_CLONE_STUB_PCM=1 scripts/eval/flash-ttfa.sh
```

The default line is `measured: false` and carries no `ttfa_ms`. The opt-in line may print a number with `engine` = `protocol-stub` and `product_claim` = false. A product TTFA is a run whose engine is a model, recorded after the fact. Doc: [FLASH-EVAL.md](../FLASH-EVAL.md), [CLONE-WORKER.md](../CLONE-WORKER.md).

### Staging

The app binds `127.0.0.1` only. A phone microphone needs HTTPS. Staging puts auth on the tunnel and sets the CORS origin to that exact name. Put the password in the environment.

```bash
scripts/run-v1-screen.sh
# second terminal
ngrok http 127.0.0.1:7878 --basic-auth "$GALION_NGROK_USER:$GALION_NGROK_PASS"
```

Then, with `GALION_CORS_ORIGINS` set to the `https://` origin ngrok printed:

```bash
scripts/check-phone-pipeline.sh --no-start
NGROK_URL=https://<the-name> scripts/check-phone-pipeline.sh --no-start
```

Deploy notes print steps and open no connection. `--execute` exits 2.

```bash
scripts/deploy/harden-dryrun.sh
scripts/deploy/release-dryrun.sh
```

Docs: [PLAN-V5.md](../../ops/PLAN-V5.md), [SECURITY-EDGE.md](../SECURITY-EDGE.md), [EDGE-NGROK.md](../../agent-v1-demo/EDGE-NGROK.md), [TITANAXE-VPS.md](../../deploy/TITANAXE-VPS.md).

### Whole workspace

```bash
cargo test --offline --workspace
```

Rust 1.85. This is the local check. It does not fetch weights and it does not open a tunnel.

## Reading order

1. This index.
2. [EUREKA.md](EUREKA.md), then [HEARTBEAT.md](HEARTBEAT.md) and [80MS.md](80MS.md).
3. [COMPILER.md](COMPILER.md), [CLONE.md](CLONE.md), [CLIP-STORE.md](CLIP-STORE.md), [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md).
4. [PERSONA.md](PERSONA.md), [SAMANTHA-EMOTIONS.md](SAMANTHA-EMOTIONS.md).
5. [ONE-DOT.md](ONE-DOT.md), [AUTO-WORKFLOW.md](AUTO-WORKFLOW.md).
6. [ONE-PERCENT.md](ONE-PERCENT.md), [TTFA.md](TTFA.md), [SELF-EVAL.md](SELF-EVAL.md).
7. [PLUS-ULTRA-RESEARCH.md](PLUS-ULTRA-RESEARCH.md), [RUST-TRANSLATION.md](RUST-TRANSLATION.md), [STAGING.md](STAGING.md).
8. [RUNBOOKS.md](RUNBOOKS.md) when a command in this index needs a longer page.
# Galion Voice V2 symphony

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

Champion: a conversation is one heartbeat, not a pipeline.
Demo = Spec = Test = Teacher. Harness before model.

One 80 ms tick carries you, me, and feeling. Non-inference work on that tick stays inside 1% of the frame (800 µs). A late frame is a pause. `Heartbeat::step` does not block and does not allocate.

The host plan is CPU only, no GPU. The flagship app node is an AMD EPYC 9655P. Plus is CX23-class. Ultra is CX43-class. Those names are a sizing plan, not a measurement of the machine you are on. See [CPU.md](CPU.md).

## How to run

| Environment | Runbook | First command |
| Local demo | [LOCAL-DEMO.md](LOCAL-DEMO.md) | `bash scripts/run-live-demo.sh` |
| Clone worker | [CLONE-WORKER.md](CLONE-WORKER.md) | `scripts/clone/chatterbox_worker.py` with `GALION_CLONE_STUB_PCM=1` |
| One dot | [ONE-DOT.md](ONE-DOT.md) | `bash scripts/run-v1-screen.sh` |
| Clip store | [CLIP-STORE.md](CLIP-STORE.md) | leave `GALION_CLIP_STORE` unset until an owner path exists |
| Self-eval | [GATES.md](GATES.md) | `bash scripts/symphony/self-eval.sh` |

The tick bar is [LATENCY.md](LATENCY.md). The ledger is [SELF-EVAL.md](SELF-EVAL.md). A section there is complete only when its check exits 0.

— Maciej Grajczyk
