# docs/multilingual — Galion Voice multilingual design

Local-only design docs (Wave 20, 2026-11-02). **The prototype is English-only.** Priority after English: **Polish**, then Croatian.

| File | Owner | What it is |
|---|---|---|
| [MULTILINGUAL-VOICE-PLAN.md](MULTILINGUAL-VOICE-PLAN.md) | W20-18 | **Start here.** Assembled plan: Grok-style switching contract, small-model approach (CPU cascade now, student language tokens later), tiers, V4 integration, phased backlog with estimates |
| [GROK-VOICE-SWITCHING.md](GROK-VOICE-SWITCHING.md) | W20-16 | Public-docs reference for how Grok Voice handles language switching (sourced [S1]/[S2]); EN↔PL switch test set T1–T6 |
| [TOP-20-LANGUAGES.md](TOP-20-LANGUAGES.md) | W20-17 | Top ~20 spoken languages with ISO/BCP-47 codes and tiers (T0 en, T1 pl+hr, T2 rest) |

Related code (EN-only today):
- `crates/galion-ml-voice` — `LanguageId` (EN only), `<|en|>` token helpers, ASR/TTS/router stub traits (W20-19).
- `crates/galion-cascade` — cascade lane stubs: ASR / LLM / TTS / inbound emotion slots (W20-10).

Rules: no weights downloaded or vendored; no foreign runtimes or legacy container formats (see `LOCKS.toml` `[formats]`, gate G2); no new `LanguageId` variants without owner sign-off; he-IL out of scope. Model/language-coverage claims tagged `[verify]` need checking against model cards and the A21 licence memo.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
