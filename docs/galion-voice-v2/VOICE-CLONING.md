# V2-S14 · Real-voice cloning

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Priority 1 of the V2 backlog. Record only.

Master slogan: "The seamless connection with the human being, on the voice only."

The clone path is local and self-offline. It exists so the voice can carry
that connection: Samantha when the talk is human, Jarvis when the talk is a
command. No cloud TTS API. No weights in git. No film audio.

## Legal hold

Cloning a performer's voice without rights is high risk. The comparison the
owner requires is public: in May 2024 OpenAI withdrew the Sky voice after
Scarlett Johansson said it resembled her performance in *Her*. OpenAI said
Sky was not an imitation of her. The dispute is the warning, not a licence.

Jarvis, as an Iron Man film voice, and Samantha, as the voice of *Her*
(2014), are **owner-directed research targets**. One reference of about ten
seconds is the intended conditioning clip for each, once rights exist.

Before any of the following, rights, consent, and licence have to be cleared
and written in `LEDGER.md`:

- fetching or storing a film clip
- calling a clone worker with that clip
- a public demo that speaks in either voice

This change does not fetch audio and does not add a clip.

## What was chosen

Resemble AI **Chatterbox**, used as architecture to borrow, run later out of
process. Code licence read from upstream `LICENSE` on 2026-11-03: MIT,
copyright 2025 Resemble AI.

Zero-shot. No fine-tune. `generate` takes a reference wav
(`audio_prompt_path`). Conditioning, from the public Python, is three pieces:
a VoiceEncoder speaker embedding, speech-token prompt tokens, and an S3Gen
reference embedding. T3 (token-to-token) then samples speech tokens. S3Gen
and the vocoder turn them into audio. Upstream `generate()` applies a Perth
watermark before returning samples.

| Piece | What the public tree says | Galion use |
|---|---|---|
| Nano | 110M, English, same family as Turbo. README claims about 3× realtime on 8 CPU cores. | **Primary.** CPU. Vendor claim, not measured here. |
| Turbo | 350M, English, paralinguistic tags. `tts_turbo.py` asserts the reference is longer than 5 seconds. Encoder window constant on that class is 15× the 16 kHz rate; decoder window is 10× the synthesis rate. | Quality option, same worker shape, not the default. A ~10 s clip fits the owner's target and the >5 s assert. |
| Original English | 500M, exaggeration and CFG. | Not the CPU primary. |
| Multilingual V3 | 500M, 23+ languages. T3 can be a Llama backbone (`t3.py` builds `LlamaModel` unless the config says GPT-2). | Out of scope for this English phone path. The `llama` runtime name is forbidden in-tree. |
| Streaming | Master exposes `generate()`. PR [528](https://github.com/resemble-ai/chatterbox/pull/528) (open as of 2026-06-17) proposes `ChatterboxTurboTTS.stream()` for CPU playback before the utterance finishes. There is no Chatterbox model named Flash in the model zoo. | The owner's "Flash streaming eval" is this CPU time-to-first-audio question. Planned. Not run. |
| Watermark | Perth inside `generate()`. Perth is a separate package (`resemble-perth` on a git pin in `pyproject.toml`). Its licence was not read here. | Nano path should keep a watermark next to the Art. 50 disclosure. Do not vendor Perth in this change. |
| Dependencies | `pyproject.toml`: torch, torchaudio, transformers, diffusers, safetensors, plus others. Download patterns include `.safetensors` and `.pt`. | PyTorch stays out of process. `.pt` is not a lock weight format. Nothing is downloaded here. |

Output rate in the public ports is described as 24 kHz, which matches the
Galion clock. That match was not re-measured.

**Worker.** The scaffold is `docs/galion-voice-v2/CLONE-WORKER.md`: one
long-lived process, the same idea as the warm Piper worker. Text in, PCM
out, no file of samples. Nano is the primary. Turbo is the option on the
same stdin line. If the pinned weights are missing, Piper
(`en_US-ljspeech-medium`) speaks. Flite remains the pack-missing fallback
under that. The Python stub does not call generate and refuses a reference
clip. Weights are fetched only with `GALION_CLONE_FETCH=1`. The build plan
for wiring `generate`, and for `audio_prompt_path` after rights, is
`CHATTERBOX-WORKER.md`. That plan does not edit `LOCKS.toml`.

**Personas.**

- **Samantha** — conversational and emotional. Paralinguistic tags are for her. The slogan's "seamless connection" is her register: warmth, timing, pauses. The film voice is a research target under the legal hold.
- **Jarvis** — calm, precise, command and interface. Short lines. Tags off. The film voice is a research target under the same hold.

## CPU and memory

t204u and t205u: admin approves full threading, SIMD, and maximum memory, with
no artificial caps, for Chatterbox Nano/Turbo workers, for every Galion Voice
worker, and for the Linux Cursor host. The work stays self-offline. No cloud
accelerator. Recording the approval does not change host settings.

## LOCKS

t230u wrote `[formats.exceptions.v2s14_chatterbox]`. It is an out-of-process
PyTorch worker only, sunset `2027-02-01` (90 days from 2026-11-03). The A21
Piper pin is unchanged. Global `foreign_runtime_allowed` stays false.

- The worker is a separate process. Nothing in the Galion crates links PyTorch.
- `in_tree_ort`, `in_tree_ggml`, and `in_tree_llama` are false on that pin. A community ONNX export does not ride the Piper pin. A GGML-family port is still the wrong shape.
- `weights_vendored = false`. Do not commit Nano or Turbo weights. `scripts/clone/fetch-clone-weights.sh` refuses unless `GALION_CLONE_FETCH=1`.
- `reference_clips = false`. No film audio.
- Do not "temporarily" port the net onto candle.

## Dropped external APIs

Looked up on public pages on 2026-11-03, then **dropped**. Not the path.
Prices move. Treat every figure as a snapshot, not a quote.

| Vendor | Snapshot | Why it is not the path |
|---|---|---|
| ElevenLabs | API TTS v4 $0.08 / 1k characters; v4 Turbo and Flash/Turbo $0.04 / 1k. [Pricing](https://elevenlabs.io/pricing/api). Instant clone marketed from the Starter plan ($6/mo; a first-month promo was on the marketing page). | Cloud API. Superseded. |
| Cartesia | Instant cloning on Pro ($5/mo). Professional slots from Startup ($49/mo, 2) and Scale ($299/mo, 4). Training listed free; speech about 1 credit per character. [Pricing](https://www.cartesia.ai/pricing). | Cloud API. Superseded. |
| Play.ht / PlayAI | Secondary reports: API offline around July 2025, service shut 2025-12-31 after the team moved to Meta. Not re-checked by calling it. | Not a live option. A different host named playht.co was not treated as this vendor. |
| Resemble hosted cloning | Docs still describe a clone API. August 2026 writeups say new customers are not being sold voice generation (detection pivot). Those writeups were not an account check. | Hosted product is not the path. The MIT Chatterbox code is. |
| Inworld | Instant cloning on the published plans. TTS-2 $25 / 1M characters on demand, Flash $15 / 1M; Growth lists $12.50 and $7. [Pricing](https://inworld.ai/pricing). | Cloud API. Superseded. |
| Fish Audio | API pay-as-you-go. `s2.1-pro` $15 / 1M UTF-8 bytes. `s2.1-pro-free` listed at $0 on the pricing doc. [Pricing](https://docs.fish.audio/developer-guide/models-pricing/pricing-and-rate-limits). | Cloud API. Superseded. Confirm any free-tier end date before relying on the $0 line. |
| Azure Personal Voice | Limited access, verbal consent, 5–90 s sample. [Overview](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/personal-voice-overview). The pricing HTML did not render dollar amounts in this session. | Cloud, gated. Superseded. Do not budget from a secondary scrape. |

## Sixteen seats

Planned. Not spawned. None of these seats download a model, call a paid API,
or ingest a clip. Replaces the external-API swarm, which is dropped.

| # | Role | Deliverable | Status |
|---|---|---|---|
| 1 | Architecture map (T3, VoiceEncoder, S3Gen, vocoder) | `docs/galion-voice-v2/VOICE-CLONING.md` | planned-not-spawned |
| 2 | Nano 110M CPU worker contract | `docs/galion-voice-v2/CLONE-WORKER.md` | scaffolded, no weights, no generate |
| 3 | Turbo quality option, same process | `docs/galion-voice-v2/CLONE-WORKER.md` | scaffolded, same stub |
| 4 | CPU streaming / Flash eval plan (no run) | `docs/galion-voice-v2/FLASH-EVAL.md` | plan + harness stub, no number |
| 5 | Zero-shot ~10 s conditioning, no fine-tune | `docs/galion-voice-v2/CHATTERBOX-WORKER.md` | plan written; `reference_clips` stays false; seat not spawned |
| 6 | Jarvis research-target mapping | `docs/galion-voice-v2/VOICE-CLONING.md` | planned-not-spawned |
| 7 | Samantha tags and emotional register | `docs/galion-voice-v2/VOICE-CLONING.md` | planned-not-spawned |
| 8 | Rights, consent, licence gate | `LEDGER.md` | planned-not-spawned |
| 9 | Art. 50 watermark note (no Perth vendor) | `docs/galion-voice-v2/VOICE-CLONING.md` | planned-not-spawned |
| 10 | Piper fallback and warm-worker shape | `docs/galion-voice-v2/CLONE-WORKER.md` | scaffolded; Piper speaks when the clone slot is absent |
| 11 | TTFA eval plan (no measurement) | `docs/galion-voice-v2/FLASH-EVAL.md` | harness prints `measured: false` until a flagged run |
| 12 | Presence and barge-in with a clone | `docs/galion-voice-v2/GALION-VOICE-V2.md` (V2-S11) | planned-not-spawned |
| 13 | No PCM on disk, no film audio in git | `docs/galion-voice-v2/SECURITY-EDGE.md` | planned-not-spawned |
| 14 | CI: no download, no paid API | `scripts/clone/fetch-clone-weights.sh` | fetch refuses unless `GALION_CLONE_FETCH=1`; CI does not set it |
| 15 | CPU/RAM policy (record only) | `LEDGER.md` | planned-not-spawned |
| 16 | Docs and LEDGER upkeep | `LEDGER.md` | planned-not-spawned |

Slogan seats are separate and also not spawned: `SLOGAN-AGENTS.md`.

## Sources

Read 2026-11-03. No model download. No API call that spends money.

- [Chatterbox repository](https://github.com/resemble-ai/chatterbox)
- [MIT licence](https://raw.githubusercontent.com/resemble-ai/chatterbox/master/LICENSE)
- [pyproject.toml](https://github.com/resemble-ai/chatterbox/blob/master/pyproject.toml)
- [tts_turbo.py](https://github.com/resemble-ai/chatterbox/blob/master/src/chatterbox/tts_turbo.py)
- [t3.py](https://github.com/resemble-ai/chatterbox/blob/master/src/chatterbox/models/t3/t3.py)
- [PR 528, Turbo streaming, open](https://github.com/resemble-ai/chatterbox/pull/528)
- Risk examples, not dependencies: [chatterbox.cpp](https://github.com/gianni-cor/chatterbox.cpp), [community ONNX](https://huggingface.co/onnx-community/chatterbox-ONNX)

— Maciej Grajczyk
