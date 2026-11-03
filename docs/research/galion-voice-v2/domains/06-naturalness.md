> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 06 — Naturalness: backchanneling, pacing, emotion

Status: research brief for Galion Voice V2. No product edits. Where this says "Galion", it means the existing constraints: emotion consent is deny-by-default, Rika lean-in, CannedLlm, counterbalancing emotion design, chrome_allowed=false.

## 1. What the research says

### 1.1 Timing does more for naturalness than voice quality
- Across 10 languages, the most common gap between turns is 0–200 ms. The cross-language mean is +208 ms. Languages differ by about ±250 ms, which is a calibration difference, not a different system. Slower answers go with dispreferred or "no" responses. [S1]
- So a 1–2 s silence before every reply sounds robotic, or like disagreement, no matter how good the TTS is. The gap target from domain 03 (≤800 ms p50) is already 4× the human mean. Speed matters more here than extra prosody work.
- Voice Activity Projection (VAP) predicts the joint voice activity of both speakers over the next 2 s, in bins of 200, 400, 600 and 800 ms. It tells turn shifts apart from backchannels without labeled data [S2, S3]. A fine-tuned VAP predicts *when* a backchannel should happen and *which kind* ("yeah", "un", "oh"), frame by frame and in real time [S4]. Multimodal VAP adds text and facial input and improves both end-of-turn and backchannel prediction. Because the model sees the system's own voice activity, it avoids repeating a backchannel. [S5]

### 1.2 Backchannels
- Definition used in the literature: a listener sound under 1 s, isolated by about 1 s of silence before it and 2 s after, that follows the other speaker's speech [S2]. It shows attention without taking the turn [S4].
- What this means for the design: (a) if the agent hears a user backchannel, it should *not* stop talking; (b) the agent producing backchannels is optional and risky. If the timing is wrong, it sounds like an interruption, and a fixed-interval "mhm" sounds mechanical.

### 1.3 Disfluency and pacing
- Filled pauses ("um", "uh") put in the *right* places raise perceived naturalness. Put in random places, they hurt it. [S6, S7]
- Disfluency inserted by an LLM raises perceived spontaneity but slightly lowers intelligibility [S8].
- Fillers, especially in the middle of an utterance, plus slow speech and high f0 lower perceived *confidence*. Fast speech, low f0 and no filler sound most confident. The effects add up. [S9]
- Fillers also change perceived personality: a fluent voice reads as more conscientious and less neurotic [S10].
- Takeaway: disfluency is a dial, not a free win. Use none when stating facts, numbers or safety information. A few, at the *start* of an utterance only, are fine for thinking-aloud or social chat.

### 1.4 Emotion: expressing vs. inferring
- CASA: people apply social rules to computers. Matching the agent's emotional tone to the user's state can help (Nass et al. 2005, the car-voice study cited in [S11]). Emotional TTS has been feasible since Cahn's parametric work [S12]. Mismatched modalities (voice vs. content) hurt processing [S11, S12].
- The *uncanny valley* for prosody was **not** replicated. Ratings for eeriness, anthropomorphism and trust moved linearly with roboticness, with no valley, and eeriness mediated trust. [S13] So more human-like prosody is not automatically creepy. Overclaiming feelings is a separate risk.
- *Inferring* emotion is weakly grounded science. Expressions vary across cultures, situations and individuals, and lack reliability and specificity [S14]. The EU AI Act cites the same problems (Recital 44) and bans emotion inference from biometric data, which includes voice, in workplace and education settings, except for medical or safety purposes (Art. 5(1)(f)) [S15, S16]. A raised voice or whispering on its own, or "readily apparent" detection that draws no inference about emotion, is outside the definition [S16, S17].

## 2. Galion recommendations

1. **Gap first.** Target a human-band median gap (≤500 ms p50 stretch, keep ≤800 ms as the gate from 03). When the LLM is slow, play a *short, consent-neutral* acknowledgement from **CannedLlm** (e.g. "Okay—", "Let me check.") at ~300 ms, so the agent never sits in a 1 s+ silence. Canned acks must contain no emotion words and make no affect claims.
2. **Inbound backchannels never take the turn.** Reuse the domain-03 ladder (<150 ms ignore, 150–400 ms duck, >400 ms or ≥2 content words yield). Add a lexical allow-list ("mhm", "yeah", "okay", "right", "uh-huh", plus local-language equivalents) that keeps classifying a sound as a backchannel up to 1 s, per the [S2] definition.
3. **Outbound backchannels are off by default (V2.0).** Later behind a flag: VAP-based timing [S4] with a rate cap (≤1 per 8 s, never during the user's first 2 s, never repeated back to back), neutral types only ("mhm", "okay"). Evaluate against no backchannels in the harness before shipping.
4. **Rika lean-in = pacing, not emotion.** Make lean-in prosodic and structural only: slightly faster turn pickup, a shorter first sentence, and a follow-up question. That raises engagement *signals* without inferring the user's feelings. Lean-in must not escalate warmth over a session (counterbalancing).
5. **Emotion consent deny-by-default, made concrete:**
   - Default: no affect inference from voice or face. Allowed without consent: physical/acoustic signals only (loudness, speech rate, overlap, silence), used for *turn/volume control*, never labeled as emotion. This matches the Art. 5 "readily apparent / physical state" carve-out [S16].
   - With explicit opt-in: coarse inference (e.g. "seems frustrated → offer to slow down / hand off"), shown to the user, revocable, never logged as an emotion label tied to identity, never used in workplace or education deployments (hard block by deployment type).
   - Treat every inference as uncertain [S14]: the agent's reply must still make sense if the inference is wrong. Use "Want me to slow down?", never "You sound upset."
6. **Counterbalancing emotion design.** Expressed affect is mirrored *at most one step* toward the user's tone and then pulled back toward neutral. No sustained mirroring, no escalation, and no "I feel" claims. Confident delivery (no filler, steady rate) for facts and safety information [S9]. Warmer prosody is allowed for greetings and apologies only.
7. **Disfluency policy.** Default 0 fillers. If enabled for the social persona: at most 1 per turn, utterance-initial only, never in numbers, instructions, or confirmations [S6, S9, S8].
8. **Pacing.** Speech rate about 160–180 wpm by default. Slow by 10–15% for lists, numbers and confirmations, and after the user repeats themselves. Put a short pause (150–250 ms) before key information instead of a filler.
9. **chrome_allowed=false.** No UI "emotion meters", mood badges or affect visualizations. Naturalness has to come from audio behavior alone. Status shown to the user is limited to listening/speaking/thinking state and the consent state.
10. **Metrics (presence-safe):** gap p50/p95, false-yield rate on backchannels, talk-over rate, filler rate, MOS/CMOS naturalness on a fixed script, and A/B for the backchannel flag. No engagement or session-length optimization, and no emotion-label telemetry.

## 3. Open questions
- Locale tuning: Croatian and other target languages may sit at different points within Stivers' ±250 ms band [S1]. Measure per locale.
- Whether VAP latency fits the PhoneLane 8 kHz path. VAP was mostly trained on wideband audio and needs testing.
- Legal review of the call-centre customer-emotion carve-out [S17] before relying on it.

## Sources
- S1 Stivers et al. 2009, PNAS, "Universals and cultural variation in turn-taking" — https://pmc.ncbi.nlm.nih.gov/articles/PMC2705608/
- S2 Ekstedt & Skantze 2022, VAP — https://arxiv.org/html/2205.09812
- S3 VAP project page — https://erikekstedt.github.io/VAP/
- S4 Inoue et al., NAACL 2025, "Yeah, Un, Oh" — https://aclanthology.org/2025.naacl-long.367.pdf
- S5 Multimodal VAP, ICMI 2025 — https://dl.acm.org/doi/full/10.1145/3716553.3750781
- S6 Adell et al. 2007, Filled pauses in speech synthesis — https://www.cs.upc.edu/~nlp/papers/adell07.pdf
- S7 Dall et al., Interspeech 2014 — https://www.isca-archive.org/interspeech_2014/dall14_interspeech.pdf
- S8 Disfluency insertion in LLM utterances, 2024 — https://arxiv.org/html/2412.12710v1
- S9 Kirkland et al., Interspeech 2022 (KTH), filler location/rate/f0 & confidence — https://www.speech.kth.se/~jocke/publications/interspeech_2022.pdf
- S10 Wester et al., Interspeech 2015, Artificial personality & disfluency — https://www.isca-archive.org/interspeech_2015/wester15_interspeech.pdf
- S11 Gambino et al. 2020, Building a Stronger CASA — https://doi.org/10.30658/hmc.1.5
- S12 Nass & Gong 2000, CACM, Speech interfaces from an evolutionary perspective — https://dl.acm.org/doi/fullHtml/10.1145/348941.348976
- S13 Uncanny valley effect for prosody (Tilburg thesis) — http://arno.uvt.nl/show.cgi?fid=162647
- S14 Barrett et al. 2019, Emotional Expressions Reconsidered — https://pmc.ncbi.nlm.nih.gov/articles/PMC6640856/
- S15 EU AI Act Recital 44 — https://ai-act-service-desk.ec.europa.eu/en/ai-act/recital-44
- S16 FPF, Art. 5(1)(f) analysis — https://fpf.org/blog/red-lines-under-eu-ai-act-unpacking-the-prohibition-of-emotion-recognition-in-the-workplace-and-education-institutions/
- S17 Dutch AP summary of consultation (call-centre customer emotion not prohibited) — https://www.autoriteitpersoonsgegevens.nl/en/system/files?file=2025-02%2FSummary+responses+emotion+recognition+workplace+education+institutions.pdf

— Maciej Grajczyk
