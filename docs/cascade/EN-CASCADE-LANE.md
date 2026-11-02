# EN cascade lane — design stub (W20-10, sector 11)
Fallback lane, never default (feature `cascade`). Zero legacy container formats or third-party runtimes (G2/X3); .gq only. No weights in repo.

```
mic PCM 16k ─┬─> Asr  (Whisper-class / Parakeet-class)  ─> Transcript{en}
             └─> EmotionIn (SenseVoice-Small reference, advisory) ─> Emotion
Transcript + Emotion ─> Llm (small LLM, .gq-only native runtime) ─> reply text
reply ─> Tts (Piper / SpeechT5 / Pocket-TTS-class) ─> PCM out
```
Budgets (A18): onset p50 ≤ 800 ms, barge-in ≤ 200 ms. Rika-in-cascade: NO.
Code: `crates/galion-cascade` — traits `Asr`, `EmotionIn`, `Llm`, `Tts`; mocks; `Cascade::turn`.
Language: `Lang::En` only; hr/pl/others rejected until model picks + A21 licence rows.

## Candidate slots (to verify in A21 table — not downloaded)
| Slot | Candidates | Licence to verify |
|---|---|---|
| ASR | Parakeet-TDT (NVIDIA), Whisper-class | CC-BY-4.0 / MIT |
| Emotion | SenseVoice-Small | model licence (FunASR) — check |
| LLM | small instruct model via galion-tensor / ONNX | per model |
| TTS | Piper, SpeechT5, Pocket-TTS-class | MIT/GPL voices vary; MIT; check |
Runtime: `galion-tensor`; sherpa-onnx reference only (K16).
