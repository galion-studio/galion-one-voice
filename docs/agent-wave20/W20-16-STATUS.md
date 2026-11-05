# W20-16 — Multilingual research: Grok Voice switching (2026-11-02, CET)

## Changed
- NEW `docs/multilingual/GROK-VOICE-SWITCHING.md`: a public-docs reference for how Grok Voice handles language switching. Every fact is tagged with its source: [S1] https://docs.x.ai/developers/model-capabilities/audio/speech-to-speech, [S2] https://x.ai/news/grok-voice-agent-api. Both were fetched 2026-11-02.
  - Covers the realtime WebSocket API, auto-detect and mid-conversation switching, `language_hint` (BCP-47, live-updatable, unknown codes fall back silently, es/pt need a region), the in-house VAD/tokenizer/audio models, 20+ languages (with the list), <1 s TTFA, and parameter count (not published).
  - §2 lists what xAI has not published (topology, how language ID works, tokenizer and VAD design, model sizes). None of it is guessed. "Native speech-to-speech" is treated as the API name, not an architecture claim.
  - §4 lists Galion design inferences G1–G7 and proposes an EN↔PL switch test set (T1–T6).

## Key finding
- **Polish is NOT in Grok Voice's officially supported list.** It falls in the "additional languages … varying degrees of accuracy" tier, so `language_hint: "pl"` is likely ignored (inference, not tested). Grok gives us no PL baseline, so PL needs its own first-class evaluation in Galion.

## Open
- No live xAI API test was run (no key, and the task was local-only). Behaviour of the T1–T6 test set is unverified.
- Switch latency and code-switch granularity are not published. We need our own metric (G6).

## Blockers
- None for this doc. A live comparison needs an XAI_API_KEY and an owner go-ahead.

No git commit/push.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
