> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 5: Open-source stack, adopt or replace vs V1

Locks: no GGML (whisper.cpp/llama.cpp are out), no ort or candle in-tree. Prefer Apache-2.0 or MIT. Letter A ASR = Vosk (see domain 01).
Out-of-process runtimes (a separate service/binary) are allowed only if the parent confirms the lock is scoped to "in-tree". Every such case is flagged ⚠.

## Keep / replace / trial

| V1 component | Decision | Candidate | License | Rationale | Source |
|---|---|---|---|---|---|
| PhoneLane interim ASR | **Replace** | Vosk (Kaldi) streaming, small en model | Apache-2.0 | Partial results, CPU, small models (~50 MB), no GGML/ort. Fits Letter A. | https://github.com/alphacep/vosk-api , https://alphacephei.com/vosk/models |
| (ASR alt) | Trial only ⚠ | sherpa-onnx streaming Zipformer | Apache-2.0 | Better WER than Vosk small, but uses onnxruntime, so it only works as an out-of-tree sidecar | https://github.com/k2-fsa/sherpa-onnx |
| (ASR alt) | Reject | whisper.cpp / faster-whisper | MIT | whisper.cpp is GGML. faster-whisper needs CTranslate2/Python and isn't streaming-native | https://github.com/ggerganov/whisper.cpp |
| VAD (new) | **Adopt** | WebRTC VAD (libfvad) | BSD-3 | Tiny C library, no ML runtime, gates ASR and handles barge-in | https://github.com/dpirch/libfvad |
| (VAD alt) | Trial ⚠ | Silero VAD | MIT | More accurate, but needs ONNX/PyTorch | https://github.com/snakers4/silero-vad |
| CannedLlm | **Keep as fallback, add a trial** | Keep canned/intent router. Trial an external OpenAI-compatible HTTP endpoint (e.g. vLLM, Apache-2.0) behind a trait | Apache-2.0 | vLLM runs out-of-process, so nothing goes in-tree. Canned stays as the offline/deterministic path | https://github.com/vllm-project/vllm |
| Piper (voice A21) | **Keep** (watch) | Piper | MIT (rhasspy/piper; check voice model licenses per voice) | Fast CPU VITS. Note: Piper uses onnxruntime internally. If that counts against the ort lock, keep Piper only as an external process (the current V1 shape, presumably). Upstream moved to OHF-Voice/piper1-gpl (**GPL-3.0**), so pin the MIT rhasspy release | https://github.com/rhasspy/piper , https://github.com/OHF-Voice/piper1-gpl |
| (TTS alt) | Trial ⚠ | Kokoro-82M | Apache-2.0 (weights) | Higher quality at a small size. Needs an ONNX/PyTorch sidecar | https://huggingface.co/hexgrad/Kokoro-82M |
| flite | **Keep as last-resort fallback** | flite | BSD-style | Zero-dependency, instant output, robotic. Good as an "always speaks" floor | https://github.com/festvox/flite |
| (phonemizer) | Watch | espeak-ng (Piper dependency) | **GPL-3.0** | License risk. Keep it process-isolated. Do not link it in-tree | https://github.com/espeak-ng/espeak-ng |
| JS spring orb | **Keep** | Optionally drive it from audio RMS/viseme events | n/a (in-house) | No OSS replacement needed. Wire amplitude from the TTS stream | — |
| galion-render CPU | **Keep, trial GPU path** | wgpu (Rust) for an optional GPU backend | MIT/Apache-2.0 | Keep CPU as the baseline. Use wgpu behind a feature flag if render cost shows in latency budgets | https://github.com/gfx-rs/wgpu |
| Audio I/O / resample (infra) | Adopt if missing | cpal + rubato (Rust) or speexdsp | Apache/MIT, BSD | Clean 16 kHz pipeline. speexdsp also provides AEC for barge-in | https://github.com/RustAudio/cpal , https://github.com/HEnquist/rubato , https://github.com/xiph/speexdsp |

## Open questions for the parent
1. Does "no in-tree ort" allow ORT inside an external sidecar process? This decides Piper, Kokoro, sherpa-onnx and Silero.
2. Audit Piper voice A21's model card license. Some voices are CC-BY-NC.
3. espeak-ng is GPL and stays process-isolated.

Note: these citations are canonical repo URLs written from domain knowledge. They were not re-fetched this run, so verify license files before locking.

— Maciej Grajczyk
