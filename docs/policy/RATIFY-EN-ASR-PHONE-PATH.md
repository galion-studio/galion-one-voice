# DRAFT · Ratification · English ASR on the phone path (CPU, out of process)

**Status: letter A chosen by Maciej Grajczyk, 2026-11-03 (Europe/Zagreb).**
Vosk, out of process, small-en (`vosk-model-small-en-us-0.15`), grammar mode
for intents. `interim` stays until the WER bar. The Vosk exception is **not**
in `LOCKS.toml`. The only lock text added with this decision is the A21 Piper
sunset pin. Prepared as a draft 2026-11-02. Companion note `RATIFY-A21-ONNX-EXCEPTION.md`
was named and is not in this tree; the Piper pin lives in `LOCKS.toml`.

## Facts
- `LOCKS.toml [formats]`: `ggml_container_allowed = false` ("zero llama.cpp/GGML-family weight containers anywhere, cascade lane included"), `foreign_runtime_allowed = false`, `forbidden_runtimes = ["candle","ort","tract","burn","llama","ggml"]`, `weight_formats = ["safetensors"]`.
- The EN cascade lane (`docs/cascade/EN-CASCADE-LANE.md`, W21-03) has an `Asr` slot (Whisper/Parakeet class). Today it is `StubAsr`. Real ASR is blocked on licence/format clearance (W21-03 status, item 4).
- The phone path means CPU only, 16 kHz mono, English only (`EnglishOnlyRouter`). It is the fallback lane and never the 80 ms primary path.

## Candidates
| Option | Runtime / format | Licence (code / model) | Lock conflict |
|---|---|---|---|
| whisper.cpp (tiny.en / base.en) | ggml, `.bin` GGML-family weight container | MIT / MIT | **Hard.** It breaks `ggml_container_allowed=false` and `forbidden_runtimes` "ggml". Even out of process, the GGML container ban covers "anywhere". The in-repo gate also rejects the four-letter name of that container, so this row does not spell it. |
| Vosk (`vosk-model-small-en-us-0.15`, ~40 MB) | Kaldi, own model dir (not ggml/onnx) | Apache-2.0 / Apache-2.0 | Soft. It is a foreign runtime only. Kaldi is not in `forbidden_runtimes`. |
| sherpa-onnx / Parakeet / Moonshine `.onnx` | onnxruntime | Apache-2.0 / varies (CC-BY-4.0, MIT) | Same as A21 (`ort` forbidden in-tree; out of process only). |
| Whisper `.safetensors` (HF openai/whisper-tiny.en) on galion-tensor | native | MIT | **None.** But no native decoder exists yet (work item, not a dependency). |

## Options (pick one)
**A · Scoped exception for Vosk, out of process (recommended).** Keep every global lock. Add a sunset-dated exception mirroring A21:
```toml
# proposed — NOT applied
[formats.exceptions.en_asr_phone]
artifact = "weights/asr/vosk-model-small-en-us-0.15/"   # pin sha256 of the zip on download
sha256 = "<TO PIN>"
runtime = "vosk / Kaldi (Apache-2.0), separate process over stdin/stdout PCM16, not linked"
lane = "EN cascade ASR, phone path only"   # never primary path, never LLM/TTS
sunset = "Whisper-tiny.en .safetensors on galion-tensor PASS"
decision = "<owner id>"
sources = ["LEDGER <date> EN-ASR"]
```
Why: no GGML, no ONNX, a permissive licence, it fits CPU real time on phone-class hardware, and its streaming partial results suit the barge-in gates. Accuracy is lower than Whisper (WER is higher on open speech). That is acceptable for a fallback lane. `check-locks.sh` should accept exactly this pin, and CI should fail on any other ASR artifact.

**B · Extend the A21 ONNX exception to ASR (sherpa-onnx + Parakeet/Moonshine).** It reuses one already-argued exception class (out-of-process onnxruntime) with better accuracy than Vosk. It widens A21's "never ASR" scope and adds a second ONNX pin. Choose B only if A21 is ratified as A and Vosk WER is unacceptable in testing.

**C · No foreign ASR. Hold the stub until native Whisper-tiny.en `.safetensors` runs on galion-tensor.** Zero lock change, but the phone demo has no real speech input in the meantime.

**Explicitly rejected: whisper.cpp.** It needs `ggml` to be lifted globally, which reverses Q-RESEARCH.Q12 / Q-BUILD.Q7 for every lane. It is not offered as an option.

## Checklist to ratify
- [x] Owner picks A, B or C (reply with letter). **A**, 2026-11-03. Signature below.
- [ ] Download and pin the Vosk sha256, then a later owner edit may add a sunset exception. Not done. Do not widen the Piper pin to cover it. `scripts/check-locks.sh` and `check-no-forbidden.sh` stay the gates.
- [ ] Licence/NOTICE entry for the chosen runtime and model (Apache-2.0 attribution).
- [ ] Gate check on the phone target: ASR partials do not push `onset_p50_max_ms = 300` / `barge_in_max_ms = 200` over budget.

Owner: Maciej Grajczyk — letter A, 2026-11-03 (Europe/Zagreb). Vosk LOCKS text not applied.
