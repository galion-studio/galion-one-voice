# Voice datasets and a PersonaPlex data model

**Label: proposed data model plus an inventory of code that already exists.** Nothing in this file is evidence that PersonaPlex reinforcement learning has been run, that a universal voice database exists, or that indoor recordings have been ingested.

No training job, no download, and no audio was added here.

Voice tip: `3e1f5a12`. Plan date for the re-anchor: `docs/galion-voice-v2/PERSONAPLEX-REANCHOR.md`, 2026-11-04 16:01 CET, updated the same day at 20:45 CET (no third-party teacher). Open Mind README calls `persona-plex-relay` a scaffold: it loads config and prints a plan, opens no socket, and has no GPU. NVIDIA weights are not in that repository. The NVIDIA paper, as cited in the re-anchor note, does not describe an RL stage. The RL section there is Galion’s own proposal.

## Planned versus measured

[Pull request 109](https://github.com/galion-studio/galion-one-voice/pull/109) (`2a3a49c5`, branch `cursor/voice-v2-universal-datasets-7d6b`, files `05-voice-v2-universal-datasets.md` and `05-voice-v2-universal-datasets.schema.json`) publishes a **proposed** join of persona, emotion, and indoor environment. It does not populate a corpus and it does not train a model. This note treats that schema as a proposal sitting next to the code inventory below.

| Claim | Class |
|---|---|
| Persona / emotion / indoor-environment join | Proposed. The schema file is a document. |
| Emotion V2 pipeline and a Moshi-shaped PersonaPlex exporter exist as code or docs | Present as programs and notes. Not a training run. `exporters.py` puts the line on the left and writes silence on the right (`np.zeros`). The module says these clips are single-speaker lines, not duplex conversations. |
| Indoor PersonaPlex RL | Plan only. [Pull request 110](https://github.com/galion-studio/galion-one-voice/pull/110) (`db35455b`, `11-personaplex-indoor-rl.md`). `crates/galion-mimi` `IS_STUB = true` and the crate comment forbids adding weights. The student backend is unplaced. No training and no download. |
| `datasets/emotion-v2/` | Ignored (`.gitignore`). No tracked populated dataset. |
| Student slot | Stub. Configs, no trained weights in git. |
| Active conditioning pins | CC0 Amy Koenig (Samantha register) and Alan Davis Drake (Jarvis register), named in the persona voice notes. Clips are gitignored. |
| Old movie references | Retired as conditioning sources. Not re-cleared here. |
| Rights and consent gates | Existing public docs and locks (default deny, capture consent separate from REMEMBER). Not a new rights decision and not proof a live host is enforcing them. |

## What exists (inventory)

### Emotion V2 design inventory

`emotion-v2/emotions.toml`, schema `galion-emotion-v2/1`, persona `samantha`, status `design`. Twelve names plus pitch centre 432 Hz. Valence and arousal are design coordinates, not a claim of Russell’s measurements. SUPERB and emotion2vec label lists are named. CI does not download those weights (`emotion-v2-ci.yml` is a CPU stub).

### Capture spec, not a corpus

`docs/emotion-v2/capture-spec.toml` and `docs/emotion-v2/CAPTURE-PROTOCOL.md`. Consent line in the spec: `owner-own-recordings-only`. Format 48 kHz, 24-bit, mono. Five takes per emotion, minimum three. The spec says not to put audio next to the file. This review did not find a committed captured corpus.

### Synthetic dataset pipeline

`tools/emotion-v2/dataset/emotion_v2_dataset/`. Manifest schema `emotion-v2.manifest.v1` (`manifest.py`). Required keys include `audio_path`, `text`, `emotion`, `intensity`, `environment`, `snr_db`, `voice_id`, `scores`, `sha256`. Extra fields record sample rate, pitch standard, `render_backend`, `synthetic_signal`, impulse-response source and licence, noise source and licence.

`environments.py` states impulse responses and noise are synthesized in process. Licence strings on those generators are `CC0-1.0`. Names: dry studio, small room, living room, car, cafe/babble, street, phone/narrowband 8 kHz, headset/bluetooth codec. Babble is detuned tones, not recorded speech. The headset path is an in-process filter plus a G.711 μ-law round trip, not a vendor codec. Default render backend in the pipeline module is `stub`.

This is a **generator of labelled synthetic clips**, not a database of indoor PersonaPlex sessions.

### Scorers

`tools/cloning-v2/scorers/persona/py/nebula.py` scores a distribution the caller supplies. It does not read a wav for the nebula field and does not move `emotion_match`. Gates stay WER, ECAPA, and `emotion_match` as documented in `NEBULA-SCORING.md`. `tools/emotion-v2/thresholds.toml` is the threshold file. This note does not edit it.

### Clone references already described in tree

Persona voice notes (not audio) name the current CC0 donors: Amy Koenig for the Samantha register and Alan Davis Drake for the Jarvis register (`docs/galion-voice-v2/personas/voice/samantha.toml` and `jarvis.toml`, owner note t623u). Those clips are gitignored. They are **source speakers**, not the personas, and not the owner’s voice. Retired film performances are not the conditioning set. No owner recording belongs in the routine harness. This pull request does not add the wav files.

### What this inventory is not

Listening-set wav files under `docs/galion-voice-v2/evals/` are eval renders. They are not a PersonaPlex training split. They were not re-uploaded.

## Proposed model (not implemented)

Owner direction: universal voice databases where **Samantha** and **Jarvis** are two subject personas, joined to emotions and to **indoor** environments, for later PersonaPlex-style reinforcement learning.

### Identifiers that must not be collapsed

| Id | Meaning |
|---|---|
| `subject_id` | The legal subject of the recording. For the CC0 donors, the named speaker. For a future owner take, the owner, with a separate consent record. |
| `source_speaker_id` | The voice that was recorded. May match `subject_id`. Never a film character name. |
| `persona_id` | `samantha` or `jarvis` as a **register** the product speaks, not a claim that the subject is that character. |
| `utterance_id` | One take: text, audio hash, duration, sample rate, channels, bit depth. |
| `emotion_id` | One label from the Emotion V2 inventory, plus intensity. Out-of-set names stay absent, not zero. |
| `environment_id` | A real indoor condition once recordings exist. The synthetic names above may be used only when `synthetic_signal` is true. |
| `session_id` | One sitting: room, device, clock, consent id. |
| `provenance` | Who captured it, where the bytes live, hash, licence SPDX. |

A row that sets `persona_id` without `subject_id` and `source_speaker_id` is invalid in this proposal. Setting `persona_id` to `samantha` or `jarvis` is not permission to train on a private wav. Those names are registers. A training byte still needs its own consent record, separate from REMEMBER.

### Indoor environment

When the clip is a real room, the proposal wants: room class (the indoor subset of the synthetic list is a hint, not a measurement), device, sample rate, a noise-floor method, and whether the impulse response was measured or is still the synthetic generator. Outdoor names in the current generator (street, car) are not indoor evidence.

### Splits

Train, validation, and test are **subject-disjoint**. No `subject_id` appears in more than one split. A persona may appear in all three only because different subjects were assigned to that register. Hashing the file is not a split. The same utterance re-encoded is the same `utterance_id`.

### Live affect versus offline labels

Offline: the capture label and any later scorer report. Live: `EmotionWire` and the RMS stub, which does not know valence. A training row must not copy a live stub label into `emotion_id` and call it the take’s emotion. The nebula distribution, when present, is an extra report and does not replace the capture label.

### Consent, REMEMBER, retention

Capture consent (`owner-own-recordings-only` or a CC0 donor consent file beside a gitignored clip) is not the REMEMBER bit. REMEMBER is permission to keep a caller baseline past the session (`galion-emotion` `store.rs`). A dataset row does not set REMEMBER. Withdrawal deletes the bytes and the index row. The store’s drop-wipe is process memory only today, so a dataset store would need its own delete path. That path is not written here.

Security: hashes in git, audio out of git until a cleared release, no weights, no film references, no private owner audio in CI.

### Reward (proposal only)

A later RL step, if the owner asks, would need a reward that is not “match the film voice”. Candidates already named in the re-anchor and the gates: intelligibility (WER), speaker similarity only against the consented source speaker (ECAPA), emotion distance that does not lower the 0.83 gate by itself, and a penalty for policy violations from `check_counterbalance`. None of those rewards have been optimized in a training run in this repository.

### Unverified dependencies

- Mimi weights and licence (`galion-mimi` is a stub in the re-anchor note).
- A GPU host for PersonaPlex (Open Mind: not provisioned).
- S0 student weights (re-anchor: `CannedLlm` until they exist; teacher source still an owner decision).
- Indoor recordings that do not exist in git.
- The unread VPS briefing, which might name a corpus this review has not seen.

Until those exist, the manifest schema above is a document, not a dataset.
