# C — reactions, incremental speech, and duplex on CPU

Research only. Written 2026-11-28. No product code, no pull request, no merge.
Numbers below are either in this tree (path named) or copied from a page fetched
that day (link named). A figure this note did not read is **unknown**.
This checkout did not run any of the outside models, so none of their speed
numbers are a Galion measurement.

Real-time factor (RTF) in this note means seconds of audio divided by seconds
of compute, unless a source defines the ratio the other way. Greater than 1
is faster than real time. Where a source uses the inverse, the sentence says so.

A note titled “HORBM interactive (12)” is **not in this checkout** and was not
on `origin/main` at `2b6014d5`. The PersonaPlex and RL notes that are here are
[`docs/galion-voice-v2/PERSONAPLEX-REANCHOR.md`](../../galion-voice-v2/PERSONAPLEX-REANCHOR.md)
and [`docs/galion-context/2026-11-08/11-personaplex-indoor-rl.md`](../../galion-context/2026-11-08/11-personaplex-indoor-rl.md).
`FARGAN` does not appear in this tree. This note does not propose a vocoder rewrite.

## Ten lines: streaming, TTFA, turn-taking

1. The listening socket is `voice-stack/local-server`: mic frames, then an energy gate, then one `horbm_voice::VoiceLoop` tick (ear, brain, mouth). CPU is the boot device. Piper is linked only with `--features piper`. `NativeMouth` (`GALION_MOUTH=native`) speaks `TurnOut.first_clause` then `rest` as text and returns no PCM (`voice-stack/local-server/src/mouth.rs`, `voice-stack/local-server/src/live.rs`).
2. The s711 Chatterbox clone is selected only when a local wav, a local checkpoint, and `GALION_VOICE_DEVICE=cuda` are all set. The mouth module records no measured s711 CPU real-time factor and does not treat a Chatterbox `generate` on CPU as able to feed the 80 ms speaker clock (`mouth.rs`).
3. Ear off: a quiet gap of 640 ms ends the turn. Ear on: the bundled end-of-turn scorer can yield at its mild pause (160 ms) or hold until its hold pause (720 ms), and never later than 640 ms plus one 80 ms frame (`live.rs`; `crates/horbm-ear/eot.toml` `mild_pause_ms`, `hold_pause_ms`).
4. The phone path is a different cascade, `galion-cascade` `PhoneLane`: Vosk (or a stub), a loopback Hermes completion, then Piper or flite at 24 kHz. `EarlyReply` speaks a sentence on `.` `!` `?`, or at 16 words if no mark has arrived, while later tokens are still coming (`crates/galion-cascade/src/early_reply.rs` `FIRST_SENTENCE_WORDS`; `crates/galion-cascade/src/phone.rs` `speak_transcript`). `docs/cascade/PHONE-TURN-API.md` still says 8 words. The constant in code is 16.
5. A product TTFA is not published from this tree. `scripts/live-voice-check.sh` defines `ttfa_ms` as wall time from the server vad-end frame to the first speaker PCM, and prints `unmeasured` when that clock was not seen (`docs/horbm/README.md`). The symphony harness defaults to `measured: false` (`docs/galion-voice-v2/symphony/TTFA.md`).
6. `horbm-duplex` steps both bands on one virtual 80 ms frame (16 kHz, 1280 samples). Overlap other than a listed backchannel stops the me band on the frame that starts 160 ms after onset, inside a 200 ms barge-in lock (`docs/horbm/foundations/F-duplex-s26.md`). The S43 stand-in, not Piper, printed turn-taking latency 160 ms p50 and p95, and interruption stop 160 ms (`docs/horbm/foundations/F-turn-eval-s43.md`). Those are frame counts.
7. A listening nod is one of `mm-hm`, `yeah`, `right`, or a soft breath-laugh, at least 4.8 s after the last, on the quiet frame after a clause-final fall, while the user still holds the turn. It does not call the brain (`horbm/crates/horbm-voice/src/backchannel.rs`; `docs/horbm/foundations/S38-listening-backchannel.md`).
8. `horbm-breath` plans phrase pauses and steps lung volume, flow, open quotient, breathiness, nasal coupling, and mouth openness on that 80 ms frame. `galion-source-filter` shapes Piper or flite PCM from the frame and mixes noise on exhale (`horbm/crates/horbm-breath/src/lib.rs`; `docs/horbm/foundations/F-mouth-source-filter.md`). The duplex breath port writes no samples (`horbm/crates/horbm-duplex/src/breath.rs`).
9. `emotion-in` reads prosody and stays denied until `--sense` or `GALION_EMOTION_SENSE=1` (`voice-stack/emotion-in/src/bin/emotion-in.rs`). `Brain::turn_with_ear` does not call `Brain::step_moment` (`docs/horbm/PLAN-brain.md`). The turn is a lookup. A miss is `I don't know.` There is no student decode on the tick.
10. PersonaPlex and the indoor RL plan are documents. Weights are not in the tree. The re-anchor says PersonaPlex-7B does not fit the described CPU box, Piper stays the renderer on path A, and the RL stages have not been run (`PERSONAPLEX-REANCHOR.md`; `11-personaplex-indoor-rl.md`).

## What would make the voice feel live, and what we already have

A live human conversation has three cheap pieces and one expensive one.

The cheap pieces are already sketched in this tree: a breath or a laugh that is
not a sentence, a first clause that starts before the rest of the reply exists,
and a short “mm-hm” while the other person still holds the floor. The breath
crate and the source filter shape speech. They do not emit a laugh, a sigh, or
a backchannel as its own sound. The nod list is four tokens. The default mouth
for those tokens is a generated tone unless Piper is built in
(`S38-listening-backchannel.md`).

The expensive piece is a model that listens and speaks on the same 80 ms frame,
the way Moshi and PersonaPlex do. That is not running here. The phone lane waits
for a sentence or for 16 words, then calls the speech stack. The 7881 loop waits
for end of turn, then speaks.

## Candidates

Stars and last push are GitHub `stargazers_count` and `pushed_at` read on
2026-11-28. “Last push” is not a commit message.

### NVIDIA PersonaPlex

| Field | Value |
|---|---|
| Repo | [NVIDIA/personaplex](https://github.com/NVIDIA/personaplex) |
| Stars / last push | 12370 / 2026-11-06 |
| Code license / language | MIT / Python |
| Weights | [nvidia/personaplex-7b-v1](https://huggingface.co/nvidia/personaplex-7b-v1). NVIDIA Open Model License, plus CC-BY-4.0. Card says 7B parameters and also “Model size 8B params”, BF16. |
| What it is | Full-duplex speech-to-speech on Moshi. Voice prompt plus text role prompt. Listens and speaks at once. |
| Published GPU timing | FullDuplexBench on an A100 80 GB: smooth turn-taking latency 0.170 s, user-interruption latency 0.240 s, backchannel TOR 0.273 ([model card](https://huggingface.co/nvidia/personaplex-7b-v1)). |
| CPU | Not faster than real time on the measurements found. [TTS Navi, 2026-11-25](https://ttsnavi.com/blog/personaplex-7b-full-duplex-speech-to-speech-english-only-cpu-test-18x-slower/) ran official BF16 weights on a 4-core ARM box, no GPU: about 1.49 s per 80 ms frame, about 18.6× slower than real time, one 10 s sample. The NVIDIA README’s `--cpu-offload` flag is for a GPU that is short of VRAM, and it also says a CPU-only PyTorch install can run offline evaluation ([README](https://github.com/NVIDIA/personaplex)). |
| ONNX | [soniqo/PersonaPlex-7B-ONNX](https://huggingface.co/soniqo/PersonaPlex-7B-ONNX) publishes RTF on an RTX 5090, not on CPU. Their RTF is frame latency divided by the 80 ms interval, so below 1 is faster. Best listed variant is 1.12×, slightly slower than real time on that GPU. [elbruno/ElBruno.PersonaPlex](https://github.com/elbruno/ElBruno.PersonaPlex) (6 stars, MIT, C#, last push 2026-11-18) says FP16 ONNX on CPU is about 0.4 s per step. It does not publish an RTF. Its Blazor UI is server-side and half-duplex. It says it does not run the model in the browser. |
| Apple | [mu-hashmi/personaplex-mlx](https://github.com/mu-hashmi/personaplex-mlx), 91 stars, MIT, Python, last push 2026-02-18. MLX on Apple silicon. No x86 CPU RTF in the description. |
| Mobile | No phone build with a measured RTF was found. |
| Reuse | The benchmark row and the idea of a voice prompt plus a role text, which the re-anchor already treats as a plan. Do not load the 7B. The card does not describe an RL stage. Whether the PDF does is **unknown** here; this pass did not read the PDF past the card’s citation ([arXiv:2602.06053](https://arxiv.org/abs/2602.06053)). |

### Moshi / Kyutai, and the Rust stack

| Field | Value |
|---|---|
| Repo | [kyutai-labs/moshi](https://github.com/kyutai-labs/moshi) |
| Stars / last push | 11611 / 2026-09-09 |
| Code license / language | Apache-2.0 / Python, plus a Rust tree |
| Weights | CC-BY 4.0, stated in the [README](https://github.com/kyutai-labs/moshi/blob/main/README.md). Candle int8: [kyutai/moshika-candle-q8](https://huggingface.co/kyutai/moshika-candle-q8) and the Moshiko twin. |
| What it is | 7B temporal transformer plus a depth transformer. Two audio streams. Text inner monologue. Mimi at 12.5 Hz. Theoretical latency 160 ms (80 ms frame + 80 ms acoustic delay). Practical latency “as low as 200 ms on an L4 GPU” ([README](https://github.com/kyutai-labs/moshi/blob/main/README.md)). |
| Rust | Not a separate `moshi-rs` repository. GitHub repository search for `moshi-rs` returned 0 hits on 2026-11-28. The Rust code is `kyutai-labs/moshi/rust` (`moshi-backend`, `moshi-core`, `moshi-cli`). Candle q8 is selected with `config-q8.json`. The documented server commands are `--features cuda` or, on macOS, `--features metal`. |
| CPU | [Issue 181](https://github.com/kyutai-labs/moshi/issues/181), Laurent Mazare, 2025-01-05: the PyTorch server needs an NVIDIA GPU, and real time “would be very tricky on cpu only even with 120 cores.” The Rust build “should be cpu compatible” and “the model [is] likely not being real-time.” No measured CPU RTF was found. |
| Mobile | The README assigns `moshi_mlx/` to “on-device inference on iPhone and Mac.” A measured iPhone RTF was not in that section. MLX int4 and int8 checkpoints are listed. They are Apple GPU/ANE paths, not an ONNX CPU path. |
| Reuse | The 80 ms dual-stream clock, which this tree already uses, and the inner-monologue idea (text beside audio). Do not run the 7B on this CPU. `crates/galion-mimi` stays a stub in the re-anchor. Do not import Mimi weights in this note. |

Related, not Moshi itself: [kyutai-labs/delayed-streams-modeling](https://github.com/kyutai-labs/delayed-streams-modeling), 3057 stars, Apache-2.0, Python, last push 2026-01-26. Speech-to-text and text-to-speech, not the full-duplex dialogue model. CPU RTF **unknown** in this pass.

### Sesame CSM

| Field | Value |
|---|---|
| Repo | [SesameAILabs/csm](https://github.com/SesameAILabs/csm) |
| Stars / last push | 14761 / 2025-05-27 |
| License / language | Apache-2.0 / Python |
| What it is | Conversational speech model. Llama backbone plus a small decoder that emits Mimi codes. A fine-tune powers their demo. It is a TTS-style generator, not a full-duplex listener. |
| CPU | The README requires a CUDA GPU. The sample then sets `device = "cpu"` if neither MPS nor CUDA is present. No CPU RTF is published. [Issue 78](https://github.com/SesameAILabs/csm/issues/78): one report says CSM takes a couple of seconds to generate one second of audio on an RTX 3060. That is slower than real time, and it is a GPU anecdote, not a CPU measurement. |
| Mobile | **Unknown.** |
| Reuse | The idea of a conversation context (previous lines) in front of TTS. The 1B Llama backbone is the wrong size for this CPU mouth. |

### Orpheus

| Field | Value |
|---|---|
| Repo | [canopyai/Orpheus-TTS](https://github.com/canopyai/Orpheus-TTS). `canopylabs/orpheus-tts` returned 404. |
| Stars / last push | 6417 / 2025-12-05 |
| License / language | Apache-2.0 / Python |
| What it is | Llama-based speech model, about 3B in the finetune card. Emotion tags. Streaming is claimed at about 200 ms, and about 100 ms with input streaming, on the [GGUF card](https://huggingface.co/QuantFactory/orpheus-3b-0.1-ft-GGUF), which does not name a CPU. |
| CPU path | Official [no-GPU example](https://github.com/canopyai/Orpheus-TTS/blob/main/additional_inference_options/no_gpu/README.md) uses `orpheus-cpp` and a CPU wheel of `llama-cpp-python`. It publishes no RTF. [yuv008/Orpheus-tts](https://github.com/yuv008/Orpheus-tts) reports about 7.5 tokens/s and about 95 s per sentence on an Azure D4s_v3 CPU, against about 65 tokens/s and about 3 s on a T4. That is slower than real time. A separate SNAC decoder port measured a first CPU decoder at 992 ms for 85.3 ms of audio, later about 1.01× at best, on a machine the post does not fully specify here ([acceldium, 2026-06-30](https://acceldium.com/blog/5_snac_decoder/)). |
| GPU contrast | [ipritamdash/maya-orphues-whole-pipeline](https://github.com/ipritamdash/maya-orphues-whole-pipeline) reports Orpheus Q4_K_M at RTF 0.64 (faster) and 129 tokens/s on an A10G. That is a GPU number. |
| Mobile | **Unknown.** |
| Reuse | The tag vocabulary and the sentence-streaming overlap (LLM tokens into TTS before the paragraph ends). The 3B GGUF is not a CPU real-time mouth on the evidence above. |

### Dia and Dia2

| Field | Value |
|---|---|
| Dia | [nari-labs/dia](https://github.com/nari-labs/dia). 19431 stars, Apache-2.0, Python, last push 2025-11-19. |
| Dia2 | [nari-labs/dia2](https://github.com/nari-labs/dia2). 1192 stars, Apache-2.0, Python, last push 2025-11-29. |
| Tags | Parentheses, not brackets. The Dia README lists `(laughs)`, `(sighs)`, `(inhales)`, `(exhales)`, `(coughs)`, `(gasp)` and others, inside `[S1]` / `[S2]` dialogue. |
| GPU | Dia README, RTX 4090: float16 with compile is ×2.2 real time, about 4.4 GB. “CPU support is to be added soon.” |
| CPU | Dia: not supported in that README. Dia2’s CLI “auto-selects CUDA when available (otherwise CPU)” ([README](https://github.com/nari-labs/dia2)). No CPU RTF was found. Treat Dia2-on-CPU speed as **unknown**, and do not assume it is faster than real time. |
| Mobile | **Unknown.** The public demo named in the Dia README is a Hugging Face ZeroGPU space. |
| Reuse | The tag list, as a vocabulary our breath planner could emit. Not the 1.6B or 2B weights. |

### Chatterbox

| Field | Value |
|---|---|
| Repo | [resemble-ai/chatterbox](https://github.com/resemble-ai/chatterbox) |
| Stars / last push | 28173 / 2026-11-04 |
| License / language | MIT / Python |
| Status | The README says that as of August 2026 the project is retired research and is no longer maintained. The same README still documents the model zoo below. |
| Exaggeration | Original model: default `exaggeration=0.5`, `cfg_weight=0.5`. For dramatic speech, exaggeration around 0.7 or higher, and a lower `cfg_weight` ([README](https://github.com/resemble-ai/chatterbox/blob/master/README.md)). |
| Tags | Turbo and Nano: `[laugh]`, `[chuckle]`, `[cough]`, and “more”. A Resemble blog also names `[sigh]` ([2026-07-06](https://www.resemble.ai/resources/chatterbox-nano-and-flash-speed-at-the-edge-throughput-at-scale)). |
| CPU | Nano, 110M: the README says 3× faster than real time on 8 CPU cores. The blog says the same, “8 threads”, and 10× on GPU. The CPU model name is not in the README. Turbo and the original 500M model are a different story: a third-party bench lists warm CPU RTFx 0.67 for Turbo and 0.33 for original Chatterbox, which is slower than real time ([tts-bench linux-cloning speed](https://5uck1ess.github.io/tts-bench/linux-cloning/speed.html)). That page did not, in the excerpt kept here, name the CPU. |
| Watermark | Every generated file is watermarked with Perth. |
| Voice | Turbo and Nano `generate` in the README take a reference clip. |
| Mobile | “On-device” is the README’s phrase for Nano. No phone binary or phone RTF was found. |
| Reuse | Nano is the only Chatterbox size with a vendor CPU real-time claim, and it has laugh tags. It is a poor fit as a drop-in for the 80 ms mouth: our server already refuses to treat Chatterbox `generate` as that clock, the weights need a reference clip, the file is watermarked, and the repo is marked unmaintained. A listening test of Nano is still fair. Do not point the live mouth at it. |

### Bark

| Field | Value |
|---|---|
| Repo | [suno-ai/bark](https://github.com/suno-ai/bark) |
| Stars / last push | 39426 / 2024-08-19 |
| License / language | MIT / Python (repo language is Jupyter Notebook on the API). README says checkpoints are available for commercial use. |
| What it is | Text-to-audio, including laughter, sighing, and crying. Not a turn-taking model. |
| CPU | README: tested on CPU and GPU. Roughly real time on enterprise GPUs with a nightly PyTorch. On older GPUs, Colab, or CPU, “significantly slower.” Full model about 12 GB VRAM; small models about 8 GB, with `SUNO_USE_SMALL_MODELS`. No numeric CPU RTF. |
| Mobile | **Unknown.** |
| Reuse | Proof that a generative model can emit a laugh without a separate clip. Too slow and too unmaintained (last push 2024-08-19) to be the mouth. |

### XTTS streaming

| Field | Value |
|---|---|
| Code | [coqui-ai/TTS](https://github.com/coqui-ai/TTS), 46292 stars, MPL-2.0, Python, last push 2024-08-16. |
| Weights | Coqui Public Model License, non-commercial. This note does not quote the licence text. Confirm it before any use beyond a private listen. |
| Streaming | Coqui’s XTTS page says streaming inference under 200 ms to the first chunk. The manual example calls `model.cuda()` ([docs](https://docs.coqui.ai/en/stable/models/xtts.html)). |
| CPU | [coqui-ai/xtts-streaming-server](https://github.com/coqui-ai/xtts-streaming-server), 364 stars, MPL-2.0, last push 2024-06-26, ships a CPU image and labels it “not recommended.” The third-party bench above lists XTTS-v2 warm CPU RTFx 0.79× and warm TTFA 10.05 s (slower than real time). |
| ONNX | [sjamthe/XTTSv2-Streaming-ONNX](https://github.com/sjamthe/XTTSv2-Streaming-ONNX), 0 stars, no SPDX licence on the API, last push 2026-03-19. INT8 GPT, ONNX Runtime, streaming chunks. The README does not publish an RTF. |
| Mobile | **Unknown.** |
| Reuse | The streaming-chunk idea only. The licence and the CPU numbers argue against adopting the model. |

### CosyVoice 2 streaming

| Field | Value |
|---|---|
| Repo | [QwenAudio/CosyVoice](https://github.com/QwenAudio/CosyVoice). `FunAudioLLM/CosyVoice` resolves to this repository. |
| Stars / last push | 24550 / 2026-05-25 |
| License / language | Apache-2.0 / Python |
| Streaming | README: text-in and audio-out streaming, “latency as low as 150 ms.” The sentence does not say CPU. |
| Paper | [arXiv:2412.10117](https://arxiv.org/abs/2412.10117). Chunk-aware causal flow matching so one model does streaming and non-streaming. Section 2.8 is DPO on speaker similarity and ASR word error, plus a differentiable ASR reward. That is training, on GPU-class hardware in the paper’s tables. It is not a published CPU inference RTF. |
| CPU | **Unknown.** No CPU real-time number was found in the README lines read here. |
| Mobile | **Unknown.** |
| Reuse | The training lesson in the RL section below, and the “start from a short chunk” streaming shape. The flow-matching decoder is not a CPU mouth on the evidence in hand. |

### GLM-4-Voice

| Field | Value |
|---|---|
| Repo | [zai-org/GLM-4-Voice](https://github.com/zai-org/GLM-4-Voice). `THUDM/GLM-4-Voice` resolves here. |
| Stars / last push | 3248 / 2024-12-05 |
| License / language | Apache-2.0 / Python |
| What it is | Speech tokenizer at 12.5 tokens/s, a 9B speech LM, and a CosyVoice-style decoder. The README says the decoder can start after 10 speech tokens. |
| Latency | The paper sets the streaming block to 0.8 s, so the first waveform waits for that block ([arXiv:2412.02612](https://arxiv.org/abs/2412.02612)). |
| CPU | **Unknown.** A secondary write-up says CUDA is required ([DeepWiki requirements](https://deepwiki.com/THUDM/GLM-4-Voice/2.1-requirements)). That page is not the paper. Treat “GPU only” as the practical reading, and the CPU RTF as unpublished. |
| Mobile | **Unknown.** |
| Reuse | The 12.5 Hz token rate matches our frame. The 9B model does not fit the CPU rule. |

### Mini-Omni and Mini-Omni2

| Field | Value |
|---|---|
| Mini-Omni | [gpt-omni/mini-omni](https://github.com/gpt-omni/mini-omni). 3587 stars, MIT, Python, last push 2024-11-05. |
| Mini-Omni2 | [gpt-omni/mini-omni2](https://github.com/gpt-omni/mini-omni2). 1940 stars, MIT, Python, last push 2025-01-16. Description: vision, speech, and duplex, “towards open-source GPT-4o.” |
| What it is | Speech in, text and audio out together, streaming audio. The README claims real-time speech-to-speech with no extra ASR or TTS. |
| CPU | **Unknown.** The README excerpt does not name a device or an RTF. |
| Mobile | **Unknown.** |
| Reuse | “Talk while thinking”: emit audio tokens before the text answer is finished. Our phone lane already speaks a 16-word prefix. That is the same idea at text level, without their speech LM. |

### LLaMA-Omni and LLaMA-Omni2

| Field | Value |
|---|---|
| LLaMA-Omni | [ictnlp/LLaMA-Omni](https://github.com/ictnlp/LLaMA-Omni). 3146 stars, Apache-2.0, Python, last push 2025-05-19. Built on Llama-3.1-8B-Instruct. |
| Latency | README: “as low as 226 ms.” The ICLR 2025 paper measures 236 ms minimum on one NVIDIA L40, with unit chunk size 10 ([paper PDF](https://proceedings.iclr.cc/paper_files/paper/2025/file/90d1fc07f46e31387978b88e7e057a31-Paper-Conference.pdf)). Both figures are GPU. |
| LLaMA-Omni2 | [ictnlp/LLaMA-Omni2](https://github.com/ictnlp/LLaMA-Omni2). 282 stars, no SPDX licence on the GitHub API, Python, last push 2025-05-19. Latency **unknown** in this pass. |
| CPU | **Unknown.** No CPU RTF was found. An 8B Llama backbone is not a CPU real-time candidate on the Moshi author’s own 7B warning. |
| Mobile | **Unknown.** |
| What it is not | It is speech-in, speech-out with a streaming vocoder chunk. It is not Moshi’s two live audio streams. |
| Reuse | The chunk-size knob (their Ω) is the same trade our `EarlyReply` word cap already is: shorter chunks, earlier audio, more seams. |

### RL-trained prosody

These are training methods. None of them is a CPU speech engine.

| Source | What it actually did | CPU inference |
|---|---|---|
| CosyVoice 2, [arXiv:2412.10117](https://arxiv.org/abs/2412.10117) §2.8 | DPO on preferred vs rejected speech tokens. Rewards are ASR word error and speaker similarity, plus a frozen ASR likelihood on the LM’s own tokens. Table 11 is one target speaker. | Not published. Training is not a CPU job. |
| [arXiv:2509.18531](https://arxiv.org/html/2509.18531), “No Verifiable Reward for Prosody” | GRPO on CER/NLL lowered error and made speech monotone. Iterative DPO with 200 new human pairs per round restored preference (ELO) on Korean call-center speech. | Not published. |
| CraftTTS, [Interspeech 2026, DOI 10.21437/interspeech.2026-2018](https://doi.org/10.21437/interspeech.2026-2018) | Word-level prosody tags on CosyVoice 2, then SFT+DPO so the model stops ignoring the tags. | Not published. |

Our own RL sketch is still a plan: DPO then GRPO against a virtual user, rewards for barge-in inside 200 ms, and an explicit ban on engagement rewards (`PERSONAPLEX-REANCHOR.md`). `11-personaplex-indoor-rl.md` says that work has not started. The 2509.18531 result is a warning for that plan: a reward that only checks the words can flatten the voice. It is not a reason to start training on this box. The re-anchor already says training does not fit the machine.

## Non-verbal vocalization and breath, including small repos

### Flock Speech — 0 stars, the one that matches the CPU rule

| Field | Value |
|---|---|
| Repo | [flockspeech/flockspeech](https://github.com/flockspeech/flockspeech) |
| Stars / last push | 0 / 2026-11-11 |
| License / language | Apache-2.0 for code and voice weights. Bundled Vocos weights MIT. Phonemizer and espeak-ng, installed beside it, are GPL-3.0 ([README](https://github.com/flockspeech/flockspeech/blob/main/README.md)). |
| What it is | Two English ONNX voices, 44.2 MB each, 11.4M acoustic parameters plus a 13.5M vocoder. No PyTorch. Goldfinch was trained on DisfluencySpeech and accepts tags. Junco ignores tags. |
| Tags | `[laugh]`, `[breath]` (also `[inhale]` / `[exhale]`), `[throat]`, `[lipsmack]`. `[sigh]` and `[noise]` are marked weak. Fillers are written as words (`uh`, `um`). |
| CPU | README: 8.7× real time on one thread, 19× on four, Apple M1. First audio when streaming about 0.25 s. Peak memory about 215–265 MB. Streaming is sentence by sentence, not token by token. |
| Mobile | README: 64-bit Raspberry Pi OS. Not a phone app. |
| Reuse | Best first listen. A side process that speaks one tagged clause. Do not replace Piper, and do not replace the source filter. The GPL phonemizer has to stay out of a closed binary. |

### Inflect Mini — events on a named CPU

| Field | Value |
|---|---|
| Repo | [owenawsong/Inflect](https://github.com/owenawsong/Inflect) |
| Stars / last push | 191 / 2026-11-12 |
| License / language | Apache-2.0 for Inflect-owned code and weights. Third-party pieces keep their own terms ([README](https://github.com/owenawsong/Inflect/blob/main/README.md)). |
| Events | Mini only: `[laugh]`, `[sigh]`, `[gasp]`, `[cough]`, `[yawn]`, `[inhale]`, `[exhale]`, `[clear_throat]`. The event command “runs on CPU at native 24 kHz.” The README says these are not emotion controls, and “do not assume streaming” or mobile exports. |
| CPU speech | Warm CPU throughput 1.81× real time on a Ryzen 9 3900X, 2 Torch threads, a 12-render probe of Mini native 24 kHz speech, not of the events. Warm GPU 162.1× on an RTX 5090, same caveat. |
| Mobile | README says not to assume a mobile export. |
| Reuse | A second listen if Goldfinch’s sigh is too weak. The event audio is assembled beside speech, which is how a breath sample can sit next to Piper without a new vocoder. |

### vox-tts — 0 stars, Rust, two speeds

| Field | Value |
|---|---|
| Repo | [Zertrox123/vox-tts](https://github.com/Zertrox123/vox-tts) |
| Stars / last push | 0 / 2026-11-20 |
| License / language | MIT code. Chatterbox MIT, Kokoro Apache-2.0, ONNX Runtime MIT, optional eSpeak GPL-3.0, Piper voices vary ([README](https://github.com/Zertrox123/vox-tts/blob/main/README.md)). |
| What it is | Rust CLI. Expressive path is Chatterbox Multilingual 0.5B with `exaggeration` and cached mood prompts. Lite path is Kokoro-82M or Piper, with DSP for whisper and breath. Markup includes `[breath]`, `[laughs]`, `[sighs]`. |
| CPU | Author: expressive engine roughly 0.3–0.5× real time on an Apple-silicon laptop (slower than real time). Lite personas 5–13×. Load about 20 s. |
| Mobile | No phone build named. Linux arm64 is listed. |
| Reuse | The markup parser and the idea that a lite Piper/Kokoro voice gets breath from DSP, while a big model is allowed to be slow. The 0.5B path fails the real-time bar by the author’s own number. The lite number is the interesting one, and it is one laptop, not our box. |

### Affectron and the benchmark

| Repo | Stars / last push | License | Note |
|---|---|---|---|
| [Choddeok/Affectron](https://github.com/Choddeok/Affectron) | 21 / 2026-07-16 | MIT, Python | ACL 2026 Findings. Emotional TTS with nonverbal vocalizations aligned to the text. A CPU/GPU/RTF line was not found in the README search. Speed **unknown**. |
| [ishine/Affectron](https://github.com/ishine/Affectron) | 0 / 2026-01-14 | no SPDX on the API | Same title. Not a second model. |
| [AmphionTeam/NV-Bench](https://github.com/AmphionTeam/NV-Bench) | 14 / 2026-06-04 | no SPDX on the API | A benchmark of nonverbal vocalization in TTS, not a synthesizer ([page](https://charlesnii.github.io/nvbench.github.io/)). Useful later as a score, not as a mouth. |
| [trondarild/nonverbal-sound-synthP5](https://github.com/trondarild/nonverbal-sound-synthP5) | 0 / 2022-02-21 | no SPDX on the API, Processing | Sound synthesis for nonverbal cues in Processing. Not speech, not a TTS model. Listed because the search returned it. Do not port it. |

GitHub repository search for the phrases “breath insertion TTS” and “laugh tts nonverbal” returned no hits. The repos above were found by searching “breath insertion text-to-speech”, “nonverbal vocalization”, and the model names.

## Ranked top 5 that can be real on CPU

Full-duplex 7B models are not on this list. PersonaPlex, Moshi, GLM-4-Voice, Mini-Omni, and LLaMA-Omni stay GPU references. Orpheus, Dia, Bark, CSM, XTTS, and CosyVoice 2 do not have a published CPU RTF above 1 from the pages read here. Where a CPU number exists, it is slower (Orpheus on a D4s_v3, XTTS and full Chatterbox on the third-party bench, Bark’s own “significantly slower”).

1. **Flock Speech Goldfinch.** ONNX, no PyTorch, author-measured 8.7× on one M1 thread, `[breath]` and `[laugh]` in the text. 0 stars, so the number has to be re-timed here before anyone believes it. GPL phonemizer stays a separate process.
2. **Inflect Mini events.** 1.81× on a named desktop CPU for the speech, and a CPU event command for inhale, exhale, laugh, and sigh. No streaming promise. Second because it is slower and heavier than Goldfinch, and the event RTF itself was not separated from the speech RTF.
3. **Chatterbox-Nano, as a listen only.** Vendor claim of 3× on 8 CPU cores, with paralinguistic tags, MIT. Held back by the watermark, the reference clip, the August 2026 retirement line, and this tree’s existing refusal to put Chatterbox `generate` on the 80 ms clock.
4. **vox-tts lite path, the idea more than the binary.** Rust, `[breath]` / `[laughs]` / `[sighs]`, and a claimed 5–13× lite RTF on Piper or Kokoro. 0 stars. The expressive Chatterbox path in the same binary is slower than real time and should not be turned on.
5. **The mouth we already have, plus the breath samples it already computes.** Piper (or flite) on CPU, `EarlyReply` at a sentence or 16 words, and `horbm-breath` inhale noise into a caller buffer. A nod is already a token (`mm-hm`, `yeah`, `right`, breath-laugh) at least 4.8 s apart. The missing sound is a short non-speech event at a pause the lungs already planned. That does not require a new model and does not touch a vocoder.

## Best first test

Run Goldfinch offline, on this CPU, on one sentence that contains `[breath]` and `[laugh]`, and on the same sentence with the tags removed. Record wall time, audio duration, and time to the first buffer. Do not open port 7881, do not download PersonaPlex or Moshi, and do not change Piper.

Pass looks like this: RTF above 1 on the cores this box actually has, a breath and a laugh a person can hear, and a first buffer that is not slower than one Piper clause of the same text. The M1 figure of 8.7× is the author’s, not the pass line.

If that fails, stop. The next model on the list (Inflect) is the fallback listen, not a second integration. If it passes, the only wiring worth sketching is: when `horbm-voice` already chooses a breath-laugh or when `horbm-breath` is in an inhale at a phrase break, hand that one tag to the small ONNX voice and mix the short clip. The reply text still goes through Piper. The source filter still shapes Piper. Nothing in that test rewrites a vocoder.

## What this pass did not settle

- A measured iPhone or Android RTF for Moshi MLX, PersonaPlex, Orpheus, or Nano.
- Dia2’s CPU speed. The CLI can select CPU. No number was published in the pages read.
- Mini-Omni’s device and RTF.
- Whether the PersonaPlex PDF describes an RL stage. The model card does not.
- Any of these RTF numbers on the Galion machine. They stay outside claims until a command on this box prints them.

## Sources

- This tree, commit `2b6014d5` on 2026-11-28: `voice-stack/local-server/src/mouth.rs`, `voice-stack/local-server/src/live.rs`, `crates/galion-cascade/src/early_reply.rs`, `crates/galion-cascade/src/phone.rs`, `crates/horbm-ear/eot.toml`, `horbm/crates/horbm-breath/src/lib.rs`, `horbm/crates/horbm-duplex/src/breath.rs`, `horbm/crates/horbm-voice/src/backchannel.rs`, `docs/horbm/README.md`, `docs/horbm/PLAN-brain.md`, `docs/horbm/foundations/F-duplex-s26.md`, `docs/horbm/foundations/F-turn-eval-s43.md`, `docs/horbm/foundations/S38-listening-backchannel.md`, `docs/horbm/foundations/F-mouth-source-filter.md`, `docs/galion-voice-v2/PERSONAPLEX-REANCHOR.md`, `docs/galion-voice-v2/symphony/TTFA.md`, `docs/cascade/PHONE-TURN-API.md`.
- [NVIDIA/personaplex](https://github.com/NVIDIA/personaplex) (pushed 2026-11-06)
- [nvidia/personaplex-7b-v1](https://huggingface.co/nvidia/personaplex-7b-v1)
- [PersonaPlex preprint](https://arxiv.org/abs/2602.06053)
- [TTS Navi CPU run](https://ttsnavi.com/blog/personaplex-7b-full-duplex-speech-to-speech-english-only-cpu-test-18x-slower/) (2026-11-25)
- [soniqo/PersonaPlex-7B-ONNX](https://huggingface.co/soniqo/PersonaPlex-7B-ONNX)
- [elbruno/ElBruno.PersonaPlex](https://github.com/elbruno/ElBruno.PersonaPlex)
- [mu-hashmi/personaplex-mlx](https://github.com/mu-hashmi/personaplex-mlx)
- [kyutai-labs/moshi](https://github.com/kyutai-labs/moshi) and [issue 181](https://github.com/kyutai-labs/moshi/issues/181)
- [kyutai/moshika-candle-q8](https://huggingface.co/kyutai/moshika-candle-q8)
- [kyutai-labs/delayed-streams-modeling](https://github.com/kyutai-labs/delayed-streams-modeling)
- [SesameAILabs/csm](https://github.com/SesameAILabs/csm) and [issue 78](https://github.com/SesameAILabs/csm/issues/78)
- [canopyai/Orpheus-TTS](https://github.com/canopyai/Orpheus-TTS) and [no-GPU README](https://github.com/canopyai/Orpheus-TTS/blob/main/additional_inference_options/no_gpu/README.md)
- [QuantFactory/orpheus-3b-0.1-ft-GGUF](https://huggingface.co/QuantFactory/orpheus-3b-0.1-ft-GGUF)
- [yuv008/Orpheus-tts](https://github.com/yuv008/Orpheus-tts)
- [SNAC in llama.cpp](https://acceldium.com/blog/5_snac_decoder/) (2026-06-30)
- [nari-labs/dia](https://github.com/nari-labs/dia) and [nari-labs/dia2](https://github.com/nari-labs/dia2)
- [resemble-ai/chatterbox](https://github.com/resemble-ai/chatterbox)
- [Chatterbox Nano blog](https://www.resemble.ai/resources/chatterbox-nano-and-flash-speed-at-the-edge-throughput-at-scale) (2026-07-06)
- [tts-bench speed table](https://5uck1ess.github.io/tts-bench/linux-cloning/speed.html)
- [suno-ai/bark](https://github.com/suno-ai/bark)
- [coqui-ai/TTS](https://github.com/coqui-ai/TTS), [XTTS docs](https://docs.coqui.ai/en/stable/models/xtts.html), [xtts-streaming-server](https://github.com/coqui-ai/xtts-streaming-server), [XTTSv2-Streaming-ONNX](https://github.com/sjamthe/XTTSv2-Streaming-ONNX)
- [QwenAudio/CosyVoice](https://github.com/QwenAudio/CosyVoice) and [arXiv:2412.10117](https://arxiv.org/abs/2412.10117)
- [zai-org/GLM-4-Voice](https://github.com/zai-org/GLM-4-Voice) and [arXiv:2412.02612](https://arxiv.org/abs/2412.02612)
- [gpt-omni/mini-omni](https://github.com/gpt-omni/mini-omni) and [gpt-omni/mini-omni2](https://github.com/gpt-omni/mini-omni2)
- [ictnlp/LLaMA-Omni](https://github.com/ictnlp/LLaMA-Omni), [LLaMA-Omni2](https://github.com/ictnlp/LLaMA-Omni2), [ICLR PDF](https://proceedings.iclr.cc/paper_files/paper/2025/file/90d1fc07f46e31387978b88e7e057a31-Paper-Conference.pdf)
- [arXiv:2509.18531](https://arxiv.org/html/2509.18531)
- [CraftTTS DOI](https://doi.org/10.21437/interspeech.2026-2018)
- [flockspeech/flockspeech](https://github.com/flockspeech/flockspeech)
- [owenawsong/Inflect](https://github.com/owenawsong/Inflect)
- [Zertrox123/vox-tts](https://github.com/Zertrox123/vox-tts)
- [Choddeok/Affectron](https://github.com/Choddeok/Affectron)
- [AmphionTeam/NV-Bench](https://github.com/AmphionTeam/NV-Bench)
