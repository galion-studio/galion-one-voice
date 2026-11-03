# Galion Voice V2 research

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. **Research synthesis dated 2026-11-03.** Docs only.

This tree is the desk synthesis Maciej Grajczyk authorized to land. It is not
the V2 program plan (`docs/galion-voice-v2/GALION-VOICE-V2.md`) and it is not
the loopback harness (`docs/eval/PRESENCE-EVAL.md`).

**LOCKS.toml overrides any conflicting domain text.**

- No whisper.cpp. No GGML-family containers.
- No in-tree ORT. `formats.foreign_runtime_allowed=false`. `forbidden_runtimes` includes `ort`.
- `paid_tiers=false`. Hosting notes in the synthesis are not product prices.
- `chrome_allowed=false`. Consent deny-default.

Nothing here edits `LOCKS.toml`. Nothing here is product code. Open decisions
in the master (O1–O12) are not ratified by landing these pages.

Where this tree and `LEDGER.md` disagree about what has already landed, the
ledger and the code win. Percentages and swarm status are the 2026-11-03
snapshot.

## Read this

| Doc | What it is |
|---|---|
| [master/GALION-VOICE-V2-RESEARCH-BRIEF.md](master/GALION-VOICE-V2-RESEARCH-BRIEF.md) | Master synthesis. Locks in §0 override domain text. |
| [master/INDEX.md](master/INDEX.md) | One row per domain, with the lock note. |
| [eval/EVALUATION-V2-2026-11-03.md](eval/EVALUATION-V2-2026-11-03.md) | Sector percentages. Overall V2 program 36% in that snapshot. |
| [eval/SELLER-EVAL-V5-2026-11-03.md](eval/SELLER-EVAL-V5-2026-11-03.md) | Stakeholder snapshot the same day (~40% after the orb PR). |

## Domain briefs

`domains/` holds the ten source briefs. Each page keeps the LOCKS precedence
banner. Citations are only those written in the source. None were added on
landing. On domain 10, any whisper.cpp or faster-whisper recommendation is void.

| # | Brief |
|---|---|
| 01 | [domains/01-asr-stt.md](domains/01-asr-stt.md) |
| 02 | [domains/02-tts-prosody.md](domains/02-tts-prosody.md) |
| 03 | [domains/03-dialogue-bargein.md](domains/03-dialogue-bargein.md) |
| 04 | [domains/04-latency-streaming.md](domains/04-latency-streaming.md) |
| 05 | [domains/05-oss-vs-v1.md](domains/05-oss-vs-v1.md) |
| 06 | [domains/06-naturalness.md](domains/06-naturalness.md) |
| 07 | [domains/07-security-privacy.md](domains/07-security-privacy.md) |
| 08 | [domains/08-evaluation.md](domains/08-evaluation.md) |
| 09 | [domains/09-competitive.md](domains/09-competitive.md) |
| 10 | [domains/10-hardware-edge.md](domains/10-hardware-edge.md) |

## Provenance

The local paths named inside the briefs were:

- `/workspace/galion-voice-research/master/GALION-VOICE-V2-RESEARCH-BRIEF.md`
- `/workspace/galion-voice-research/eval/EVALUATION-V2-2026-11-03.md`
- `/workspace/galion-voice-research/eval/SELLER-EVAL-V5-2026-11-03.md`
- `/workspace/galion-voice-research/domains/` (landed as `domains/01`–`10`)

Layout here keeps `master/`, `eval/`, and `domains/` so the brief's
`../domains/` and `../eval/` links still resolve. The all-nighter runbook was
not part of this landing.

— Maciej Grajczyk
