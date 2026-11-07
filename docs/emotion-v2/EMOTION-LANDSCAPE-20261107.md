# Emotion landscape — 7 November 2026

**Status.** Research note for Galion One voice (Galion Voice is the voice model inside Galion One). This file is not a recording, a clone, a scoreboard, or a licence to use any third-party model or dataset. Do not synthesize, clone, or retune any deployed or real third-party voice from this file. No voice audio is included.
**Written.** 7 November 2026, Europe/Zagreb (UTC+1).
**Method.** Licences and capabilities below were checked against official project pages, Hugging Face model/dataset cards, or primary papers the same day. This is not legal advice. Where a full article was paywalled or a PDF could not be retrieved, that gap is marked.
**Where this lives.** `12-mod-packs/` already exists in this design package, so these voice notes are `15-voice/` (next free number after `14-pricing/`). Companion spec: `EMOTION-V2-FOUNDATION-SPEC.md`.

The whole Galion system, including speaking voices, runs on **432 Hz just intonation**. Voice pitch centres stay locked to a just-ratio of 432 Hz (Galion One 216 Hz = 432 × 1/2; Samantha 180 Hz = 432 × 5/12; Jarvis 135 Hz = 432 × 5/16), with the warm hum at 216/108 Hz. Emotion work must not break that lock.

Closed products are listed only so Galion can borrow *ideas*. Never their code, weights, or recorded voices.

---

## 1. Open TTS with emotion or prosody control

| System | Licence (verified) | What it is | Idea worth borrowing |
|---|---|---|---|
| **Chatterbox** (Resemble AI) — Galion's current cloner | **MIT** for the public archive (GitHub `resemble-ai/chatterbox` and Hugging Face `ResembleAI/chatterbox`). README states that as of August 2026 Chatterbox is retired research, no longer maintained, and is not a Resemble AI product. | Zero-shot voice cloning. Original English model exposes `exaggeration` (wired as `emotion_adv`) and `cfg_weight`. Defaults `exaggeration=0.5`, `cfg_weight=0.5`. For more expressive speech the README suggests `cfg_weight≈0.3` and `exaggeration≈0.7+`; higher exaggeration tends to speed speech up. Chatterbox-Turbo / Nano add paralinguistic tags such as `[laugh]`, `[chuckle]`, `[cough]`. Outputs include Resemble PerTh watermarking. | Keep the intensity knob (`exaggeration`) and the CFG pacing trade-off. Treat paralinguistic tags (`[laugh]`, `[sigh]`) as a layer *beside* emotion sectors, not as the sectors themselves. A single global exaggeration is not a 12-way emotion set — Galion still has to condition per sector under the 432 Hz pitch lock. |
| **StyleTTS2** (yl4579) | **Code: MIT.** Pretrained models carry an extra rule: tell listeners the speech is synthesized unless you have permission to use the cloned voice. Inference in the official repo depends on a GPL phonemizer; a separate MIT-licensed package exists (lower quality). | Style diffusion samples a style suited to the text. `embedding_scale` is classifier-free guidance: higher makes style more conditional on the text and therefore more emotional (demo uses 1 vs 2). Author note (issue #203): a *prosody* token and an *acoustic/speaker* token can take different references — emotional reference for prosody, neutral reference for timbre. | Disentangle speaker timbre from emotion/prosody. Do not feed one mixed clip into both paths. Use text-conditioned style strength as a secondary intensity control. Honour the “permission or disclose synthesis” rule — Galion already requires the owner's own recordings only. |
| **Coqui XTTS-v2** | **Code** (Coqui TTS): **MPL-2.0**. **Weights** (Hugging Face `coqui/XTTS-v2`): **Coqui Public Model License (CPML)** — non-commercial use of the model *and its outputs*. Fine-tunes remain CPML. Coqui declined to relicense after shutdown (issue #3490). | Zero-shot clone from about 6 s. Feature list includes “emotion and style transfer by cloning” (the reference clip's delivery is copied, not selected from a labelled set). 17 languages. | Idea only, and not a production dependency: emotion can ride along with a reference take. CPML blocks commercial use of the weights and outputs, so Galion should not ship XTTS. |
| **F5-TTS** (SWivid) | **Code: MIT** (`SWivid/F5-TTS`). **Official pretrained weights: CC-BY-NC-4.0** because they were trained on the in-the-wild Emilia dataset (stated on the GitHub README and the Hugging Face card). | Flow-matching TTS with zero-shot voice cloning via speech infilling. Official feature list includes multi-style / multi-speaker generation, not a first-party labelled emotion set. Community work (for example `RaduBolbo/F5-TTS-Emotional-CFG`, MIT code; SpeD 2025 paper on multi-term CFG) adds Neutral / Happy / Sad / Angry / Surprise by fine-tuning on ESD. | Official F5 copies whatever emotion leaked into the reference. The useful idea is *explicit emotion CFG separate from speaker CFG* (three-way: conditional, fully unconditional, emotion-unconditional). Weights are non-commercial unless Galion trains its own. |
| **CosyVoice** (FunAudioLLM / QwenAudio) | **Apache-2.0** on `QwenAudio/CosyVoice` and on the Fun-CosyVoice3 Hugging Face card (`license: apache-2.0`). Check NOTICE files for third-party components. | Instruct mode (`inference_instruct2` on CosyVoice 2/3) takes natural-language instructions for emotion, speed, volume, dialect and style, combined with a prompt wav for zero-shot cloning. | Natural-language emotion instructions *plus* a speaker prompt, without replacing the speaker embedding. Galion can offer an instruct-style control surface while keeping sector IDs as the canonical control. |
| **EmotiVoice** (NetEase Youdao) | **Apache-2.0** for the engine (`netease-youdao/EmotiVoice` README and LICENSE). The interactive page has a separate User Agreement. | Prompt-controlled English and Chinese TTS, 2000+ voices. Emotional synthesis (happy, excited, sad, angry and others) via a style/emotion prompt. Built on the PromptTTS idea. Uses pitch, speed, energy and emotion as style factors. Also has a personal-data voice-cloning recipe. | Prompted emotion (a short style phrase) as one control language. Keep style factors (pitch, speed, energy, emotion) explicit. Any cloning path must stay on the owner's recordings. |
| **Parler-TTS** (Hugging Face) | **Apache-2.0** (library and official model cards, including `parler-tts-large-v1` and `parler-tts-mini-expresso`). | Description-prompted TTS: a second text field describes the speaker and style (“speaks moderately slowly in a sad tone…”). The Mini Expresso model is trained to respond to style words such as happy, sad, whisper, emphasis, laughing, confused. | A human-readable style prompt alongside the transcript. Useful as an authoring UI, not as the only CI signal (prompts are loose). |
| **OpenVoice** (MyShell / MIT authors) | **MIT** for V1 and V2 since April 2024. README: free for commercial and research use. | Instant tone-colour cloning. The ToneColorConverter copies the reference speaker's tone colour, *not* emotion or accent. Emotion and style (friendly, cheerful, excited, sad, angry, terrified, shouting, whispering, default) come from the *base TTS speaker/style* before conversion. Paper: arXiv:2312.01479. | The cleanest open statement of the idea Galion needs: **clone timbre separately from emotion**. Emotion lives in the base/style path; the converter should not wash it out or invent it from room tone. |
| **Fish Speech** (Fish Audio) | Current GitHub `LICENSE` is the **Fish Audio Community License Agreement v1.0** (updated 13 October 2026), not Apache-2.0. Research and non-commercial use are allowed. Commercial use is allowed only while annual revenue is under **USD 500,000**; hosted TTS/voice-cloning services need a separate licence. Older v1.5.1 code was Apache-2.0; do not assume that still applies. The licence forbids cloning an identifiable person without consent, and forbids using outputs to train another foundational speech model. | Open TTS with inline emotion/style tags in documentation (for example `[happy]`, `[sad]`, `[angry]`, `[whisper]`, `[laughing]`). | Inline tags as an authoring convenience. The consent and no-third-party-clone clauses match Galion's own rule. The revenue cap and hosted-service clause make Fish Speech a poor default dependency for a shipped product. |
| **Orpheus TTS** (Canopy Labs) | **Apache-2.0** (`canopyai/Orpheus-TTS`). | LLM-style TTS with emotion/style words in `emotions.txt`: happy, normal, disgust/disgust, longer, sad, frustrated, slow, excited, whisper, panicky, curious, surprise, fast, crying, deep, sleepy, angry, high, shout. Community discussion uses verbal cues in angle brackets (for example `<laugh>`). | A small, named tag vocabulary — and the reminder that some “emotions” are actually *delivery* (slow, fast, whisper, shout) or *quality* (deep, high). Galion should keep sectors (feelings) separate from delivery modifiers. |
| **Dia** (Nari Labs) | **Apache-2.0** (`nari-labs/dia`). | 1.6B dialogue TTS. Speaker tags `[S1]` / `[S2]` and nonverbal cues such as `(laughs)` and `(coughs)`. Emotion/tone can be steered by an audio prompt plus its transcript. | Dialogue-native nonverbal events and speaker turns. Useful for multi-voice scenes; not a labelled 12-emotion set. |
| **Sesame CSM** (SesameAILabs) | **Apache-2.0** for the public `SesameAILabs/csm` code (and commonly the CSM-1B weights on Hugging Face, which may still require accepting access conditions). The commercial Sesame product is separate and closed. | Conversational Speech Model: context-aware spoken conversation rather than a slot of named emotions. | Conversation context should move emotion (the previous turn colours the next). Do not treat a single line as an isolated acting cue. |
| **Kokoro** (hexgrad) | **Apache-2.0** for `hexgrad/kokoro` and Hugging Face `hexgrad/Kokoro-82M`. | Small (82M) high-quality TTS. No first-party emotion or style control; third parties have tried style optimisation or emotion-specific voicepacks. | A reminder that a good base voice is not an emotion system. Do not ship “one fine-tune per emotion” as the primary design (it cannot change mid-sentence). |
| **Piper** | Original `rhasspy/piper` is **MIT** and archived. Current development is `OHF-Voice/piper1-gpl`, **GPL-3.0**. | Fast local TTS. No general emotion-control parameter (project discussion in `rhasspy/piper#150` declined built-in expressions). The only practical workaround is separate speaker IDs / models per emotion, which cannot vary inside a sentence. | Keep Piper in mind as a lightweight fallback voice, not as the Emotion V2 engine. A per-emotion speaker-ID hack is not the foundation. |

---

## 2. Emotion recognition models

| Model | Licence (verified) | What it measures | Idea worth borrowing |
|---|---|---|---|
| **superb/wav2vec2-base-superb-er** (current Galion scorer) | Hugging Face card: **Apache-2.0**. Fine-tuned for SUPERB Emotion Recognition on **IEMOCAP**, which itself is a research-only USC corpus. `config.json` labels: `0=neu`, `1=hap`, `2=ang`, `3=sad`. Input 16 kHz. Session-1 accuracy about 0.63. | Four-class categorical ER. Galion's current `emotion_match` mixes this 4-class head with embedding cosine at 0.70 / 0.30 against a *single* reference per voice. | Keep it as a *guard* and as a mapping target for Neutral / Happy / Angry / Sad. It cannot see Fearful, Surprised, Disgusted, Tender, Amused, Excited, Calm, or Disappointed as themselves. That is a root cause of a low Samantha score: the scorer is too coarse, and a single reference cannot represent twelve sectors. Review commercial use because of the IEMOCAP training set. |
| **emotion2vec / emotion2vec+** (FunASR / ddlBoJack) | FunASR *code* is **MIT**. Weights on Hugging Face (`emotion2vec/emotion2vec_plus_large`) are `license: other` / **`model-license`**, linking to FunASR's custom model licence — not a standard OSI licence. Legal review before shipping in a commercial product. | emotion2vec+ large (~300M) categories: **angry, disgusted, fearful, happy, neutral, other, sad, surprised, unknown**. Also yields embeddings at utterance or frame (50 Hz) granularity. Paper: Ma et al., 2023, arXiv:2312.15185. | The best open *categorical* coverage that lines up with Galion's starter set (adds disgust, fear, surprise on top of the SUPERB four). Use embeddings for per-sector similarity, not only the argmax label. |
| **audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim** | Hugging Face card: **CC BY-NC-SA 4.0**. Research-only note on the card; commercial licence available from audEERING. | Dimensional speech emotion: **arousal, dominance, valence** in about 0…1, plus pooled transformer states. Fine-tuned on MSP-Podcast v1.7 (pruned wav2vec2-large-robust, 12 layers). Paper: Wagner et al., 2022, arXiv:2203.07378 (“closing the valence gap”). | This is the open dimensional instrument Galion should emulate. Use VAD as a continuous check that a sector landed in the right region of Russell's circumplex, not only that a 4-class argmax matched. Because of CC-BY-NC-SA, CI-as-research is fine; a shipped commercial scorer needs a different model, a commercial licence, or Galion's own dimensional head. |
| **SpeechBrain emotion-recognition-wav2vec2-IEMOCAP** | Hugging Face card: **Apache-2.0**. Trained on IEMOCAP (research-only corpus). Reported IEMOCAP accuracy 78.7% (avg 75.3%) on the 19-10-21 release. 16 kHz. | Another 4-class IEMOCAP-style classifier with embeddings. | Useful as a second categorical opinion in research CI. Same IEMOCAP caveat as SUPERB ER. |

---

## 3. Datasets

These are listed so Galion knows what emotion labels the field actually records, and which corpora cannot be used to train or ship a commercial voice. **Galion does not clone speakers from these sets.** Owner recordings only.

| Dataset | Licence / access (verified) | Emotion / style set | Note for Galion |
|---|---|---|---|
| **ESD** (HLTSingapore Emotional Speech Database; Zhou et al.) | Research use; complete the provided licence and request access (project page `hltsingapore.github.io/ESD`). | Parallel English and Mandarin; 20 speakers; **neutral, happy, angry, sad, surprise**. 350 utterances per speaker. | The classic *parallel same-sentence, many-emotions* design. This is the capture idea Galion should copy with the owner's voice: same text, same room, one take per sector. |
| **RAVDESS** (Livingstone & Russo) | **CC BY-NC-SA 4.0** (Zenodo record 1188976). | Speech: **neutral, calm, happy, sad, angry, fearful, disgust, surprised**. Song drops disgust and surprise. | Adds *calm* (low-arousal positive/neutral) which most 4-class scorers lack. Non-commercial share-alike. |
| **CREMA-D** | **ODbL 1.0** (database) / **DbCL 1.0** (contents), `CheyneyComputerScience/CREMA-D`. | **Anger, Disgust, Fear, Happy/Joy, Neutral, Sad.** | Open-ish crowd-sourced acted set covering Ekman's core plus neutral. |
| **IEMOCAP** (USC SAIL) | Academic, non-transferable, **research-only**; signed release; no redistribution; not commercial (USC data-release form). | Many labels in the full set; the conventional ER protocol keeps **angry, happy, neutral, sad** (excited often folded into happy). | Source of SUPERB ER and SpeechBrain ER. Do not redistribute. Do not treat its 4-class protocol as the full emotion world. |
| **MSP-Podcast** (UT Dallas MSP Lab) | Free under an academic data-transfer agreement; commercial use needs a separate licence. | Categorical labels **and** dimensional **activation/arousal, valence, dominance**. | The right annotation *shape* for V2: categories plus VAD. Training data for the audeering dimensional model. |
| **EmoV-DB** (Adigwe, Tits, El Haddad, Ostadabbas, Dutoit, 2018) | Custom **non-commercial** licence (`numediart/EmoV-DB` LICENSE.md). Commercial use requires contacting the authors. | **Neutral, sleepiness, anger, disgust, amused.** Four speakers. Built for emotional speech synthesis (CMU Arctic sentences). | Useful because *amused* and *sleepy* are not in Ekman's six, and because it was collected specifically to control emotion in generation. |
| **Expresso** (Meta FAIR; Nguyen et al., 2023) | **CC BY-NC 4.0** (official page `speechbot.github.io/expresso`). | 40+ hours at **48 kHz / 24-bit**, studio, four speakers. 8 read styles and 26 improvised styles, including angry, awe, bored, calm, confused, default, desire, disgusted, fearful, happy, laughing, sad, sarcastic, sleepy, sympathetic, whisper, plus delivery styles (fast, projected, enunciated). | Gold-standard *capture environment*: professional studio, documented sample rate and bit depth, styles recorded on purpose. Non-commercial. Borrow the protocol, not the voices. |
| **MEAD** (Wang et al., ECCV 2020; `uniBruce/Mead`) | GitHub code is **MIT**. The *dataset* itself is a requested research corpus of audiovisual acted emotion; do not treat the code licence as a dataset licence. | 60 actors; **neutral plus angry, disgust, contempt, fear, happy, sad, surprised**; three intensities (weak / medium / strong). | Intensity levels (weak / medium / strong) are the right companion to Chatterbox-style exaggeration. |
| **CASIA** Chinese Emotional Speech Corpus | Distributed commercially through Chinese LDC / language-resource catalogues (research and commercial price lists). Not an open licence. Some unofficial redistributions claim Apache-2.0; that does **not** establish the original terms. | Commonly described as **surprise, happiness, sadness, anger, fear, neutral**. | Listed for completeness. Do not scrape unofficial mirrors. |

---

## 4. Taxonomies

| Taxonomy | Source | Set | What to borrow |
|---|---|---|---|
| **Ekman 6** | Paul Ekman, “An Argument for Basic Emotions” (1992), *Cognition & Emotion*. | Anger, disgust, fear, happiness (enjoyment), sadness, surprise. | The core that almost every speech dataset and recogniser can see. Necessary, not sufficient. |
| **Plutchik 8** | Robert Plutchik, general psychoevolutionary theory / wheel of emotions (1980). | Joy, trust, fear, surprise, sadness, disgust, anger, anticipation. Four opposite pairs; intensity varies toward the centre; combinations yield dyads. | Opposite pairs and *intensity* as a radial axis. Trust/caring is the root of Galion's Tender sector. Anticipation relates to Excited. |
| **Russell circumplex** | James A. Russell, “A Circumplex Model of Affect” (1980), *Journal of Personality and Social Psychology*, 39(6), 1161–1178. | Two bipolar dimensions: **valence** (pleasure–displeasure) and **arousal** (activation–deactivation). Later work often adds **dominance** (PAD; Mehrabian & Russell, 1974). | The continuous overlay for every named sector. A sector is a region, not a point. This is what audeering-style VAD measures. |
| **GoEmotions 27 + Neutral** | Demszky et al., ACL 2020; dataset **Apache-2.0** (`google-research-datasets/go_emotions`). Text (Reddit), not speech. | admiration, amusement, anger, annoyance, approval, caring, confusion, curiosity, desire, disappointment, disapproval, disgust, embarrassment, excitement, fear, gratitude, grief, joy, love, nervousness, optimism, pride, realization, relief, remorse, sadness, surprise, plus **neutral**. | Proof that ~27 is too large for V2, and a menu to pick conversational extras from: **amusement, caring, disappointment, excitement** sit naturally on top of Ekman's six plus neutral. |
| **Cowen & Keltner — speech prosody, about 12** | Cowen, Laukka, Elfenbein, Liu & Keltner (2019), “The primacy of categories in the recognition of 12 emotions in speech prosody across two cultures,” *Nature Human Behaviour* 3:369–382. [https://doi.org/10.1038/s41562-019-0533-6](https://doi.org/10.1038/s41562-019-0533-6). Author summary: [https://www.alancowen.com/prosody](https://www.alancowen.com/prosody). | Abstract (retrieved from the Nature page this turn): US and Indian listeners judged 2,519 speech samples from 100 actors in 5 cultures and could reliably infer **at least 12 distinct kinds of emotion** from prosody. **Categories drive recognition more than broad affective features such as valence.** Categories are **bridged by gradients** (blends), not isolated islands. The named 12-category list is not reprinted here: the full article was paywalled and the PMC PDF was not obtained this turn. | This is the scientific reason V2 is “about 12, not 40+.” Keep named sectors *and* allow blends. Do not collapse everything onto valence. Do not invent a 40-class scoreboard. |
| **Cowen & Keltner — vocal bursts, 24** | Cowen et al., *American Psychologist* (“Mapping 24 Emotions Conveyed by Brief Human Vocalization”); author summary at [https://www.alancowen.com/vocs](https://www.alancowen.com/vocs). | Brief nonverbal vocalizations (laughs, screams, sighs and similar bursts) convey **24** kinds of emotion. | Treat laughs, sighs, gasps as a *nonverbal-events* layer, not as extra speaking sectors. Matches Chatterbox-Turbo / ElevenLabs / Dia tags. |

---

## 5. Research on prosody, emotion, and emotion-in-cloning

Ideas only, drawn from papers and first-party write-ups checked this turn:

- **Style as a latent, text-conditioned variable** — StyleTTS 2 (Li et al., 2023, arXiv:2306.07691). Emotion can come from the text, from a style reference, or from both; speaker and prosody need not share a reference.
- **Prompted style** — PromptTTS (the stated basis of EmotiVoice): a short style phrase conditions pitch, speed, energy and emotion.
- **Timbre converter vs style engine** — OpenVoice (Qin et al., 2023, arXiv:2312.01479): clone tone colour; keep emotion in the base/style path.
- **Reference leakage** — F5-TTS and XTTS copy the emotion that happens to sit in the prompt wav. Community ESD fine-tunes show you must *force* the model to read an emotion label (drop emotion during training, separate CFG term) or it will ignore the label and copy the reference.
- **Intensity is a first-class axis** — MEAD's weak/medium/strong; Chatterbox `exaggeration`; Cartesia's “guidance not a strict multiplier.”
- **Dimensional SER still struggles with valence** — Wagner et al., 2022 (the audeering model paper) is explicitly about closing the valence gap. Galion should score valence and arousal, not only a 4-class label.
- **Vocal expression review** — Juslin & Laukka (2003), Scherer (1986) and later: emotion in the voice is carried by F0 level and contour, tempo, intensity, and spectral tilt. Under a 432 Hz lock, those are the lawful knobs (just-ratio steps around the pitch centre, energy, tilt) — not a retune of A4.
- **EmoV-DB** (Adigwe et al., 2018, arXiv:1806.09514) and **Expresso** (Nguyen et al., 2023): collect emotion *for generation*, in controlled conditions, if you want generation to be controllable.
- **Parallel same-text takes** (ESD, RAVDESS, CREMA-D, MEAD): the only clean way to change emotion without also changing words, room, or mic.

---

## 6. Closed systems (ideas only — never code, weights, or voices)

| System | What the official pages say | Idea to borrow |
|---|---|---|
| **ElevenLabs v3 / v4** | Docs: wrap a short instruction in square brackets where delivery should change. Examples: emotions `[curious]` `[crying]` `[mischievously]`; delivery `[whispers]` `[shouts]`; reactions `[laughs]` `[clears throat]` `[sighs]`. Same tags on v3 and v4. [https://elevenlabs.io/docs/help-center/product/core-capabilities/text-to-speech/how-do-audio-tags-work-with-eleven-v3-and-v4](https://elevenlabs.io/docs/help-center/product/core-capabilities/text-to-speech/how-do-audio-tags-work-with-eleven-v3-and-v4) | Inline audio tags as an authoring and runtime language, separate from the transcript. |
| **Hume AI — Octave and EVI** | Octave (23 Dec 2024 blog): a speech-language model that builds a *personality* (accent, register, emotional intonation, role) from a descriptive prompt or a recording as short as 5 s. EVI: real-time speech-to-speech that listens to the caller's prosody and adapts tone, rhythm and timbre. Expression Measurement API scores expressions from audio. [https://www.hume.ai/blog/introducing-octave](https://www.hume.ai/blog/introducing-octave) | Measure the *caller's* emotion and choose the *voice's* sector as one loop — but keep those two modules distinct (situational awareness ≠ acoustic sector match). Promptable personality is how a voice *stays itself* while moving. |
| **OpenAI gpt-4o-mini-tts / steerable TTS** | OpenAI text-to-speech guide: an `instructions` parameter steers accent, emotion, intonation, tone, speed and whispering independently of the transcript. | A side-channel instruction, not a rewrite of the words. Galion can accept “speak gently and slowly” without putting those words in the spoken text. |
| **Sesame** (commercial companion) | Distinct from the Apache-2.0 CSM-1B drop. The product is a conversational voice that aims to sound present in a dialogue. | Context-carrying conversation, not line-by-line acting. |
| **Cartesia Sonic** | Official docs: `generation_config.speed` (0.6–1.5), `volume` (0.5–2.0), optional `emotion` (English only). Treated as *guidance*, not a strict multiplier; the emotion must fit the transcript. Primary emotions with the most data: `neutral`, `calm`, `angry`, `content`, `sad`, `scared`. A longer optional list exists. `[laughter]` inserts a laugh. SSML-like `<emotion value="…"/>` for mid-utterance shifts. [https://docs.cartesia.ai/build-with-cartesia/capability-guides/volume-speed-emotion](https://docs.cartesia.ai/build-with-cartesia/capability-guides/volume-speed-emotion) | Named emotion + separate speed/volume + “guidance, not a hard override” + mid-line shifts. Primary set is close to Galion's sectors. |
| **Microsoft Azure neural TTS** | `mstts:express-as` `style` on voices that advertise that style (`cheerful`, `sad`, `angry`, and others). Unsupported styles are ignored (voice stays neutral). Docs: [Speech Synthesis Markup Language voice](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/speech-synthesis-markup-voice). | A structured style element bound to a voice capability table — if a voice has no Tender take, do not silently pretend it has. |
| **Google Cloud TTS / Gemini-TTS** | Gemini-TTS: natural-language style prompts (for example “cheerful and friendly”, “whispered, nervous”). Classic SSML `<google:style>` is limited to **apologetic, calm, empathetic, firm, lively**. Momentary events use inline tags such as `<sigh>`. | Two-level control: sustained style (a sector) and momentary events (a sigh, a laugh). |

None of these systems publish a controlled-reference capture protocol for *your* clone, a 432 Hz pitch lock, or a CI harness that rejects off-spec references. That is the gap Galion has to fill.

---

## 7. What Galion must build itself

Nothing above is an Emotion V2 foundation. The missing pieces, which the companion spec defines, are:

1. **A small named sector set (~12)** grounded in Ekman + Neutral + conversational picks from GoEmotions / Plutchik / Cowen's “about twelve speech-prosody categories,” mapped onto Russell VAD and onto the labels that SUPERB ER, emotion2vec+ and dimensional models can actually see.
2. **Controlled-reference capture** — same room, mic, gain, distance, sample rate, loudness target and noise floor; per-emotion takes per voice; owner's own recordings only; originals read-only.
3. **An off-spec rejector** that refuses a reference which fails those checks, so the scorer measures voice and emotion, not room or mic.
4. **A careful normalization pass on copies only** that matches loudness and strips rumble without flattening the voice or cutting into speaking fundamentals (Jarvis's centre is 135 Hz; cadences may sit at 108 Hz).
5. **Per-sector emotion conditioning under the 432 Hz pitch lock** — not a global exaggeration slider alone, and not a 12-TET retune of A4. Timbre path and emotion path stay separate (OpenVoice / StyleTTS2 idea).
6. **Intensity** per sector (MEAD weak/medium/strong; Chatterbox exaggeration) and **blends** (Cowen gradients), so the voice can move rather than jump.
7. **A nonverbal-events layer** (`[laugh]`, `[sigh]`, `[chuckle]`) distinct from the twelve speaking sectors.
8. **A CI and test harness** with fixed scenario texts per sector, categorical + dimensional + embedding-to-*that-sector's*-reference checks, plus WER and ECAPA guards; CPU stub mode in PR CI; GPU nightly. Reproducible seeds.
9. **The foundation before any scoreboard** — capture, sectors, flow, tests. The current 0.83 `emotion_match` number is a release bar, not the product.
10. **Consent, no third-party voices, no film copies.** Samantha-only **Seductive** stays Samantha-only (`07-sound-guide/VOICE-SPEC.md`).

Chatterbox remains a valid cloner backend. It does not, by itself, provide items 1–9.

---

## 8. Sources checked this turn

- Chatterbox: [https://github.com/resemble-ai/chatterbox](https://github.com/resemble-ai/chatterbox), [https://huggingface.co/ResembleAI/chatterbox](https://huggingface.co/ResembleAI/chatterbox)
- StyleTTS2: [https://github.com/yl4579/StyleTTS2](https://github.com/yl4579/StyleTTS2), [https://arxiv.org/abs/2306.07691](https://arxiv.org/abs/2306.07691)
- Coqui XTTS-v2: [https://huggingface.co/coqui/XTTS-v2](https://huggingface.co/coqui/XTTS-v2), CPML text via model licence; Coqui TTS [https://github.com/coqui-ai/TTS](https://github.com/coqui-ai/TTS)
- F5-TTS: [https://github.com/SWivid/F5-TTS](https://github.com/SWivid/F5-TTS), [https://huggingface.co/SWivid/F5-TTS](https://huggingface.co/SWivid/F5-TTS)
- CosyVoice: [https://github.com/QwenAudio/CosyVoice](https://github.com/QwenAudio/CosyVoice)
- EmotiVoice: [https://github.com/netease-youdao/EmotiVoice](https://github.com/netease-youdao/EmotiVoice)
- Parler-TTS: [https://github.com/huggingface/parler-tts](https://github.com/huggingface/parler-tts)
- OpenVoice: [https://github.com/myshell-ai/OpenVoice](https://github.com/myshell-ai/OpenVoice)
- Fish Speech licence: [https://github.com/fishaudio/fish-speech/blob/main/LICENSE](https://github.com/fishaudio/fish-speech/blob/main/LICENSE)
- Orpheus: [https://github.com/canopyai/Orpheus-TTS](https://github.com/canopyai/Orpheus-TTS), `emotions.txt`
- Dia: [https://github.com/nari-labs/dia](https://github.com/nari-labs/dia)
- Sesame CSM: [https://github.com/SesameAILabs/csm](https://github.com/SesameAILabs/csm)
- Kokoro: [https://github.com/hexgrad/kokoro](https://github.com/hexgrad/kokoro), [https://huggingface.co/hexgrad/Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M)
- Piper: [https://github.com/rhasspy/piper](https://github.com/rhasspy/piper), [https://github.com/OHF-Voice/piper1-gpl](https://github.com/OHF-Voice/piper1-gpl)
- SUPERB ER: [https://huggingface.co/superb/wav2vec2-base-superb-er](https://huggingface.co/superb/wav2vec2-base-superb-er)
- emotion2vec+: [https://huggingface.co/emotion2vec/emotion2vec_plus_large](https://huggingface.co/emotion2vec/emotion2vec_plus_large)
- audeering dimensional: [https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim](https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim)
- SpeechBrain ER: [https://huggingface.co/speechbrain/emotion-recognition-wav2vec2-IEMOCAP](https://huggingface.co/speechbrain/emotion-recognition-wav2vec2-IEMOCAP)
- ESD: [https://hltsingapore.github.io/ESD/](https://hltsingapore.github.io/ESD/)
- RAVDESS: [https://zenodo.org/records/1188976](https://zenodo.org/records/1188976)
- CREMA-D: [https://github.com/CheyneyComputerScience/CREMA-D](https://github.com/CheyneyComputerScience/CREMA-D)
- IEMOCAP: [https://sail.usc.edu/iemocap/](https://sail.usc.edu/iemocap/)
- MSP-Podcast: [https://lab-msp.com/MSP/MSP-Podcast.html](https://lab-msp.com/MSP/MSP-Podcast.html)
- EmoV-DB: [https://github.com/numediart/EmoV-DB](https://github.com/numediart/EmoV-DB)
- Expresso: [https://speechbot.github.io/expresso/](https://speechbot.github.io/expresso/)
- MEAD: [https://github.com/uniBruce/Mead](https://github.com/uniBruce/Mead)
- GoEmotions: [https://huggingface.co/datasets/google-research-datasets/go_emotions](https://huggingface.co/datasets/google-research-datasets/go_emotions)
- Cowen speech-prosody paper: [https://doi.org/10.1038/s41562-019-0533-6](https://doi.org/10.1038/s41562-019-0533-6)
- ElevenLabs audio tags: [https://elevenlabs.io/docs/help-center/product/core-capabilities/text-to-speech/how-do-audio-tags-work-with-eleven-v3-and-v4](https://elevenlabs.io/docs/help-center/product/core-capabilities/text-to-speech/how-do-audio-tags-work-with-eleven-v3-and-v4)
- Hume Octave: [https://www.hume.ai/blog/introducing-octave](https://www.hume.ai/blog/introducing-octave)
- Cartesia emotion docs: [https://docs.cartesia.ai/build-with-cartesia/capability-guides/volume-speed-emotion](https://docs.cartesia.ai/build-with-cartesia/capability-guides/volume-speed-emotion)
- Azure SSML voice: [https://learn.microsoft.com/en-us/azure/ai-services/speech-service/speech-synthesis-markup-voice](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/speech-synthesis-markup-voice)
- Google Gemini-TTS: [https://docs.cloud.google.com/text-to-speech/docs/gemini-tts](https://docs.cloud.google.com/text-to-speech/docs/gemini-tts)
- Related Galion notes: `07-sound-guide/VOICE-SPEC.md`, `07-sound-guide/SOUND-LANGUAGE.md`

---

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.
