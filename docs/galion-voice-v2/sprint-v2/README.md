# Galion Voice V2 sprint: results and notes (2026-11-03 .. 11-05)

Copied from the sprint working directory (results, reports, research). Text artefacts only, except the
listening set (`../evals/listening-set/`, 82 Piper WAVs, about 15 MB: persona emotion sweep and premium
Piper voice candidates). No model weights, venvs or third-party sources are committed.

- `SPRINT-REPORT.md` — sprint report.
- `bakeoff-t648/` — clone bake-off report (`REPORT.md`), scoring tools (`tools/`, eval-only, not shipped),
  run logs and per-config score JSON. Winner for the separate cloning worker: Chatterbox Turbo, fp32 ONNX
  (`out/C-fp32`); the quantized model fails to load on the bundled ONNX Runtime.
- `latency-240/` — latency lane notes and summaries (wt-14: eos -> first audio 178.9/221.9 ms p50/p95, canned, n=24).
- `research/` — P95 tail research, symphony stack plan, TTS alternatives.
- `results/` — summaries from call, hard-loop, clone, emotion and Samantha investigation runs.
