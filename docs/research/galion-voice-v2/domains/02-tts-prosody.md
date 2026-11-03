> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 02 — TTS engines & voice quality/prosody (Galion Voice V2)

Constraints: CPU-first, open-source, Rust/local-first, no GGML; LOCKS forbids ort/candle/ggml in-tree. V1 = Piper `en_US-ljspeech-medium` out-of-process + flite fallback.

## Engine comparison
| Engine | Quality | CPU speed | Streaming | Licence | Fit |
|---|---|---|---|---|---|
| **Piper** (VITS, ONNX) | Good, flat prosody | Very fast (RTF ≪1 on Pi 4) | Sentence-chunk; raw PCM via `--output-raw` | Code MIT (rhasspy/piper archived) → **GPL-3** in OHF-Voice/piper1-gpl; depends on espeak-ng (GPL-3) phonemizer | Keep (out-of-process isolates GPL) |
| **Kokoro-82M** (StyleTTS2-derived) | Best naturalness/size on CPU | ~RT to 3-5× RT on modern CPU | Chunked | Weights Apache-2.0; uses misaki/espeak-ng fallback | Best upgrade candidate, out-of-process |
| **StyleTTS2** | Near-human (LJSpeech) | Slower; diffusion step | No native | Code MIT; pretrained-model use terms require disclosure | Research only |
| **Coqui XTTS-v2** | Excellent, cloning, emotion via ref | Too slow on CPU (GPU wanted) | Yes (~200 ms GPU) | **CPML non-commercial**; Coqui company shut down | Reject |
| **espeak-ng** | Robotic | Instant | Yes | **GPL-3** | Phonemizer only, out-of-process |
| **flite** | Robotic | Instant, tiny | Yes | BSD-like (CMU) | Keep as last-resort fallback |

Sources:
- Piper (archived): https://github.com/rhasspy/piper ; successor GPL: https://github.com/OHF-Voice/piper1-gpl
- Piper voices/samples: https://huggingface.co/rhasspy/piper-voices , https://rhasspy.github.io/piper-samples/
- Kokoro: https://huggingface.co/hexgrad/Kokoro-82M , https://github.com/hexgrad/kokoro ; ONNX build: https://github.com/thewh1teagle/kokoro-onnx
- StyleTTS2: https://github.com/yl4579/StyleTTS2 (paper https://arxiv.org/abs/2306.07691)
- XTTS-v2 + CPML: https://huggingface.co/coqui/XTTS-v2 , https://coqui.ai/cpml
- espeak-ng (GPL-3): https://github.com/espeak-ng/espeak-ng
- flite: https://github.com/festvox/flite
- TTS Arena (quality ranking): https://huggingface.co/spaces/TTS-AGI/TTS-Arena

## Streaming latency & warm process
- Biggest win is **time-to-first-audio (TTFA)**, not RTF: split LLM tokens at clause/sentence boundaries, synth first clause immediately, pipeline the rest.
- **Warm process**: keep one Piper (or Kokoro) worker resident reading JSON lines on stdin (`piper --json-input --output-raw`) — avoids ~0.3–1 s model load per utterance. Pre-warm with a dummy sentence at boot.
- Target budget: TTFA ≤150 ms for first clause on medium Piper on x86; stream 16/22.05 kHz PCM frames (20 ms) to playback; barge-in = kill/flush the queue, not the process.
- Piper medium vs low: low ≈2× faster, audibly worse; keep medium for EN.

## Prosody / emotion hooks
- Piper: `length_scale` (rate), `noise_scale` (expressivity), `noise_w` (rhythm variance), `sentence_silence`; multi-speaker IDs in some voices. No SSML. Map dialog state → these params (e.g. question → slightly slower, excited → noise_scale↑).
- Kokoro: voice style vectors (blendable), speed; richer natural intonation.
- Text-level hooks (engine-agnostic): punctuation normalisation, number/date expansion, comma insertion for pauses, short filler/backchannel clips pre-rendered and cached.
- Full emotion control (StyleTTS2 ref style, XTTS) costs too much CPU or licence.

## Keep vs replace in V1
- **Keep**: Piper out-of-process (MIT/GPL isolation satisfies LOCKS: no ort in-tree); flite fallback; ljspeech-medium as default.
- **Fix**: make the Piper worker persistent + streaming raw PCM; clause chunker; prosody param mapping; phrase cache.
- **Add (optional, V2.x)**: Kokoro-82M as a second out-of-process "HQ voice" sidecar (Python/ONNX in its own process) behind the same IPC trait; A/B via MOS.
- **Replace/Reject**: XTTS (non-commercial), StyleTTS2 in prod, espeak-ng as voice.

## Licence risks
- espeak-ng **GPL-3**: linking it into Galion's binary would GPL the binary. Piper links it → run Piper as separate process (mere aggregation) and ship its source/notice. piper1-gpl is GPL-3 itself — same rule.
- ljspeech dataset is public domain; check each Piper voice MODEL_CARD (some voices are CC-BY-NC/research-only).
- XTTS CPML forbids commercial use. Kokoro Apache-2.0 safe, but its phonemizer fallback may pull espeak-ng → also out-of-process.

## Galion-specific recommendations
1. Implement a `TtsBackend` trait in Rust with IPC (stdin JSON / stdout PCM) — Piper primary, flite fallback, Kokoro optional; no ort/candle/ggml in-tree.
2. Persistent pre-warmed Piper worker + clause-level streaming; measure TTFA p50/p95 in CI.
3. Prosody mapper from dialogue act → length/noise params; cache common phrases ("Galion Voice here", acks).
4. Licence file listing GPL-3 components shipped as separate executables with source offer.
5. Evaluate Kokoro-82M on the target CPU (RTF, TTFA, MOS vs Piper) before adopting as HQ voice.

— Maciej Grajczyk
