# Grok Voice: how language switching works (public-docs reference)

**Owner:** W20-16 (multilingual research) · **Date:** 2026-11-02 · **Scope:** research note, local only

**Sources (both read 2026-11-02):**
- [S1] xAI docs: Speech to Speech: https://docs.x.ai/developers/model-capabilities/audio/speech-to-speech
- [S2] xAI news: Grok Voice Agent API: https://x.ai/news/grok-voice-agent-api

> **Rule for this doc:** every claim about Grok Voice is tagged [S1] or [S2]. Any claim without a tag is a Galion design inference, not an xAI fact. xAI has not published model internals: no architecture diagram, no parameter count, no training-data mix, and no description of how the language-ID mechanism works. This doc does not guess at any of them.

---

## 1. What xAI has published

| Topic | Published fact | Src |
|---|---|---|
| Product shape | Realtime voice API that streams audio and text both ways over WebSocket: `wss://api.x.ai/v1/realtime?model=…` | S1 |
| Other transports | WebRTC and telephony tester apps; SIP, WebSocket, or LiveKit; native G.711 μ-law/A-law | S1 |
| Models | `grok-voice-latest` → `grok-voice-3.0` (flagship); `grok-voice-2.0` (deprecated). Same endpoint, parameters, and events | S1 |
| In-house stack | "We built the entire voice stack in-house, training our own voice activity detection (VAD), tokenizer, and audio models from scratch." | S2 |
| Shared with consumer | "built on the same stack that powers Grok Voice for millions in our mobile apps and Tesla vehicles" | S2 |
| Latency | "average time-to-first-audio of less than 1 second" | S2 |
| Auto language | "The model automatically detects the input language and responds naturally in the same language — no configuration required." | S1 |
| Mid-conversation switch | "trained to automatically respond in the language spoken by the user and can seamlessly switch languages mid-conversation" | S2 |
| Code-switching | "seamless code-switching between languages in the same conversation" | S1 |
| Forced language | The system prompt can pin a response language ("always respond in a specific language via system prompt") | S1, S2 |
| Per-turn override | `response.create.response.instructions` overrides the session prompt for a single response (example: "Respond in Spanish for this turn only.") | S1 |
| ASR bias | `audio.input.transcription.language_hint` takes a BCP-47 code, biases ASR toward that language, and **can be updated mid-session** | S1 |
| Hint rules | Spanish and Portuguese require a region (`es-MX`, `es-ES`, `pt-BR`, `pt-PT`). Bare `es`/`pt` are rejected. **Unrecognized codes are silently ignored and fall back to auto-detection** | S1 |
| Keyterms | `audio.input.transcription.keyterms`: up to 100 terms of ≤50 chars each, updatable mid-session | S1 |
| Turn-taking | `turn_detection.type: "server_vad"`: the server detects end of speech, and barge-in is recommended | S1 |
| Pronunciation | `replace` map, applied "in the model's output … before text-to-speech". Only the spoken audio changes, not the transcript | S1 |
| Audio I/O | PCM16 at 8k–48k (24 kHz default), μ-law, A-law, Opus 24k. JSON/base64 or binary frames | S1 |
| Reasoning | `reasoning.effort` defaults to `"high"`. `"none"` disables it | S1 |
| Language count | "20+ languages"; "dozens of languages" | S1, S2 |
| Parameter count | **Not published.** | — |

### 1.1 Officially listed languages [S1]
`en`, `ar-EG`, `ar-SA`, `ar-AE`, `bn`, `zh`, `fr`, `de`, `hi`, `id`, `it`, `ja`, `ko`, `pt-BR`, `pt-PT`, `ru`, `es-MX`, `es-ES`, `tr`, `vi`.

S1 adds that the model "is also capable of conversing in additional languages beyond those listed above, with varying degrees of accuracy."

> ⚠️ **Polish (`pl`) is not on the list.** Galion's priority pair is EN + PL. On Grok Voice, Polish falls into the "varying degrees of accuracy" tier. Because unknown hint codes are silently ignored, `language_hint: "pl"` probably has no effect (inference from the S1 rule; we have not tested it). Do not use Grok Voice as evidence of Polish quality.

---

## 2. What xAI has NOT published (do not fill these in)

- Whether a single end-to-end model produces the audio, or an ASR → LLM → TTS cascade does. S1 mentions transcription (`language_hint`, `grok-transcribe`) and a step "before text-to-speech" (`replace`). That shows text exists somewhere in the pipeline. It does not show the topology. **Treat the "native speech-to-speech" label as the product/API name, not as an architecture claim.**
- How language ID works: a separate classifier, implicit in the model, frame-level or utterance-level. Nothing is published.
- Switch latency, or the minimum audio needed before a switch.
- Tokenizer design (codec type, frame rate, codebooks) and the VAD model's design. S2 says only that both were trained in-house from scratch.
- Parameter counts, model sizes, training data, per-language data balance.
- Per-language WER/MOS numbers. S2 reports only blind win rates against the OpenAI Realtime API, with no per-language table in the text.

---

## 3. The behaviour contract, from the docs

From [S1]/[S2], a client can expect the following:

1. **Default (no config):** the reply language follows the user's spoken language, turn by turn. A switch mid-conversation needs no client action.
2. **Within-utterance code-switching:** advertised as "seamless" [S1]. Granularity is not specified.
3. **Pinning:** the session `instructions` can fix the output language. `response.instructions` can fix it for a single turn.
4. **ASR steering:** `language_hint` biases *input transcription* only, and can be changed live with `session.update`. Docs do not say it changes the *reply* language.
5. **Failure mode:** an invalid or unknown hint does not raise an error. The session falls back to auto-detect.

---

## 4. Implications for Galion Voice (design inferences, not xAI facts)

These are our choices, informed by the contract above. They do not describe Grok internals.

| # | Galion decision | Why |
|---|---|---|
| G1 | Copy the **API contract**: auto-detect by default, with a live-updatable `language_hint` (BCP-47) and a prompt- or turn-level pin | Users already know this pattern, and it is OpenAI-Realtime-compatible [S1] |
| G2 | Hint semantics: **bias, never hard-lock**. Unknown codes fall back to auto. Region is required wherever our voices differ by region | Mirrors S1. Avoids hard failures |
| G3 | Do **not** pick an architecture by claiming parity with Grok. Choose cascade vs. end-to-end on our own benchmarks | Grok's topology is unpublished (§2) |
| G4 | Owning VAD and the tokenizer in-house is a sound goal. Galion's Mimi-based tokenizer plus our own VAD line up with S2's "own the whole stack" principle | S2 states the principle, not the design |
| G5 | Polish gets **first-class** evaluation (WER, MOS, switch tests). Grok does not list it, so we have no baseline from them | §1.1 |
| G6 | Need our own switch-latency metric: time from the first L2 phoneme to the first L2 audio out. Target it against the overall <1 s TTFA bar | S2 gives only overall TTFA |
| G7 | `keyterms`-style bias list plus `replace`-style pronunciation map, per language | Useful in practice for proper nouns across languages [S1] |

### 4.1 Proposed EN↔PL switch test set (prototype, English-only today)
- T1: full turn in EN, then a full turn in PL, with no hint. Expect a PL reply.
- T2: EN sentence with a Polish proper noun (place name). The reply should stay EN, with the noun pronounced correctly.
- T3: intra-sentence code-switch (EN→PL halfway through).
- T4: hint=`en` but user speaks PL. Measure whether the hint hurts PL ASR.
- T5: hint set to an unknown code. Expect a silent fallback to auto.
- T6: pin "always respond in English" with PL input. Expect an EN reply.
- When the multilingual path exists, run these against Galion. Optionally run them against Grok as an external reference, but **only the 20 listed languages count as supported baselines**. PL results on Grok are informational.

---

## 5. Minimal session example (from S1 fields only)

```json
{
  "type": "session.update",
  "session": {
    "voice": "eve",
    "instructions": "You are a helpful assistant. Reply in the user's language.",
    "turn_detection": { "type": "server_vad" },
    "audio": {
      "input":  { "format": { "type": "audio/pcm", "rate": 24000 },
                  "transcription": { "language_hint": "de", "keyterms": ["Galion"] } },
      "output": { "format": { "type": "audio/pcm", "rate": 24000 } }
    }
  }
}
```
Live switch of the ASR bias (mid-session) [S1]:
```json
{ "type": "session.update", "session": { "audio": { "input": { "transcription": { "language_hint": "ja" } } } } }
```
Turn-only language pin [S1]:
```json
{ "type": "response.create", "response": { "instructions": "Respond in Spanish for this turn only." } }
```

No live API calls were made for this note. It is based on documentation only.
