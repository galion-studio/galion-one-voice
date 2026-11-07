# Emotion V2 foundation spec

**Status.** Design foundation for Galion One voice, written for Samantha (Galion Voice inside Galion One) and applying to Jarvis and Galion One under the same rules. This is not a scoreboard, a recording, a clone, or an instruction to change any deployed voice. Do not synthesize, clone, or retune any deployed voice or any real third-party voice from this file. No voice audio is included. No film interfaces are copied.
**Covers.** The emotional flow of Galion Voice (the voice model inside Galion One): sectors, controlled capture, conditioning, rendering, post, and CI.
**Companion.** Landscape with sources and licences: `EMOTION-LANDSCAPE-20261107.md`. Pitch centres and personalities: `../07-sound-guide/VOICE-SPEC.md`. Interface tones (exactly seven, not speech): `../07-sound-guide/SOUND-LANGUAGE.md`.
**Written.** 7 November 2026, Europe/Zagreb (UTC+1).
**Folder.** `15-voice/` — `12-mod-packs/` already exists in this package.

Emotion V2 is the whole emotional flow of the voice. It is not a number bolted on after a render.

---

## 1. What Emotion V2 is

A spoken line in Galion One moves through this flow:

1. **Situation** — what is happening in the conversation (urgent, light, grieving, celebrating). This is context, not a sector ID.
2. **Sector** — which of the twelve named emotions the voice should inhabit, plus an intensity and an optional blend.
3. **Conditioning** — how that sector reaches the cloner (per-sector reference, style vector, instruct text, exaggeration), without moving the pitch centre off its 432 Hz just-ratio.
4. **Render** — waveform from Galion Voice.
5. **Post** — loudness and rumble alignment on a *copy*, never on the read-only original references; no character-flattening EQ that would hide the speaker.
6. **Check** — real tests: did this line land in the intended sector, stay intelligible, and stay the same person?

A scoreboard that compares one render to one generic reference is a later instrument. It is not the foundation.

Situational awareness (“the caller sounds shaken, so warm and slow down”) is a *chooser* of sectors. Acoustic emotion match (“this render sits in Tender, intensity medium”) is a *property* of the voice. The current scorer measures the second thing. V2 must do the second thing properly before anyone trusts the first.

---

## 2. The starter set: twelve sectors

About twelve, not forty. That size is grounded in:

- Cowen, Laukka, Elfenbein, Liu & Keltner (2019), *Nature Human Behaviour*: listeners across two cultures reliably recognise **at least 12 distinct kinds of emotion in speech prosody**; **categories drive recognition more than valence alone**; categories are **bridged by gradients** (blends). The paper’s named list is not reprinted here (full text was not retrieved this turn); Galion takes the *count*, the *category primacy*, and the *blend* finding.
- Ekman’s six basic emotions (1992), which almost every open speech dataset and recogniser can see.
- Neutral, which every cloning and SER protocol needs and which Cowen’s acted-emotion studies do not treat as a “kind of emotion.”
- Conversational extras that GoEmotions (27 text emotions, Apache-2.0) and speech corpora actually name: **amusement**, **caring**, **excitement**, **disappointment**, and RAVDESS / Cartesia **calm**.
- What open recognisers can detect today (SUPERB ER 4-class; emotion2vec+ 9-class; audeering-style valence / arousal / dominance).

**Seductive** is not one of the twelve. It is a Samantha-only overlay, as locked in `VOICE-SPEC.md`. It must not be applied to Galion One or to Jarvis.

### 2.1 The twelve

Valence (V) and arousal (A) below are **design coordinates** on Russell’s 1980 circumplex, scaled to approximately −1…+1. They are not measured values from a published table. Dominance (D) is the third PAD axis, sketched the same way. Intensity is a separate radial control (0 = almost Neutral, 1 = full sector).

| ID | Sector | Feel (plain) | V | A | D | Recogniser map | Taxonomy roots |
|---|---|---|---:|---:|---:|---|---|
| `neutral` | Neutral | At rest, clear, no extra colour | 0.00 | 0.00 | 0.10 | SUPERB `neu`; e2v `neutral` | Baseline for every speech corpus |
| `happy` | Happy | Warm gladness, a smile in the voice | +0.80 | +0.45 | +0.30 | SUPERB `hap`; e2v `happy` | Ekman happiness; Plutchik joy; GoEmotions joy |
| `amused` | Amused | Light, playful, a laugh waiting | +0.75 | +0.55 | +0.25 | SUPERB `hap`; e2v `happy` (nonverbal `[laugh]` is separate) | GoEmotions amusement; EmoV-DB amused; Cowen’s emphasis that amusement is distinct from generic happiness |
| `excited` | Excited | High energy, forward, delighted | +0.70 | +0.85 | +0.45 | SUPERB `hap`; e2v `happy` (may leak `surprised`) | GoEmotions excitement; Plutchik anticipation; Orpheus/Cartesia `excited` |
| `tender` | Tender | Close, caring, soft | +0.65 | −0.15 | −0.10 | SUPERB often `hap` or `neu`; e2v `happy` / `neutral` | Plutchik trust + joy; GoEmotions caring / love; Hume “gentle therapist” idea |
| `calm` | Calm | Steady, reassuring, unhurried | +0.35 | −0.55 | +0.05 | SUPERB `neu` (sometimes `hap`); e2v `neutral` | RAVDESS calm; Cartesia primary `calm` / `content` |
| `sad` | Sad | Low, gentle weight | −0.70 | −0.40 | −0.35 | SUPERB `sad`; e2v `sad` | Ekman sadness; Plutchik sadness |
| `disappointed` | Disappointed | Let down, quieter than sad, less heat | −0.55 | −0.20 | −0.25 | SUPERB `sad`; e2v `sad` or `other` | GoEmotions disappointment; Cowen speech-prosody work treats disappointment as its own kind |
| `angry` | Angry | Sharp, firm, heat | −0.55 | +0.80 | +0.70 | SUPERB `ang`; e2v `angry` | Ekman anger; Plutchik anger |
| `fearful` | Fearful | Tight, alarmed, less dominant | −0.70 | +0.75 | −0.55 | SUPERB poorly covered (leaks `ang`/`hap`); e2v `fearful` | Ekman fear; CREMA-D / RAVDESS fear |
| `surprised` | Surprised | Lifted, open, a catch of breath | +0.10 | +0.85 | 0.00 | SUPERB poorly covered; e2v `surprised`; ESD Surprise | Ekman surprise |
| `disgusted` | Disgusted | Recoil, distaste | −0.75 | +0.35 | +0.20 | SUPERB often `ang`; e2v `disgusted` | Ekman disgust; CREMA-D / RAVDESS / EmoV-DB disgust |

**Samantha-only overlay**

| ID | Overlay | Rule |
|---|---|---|
| `seductive` | Seductive | Samantha only. Never Galion One, never Jarvis. Recorded and tested as her extra take, not as a shared sector. |

### 2.2 Grouped into sectors of sectors

Four families, plus Neutral at the origin. Families are for authoring, blending and reporting — not extra labels the model must predict.

| Family | Members | Circumplex region | Conversational use |
|---|---|---|---|
| **Grounded** | Neutral, Calm | Centre and low arousal | Default presence, steady rest |
| **Bright** | Happy, Amused, Excited, Tender | High valence; Tender sits at the quiet edge of this family | Good news, play, celebration, care |
| **Heavy** | Sad, Disappointed | Low valence, low-to-mid arousal | Bad news, empathy, loss |
| **Sharp** | Angry, Fearful, Surprised, Disgusted | High arousal, mixed or low valence | Urgency, alarm, shock, refusal |

Tender is listed with the Bright family because it is high-valence care, not a resting Neutral. Its arousal is low, so blends with Calm remain lawful.

Urgency is a *situation*, not a thirteenth emotion. An urgent line usually chooses Angry or Fearful (or Calm spoken faster, if the job is to steady someone). Lightness usually chooses Amused or Happy. The CI scenarios in §6 exercise those situations on top of the twelve.

### 2.3 Intensity and blends

- **Intensity** ∈ [0, 1] per line. 0 hugs Neutral; 1 is the full sector. This is the lawful home of Chatterbox `exaggeration` and of MEAD’s weak / medium / strong idea.
- **Blend** (optional): a second sector at a lower weight, following Cowen’s finding that categories are bridged by gradients. Example: Tender 0.7 + Sad 0.3 for a kind condolence.
- Delivery modifiers are *not* sectors: `whisper`, `shout`, slower, faster. They may travel with a sector (Calm+slow, Angry+firm) the way Orpheus mixes `whisper` and `angry` in one tag list. CI must test them separately so a whisper is not scored as Sad just because it is quiet.

### 2.4 432 Hz movement inside a sector

Each voice keeps its pitch centre from `VOICE-SPEC.md`. Emotion moves on the just-ratio grid around that centre, never by sliding A4.

| Voice | Centre | Lawful lifts | Lawful rest |
|---|---|---|---|
| Galion One | 216 Hz = 432 × 1/2 | 243 (9/8), 270 (5/4), 324 (3/2) | 216, or 108 (1/4) |
| Samantha | 180 Hz = 432 × 5/12 | 202.5 (9/8), 225 (5/4), 270 (3/2) | 180 |
| Jarvis | 135 Hz = 432 × 5/16 | 151.875 (9/8), 168.75 (5/4), 202.5 (3/2) | 135, or 108 (1/4) |

Bright and Sharp sectors are allowed to visit the lifts more often and to carry more energy and a more open spectral tilt. Grounded and Heavy sectors return to centre (or to 108 Hz) sooner and carry less energy. Emotion must not retune the centre itself.

---

## 3. Emotional flow through cloning

```
consent → controlled capture → reject/accept
        → read-only originals
        → working copies (normalise)
        → per-sector conditionals
        → render (pitch centre locked)
        → post on the render copy
        → check (categorical + VAD + embedding + WER + ECAPA)
```

### 3.1 Consent

- Owner’s own recordings only. Written consent stored beside the archive.
- No third-party voices, no celebrity clones, no film voices, no voices scraped from the public web.
- A minor’s voice is out of scope.
- Synthesis of a voice that is not the owner’s, and is not a Galion-designed persona recorded by that owner, is out of scope for this spec.

### 3.2 Controlled reference capture

Every voice (Samantha, Jarvis, Galion One, and any later owner voice) gets a **per-sector reference set** recorded in one session window, in one room, on one chain.

| Item | Spec |
|---|---|
| Room | The same quiet room for the whole session. No location changes between sectors. |
| Microphone | The same microphone, same capsule, same axis. |
| Distance | 15–20 cm on-axis, stand marked so it cannot drift. |
| Gain | The same preamp gain for every sector. Do not ride the knob when someone gets louder; leave headroom instead. |
| Sample rate / bit depth | **48 kHz, 24-bit PCM WAV** (matches Expresso’s studio capture and the design package’s interface-tone format). |
| Channels | Mono preferred. If a stereo file arrives, keep the closer capsule and archive the original untouched. |
| Loudness target | **−18 LUFS ± 1 LU** integrated, measured on a copy after a successful take. (Interface tones in `SOUND-LANGUAGE.md` sit near −16 LUFS; speech keeps 2 LU more headroom for expressive peaks.) |
| True peak | **≤ −1.5 dBTP** (same ceiling as the seven interface tones). |
| Noise floor | A 1 s pause in the same position must sit at **≤ −50 dBFS RMS**, or SNR ≥ 40 dB against the speech. Prefer a recapture over denoising. |
| Format extras | No processing on the way in: no EQ, compression, limiting, reverb, noise suppression, or “enhancers.” Dry. |
| Tuning | The talker aims at that voice’s pitch centre. Emotion is colour around the centre, not a new key. |
| Takes per sector | At least **8 spoken lines** of that sector’s scenario texts (§6.2) plus **2 calibration lines** shared across sectors (so the same words exist in every emotion, ESD-style). Neutral first, then Grounded, Bright, Heavy, Sharp. |
| Overlay | Samantha additionally records `seductive` under the same protocol. |
| Session notes | Room, mic, distance, gain, date (Europe/Zagreb), talker condition (well / tired), and consent id, written into a sidecar — not into the audio stream. |

This is the environmental variable, controlled from the start. If the room or the mic changes, the scorer starts measuring furniture.

### 3.3 Originals are read-only

- Raw session files land in an originals archive and are never overwritten, resampled in place, normalised in place, or deleted. Archive, do not delete.
- All scoring, normalisation and training read **working copies**.
- Deployed voices are not replaced by anything in this spec. A passing candidate remains a candidate until Slavik’s own explicit yes.

### 3.4 Normalization pass (copies only)

Applied identically to reference copies *and* to renders before they are compared, so the comparison is voice and emotion, not room tone.

1. Resample to 48 kHz only if needed, on the copy.
2. Loudness match to −18 LUFS ± 1 LU; true-peak ceiling −1.5 dBTP.
3. Rumble cut: high-pass at **40 Hz** only. Do **not** high-pass at 80–100 Hz — that would chew Jarvis’s 135 Hz centre and the 108 Hz rest tone.
4. Optional denoise only if the noise-floor check failed *and* a recapture is impossible. Prefer reject-and-rerecord.
5. No “mic flattening” EQ, no style transfer, no pitch correction, no tempo matching that would hide emotion.
6. Write the treated copy next to the original; never over it.

Normalization of a *single* mixed reference does not create missing emotion targets. Laptop scoring on 7 November 2026 showed Samantha’s `emotion_match` moving from **0.491 raw to 0.461 normalized** — slightly worse. Environment matching stays required; it is not a substitute for one controlled take per sector.

### 3.5 Reject checker

A reference (or a whole sector folder) is rejected, not silently used, when any of these fail:

- Sample rate other than 48 kHz (after an allowed copy-resample, the *original* may be 48 kHz only — if the original was 8 kHz phone audio, reject).
- Bit depth under 16-bit, or a lossy original (mp3, aac) presented as a master.
- Clipping (any sample at 0 dBFS) or true peak above −1.5 dBTP that cannot be repaired by a 1 dB gain drop on the copy.
- Integrated loudness outside −18 ± 3 LU before correction, or outside −18 ± 1 LU after correction.
- Noise floor louder than −50 dBFS RMS in the pause, unless a documented recapture exception is signed.
- Estimated speaking fundamental wildly off the voice’s design centre (more than about a just minor third away for Neutral takes) — this is a recapture trigger, not an auto-retune.
- Wrong duration (empty, under 0.4 s of speech, or a take that is only breath).
- Missing sidecar consent, or a filename that does not name the sector.
- Any file whose talker is not the consented owner.

Rejected files stay in the archive, marked rejected. They never enter conditioning or CI.

### 3.6 Conditioning

For each voice and each sector the foundation stores:

- The accepted reference takes for that sector (working copies).
- A Neutral timbre anchor (the Neutral references), used whenever the backend can separate speaker colour from prosody.
- A sector embedding (mean of emotion2vec-style embeddings of that sector’s references).
- Optional instruct text (“speak calmly and quietly”) as a side channel, never mixed into the spoken transcript.
- Default intensity (0.5) and lawful `exaggeration` / `cfg_weight` ranges if the backend is Chatterbox.

Backend rules, ideas borrowed, no third-party code copied:

- **Timbre from Neutral, emotion from the sector take** when the model allows two references (StyleTTS2 / OpenVoice idea). Do not let a happy room or a bright mic become “the voice.”
- **Chatterbox:** `audio_prompt_path` should be the *sector* reference for emotion colour, with Neutral available as the identity guard (ECAPA is checked against Neutral *and* against the sector take). `exaggeration` tracks intensity; `cfg_weight` is lowered (toward 0.3) only when a high intensity starts to rush the line.
- **Instruct / tags** are optional overlays, never the only control.
- Nonverbal events (`[laugh]`, `[sigh]`, `[chuckle]`) are requested explicitly. They are not implied by Amused or Sad.

### 3.7 Rendering

- Pitch centre locked to the voice’s just-ratio of 432 Hz.
- Transcript is the words to say. Sector, intensity, blend and delivery travel beside it.
- Reproducible seed recorded in the CI manifest.
- No post-render “emotioniser” that paints affect on after the fact. If the model cannot speak Tender, recapture and recondition — do not EQ a smile.

### 3.8 Post

The same normalization pass as §3.4, on the render copy. Then the checks in §6. No extra interface sounds (the OS still has exactly seven).

---

## 4. Environmental variable, controlled from the start

Three layers, all required:

1. **Capture protocol** (§3.2) — prevent the variable.
2. **Reject checker** (§3.5) — refuse off-spec references before they poison conditioning.
3. **Normalization pass** (§3.4) — remove residual loudness and rumble so Samantha and Jarvis are compared on voice and emotion, not on two different kitchens.

Each voice is still scored **against its own per-sector references**, not against the other voice. Normalising both sets does not make Samantha “sound like Jarvis”; it stops the classifier from chasing mic colour. Laptop results on 7 November 2026 confirm the limit of environment-only matching: Samantha went from 0.491 raw to 0.461 normalized against her single mixed reference. The missing piece is per-emotion controlled takes, not a stronger EQ.

---

## 5. What the foundation requires before any scoreboard is built

Build these, in order. Do not start a public leaderboard, a 40-emotion dashboard, or a live voice switch first.

1. The twelve sector IDs, families, intensity, blend rules, and the Samantha-only overlay — this document.
2. Capture protocol, sidecar schema, read-only originals layout, and the reject checker implemented as a real test.
3. At least one complete, consented, on-spec reference session per voice that will be scored (Neutral + all twelve; Samantha also `seductive`).
4. Working-copy normalization that cannot write to originals.
5. Conditioning that can select a sector reference (and a Neutral timbre anchor) without breaking the 432 Hz lock.
6. The CI harness in §6, green in stub mode on CPU.
7. One GPU nightly that renders the fixed scenario texts and writes categorical, dimensional, embedding, WER and ECAPA numbers per sector.

Only then may a scoreboard exist, and only as a view of those already-running checks. A single `emotion_match` float is a summary, not a substitute.

---

## 6. CI and test harness

### 6.1 Modes

| Mode | Where | What runs |
|---|---|---|
| **Stub** | Every PR / CPU CI | No GPU, no live model download required. Fixed fixture wavs (or silence marked as fixtures) produce deterministic fake recogniser outputs. Asserts the wiring: sector IDs, thresholds, reject checker, “originals are not written,” seed recorded. |
| **Nightly** | GPU | Real render through the current cloner backend, real recognisers, real WER and ECAPA. Writes a per-sector report. Does not merge or deploy. |

Seeds are fixed and checked in. A nightly that cannot reproduce yesterday’s stub fixtures within documented jitter is a failing nightly.

### 6.2 Fixed scenario texts

Every sector has a small, frozen text set. Do not improvise new scoring lines without bumping the harness version. Shared calibration lines (same words in every sector) sit beside sector-specific lines.

Shared calibration examples (same words, every sector):

1. “I’m here. Take your time.”
2. “Tell me what you need.”

Sector-specific examples:

| Sector | Example line |
|---|---|
| Neutral | “The next step is to open the file and read the first page.” |
| Happy | “That’s wonderful news — I’m really glad it worked.” |
| Amused | “All right, that was actually pretty funny.” |
| Excited | “Yes! Let’s do it — I’m ready when you are.” |
| Tender | “I’m right here with you. You’re not alone in this.” |
| Calm | “We’re steady. Nothing here needs to be rushed.” |
| Sad | “I’m sorry. That’s a real loss, and it matters.” |
| Disappointed | “I was hoping for better, and this isn’t what we wanted.” |
| Angry | “No. That crosses a line, and we are not going to let it stand.” |
| Fearful | “Something’s wrong — we need to be careful, right now.” |
| Surprised | “Wait — I did not expect that at all.” |
| Disgusted | “That’s a no. I don’t want any part of that.” |

Situation overlays, run on top of the matching sectors:

| Situation | Sectors exercised | Intent |
|---|---|---|
| Urgent | Angry, Fearful, Calm | Heat vs alarm vs steady command |
| Light | Amused, Happy, Excited | Play without tipping into shout |
| Neutral | Neutral, Calm | Resting voice, no extra colour |
| Emotional | Sad, Tender, Disappointed | Care without collapsing into Neutral |

Texts are English for V2. They are not stage directions and they are not film quotes.

### 6.3 Checks on every scored render

Run on the normalised copy.

1. **Categorical**
   - Primary research/nightly head: emotion2vec+ style 9-class (`angry, disgusted, fearful, happy, neutral, other, sad, surprised, unknown`) when the licence review allows that weight file in CI.
   - Guard head: SUPERB `wav2vec2-base-superb-er` (`neu, hap, ang, sad`) — Apache-2.0 card, IEMOCAP-trained, too coarse to be the only judge.
   - Mapped pass: the predicted label must be in that sector’s recogniser map (§2.1). Bright sectors may share `happy`; Heavy sectors may share `sad`. Sharp sectors must *not* collapse into Neutral.
2. **Dimensional**
   - Valence, arousal (and dominance if available) from a dimensional model. The audeering wav2vec2-MSP-dim model is **CC BY-NC-SA 4.0** (research-only on its card). It is allowed in research CI; a shipped commercial scorer needs another head, a commercial licence, or Galion’s own regressor.
   - Pass: predicted (V, A) lies inside the sector’s acceptance box (design boxes start as ±0.35 around the coordinates in §2.1, then tighten once nightly data exists). Intensity should move radius outward, not jump to another family.
3. **Embedding similarity to the emotion reference**
   - Cosine similarity between the render’s emotion embedding and the *mean embedding of that sector’s accepted references* for the *same voice*.
   - This replaces “one generic reference for the whole voice.”
   - A secondary cosine against the Neutral timbre embedding is informational (the voice should still be itself).
4. **WER guard** — word error rate **≤ 1.8%** on the scenario text (same gate as today). An emotional line that slurs the words fails, even if it “sounds sad.”
5. **ECAPA guard** — speaker embedding cosine **> 0.7057** against that voice’s Neutral identity set (same gate as today). Emotion must not become a different person.
6. **432 Hz sanity** — estimated centre stays in the lawful band of that voice. No auto-retune in CI; a miss is a fail and a recapture/recondition signal.

### 6.4 How a sector and a voice pass

Until nightly data retunes them, the release bars are:

| Check | V2 bar |
|---|---|
| WER | ≤ 1.8% |
| ECAPA vs Neutral identity | > 0.7057 |
| Embedding cosine vs **that sector’s** references | ≥ 0.83 (same number as today’s `emotion_match` gate, but the comparison target changes) |
| Categorical mapped match | Required on nightly (stub uses fixtures) |
| Dimensional box | Required on nightly once a licensed dimensional head is wired |
| Reject checker | Must refuse the documented off-spec fixtures in stub mode |

A voice clears Emotion V2 when **every shared sector** clears those bars on the frozen scenario set. Samantha additionally clears `seductive` against her own overlay references. One weak sector cannot be hidden inside a single blended float.

### 6.5 What CI must not do

- Must not write to originals.
- Must not download or fine-tune on IEMOCAP, ESD, RAVDESS, Expresso, MEAD, MSP-Podcast or any other third-party recorded voices.
- Must not call a closed TTS API as the system under test.
- Must not merge, deploy, or switch a live voice. Those still need Slavik’s own explicit yes.

---

## 7. Relation to the current gate and current results

Today’s rollout gate, still in force:

| Gate | Threshold | Best reported (pre-V2) |
|---|---|---|
| WER | ≤ 1.8% | Samantha 1.5% pass; Jarvis 1.8% pass |
| ECAPA | > 0.7057 | Samantha 0.711 pass; Jarvis 0.723 pass |
| `emotion_match` | ≥ 0.83 | Samantha **0.491 raw**, **0.461** after room-mic normalization, fail; Jarvis **0.807** fail |

Today’s `emotion_match` compares each render to **that voice’s single reference** with `superb/wav2vec2-base-superb-er` (4-class `neu/hap/ang/sad`) **plus** embedding cosine, weighted **0.70 / 0.30**.

### 7.1 Laptop breakdown, 7 November 2026 (Europe/Zagreb)

These numbers come from the running single-reference scorer on the laptop. They are why V2 records **one controlled reference per emotion**.

**Samantha’s one reference is not a clean emotion target.** Its 4-class posterior is:

| neu | hap | ang | sad |
|---:|---:|---:|---:|
| 0.345 | 0.203 | 0.136 | 0.317 |

That is a mixed Neutral/Sad clip (sad 0.317, neutral 0.345), not Neutral, not Sad, and not any other single sector.

**Samantha’s renders do not follow that mix.** Sad collapses to about **0.00–0.04**. The posterior leans **angry 0.5–0.7**. Against a reference that still carries 0.317 sad, the 0.70 categorical weight cannot close the gap. That mismatch **caps `emotion_match` near 0.80**, under the 0.83 gate, even when identity and words are fine.

**Room/mic normalization made the score slightly worse:** **0.491 raw → 0.461 normalized**. Matching loudness and stripping room tone on copies of a *single* mixed reference does not invent the missing per-emotion targets. Environment control stays necessary (two kitchens should not be the test); it is not the failure mode that is holding Samantha under 0.83.

**Jarvis at 0.807** looks closer to the gate because **his reference is about 0.84 Neutral**. Renders that stay near Neutral match that one clip. That is not twelve working sectors. It is one Neutral-ish reference being easy to copy, which is why a 0.807 vs 0.491 comparison is not “Jarvis is more emotional than Samantha.”

This is the concrete case for Emotion V2: **one controlled reference per emotion**, same room and chain, originals read-only, and scoring each render against *that sector’s* references.

### 7.2 Why the old float cannot be the product

- Four classes cannot see eight of the twelve V2 sectors.
- A single reference cannot represent twelve sectors. Samantha’s reference posterior (neutral 0.345 / happy 0.203 / angry 0.136 / sad 0.317) is the proof: the target itself is already a blend.
- Renders that drop sad to ~0.00–0.04 and lean angry 0.5–0.7 will hit a ceiling near 0.80 against that blend, which is exactly what the laptop measured.
- The 0.70 categorical weight makes the coarse 4-class head dominate the float.
- Environment-only normalization moved Samantha from 0.491 to 0.461. It did not clear 0.83.

V2 keeps the WER and ECAPA numbers. It keeps **0.83** as the emotion release bar, but moves the comparison to **per-sector references** and adds categorical mapping plus dimensional boxes. The old 4-class+cosine float may still be logged, side by side, so no one loses the historical series. It is no longer allowed to be the only emotion number.

Nothing in this file authorises lowering the gate, merging PR #88, or switching a live voice.

---

## 8. 432 Hz constraints (summary)

- System reference remains A4 = 432 Hz just intonation. Interface tones remain the seven sounds in `SOUND-LANGUAGE.md`.
- Each speaking voice keeps the pitch centre in `VOICE-SPEC.md`. Emotion is travel on the just-ratio grid around that centre, plus energy and spectral tilt — not a new temperament and not a 12-TET transpose.
- Normalization must not high-pass into the speaking band (rumble cut at 40 Hz only).
- No process may “correct” a voice toward 440 Hz or toward another voice’s centre.
- Warm hum at 216/108 Hz stays part of the listening field; speech may rest at 108 Hz on a close cadence without being flagged as a different person.
- This spec does not instruct anyone to retune a deployed model today.

---

## 9. Out of scope

- Synthesizing or cloning real third-party voices.
- Training Piper, or any other backend, on the owner’s private voice clips without the capture and consent rules above.
- Copying film interfaces or film voices (no Her, no Iron Man).
- A 40+ emotion taxonomy, a public leaderboard, or a live switch.
- Changing Chatterbox itself into a different architecture in this document — V2 is the flow around the cloner.
- Cloudflare DNS, HADES, or any merge/deploy.

---

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.
