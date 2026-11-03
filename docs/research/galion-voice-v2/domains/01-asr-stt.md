# D01 — ASR / STT

> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This file is a **stub**. The domain brief was not in the upload set. Do not treat this page as the domain research. The one-line finding is copied from the index so the link still resolves.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default.

**Status:** stub. Source domain file absent from the 2026-11-03 upload. Point at the master; do not invent the missing citations.

**One-line finding (index, 2026-11-03):** Streaming latency and partial stability matter more than raw WER. Vosk (letter A) via the `vosk` crate as an out-of-process sidecar is the lowest-risk lock-compliant path; Moonshine and sherpa-onnx Zipformer are B/C candidates.

**Lock note:** Web Speech and Whisper are rejected in the master. B/C engines need owner decision O1 (ONNX only as a separate process, never in-tree). whisper.cpp stays out.

The synthesis that cites this domain is [GALION-VOICE-V2-RESEARCH-BRIEF.md](../master/GALION-VOICE-V2-RESEARCH-BRIEF.md). The index row is [INDEX.md](../master/INDEX.md). Where they disagree with `LOCKS.toml`, the lock file wins.

— Maciej Grajczyk
