# Symphony puzzle pieces

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Inventory only.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

This file lists buried and named GitHub projects that already cover a seam of
Galion Voice V2 and the symphony stack. The point is to credit the authors and
to avoid writing the same piece again. It does not vendor any of them, it does
not edit `LOCKS.toml`, and it does not implement V3.

A symphony section is complete only when its gate passes in `SELF-EVAL.md`.
This inventory is not that gate.

## Filter

Fetched on 2026-11-03 from each repository's license file (GitHub
`/license`, or the license path in the git tree when the grant is not at the
repo root). Star counts are `stargazers_count` the same day. They will move.

A piece is eligible to borrow when the fetched grant is **MIT** or
**Apache-2.0**, including a clean `MIT OR Apache-2.0` dual grant. Rejected:

- GPL, AGPL, and BSD-3-Clause when that is the only grant
- a MIT text with an extra rider
- a missing license file (a `Cargo.toml` `license = "MIT"` line, or a README
  sentence, is not the grant)
- proprietary, non-commercial, and non-compete weight terms
- GitHub search badges whose license **file** was not opened (appendix)

Borrow means a later thin adapter and an author credit. Vendor means copying
the tree into this repo. The call below is borrow, or do not take the code.
Weights stay out of git (`weights_vendored = false`).

## Seams

| # | Seam | What "already solved" means here |
|---|---|---|
| 1 | Voice cloning / CPU TTS | Rust or an out-of-process CPU worker. Chatterbox Nano stays the clone primary. |
| 2 | Real-time audio | `cpal` capture, 80 ms frames, callback that does not allocate. |
| 3 | Noise / speech enhancement | User-scene layer. **Plan only.** V3 gate. Nothing in this file builds it. |
| 4 | Session facts | One SQLite file plus a small vector or factor index. Consent bit `remember` stays default deny. |
| 5 | CPU ASR / VAD / SER | Only if it is not Silero, Vosk, or Parakeet, which this tree and the research notes already name. |

## Locks this list is held to

From `LOCKS.toml` as it stands (not edited here):

- Heartbeat `frame_ms = 80`, `frame_hz = 12.5`.
- `ggml_container_allowed = false`. Forbidden runtimes include `candle`, `ort`,
  `tract`, `burn`, `llama`, `ggml`. `dev_oracle_only = ["candle"]`.
- Weight formats on the lock are `safetensors` (`.gq` later). No GGUF in Voice.
- Chatterbox is the V2-S14 exception: PyTorch, CPU, separate process,
  `chatterbox-nano` primary, `chatterbox-turbo` option, Piper fallback.
  Sunset 2027-02-01. Do not widen it.
- Piper is the A21 exception: CPU, separate process, ONNX Runtime stays inside
  that process. Sunset 2027-02-01. Do not widen it to ASR or to a second TTS.
- Consent `remember = "deny"` at session start. Persist wants
  `AffectStore<Consented>`. A session-fact store sits behind that bit.
- CPU-first. Voice session budget in the CPU foundation notes is about
  1 vCPU and about 0.65–1.15 GB RAM. A 1.7B weight file does not fit that
  budget.

In this tree today: Chatterbox and Piper are out-of-process speech backends
(`crates/galion-ml-voice`). Vosk is the letter-A ASR worker
(`docs/asr/VOSK.md`, `docs/policy/RATIFY-EN-ASR-PHONE-PATH.md`); the Vosk
exception text is not in `LOCKS.toml`. `LiveMic` is a stub with no `cpal`
and no ALSA (`crates/galion-console/src/mic.rs`). Silero and Parakeet are
named in `docs/research/galion-voice-v2/` and are not re-listed as buried
pieces.

## 1. Voice cloning / CPU TTS

Chatterbox Nano remains the clone that the symphony speaks. Pocket is an
optional borrow reference. The rows below are studies and rejects.

| Repo | Stars | SPDX (fetched) | Credit | Call |
|---|---:|---|---|---|
| [kyutai-labs/pocket-tts](https://github.com/kyutai-labs/pocket-tts) | 10181 | MIT | Kyutai Labs | Optional reference. Do not swap the primary. |
| [incordlabs/QORA-TTS-12Hz-1.7B](https://github.com/incordlabs/QORA-TTS-12Hz-1.7B) | 3 | Apache-2.0 | Copyright 2026 QORANET | Study the 12.5 Hz CPU path. Do not vendor. |
| [studio-ransom/best-tts-rust](https://github.com/studio-ransom/best-tts-rust) | 0 | Apache-2.0 | Copyright 2026 Studio Ransom | Non-clone CPU Kokoro study. Do not vendor weights. |
| [iGentAI/ferrocarril](https://github.com/iGentAI/ferrocarril) | 7 | Apache-2.0 | iGentAI (grant appendix unfilled) | Same Kokoro class, numerically checked. Do not vendor. |

### Pocket TTS

URL: https://github.com/kyutai-labs/pocket-tts

`LICENSE` is the MIT permission text, 1023 bytes, fetched from the repo root.
It has no copyright line and no rider. GitHub SPDX: `MIT`. Language: Python.
The `rust/` tree is a CPU-only CLI (`license = "MIT"` in `rust/Cargo.toml`)
on a pinned `xn-ptts` fork. That fork's license file was **not** opened.
Voice export is safetensors. README (2026-10) describes small English models
and a CPU install. 10181 stars: this is a known project, not a buried one.

**Seam 1.** Optional borrow reference beside Chatterbox Nano. A later adapter
would be out of process, same shape as the clone worker, and would not link
PyTorch or the Rust CLI into a Galion crate. Do not vendor. Do not treat
Pocket as the primary. The `xn-ptts` dependency stays unread until its
`LICENSE` is fetched.

**Locks.** Fits CPU and safetensors. It is a second TTS. The Chatterbox pin
names Piper as the fallback, not Pocket. No lock change in this inventory.

### QORA-TTS 12Hz 1.7B

URL: https://github.com/incordlabs/QORA-TTS-12Hz-1.7B

The URL `qora-protocol/QORA-TTS-12Hz-1.7B` resolves to this repository
(3 stars, Rust, pushed 2026-04-06). Fetched `LICENSE-APACHE` (Apache-2.0,
appendix `Copyright 2026 QORANET`) and `LICENSE-MIT` (standard MIT, same
copyright, no rider). The README states Apache-2.0. GitHub SPDX:
`Apache-2.0`. `Cargo.toml` depends on `safetensors` for tooling. The runtime
artifact the README describes is `model.qora-tts` (about 1559 MB, Q4), and
the README says safetensors are not required to run. Code rate in the README:
12.5 Hz, one code = 80 ms, 24 kHz. Pure Rust, no Python, no CUDA. CPU claim
is the author's, not measured here.

**Seam 1.** The 80 ms code clock matches `[clock]`. Credit QORANET if any
later note cites the architecture. Do not vendor the engine. Do not import
`model.qora-tts` (it is not safetensors and not `.gq`). 1.7B does not fit
the voice-session RAM budget. Chatterbox Nano stays primary.

**Locks.** The fetched `Cargo.toml` lists `half`, `serde`, `tokenizers`, and
`safetensors`. It does not list candle, `ort`, or a ggml crate. The runtime
file the README tells you to ship is still a custom Q4 blob, outside
`weight_formats`. Out-of-process use would be a new TTS exception. This file
does not add one.

### best-tts and ferrocarril (Kokoro, not a clone)

[best-tts](https://github.com/studio-ransom/best-tts-rust) — 0 stars.
`LICENSE` is Apache-2.0, `Copyright 2026 Studio Ransom`. README: Kokoro-82M,
CPU, int8 kernels, `std` only, no `ort`, no `tract`. The model is inside that
repository. Author's Pi 4 claim was not measured here.

[ferrocarril](https://github.com/iGentAI/ferrocarril) — 7 stars.
`LICENSE` is Apache-2.0. The appendix is still the unfilled
`Copyright [yyyy] [name of copyright owner]` template, so the credit line is
the GitHub owner **iGentAI**. README: pure-Rust Kokoro, no GPU, no ONNX
Runtime in the wasm demo, numeric checks against the Python reference.
Preset voices, not a clone.

**Seam 1, non-clone.** Piper is the non-clone exception until 2027-02-01.
These two are the buried CPU studies to read if that sunset arrives. Do not
copy weights into git. Do not replace Chatterbox. Kokoro weight license on
Hugging Face was not re-fetched.

### Seam 1 rejects

| Repo | Stars | SPDX (fetched) | Credit | Why it stays out |
|---|---:|---|---|---|
| [eugenehp/neutts-rs](https://github.com/eugenehp/neutts-rs) | 4 | none | Eugene Hauptmann (`Cargo.toml` `authors`) | No `LICENSE` file in the tree (license API 404). `Cargo.toml` says `license = "MIT"`. That line is not a grant. Backbone is GGUF via `llama-cpp-4`. Optional `burn`. `espeak` feature. |
| [Dicklesworthstone/franken_tts](https://github.com/Dicklesworthstone/franken_tts) | 47 | NOASSERTION | Copyright (c) 2026 Jeffrey Emanuel | `LICENSE` is MIT **with an OpenAI/Anthropic rider**. GitHub SPDX `NOASSERTION`. The rider is a condition, so this is not MIT. Weights notice: Qwen3-TTS Apache-2.0, artifact `.fttsq` (not GGUF). Pure Rust, CPU, 80 ms frame in the README. Interesting clock, unusable grant. Do not vendor. |
| [jingangdidi/voice_clone](https://github.com/jingangdidi/voice_clone) | 49 | MIT | Copyright (c) 2025 Song | OpenVoice tone-color tool, single executable, no Python. README depends on candle (`candle-core` / `candle-nn`). CPU timing in the README is tens of seconds, not an 80 ms stream. `candle` is a forbidden runtime. |
| [ramishi/moss-tts-nano-rust-candle](https://github.com/ramishi/moss-tts-nano-rust-candle) | 7 | Apache-2.0 | Copyright 2026 OpenMOSS Team, Fudan University, SII and MOSI | 0.1B multilingual clone, no Python. Runtime is candle. Forbidden in-tree. Do not vendor. The size is a capacity note only. |
| [openvalle/valle-tts](https://github.com/openvalle/valle-tts) | 1 | Apache-2.0 | Copyright 2026 Alibaba Cloud | README: Qwen3-TTS 12Hz 0.6B via Q8/Q4 GGUF. It says the linked GGML core is MIT-licensed. Voice locks still forbid GGML containers and the `ggml` runtime. |
| [jmicko/spake](https://github.com/jmicko/spake) | 0 | Apache-2.0 | Copyright 2026 John Micko (README comment; `LICENSE` appendix unfilled) | Desktop clone studio. README: Qwen inference through the `qwen` feature is candle (`qwen-cuda`, `qwen-metal`). Diagnostic mode has no weights and does not speak a clone. |
| [andreisuslov/qwen-tts](https://github.com/andreisuslov/qwen-tts) | 2 | MIT | Copyright (c) 2026 Andrei Suslov | Root `Cargo.toml` is a CLI (clap, serde, ratatui). No inference crate. Not a CPU engine. |
| [sburakyildirim/voice-studio](https://github.com/sburakyildirim/voice-studio) | 1 | MIT | Copyright (c) 2026 Voice Studio contributors | Tauri app: Kokoro ONNX plus Chatterbox. A second Chatterbox UI. ONNX Runtime is not an in-tree dependency we add. |
| [SallahBoussettah/lumi](https://github.com/SallahBoussettah/lumi) | 0 | MIT | Copyright (c) 2026 Salah Eddine Boussettah | See seam 4. README: `whisper-rs` 0.16 with `hipblas` (GPU), Kokoro in a Python sidecar, Silero VAD on ONNX. |

## 2. Real-time audio

`LiveMic` still returns `NoDevice`. The buried capture pieces below are
references for a later device backend. `cpal` itself is the well-known
crate and was outside the 0–50 star search; it is not a buried piece.

| Repo | Stars | SPDX (fetched) | Credit | Call |
|---|---:|---|---|---|
| [Michael-A-Kuykendall/auxide-io](https://github.com/Michael-A-Kuykendall/auxide-io) | 3 | MIT | Copyright (c) 2026 Michael A. Kuykendall | Prefer this shape when a device backend is built. |
| [godie/tardis](https://github.com/godie/tardis) | 0 | MIT | Copyright (c) 2026 godie | Capture idea only. Do not vendor the repo. |
| [Subcue/tauri-plugin-system-audio](https://github.com/Subcue/tauri-plugin-system-audio) | 1 | MIT | Copyright (c) 2026 Subcue AI LLC | Far-end reference shape. Do not vendor. |

### auxide-io

URL: https://github.com/Michael-A-Kuykendall/auxide-io

`LICENSE` is MIT. README: CPAL play/capture, ring buffer, channel map,
linear resampler when the device rate differs, `forbid(unsafe_code)`, and a
callback that does not allocate. That matches an 80 ms `MicSource` later.
Credit Michael A. Kuykendall. Thin adapter in `galion-console` when the
device work is actually scheduled. Do not copy the crate in this change.
The resampler is linear; quality resampling stays a separate choice.

**Locks.** No forbidden runtime in the fetched README. No weight format.

### tardis

URL: https://github.com/godie/tardis

`LICENSE` is MIT. README: `cpal` 0.18 capture and chunking, and a
file transcription provider backed by `faster-whisper`. This inventory did
not open the faster-whisper license. The repository is a transcription app
with a capture CLI attached. Do not vendor it. auxide-io is the capture
piece that stays inside the lock list.

### tauri-plugin-system-audio

URL: https://github.com/Subcue/tauri-plugin-system-audio

`LICENSE` is MIT. README: WASAPI loopback via cpal, WebRTC AEC3, resample to
16 kHz mono, 20 ms frames, mic and loopback kept as separate PCM. Linux and
macOS builds stub the loopback and the AEC. The AEC3 dependency's own
license file was **not** opened. Do not vendor. The useful diagram, for a
later barge-in note, is a far-end reference beside the mic at 16 kHz. Galion's
heartbeat is 80 ms, so 20 ms is an internal frame, not a second clock.

## 3. Noise separation (plan only, V3)

The user-scene layer is later. V3 is the production gate. This section
records candidates so the scene plan does not start from zero. It adds no
crate, no model, and no lock exception.

| Repo | Stars | SPDX (fetched) | Credit | Call |
|---|---:|---|---|---|
| [Allenmylath/Hush-vani](https://github.com/Allenmylath/Hush-vani) | 0 | Apache-2.0 | Allenmylath; weights Weya AI | Best lock fit for a future scene note. Do not build it now. |
| [ahmadMfouad/DeepFilterNet-Infer](https://github.com/ahmadMfouad/DeepFilterNet-Infer) | 0 | MIT OR Apache-2.0 | Copyright (c) 2021 Hendrik Schröter; fork ahmadMfouad | License fits. `tract` does not. |
| [sahenjp/auralis](https://github.com/sahenjp/auralis) | 1 | Apache-2.0 | sahenjp; model Copyright (c) 2026 Xiaobin Rong | Windows shell around an MIT model. Plan note only. |

### hush-vani

URL: https://github.com/Allenmylath/Hush-vani

Repo-root license API returned 404. The grant is `hush-vani/LICENSE`
(Apache-2.0, appendix still the unfilled template) and
`hush-vani/Cargo.toml` `license = "Apache-2.0"`. `hush-vani/NOTICE` says the
embedded weights are Weya AI's `weya-ai/hush` model under Apache-2.0, stored
as int8. README: pure Rust, no ONNX Runtime at inference, 16 kHz mono.
Author's "about 100× realtime on one core" was not measured here. GitHub
login **Allenmylath** is the credit, because the appendix has no name.
Weights credit: **Weya AI**.

**Seam 3, plan only.** If a scene design is written later, this is the
candidate that does not pull `ort` or `tract`. Weights stay outside the
Galion git tree. Do not add the crate in V2.

### DeepFilterNet-Infer

URL: https://github.com/ahmadMfouad/DeepFilterNet-Infer

Fork of [Rikorose/DeepFilterNet](https://github.com/Rikorose/DeepFilterNet)
(upstream 4941 stars on the same day, not buried). This fork has 0 stars.
Fetched `LICENSE`, `LICENSE-MIT`, and `LICENSE-APACHE`. The MIT file is
standard MIT, `Copyright (c) 2021 Hendrik Schröter`, no rider. Dual grant:
`MIT OR Apache-2.0`. GitHub detected SPDX `NOASSERTION` because the grant is
dual. README example is `df::tract::*`. `tract` is a forbidden runtime.
Credit Hendrik Schröter for the library and **ahmadMfouad** for the
inference-only fork. Do not vendor.

### auralis

URL: https://github.com/sahenjp/auralis

`LICENSE` is Apache-2.0 with an unfilled copyright appendix. Credit the
GitHub owner **sahenjp**. README (English portion): Windows 11 real-time
mic denoise, CPU, author-measured inference p95 1.35 ms and mean software
path 89.3 ms, output band limited above 8 kHz, no AEC and no speaker
separation. The weights are not trained by this repo. They are a fixed
[Xiaobin-Rong/ul-unas](https://github.com/Xiaobin-Rong/ul-unas) (219 stars,
so the model is not buried). That model's `LICENSE` was fetched: MIT,
`Copyright (c) 2026 Xiaobin Rong`.

**Seam 3, plan only.** 89 ms mean path is the author's number and sits next
to the 80 ms heartbeat, over it. Do not implement.

### Seam 3 rejects

| Repo | Stars | SPDX (fetched) | Credit | Why it stays out |
|---|---:|---|---|---|
| [shimondoodkin/deepfilter-rt](https://github.com/shimondoodkin/deepfilter-rt) | 10 | none | shimondoodkin | No project license file. Tree hits are font licenses under `docs/static.files/`. README says "MIT / Apache-2.0 (same as DeepFilterNet)". That sentence is not a grant. `Cargo.toml` links `ort` 2.0.0-rc.11 and the `deep_filter` git crate. README: 10 ms frames, 20 ms lookahead, author average 1.70 ms. Useful timing note after a real license exists. |
| [jneem/nnnoiseless](https://github.com/jneem/nnnoiseless) | 370 | BSD-3-Clause | Copyright (c) 2020 Joe Neeman; also Mozilla, Jean-Marc Valin, Xiph.Org Foundation, Mark Borgerding | `COPYING` is BSD-3-Clause only. No dual MIT or Apache grant. 370 stars, so it sits outside the buried range as well. |
| [altunenes/re-use-rs](https://github.com/altunenes/re-use-rs) | 18 | Apache-2.0 (code) | altunenes (`LICENSE` appendix unfilled) | Code grant is Apache-2.0. README weight table: real-time RE-USE weights **non-commercial**; offline RE-USE weights **OpenMDW-1.1**. Those weight terms are outside MIT/Apache. 20 ms frames are a design note only. Do not vendor code or weights. |

## 4. Vector / memory store for session facts

Session facts are consent-gated. `remember` defaults to deny. Nothing here
is wired up. The store, when it exists, is a thin adapter behind that bit,
not an always-on listener.

| Repo | Stars | SPDX (fetched) | Credit | Call |
|---|---:|---|---|---|
| [kikyujin/MindFox](https://github.com/kikyujin/MindFox) | 2 | MIT | Copyright (c) 2026 Mahito KIDA / MULTITAPPS Inc. | Prefer for tiny structured facts. |
| [pigeonflow/clawmem](https://github.com/pigeonflow/clawmem) | 0 | MIT | Copyright (c) 2026 pigeonflow | Prefer when the caller already has a vector. |
| [archit15singh/memori](https://github.com/archit15singh/memori) | 25 | MIT | Copyright (c) 2026 Archit Singh | Prefer for FTS5 + vector hybrid. Leave embeddings off. |
| [oxgeneral/agentmem](https://github.com/oxgeneral/agentmem) | 7 | MIT | Copyright (c) 2025 agentmem contributors | Python sibling. Out of process if ever used. |

### MindFox

URL: https://github.com/kikyujin/MindFox

`LICENSE` is MIT. README: SQLite only, 16-byte factor vectors defined by the
caller, decay, no ONNX model and no cloud. The NPC mood layer is out of
scope. The memory engine is the piece that fits a ≤1% non-inference budget.
Credit Mahito KIDA / MULTITAPPS Inc. Thin adapter later, behind `remember`.
Do not vendor the diplomacy crate.

### clawmem

URL: https://github.com/pigeonflow/clawmem

`LICENSE` is MIT. README: one static binary, one SQLite file, the caller
supplies the embedding, cosine rerank. No model download inside the tool.
Credit **pigeonflow** (that is the copyright line). This is the small store
to copy the *shape* of, if session facts arrive as vectors from a Galion
model that is already allowed. Do not vendor in this change.

### memori

URL: https://github.com/archit15singh/memori

`LICENSE` is MIT, `Copyright (c) 2026 Archit Singh`. Rust core plus a Python
wheel. README: one SQLite file, FTS5, 384-d cosine, reciprocal rank fusion,
decay. `memori-core/Cargo.toml` has `license = "MIT"` and an optional
`embeddings` feature on the `fastembed` crate. This inventory did not open
fastembed's license or confirm its runtime. Leave that feature off so the
adapter does not link an embedding runtime. AllMiniLM's model card was not
re-fetched. Credit Archit Singh. Thin adapter for the schema, not the CLI
and not the dashboard.

### agentmem

URL: https://github.com/oxgeneral/agentmem

`LICENSE` is MIT, `Copyright (c) 2025 agentmem contributors`. Python. README:
one SQLite file, FTS5 plus vectors, a stdlib-only mode and a sqlite-vec mode.
The sqlite-vec license was not re-fetched. Credit the copyright line and the
GitHub owner **oxgeneral**. If a Python sidecar is ever justified, it stays
out of process. The Rust stores above are the closer fit. Do not vendor.

### Seam 4 rejects and near-misses

| Repo | Stars | SPDX (fetched) | Credit | Why it stays out |
|---|---:|---|---|---|
| [Adaimade/R-Mem](https://github.com/Adaimade/R-Mem) | 8 | MIT | Copyright (c) 2024 Ad Huang | SQLite vector + graph, small RSS in the README. Fact extraction calls OpenAI, Anthropic, or Ollama. Ollama weights are the GGUF class the Voice locks refuse. Credit Ad Huang for the study of mem0. Do not vendor into Voice. |
| [rithulkamesh/veclite](https://github.com/rithulkamesh/veclite) | 4 | MIT | Copyright (c) 2024 Rithul Kamesh | Embedded vector database (HNSW, SQL). MIT grant is clean. Heavier than a session-fact file. clawmem and MindFox cover the seam. |
| [SallahBoussettah/lumi](https://github.com/SallahBoussettah/lumi) | 0 | MIT | Copyright (c) 2026 Salah Eddine Boussettah | Local voice plus memory: Tauri, `cpal`, SQLite FTS5, brute-force cosine over f32 blobs. README: GPU (ROCm or CUDA), Whisper Large-v3-Turbo through `whisper-rs` (`hipblas`), Silero ONNX, Kokoro PyTorch sidecar, Ollama. `whisper-rs` is the whisper.cpp binding, which is the GGML container class the Voice locks forbid. The always-on listen also misses consent, which defaults to deny. Credit Salah Eddine Boussettah for the product shape. Do not vendor. |

## 5. Lightweight CPU ASR / VAD / SER

Vosk is the in-tree letter-A path. Silero VAD and Parakeet are the named
research baselines (`docs/research/galion-voice-v2/domains/01-asr-stt.md`,
`domains/03-dialogue-bargein.md`). This seam lists only what those three do
not already cover, plus the rejects that would duplicate them.

| Repo | Stars | SPDX (fetched) | Credit | Call |
|---|---:|---|---|---|
| [edamlabs/e-voice](https://github.com/edamlabs/e-voice) | 2 | MIT | Copyright (c) 2026 Damien Volkov | Pipeline diagram reference. Do not vendor. |
| [cjus/audio-chat-box](https://github.com/cjus/audio-chat-box) | 2 | Apache-2.0 | cjus (`LICENSE-APACHE` has no copyright line) | JSON-line worker shape. Models are the ones we already name. |
| [wavekat/wavekat-vad](https://github.com/wavekat/wavekat-vad) | 28 | Apache-2.0 | Copyright 2026 WaveKat | Earshot backend is the only new VAD note. Do not take WebRTC or TEN. |
| [Fengur/fgvad](https://github.com/Fengur/fgvad) | 1 | MIT | Copyright (c) 2026 Fengur | Endpoint-curve idea. Do not vendor the ten-vad blobs. |
| [EliFuzz/vad-silero-rs](https://github.com/EliFuzz/vad-silero-rs) | 1 | Apache-2.0 | EliFuzz (appendix unfilled) | Second Silero. Do not add it. |

### e-voice

URL: https://github.com/edamlabs/e-voice

`LICENSE` is MIT. README: Linux, CPU, one `libonnxruntime.so` shared with
sherpa-onnx. Nodes include Silero or TEN VAD, Parakeet and other ASR,
emotion2vec SER, and a TTS side whose default is Pocket at **80 ms** frames
with barge-in inside one frame. A `neutts` backend is described as gated and
revenue-capped. That backend is out. `ort` is a forbidden in-tree runtime.
The diagram (denoise, VAD, ASR, SER on one stream, 80 ms TTS) is the seam
map. Credit Damien Volkov. Do not vendor. Do not open an ONNX exception here.
Emotion2vec is the SER the CPU foundation already names; this repo is a
wiring sketch, not a new model.

### audio-chat-box

URL: https://github.com/cjus/audio-chat-box

Fetched `LICENSE-APACHE` (Apache-2.0 terms, no copyright appendix). GitHub
SPDX: `Apache-2.0`. README: a separate process, one JSON line on stdin,
Kokoro plus Parakeet TDT 0.6B v2 plus Silero, through sherpa-onnx. The host
owns the mic and the speaker. That worker shape is the one Piper and the
clone worker already use. The models are the named stack. Credit **cjus**.
Do not vendor, because the process links ONNX Runtime.

### wavekat-vad and fgvad

[wavekat-vad](https://github.com/wavekat/wavekat-vad) — 28 stars.
`LICENSE` is Apache-2.0, `Copyright 2026 WaveKat`. Backends in the README:
WebRTC (default), Silero, TEN-VAD, FireRedVAD, Earshot. The README says the
TEN-VAD ONNX model carries a non-compete and is not standard Apache despite
the label, and it points the WebRTC backend at `kaegi/webrtc-vad`. That
dependency's license file was **not** opened. Earshot is described as pure
Rust, and their CI script checks that an earshot-only build does not link
`ort`. Silero is already our named VAD. Do not add WaveKat now. If a later
VAD study needs a non-Silero CPU backend, read the Earshot sources under
this Apache grant and ignore the other backends.

[fgvad](https://github.com/Fengur/fgvad) — 1 star. `LICENSE` is MIT,
`Copyright (c) 2026 Fengur`. It vendors ten-vad frameworks and adds a dynamic
tail so long dictation is not force-cut. The ten-vad weight terms are the
ones WaveKat flags. Do not copy `vendor/ten-vad`. The curve itself (short
commands versus long speech) is an idea to rewrite on the VAD we already
plan. Credit Fengur for that idea.

### vad-silero-rs

URL: https://github.com/EliFuzz/vad-silero-rs

`LICENSE` is Apache-2.0 with an unfilled appendix. 1 star. It is another
Silero port. Silero is already the named VAD. Do not add a second one.

## Search

GitHub repository search, 2026-11-03, stars `0..50`, license filter `mit` or
`apache-2.0`, language mostly Rust. Queries covered voice cloning, CPU
text-to-speech, speech enhancement, DeepFilter and RNNoise, SQLite vector
memory, voice-activity detection, speech emotion, Kokoro, SenseVoice, and
`cpal` pipelines. Hits whose license **file** was opened are the tables
above. The rest of the search pages were wrappers, GPL, GGML bindings,
cloud memory servers, or duplicate Silero/Parakeet/Whisper apps.

Search metadata only (license file **not** opened, not cleared, not
recommended):

| Search hit | Stars | GitHub badge | Why it was left unread |
|---|---:|---|---|
| random1st/diana-voice | 3 | Apache-2.0 | Whisper on Metal plus Qwen3-TTS. Badge only. |
| jingangdidi/gouzi | 2 | Apache-2.0 | Qwen3-Omni assistant. Badge only. |
| elijahross/Sam---Local-conversational-agent | 1 | Apache-2.0 | Menu-bar companion. Badge only. |
| RustedBytes/extract-speech | 5 | MIT | VAD extract tool. Badge only. |

## What this inventory refuses to do

- Replace Chatterbox Nano, or add Pocket, QORA, Kokoro, or Qwen as the voice.
- Import GGUF, GGML, candle, tract, burn, llama.cpp, or in-tree ONNX Runtime.
- Widen the Piper pin or the Chatterbox pin.
- Copy weights into git.
- Build the scene denoiser. That is a V3 plan, recorded here so the reading
  list exists.
- Store session facts while `remember` is deny.
- Treat a rider, a missing file, or a non-commercial weight license as MIT
  or Apache-2.0.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
