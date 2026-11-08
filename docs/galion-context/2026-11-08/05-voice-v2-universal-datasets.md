# Voice V2 universal datasets — inventory and proposed indoor PersonaPlex join

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Pair 11, PersonaPlex. Date of this inventory: 2026-11-08. This file is documentation only. It does not ingest audio, train a model, download weights, or change `LOCKS.toml`, pin files, or code.

Companion schema (also documentation, not a loader): [`05-voice-v2-universal-datasets.schema.json`](05-voice-v2-universal-datasets.schema.json). Schema id `galion-context.personaplex-indoor-join/proposed-2026-11-08`.

## What is true in this checkout

No universal voice database is populated in git. `datasets/emotion-v2/` is gitignored (`.gitignore`) and is not present as tracked files. `galion-student/README.md` states config stubs only: nothing has been trained and no weights are included. `docs/agent-wave20/A21-COUNSEL-MEMO.md` states the then-current plan as no training, and that the memo was not sent to counsel. `docs/galion-voice-v2/PERSONAPLEX-REANCHOR.md` states training is not feasible on the CPU box and that Fisher English needs counsel before use. `docs/emotion-v2/HANDOFF.md` states the dataset pipeline draft had run with a stub tone/noise generator, not real Samantha training data, and that the draft was not merged in that handoff.

This page does not claim a database was filled, a PersonaPlex or student policy was trained, or a public dataset release was licence-, consent-, and privacy-reviewed. A public release of any clip set stays closed until those three reviews exist as writings. This commit publishes the schema text only.

Host prices, recorded so they are not mistaken for a dataset budget: Plus is $20 per month (`docs/galion-voice-v2/symphony/PLAN-PLUS.md`) and Ultra is $220 per month (`docs/galion-voice-v2/symphony/PLAN-ULTRA.md`). Those are closed host figures. They are not a GPU training purchase and not a licence fee for any corpus below. PersonaPlex-7B RL on the Ultra CPU box is the plan's own negative: the re-anchor says that training is not feasible there.

## Subject persona and source speaker

These are different records. A subject persona is the test identity the product speaks as. A source speaker is the human whose recording, if any, is allowed to condition a clip. Joining them into one name is how a film voice or an owner clip gets treated as the dataset.

| Role | Ids in this proposal | What it is | What it is not |
|---|---|---|---|
| Subject persona | `samantha`, `jarvis` | Two internal test registers. Prose traits in `docs/galion-voice-v2/personas/SAMANTHA.md` and `JARVIS.md`. Piper `en_US-ljspeech-medium` is the speaking fallback and is not either persona. | Not the LibriVox reader. Not the film performer. Not Galion One. |
| Source speaker | `amy-koenig`, `alan-davis-drake` | The only active CC0 / public-domain conditioning pins. See the pin table. | Not the subject persona. A Samantha row may name Amy Koenig as source speaker. It must not say Amy Koenig is Samantha, and it must not say the film performer is the source. |
| Synthetic source | `stub-sine` | The dataset profile in `tools/emotion-v2/dataset/data/stub_profile.json`. `synthetic_signal` stays true. | Not a person and not a consent for a person. |
| Retired / forbidden | t273 commented pin rows; film performances named in `RIGHTS-CLEARANCE.md`; Armstrong; any owner private clip | Kept only as a prohibition. | Not an active pin. Not a harness input. Not an upload. |

`seductive` is a Samantha-only product label in the nebula Demo 13 table. It is not one of the twelve Emotion V2 dataset emotions (`docs/emotion-v2/DATASET-PIPELINE.md`). This proposal does not add it to the training join.

## Active pins

Machine pin file: `scripts/clone/REFS.sha256`. Decision comment in that file: t623u, 2026-11-04. Persona sidecars repeat the same digests: `docs/galion-voice-v2/personas/voice/samantha.toml`, `jarvis.toml`. Narrative: `docs/galion-voice-v2/CLONE-CC0-DONORS.md`.

Only these two rows are uncommented:

| Subject persona | Source speaker | Active sha256 (full digest is in `REFS.sha256`) | Cut on the pin row | Form |
|---|---|---|---|---|
| `samantha` | Amy Koenig, LibriVox public domain, Voice Zero CC0 | `e4bc60c33f697108cf5585dead213da1065979010f5be1d513574893584a4641` | 9.60 s, 44100 Hz, mono, 16-bit | WAV |
| `jarvis` | Alan Davis Drake, LibriVox public domain, Voice Zero CC0 | `60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6` | 10.71 s, 44100 Hz, mono, 16-bit | WAV |

`LOCKS.toml` `[formats.exceptions.v2s14_chatterbox]` is the only `reference_clips` key in that file. It is `true`, `ref_clip_decision = "t623u"`, sunset `2027-02-01`. `ref_clip_scope` names Amy Koenig for samantha and Alan Davis Drake for jarvis, store outside the tree with `consent.txt` and a sha256 sidecar, and forbids celebrity voice, film audio, and Armstrong. The global `[formats]` table has no `reference_clips` key. The comment immediately above the Chatterbox table still says "No reference clips." The key and the scope string are the machine statement; this page does not edit the lock to reconcile the comment.

`CLONE-CC0-DONORS.md` records that `consent.txt` beside each store clip says the grantor is the performer through the LibriVox public-domain dedication, that no signed release exists, that `public_demo` is no, that expiry follows the lock sunset, and that counsel was not consulted. Those files are not in git. This page does not copy them. Use of a CC0 voice still requires the in-memory session checkbox (`galion_consent::Cc0Acceptances`); acceptance is not written to disk.

The WAVs are gitignored. `docs/galion-voice-v2/personas/refs/.gitignore` ignores audio. `scripts/eval/sample-voices/SAMPLE-VOICES.md` says the Voice Zero files are not committed. This inventory does not fetch them.

## Retired film references — no ingest, no upload

`scripts/clone/REFS.sha256` comments the previous t273 rows and says they have no speaker clearance and must not be re-enabled. The commented digests stay in that file for the record. They are not active pins. This page does not repeat a procedure for cutting, fetching, or uploading them.

`docs/galion-voice-v2/RIGHTS-CLEARANCE.md` separates the session register from the film performance. Its consent, film-clip, licence, and public-demo boxes are an unchecked list. The file forbids fetching or storing a film clip, forbids a training set from a film extract, and forbids cloud upload. `docs/galion-voice-v2/VOICE-CLONING.md` is the hold those persona pages point at. Persona prose may describe public character traits. That prose is not a licence and not a clip.

No film audio is an input to the proposed indoor join.

## Owner clips are not the routine harness

Owner t273 in `LEDGER.md`, as the persona pages and `RIGHTS-CLEARANCE.md` describe it, is local reference send of a private wav. It is not `training-data-generation`. The dataset consent gate (`tools/emotion-v2/dataset/emotion_v2_dataset/consent.py`) requires `consent_scope` to include `training-data-generation`. The clone scope `local-zero-shot-conditioning` (`crates/galion-consent`, `SCOPE`) does not authorize that pipeline. `docs/emotion-v2/HANDOFF.md` says own-voice clips stay on the local machine and are not part of the GitHub handoff.

The routine harness for this proposal is the stub synthetic backend, or a future run that has its own training-data consent writing. Owner clips are out of that harness. They are not a split, not a prompt, and not an upload.

## Other named audio that is not an active pin

| Name | Where it is written down | Status for this join |
|---|---|---|
| Armstrong | `LOCKS.toml` scope and notes; `SAMPLE-VOICES.md`; `consent.py` `BLOCKED_VOICE_SUBSTRINGS` | ASR / negative example only where an older note says so. Never a clone reference. A voice id containing `armstrong` is refused by the dataset consent gate. |
| `pdx-voice.wav` | `SAMPLE-VOICES.md`; older latency notes that conditioned it | Not a row in `REFS.sha256`. Not in `ref_clip_scope`. Not an active pin. Not added here. |
| Piper `en_US-ljspeech-medium` | Persona `[fallback]` | The speaking checkpoint when the clone path is absent. Not a subject persona and not a CC0 donor pin. |
| Fisher English | `PERSONAPLEX-REANCHOR.md`; PersonaPlex citations in `formats.py` | Named as PersonaPlex's real-data source. LDC-licensed. Not in this tree. Counsel before any use. Not ingested. |
| LibriSpeech, Common Voice, Emilia-YODAS, teacher self-play, Pocket TTS synthetic users | `galion-student/configs/data.toml` | Stub shard names and a licence allow/deny list. The README says no NC data. The files of those corpora are not in this checkout. Naming a shard is not possession. |

`data.toml` allow list: `CC0`, `CC-BY-4.0`, `CC-BY-SA-4.0`, `MIT`, `Apache-2.0`, `self-generated`. Deny: `CC-BY-NC*`, `NC`, `GPL-data`, `unknown`, plus `meta-seamless-interaction` and `emilia-original-nc`.

## Actual dataset machinery (code and docs, not a filled corpus)

### Emotion V2 pipeline

Path: `tools/emotion-v2/dataset`. Contract: `docs/emotion-v2/DATASET-PIPELINE.md`. Manifest schema string in code: `emotion-v2.manifest.v1` (`manifest.py`).

A run, when someone executes it, is specified to write under `datasets/emotion-v2/<voice_id>/<run_id>/` (gitignored): `manifest.jsonl`, `rejected.jsonl`, `run.json`, `audio/<emotion>/*.wav`, and `personaplex/` exports. Default backend is stub sines and noise. It does not read a reference recording. `--backend chatterbox` refuses in this tree even with consent, because no engine is imported and nothing is downloaded. Tests use a temporary directory.

Twelve emotions, dataset order: Neutral, Calm, Happy, Amused, Excited, Tender, Sad, Disappointed, Angry, Fearful, Surprised, Disgusted. Inventory coordinates live in `emotion-v2/emotions.toml` (`schema = "galion-emotion-v2/1"`, status `design`). That file is a design inventory, not a populated speech corpus.

Manifest fields the code writes, in required-key order, then extras: `audio_path`, `text`, `emotion`, `intensity`, `environment`, `snr_db`, `voice_id`, `scores` (`wer`, `ecapa`, `emotion`, null when the scorer is absent), `sha256`, plus `schema`, sample rate, duration, 432 Hz pitch standard, pitch centre, `render_backend`, `synthetic_signal`, `scenario_id`, impulse and noise source and licence, `telephony_hz`, `element`. Quality thresholds apply only to scores that exist: WER above 0.35, ECAPA below 0.50, emotion match below 0.40. Missing scorers do not reject a line unless `--require-scorers` is set. Default profile `stub_profile.json` is `voice_id` `stub-sine`, synthetic. The only consent file in git under that package is `data/consent.example.json` for `stub-sine`, and the file says it is not a person's consent.

Clip filenames are `{scenario_id}__{environment}__snr-{tag}.wav` (`pipeline.py`).

### Environments (synthetic, not field recordings)

`tools/emotion-v2/dataset/emotion_v2_dataset/environments.py`. Impulse and noise are generated in process. Licence recorded on the generators: `CC0-1.0`. No room library is downloaded. Cafe babble is detuned tones and noise, not recorded speech. Phone is an in-process 300–3400 Hz brickwall and decimation to 8000 Hz, not a carrier recording. Headset is an in-process brickwall plus an ITU-T G.711 mu-law round trip, not a Bluetooth bitstream.

| Environment name | Nominal SNR | Indoor for the proposal below |
|---|---|---|
| dry studio | dry (`snr_db` null) | yes |
| small room | 25 dB | yes |
| living room | 20 dB | yes |
| headset/bluetooth codec | 18 dB | yes (near-field indoor simulation) |
| car | 12 dB | no |
| cafe/babble | 8 dB | no |
| street | 10 dB | no |
| phone/narrowband 8 kHz | 15 dB | no (channel, not a room) |

The indoor column is a proposal in this file. The code does not have an `indoor` flag. Extra SNR levels exist on each environment and are not the default run.

### PersonaPlex export that exists as code

`tools/emotion-v2/dataset/emotion_v2_dataset/exporters.py` and `formats.py`. NVIDIA PersonaPlex has no published fine-tune file format in the citations stored there (README, project page, paper arXiv 2602.06053, Hugging Face card `nvidia/personaplex-7b-v1`, GitHub issue 5). The default adapter follows Kyutai `moshi-finetune`: JSONL `{path, duration}`, 24 kHz stereo, left = rendered line, right = silence, sibling alignments labelled `SPEAKER_MAIN`. Word times are a uniform split of the transcript, not a forced aligner. The community adapter (`SPEAKER_BROKER` / `SPEAKER_CLIENT`, `text_prompt`) is optional and not the default. `prompts.jsonl` is side metadata. Clips are single-speaker emotion lines, not full-duplex conversations. `eval_fraction` defaults to 0, so `eval.jsonl` is empty unless a caller sets a fraction. Hold-out is `sum(clip_id bytes) % 10000` on the filename stem.

That export is a file layout. It is not evidence that PersonaPlex was fine-tuned here.

### Scorers and their licences (present as code or licence notes)

Scores in older reports are measurements of those runs, not a filled universal dataset. A score this checkout did not re-measure stays in the file that recorded it. This page does not promote those numbers into a training claim.

| Piece | Where | Licence note already in tree |
|---|---|---|
| Dataset score slots `wer`, `ecapa`, `emotion` | `manifest.py` | Null when the scorer object is absent. |
| `galion-emotion` | Crate; described in `docs/galion-voice-v2/symphony/EMOTION-BLEND.md` | Rust-first. Shipped detector described there as a loudness stub. `GatedDetector` checks consent before PCM. Default deny. |
| SUPERB four-class head | `emotion-v2/emotions.toml` `[superb]` labels `neu`, `hap`, `ang`, `sad` | Coarse labels. The toml says several Emotion V2 names are not in that set. |
| emotion2vec+ base | `tools/cloning-v2/scorers/universal/licenses/LICENSES.md` | FunASR Model License 1.1. Nine classes. Not renamed into love or warmth. |
| MS-CLAP | same licence note | Code MIT; weights card `ms-pl`. Zero-shot prompts. Noncommercial weights rejected there. |
| EfficientAT mn10_as | `licenses/models-manifest.json` and `LICENSES.md` | MIT. AudioSet 527. Yelling / crying / laughter map is written in that note. Baby-cry class excluded for adult Samantha/Jarvis eval. |
| ECAPA similarity | cloning-v2 scorer tree; CC0 donor note | Eval cosine against a pin. Not a dataset. |
| WER / MOS quality scorers | `tools/cloning-v2/scorers/quality` | Present as tooling. `EMOTION-BLEND.md` marks a fresh WER/ECAPA/MOS landing on a machine without those binaries as blocked. |
| Persona proxies | `tools/cloning-v2/scorers/persona` | Proxies. Not a universal corpus. |

`LICENSES.md` is dated 2026-11-05 in its heading. This page does not re-verify those upstream licences against the network.

### Consent and withdrawal that already exist

| Gate | Scope | Withdrawal / retention already written |
|---|---|---|
| `LOCKS.toml` `[consent]` | Bits `sense`, `adapt`, `remember`, `share`. Default deny. Workplace and education purposes prohibited and not overridable. | `withdraw_wipes_ram_history = true`. |
| `crates/galion-consent` | `local-zero-shot-conditioning` | Withdrawal is a consent state. Clone-gate tests mention written withdrawal. This is not the dataset training scope. |
| Dataset `consent.py` | `training-data-generation`, matching `voice_id`, ISO `signed_date` not in the future, no placeholders | Does not delete a store. Does not implement a withdrawal tombstone. Refuses Armstrong substring. |
| `RIGHTS-CLEARANCE.md` | Film performer consent, if it ever existed | Would require deletion of the clip and local derivatives, no training set, no cloud upload. Boxes unchecked. Not a grant. |
| CC0 session checkbox | In-memory acceptance that the caller agreed to CC0 1.0 | Not a disk record. Not a licence to publish a derivative dataset. Sunset on the Chatterbox exception: 2027-02-01. |

## Proposed: indoor PersonaPlex join for the two subject personas

Status of everything in this section: **proposed**. Not implemented. Not populated. Not a training run.

Purpose: one indoor reinforcement-learning join, later, of subject persona, emotion, and environment, for Samantha and Jarvis as two subject personas. The layout may follow the existing Moshi-shaped export only after the gates below are true. PersonaPlex's own paper mix (Fisher plus Chatterbox-rendered dialogues) is a citation, not a licence to copy Fisher into this tree.

### Join

One proposed row is the tuple:

`(subject_persona, source_speaker, emotion, intensity, environment, snr_db, scenario_id)`

- `subject_persona` is `samantha` or `jarvis` only.
- `source_speaker` is a separate object. Allowed routine values: `stub-sine` (synthetic), or the active pin that matches the persona (`amy-koenig` for samantha, `alan-davis-drake` for jarvis). Cross-pairing is a leakage error, not a feature.
- `emotion` is one of the twelve dataset emotions. Not `seductive`. Not a film label.
- `environment` is one of the four indoor names in the table above.
- `scenario_id` is the split atom. Every environment and SNR of that scenario stays in the same split.

RL rewards, if a later owner decision funds off-box training, stay the programmatic table in `PERSONAPLEX-REANCHOR.md`: barge-in yield, pause handling, turn-taking, backchannel, content, and (only under that page's D1 = B, and only after the consent gate) text/audio agreement and ECAPA to an approved reference. That page already forbids rewards for engagement, retention, session length, dwell, or elicited speech, and forbids a reward that steers the user's affect. This proposal does not add one. It also does not mark milestones M0–M4 done.

### Splits

The current exporter hold-out hashes the augmented filename. The same `scenario_id` can therefore land in both `train.jsonl` and `eval.jsonl` across environments. That is a leakage gap in the existing code, recorded here, not fixed here.

Proposed splits, all grouped by `scenario_id` within one `subject_persona`:

| Split | Proposed rule |
|---|---|
| `train` | Scenarios not in dev or eval. |
| `dev` | Scenario-grouped hold-out used only to choose a checkpoint. |
| `eval` | Scenario-grouped hold-out untouched until a reported number. |
| `reject` | Rows the existing quality gate would drop. No audio required in git. |

A synthetic stub run and a real-source run do not share a split file. `synthetic_signal: true` cannot be relabelled after the fact.

### Provenance (proposed fields)

Each row carries: schema id of this proposal, `render_backend`, `synthetic_signal`, pitch standard 432, pitch centre, impulse source and `CC0-1.0` when the existing generator is used, noise source and licence, `telephony_hz` null on the indoor set, sha256 of wav bytes if a wav exists outside git, pointer to the active pin digest when the source speaker is a CC0 donor, and the PersonaPlex adapter assumptions already emitted by `exporters.py`. Absolute local store paths stay out of git. The proposed schema uses a pin id, not a filesystem path.

### Consent, retention, withdrawal (proposed)

- Stub rows use the existing synthetic example pattern. They are not a person's consent.
- A non-synthetic row needs a `training-data-generation` consent whose `voice_id` matches the profile, stored outside git. The CC0 pin's `consent.txt` as described in `CLONE-CC0-DONORS.md` is a public-domain dedication record for local conditioning, with counsel not consulted and public demo closed. It is not, by itself, this training scope. Do not treat it as one.
- Film grants and owner t273 local-send records do not satisfy the row.
- Retention follows the Chatterbox exception sunset `2027-02-01` unless a later owner writing extends the pin. After sunset, the proposal is that derived clips and embeddings are deleted with the pin, not archived into a public dump.
- Withdrawal: delete the wav, the stereo export, embeddings, and prompt caches for that source speaker; write a tombstone with the pin id, the date, and the scope withdrawn; do not keep the audio "for research." `LOCKS.toml` already wipes RAM consent history on withdraw. The dataset pipeline does not yet do the file deletion. That is a gap.
- Workplace and education purposes stay prohibited by `[consent]`. The join is not an emotion-recognition product for those purposes.

### Leakage controls (proposed)

- Group by `scenario_id` and `subject_persona`, not by augmented filename.
- No owner clip, retired t273 digest, film extract, Armstrong, or pdx in the routine harness.
- Samantha rows do not take Alan Davis Drake as source speaker, and Jarvis rows do not take Amy Koenig.
- Right-channel silence in the current exporter is not a second speaker. A duplex user channel is a different proposal and is not defined here.
- Uniform alignment times are not phonetic supervision.
- Null scores are not passing scores.
- PersonaPlex teacher outputs, if ever used under the A21 P-A note, stay labelled as teacher outputs under the NVIDIA Open Model License questions that memo leaves open. They are not CC0. That memo's sign-off boxes are unchecked.

### Measurement gaps (actual)

These are open. None of them is closed by publishing this page.

- No tracked `datasets/emotion-v2/` run.
- Student configs are stubs. No student weights.
- PersonaPlex fine-tune was not run. NVIDIA's format is unpublished; the Moshi adapter is an assumption list in code.
- Default `eval_fraction` is 0. The filename hash splits a scenario across environments.
- Alignments are uniform. The user channel is silence.
- Scorers are optional. Handoff emotion_match for Samantha is described as below the 0.83 gate. That is not re-measured here.
- Fisher is not licensed in-tree. LibriSpeech, Common Voice, and Emilia-YODAS are names in a stub manifest, not files.
- Emotion V2 capture (`docs/emotion-v2/HANDOFF.md`, draft capture PR named there) is not a populated twelve-emotion human corpus in this checkout.
- CC0 donor ECAPA figures in `CLONE-CC0-DONORS.md` are clone-to-pin measurements from 2026-11-04, not a dataset leaderboard and not an RL reward log.
- Public dataset release: no licence review, no consent review, and no privacy review are recorded as complete for a publishable corpus. Until they are, clips stay out of git and out of upload.
- Counsel questions in `A21-COUNSEL-MEMO.md` and `RIGHTS-CLEARANCE.md` are unanswered in those files.

## What a later implementation would still have to refuse

Ingest or upload of retired film references. Owner clips on the routine harness. Any pin other than the two active CC0 rows. Armstrong. A claim that this schema file is a dataset. A claim that Ultra $220 or Plus $20 pays for training. Edits to `LOCKS.toml` or `REFS.sha256` without a separate owner decision.
