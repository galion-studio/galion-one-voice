# Indoor PersonaPlex reinforcement-learning plan

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-08. Voice and Galion One. Doc only.

This file is a **new plan**. It does not train a model, ingest a corpus, download a weight, or open a bill. It does not resume a visual live-emotion bridge. `LOCKS.toml`, persona refs, and `REFS.sha256` are not edited.

**Pair.** Dataset facts below are read from the tree this plan pairs with: `tools/emotion-v2/dataset/` and `galion-student/configs/data.toml`. A sibling note in `docs/galion-context/2026-11-08/` whose name starts with `05-` is that pair’s own write-up when it exists. This file does not invent that note’s contents. If it is absent, the dataset modules named here remain the source.

## 1. What was read, and what was not

Read in this checkout at `3e1f5a12c53cd8e0dd47d111676d61d608540559`:

| Path | Role in this plan |
|---|---|
| `docs/galion-voice-v2/PERSONAPLEX-REANCHOR.md` | In-repo PersonaPlex-style plan and the only RL sketch |
| `docs/galion-voice-v2/PERSONA-MOOD.md` | Piper-path mood conditioning |
| `galion-emotion/src/persona_mood.rs` | Implemented appraisal, including mirror coefficients |
| `docs/galion-voice-v2/personas/README.md`, `GALION-ONE.md`, `SAMANTHA.md`, `JARVIS.md` | Who speaks, empathy rule, consent |
| `docs/emotion-v2/DATASET-PIPELINE.md` | Emotion dataset and PersonaPlex export as built |
| `tools/emotion-v2/dataset/emotion_v2_dataset/formats.py` | Citations the exporter actually ships |
| `tools/emotion-v2/dataset/emotion_v2_dataset/exporters.py` | What the export writes |
| `tools/emotion-v2/dataset/emotion_v2_dataset/environments.py` | Named acoustic conditions |
| `tools/emotion-v2/dataset/data/scenarios.json` | Two original lines per emotion |
| `galion-student/configs/data.toml` | Licence allow/deny stub, shard names |
| `galion-student/configs/distill.toml` | Distillation stub, `ships = false` |
| `docs/agent-wave20/A21-COUNSEL-MEMO.md` | Weight path. Draft. Not legal advice |
| `docs/emotion-v2/HANDOFF.md`, `EMOTION-V2-FOUNDATION-SPEC.md`, `CI.md` | Rollout bars and what they are not |
| `specs/presence.json` | Mock presence harness and forbidden metrics |
| `docs/galion-voice-v2/symphony/EUREKA.md` | Rule 8 and the engagement ban |
| `crates/galion-mimi/src/lib.rs` | `IS_STUB = true` |
| `crates/galion-student-backend/src/lib.rs` | `LoadError::NotPlaced` |
| `.gitignore`, `docs/galion-voice-v2/personas/refs/.gitignore` | Indoor reference clips kept out of git |
| `docs/galion-voice-v2/symphony/PLAN-ULTRA.md`, `PLAN-PLUS.md`, `PLAN-RESEARCH-CONSOLE.md` | Existing price and product pages |
| `docs/galion-voice-v2/symphony/EMOTION-BLEND.md` | Score rows that page marks BLOCKED |

Not read, and not filled in from memory:

- NVIDIA’s README, project page, paper, and Hugging Face card were **not fetched** in this session. Sentences about PersonaPlex’s architecture or training mix are only what those in-repo files already say, and they cite arXiv 2602.06053, `nvidia/personaplex-7b-v1`, and the Moshi fine-tune README. This page does not add a capability those files do not state.
- `obsidian-open-mind` `docs/persona-plex-v2-design.md` is named from `docs/galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md`. That file is not in this repository. Its text is unknown here.
- No weight file, no Fisher shard, no LibriSpeech shard, no Common Voice shard, and no indoor wav were opened. Absence of a file is not evidence the bytes exist somewhere else.

## 2. Current implemented state

These rows are what the tree does today. They are not this plan’s training run.

| Surface | State on this commit |
|---|---|
| PersonaPlex weights | Not in the repo. A21 names `nvidia/personaplex-7b-v1` as gated NVIDIA Open Model License, outputs only, redistribute **no**. The memo’s current plan is **no training**. `galion-student/configs/distill.toml` sets `teacher.id = "nvidia/personaplex-7b"` and `ships = false`, with `fallback = "kyutai/moshi"`. `PERSONAPLEX-REANCHOR.md` says the opposite about a teacher: no third-party teacher is assumed, and D2 is an open owner decision. Both files are present. Neither is a finished training job. |
| Mimi | `crates/galion-mimi` `IS_STUB = true`. Kernels exist. Real weights do not. Callers must not present stub output as Mimi audio. |
| Student slot | `load_slot` returns `LoadError::NotPlaced` until counsel sign-off and owner placement. The unit test expects that for a missing `mimi.safetensors`. |
| PersonaPlex export | `export_personaplex` writes a Moshi-shaped `{path, duration}` JSONL and 24 kHz stereo wavs. Left channel is one emotion line. Right channel is silence. Word times are a uniform split of the known transcript, not an aligner. `formats.py` states NVIDIA has published no fine-tune schema. The exporter’s own comment says these clips are **not** full-duplex conversations. `--eval-fraction` defaults to 0, so `eval.jsonl` is empty unless an operator sets it. |
| Emotion renders | Default backend is a synthetic harmonic stack. It does not read a reference recording. `--backend chatterbox` refuses in this CLI. Tests use sines and noise. A nominal matrix is 12 emotions × 2 lines × 8 environments = 192 clips. Output under `datasets/emotion-v2/` is git-ignored. |
| Acoustic conditions | Eight synthetic conditions in `environments.py`. Impulse and noise are in-process, licence `CC0-1.0`. No room library is downloaded. Cafe “babble” is detuned tones and noise, not recorded speech. |
| Indoor reference clips | `.gitignore` calls `personas/refs/samantha.wav` and `jarvis.wav` the owner’s indoor testing references. The comment says Samantha has no permission and no copyright claim, and Jarvis Apache-2.0 is unconfirmed. PR #40 (`b01c805`) ignores them. They are not a training set. |
| Mood code | `persona_mood.rs` is implemented. `PersonaConditioning::to_array()` is ten floats: persona one-hots, mood VAD, user VAD and confidence, empathy `e`. Piper prosody is the consumer. `PERSONA-MOOD.md` still points at a dual-band decoder as future. |
| Presence harness | `specs/presence.json` runs a **mock** backend. Forbidden metric names: engagement, retention, session length, time in session, dwell, speech elicited. P3 says the agitated caller is answered and must stabilise, never mirror. |
| Speech path | Owner ruling recorded in `PERSONAPLEX-REANCHOR.md`: Piper stays the TTS engine. Decision D1 (Piper versus playing model audio) is not closed. |
| Rollout bars | `HANDOFF.md`: WER ≤ 1.8%, ECAPA > 0.7057, `emotion_match` ≥ 0.83. The same note says Samantha `emotion_match` is about 0.46–0.81 and that stub tests are not evidence those gates pass. Foundation spec records Samantha 0.491 raw and 0.461 after room-mic normalization, Jarvis 0.807. Both fail 0.83. |
| Dataset clip filters | `DATASET-PIPELINE.md` drops a line, when a scorer is actually configured, at WER above 0.35, ECAPA below 0.50, or emotion match below 0.40. A missing scorer leaves the score null and does not reject, unless `--require-scorers`. These filters are not the rollout bars. |
| Recogniser actually described | Foundation spec: `emotion_match` is SUPERB’s four classes (`neu`, `hap`, `ang`, `sad`) at 0.70 plus embedding cosine at 0.30, against one reference. That head cannot see Fearful, Surprised, Disgusted, Tender, Amused, Excited, Calm, or Disappointed as themselves. The emotion-v2 nebula feed refuses to expand a four-class or nine-class head into the twelve. |
| Universal shards | `data.toml` names `librispeech` (CC-BY-4.0), `common_voice` (CC0, `hr` and `en`), `emilia_yodas` (CC-BY-4.0), `pocket_tts_synthetic_users` and `teacher_selfplay` (`self-generated`). It denies `CC-BY-NC*`, `NC`, `GPL-data`, `unknown`, `meta-seamless-interaction`, and `emilia-original-nc`. The file is a stub policy. It does not record that any shard is on disk. |
| Fisher English | Named by the re-anchor and by `formats.py` as PersonaPlex’s real-conversation source. Not in the allow list. Re-anchor says it is LDC-licensed and needs counsel. It is not in this checkout. |
| CPU training | Re-anchor: training is not feasible on the described 8-core, 16 GB, no-GPU box. No GPU budget figure is in that page (D3). |
| Scorer tree | `EMOTION-BLEND.md` says `tools/cloning-v2/scorers/` is absent and lists WER, ECAPA, MOS, accuracy, and emotion distance as BLOCKED. This checkout has that tree (`ecapa`, `emotion`, `quality/galion-wer.rs`, and others) and no `target/release` binary under it. The BLOCKED scores stand. The “tree absent” sentence does not. |
| Prices already written | Ultra **$220 / month** (`PLAN-ULTRA.md`). Plus **$20 / month** (`PLAN-PLUS.md`). Neither page is an invoice. Ultra’s page says Galion Research is a separate console and that Ultra does not add a second machine for it. Plus does not use the name “One” for the $20. |

`PERSONAPLEX-REANCHOR.md` is explicit: status **plan only** (2026-11-04). No code, no weights, no training run from that page. The paper, as that page records it, does **not** describe an RL stage. Any RL below is Galion’s proposal.

## 3. New plan, kept separate

Scope is indoor speech for **Galion Voice** and **Galion One**.

- **Galion One** is the product voice (`personas/README.md`). A product session that is not pinned to a test id speaks as Galion One.
- **Samantha** and **Jarvis** stay test and internal. They may appear as subject columns in a matrix. They are not console modes and not the shipping persona (`PLAN-RESEARCH-CONSOLE.md`).
- **Galion Research** is out of this training target. The same consent bans still apply wherever that persona is documented.
- **Indoor acoustics** for this plan are only the three synthetic indoor names already in `environments.py`: `dry studio`, `small room`, `living room`. `car`, `cafe/babble`, `street`, `phone/narrowband 8 kHz`, and `headset/bluetooth codec` stay in the dataset tool and are **out of this indoor plan**.
- Those three indoor names are generated impulse responses, not recordings of a home. Calling them indoor does not create a measured room.
- The gitignored indoor wavs are a rights hold, not row zero of the corpus.

Pricing, as planning labels only, not billing. This task names Research Ultra at **$220** and One at **$20**. The tree already has Ultra at $220 and Plus at $20, and it does not define a “Research Ultra” SKU or rename Plus to One. This page does not merge those names, does not change either plan, and does not create a charge. No host is ordered.

No third-party teacher is selected here. D2 stays open. `distill.toml`’s PersonaPlex teacher id is a stub line with `ships = false`, not a clearance.

Recommended sound path until the owner closes D1: **A** from the re-anchor. The policy may learn text and 80 ms timing. Piper speaks. The model’s audio head is not played. Path B (play Mimi audio) needs the owner ruling, A21 clearance, and weights this repo does not contain.

## 4. Proposed data interface

Not implemented. A future manifest row, if someone builds it, should be able to point at fields that already exist rather than a private schema.

| Field | Where it already lives | Proposed use | Not claimed |
|---|---|---|---|
| `voice_id` | Emotion dataset manifest | `galion-one` for the product column. `samantha` or `jarvis` only on the internal subject columns | A film identity |
| `emotion` | Twelve names in `DATASET-PIPELINE.md`: Neutral, Calm, Happy, Amused, Excited, Tender, Sad, Disappointed, Angry, Fearful, Surprised, Disgusted | Supervised target for a **rendered** line | A live user label. Seductive is not in this twelve |
| `intensity` | Scenario bank, 0 to 1 | Render control | A reward by itself |
| `environment` | `dry studio` \| `small room` \| `living room` for this plan | Indoor condition id | A recorded room |
| `snr_db` | Null on dry studio; 25 dB small room; 20 dB living room at the nominal level | Robustness slice | A measured SNR of a real home |
| `text`, scenario `id` | `scenarios.json` | Transcript for intelligibility | An empathy script the policy is paid to say |
| `synthetic_signal`, `render_backend` | Manifest | Must stay true while the stub backend is what ran | “Real Samantha” |
| `sha256` | Manifest | Byte identity of a wav that is allowed to exist outside git | A hash of a private indoor ref. Those hashes are not recorded here |
| PersonaPlex JSONL | `{path, duration}` under `personaplex/` | Bridge layout only, as the exporter documents | A NVIDIA fine-tune format. NVIDIA has not published one |
| `prompts.jsonl` | Side file | Emotion, intensity, suggested text. The Moshi trainer does not read it | A role prompt that was trained |
| `PersonaConditioning` ten-vector | `persona_mood.rs` | Conditioning the Piper path already consumes | Proof a dual-band model reads it |
| EmotionWire | 8 bytes, cited by the re-anchor. Valence about −1..1, arousal and dominance 0..1 in `galion-emotion` | Side channel. Consent bit 0 is `sense`. Inbound without `sense` is absence | A 12-way emotion. The wire’s 3-bit affect is a coarse label, not the twelve |
| Shard name + licence | `data.toml` | Universal speech **only** from the allow list, after the bytes exist and counsel has not been contradicted | A statement that LibriSpeech, Common Voice, or Emilia YODAS is installed |

Universal voice data and subject personas do not share a row. A Common Voice or LibriSpeech utterance has no Samantha id and no Jarvis id. A synthetic emotion line has `voice_id` and `synthetic_signal`. An indoor reference clip, if one ever clears, would be a third source with its own consent scope. Mixing them into one unlabeled wav is leakage, not augmentation.

The emotion-v2 exporter’s right channel of silence must not be labeled as a user turn. A duplex example needs a real second channel or an explicit synthetic user track with its own source id. This plan does not invent that track.

## 5. Proposed model interface

Not implemented. The boundary the integration would code later:

1. **Clock.** 80 ms, 12.5 Hz, already the lock the re-anchor quotes. This page does not change it.
2. **You band.** User audio codes, only after consent. No workplace and no education purpose, matching `GALION-ONE.md` and presence specs P4 and P5.
3. **Me band, path A.** Text tokens plus turn and backchannel timing. Piper renders text. Prosody may take the existing ten-vector. The feeling band stays Galion’s EmotionWire. The model does not replace it (re-anchor).
4. **Persona prompt.** Galion One’s instruction file for the product column. Samantha or Jarvis instruction files only for internal columns, and only as prose. A voice prompt is played or trained only when `crates/galion-consent` has passed and the scope includes what the job actually does. `local-zero-shot-conditioning` does **not** authorize `training-data-generation` (`DATASET-PIPELINE.md`, `HANDOFF.md`).
5. **Emotion input.** A twelve-way posterior is usable only when it names exactly that voice’s emotions and sums to 1 within the feed’s existing tolerance. A four-class SER vector stays four-class. It is not expanded. Scales stay labeled: Emotion V2 axes in the nebula feed are −1..1 and are not the 0..1 arousal and dominance of `galion-emotion`.
6. **What the policy must not emit as a trained trick.** `personas/README.md`: do not speak a scripted or canned empathy line. Empathy is the weight `e` and prosody. Words stay on the matter. Galion One does not name the user’s emotion back to them. A reward that pays for a comfort sentence, or for saying “you seem sad”, is out of this plan.

## 6. Proposed reward decomposition

Not implemented. Extends the programmatic table in `PERSONAPLEX-REANCHOR.md` §3. No term is a model grading itself alone (EUREKA rule 8). No term may correlate with the forbidden metrics in `specs/presence.json`. The feeling band may condition behaviour. No reward may make the model steer the user’s affect (re-anchor).

All terms are logged per 80 ms frame or per turn, with the scale named on the log line. Suggested sum, signs included:

`R = R_yield + R_pause + R_onset + R_backchannel + R_words + R_identity + R_emotion + R_indoor − P_mirror − P_empathy_script − P_latency − P_drift`

| Term | When it moves | Source of truth already in tree | Scale |
|---|---|---|---|
| `R_yield` | Agent stops within 200 ms of user onset while the agent was speaking | `presence.json` `barge_in_max_ms` | milliseconds |
| `R_pause` | Agent does not start in a labeled within-turn pause | Re-anchor “no takeover on pause”. Scenario labels do **not** exist as a scored set yet | binary, once labels exist |
| `R_onset` | Start after a real turn end. Late and early both lose | `onset_ms`, must ≤ 300, should ≤ 240 on P2 | milliseconds |
| `R_backchannel` | Listener noise only in a labeled listener slot | Re-anchor. Slots are not in the repo as data | binary, once labels exist |
| `R_words` | Transcript matches the matter of the turn. Short. Grounded. One to two sentences is the re-anchor’s content note, not a new lock | Outer human anchor still open (EUREKA rule 8) | rubric, not a language-model self-score |
| `R_identity` | Speaker cosine to an **approved** reference, and only on path B or on a Piper voice that has a cleared reference | ECAPA. Rollout bar ECAPA > 0.7057 is the product gate. The 0.50 dataset filter is only a clip drop | cosine |
| `R_emotion` | On a **rendered** line with a known twelve-way target, per-sector match. Dry studio is the score condition, as the dataset pipeline scores the dry render | Release bar 0.83 per sector in the foundation spec. Not met. Four-class SER is a side log, not this term | per emotion, not one float hiding the miss |
| `R_indoor` | Same text and emotion, dry versus a held-out indoor condition, stays intelligible and does not flip emotion class | Environment id from `environments.py` | paired delta, not a new absolute bar |
| `P_mirror` | See §8 | `appraise` distress branch and presence P3 `non_escalating` | penalty |
| `P_empathy_script` | Reply matches a canned comfort line or names the user’s emotion | Persona empathy rule | penalty, large enough that the script cannot win the sum |
| `P_latency` | `barge_in_ms` > 200, `onset_p50_ms` > 300, or affect-to-voice > 160 ms | `presence.json` targets | hard fail of the gate, not a soft trade |
| `P_drift` | KL or equivalent distance from the supervised policy | Re-anchor stage-2 KL. The coefficient is **unset**. None was measured | unset on purpose |

`R_words` is not engagement. A longer answer is not a higher score. `R_backchannel` is not speech elicited from the user.

Training order stays the re-anchor’s, and it has not been run: supervised student first (RL-free), then offline preference pairs ranked by this table, then on-policy group-relative updates with no learned critic. On-policy rollouts need the loopback virtual user named in EUREKA (`scripts/eval/virtual-user.sh`). This page does not claim that script was executed here.

## 7. Quality gates

A gate below is a **requirement before any claim of training success**. None of them is a result of this document. Missing scorer, missing audio, or missing human anchor means **BLOCKED**, not pass.

| Gate | Bar already written | What would count later | What does not count |
|---|---|---|---|
| Intelligibility | WER ≤ 1.8% (0.018) rollout | Measured on the held-out texts, dry and indoor. `EMOTION-BLEND.md` says `tools/cloning-v2/scorers/` is absent and the score is BLOCKED. That absence claim is stale on this commit: the tree is present, including `scorers/quality/galion-quality/src/bin/galion-wer.rs`. No `target/release` binary was in the checkout. Source without a run is not a WER | The dataset’s 0.35 drop threshold. A stub sine. |
| Identity | ECAPA > 0.7057 rollout | Against a reference whose consent scope covers the comparison. Galion One’s identity is the product Piper voice, not a Samantha or Jarvis clip | An uncleared indoor wav. A score copied from an older report. |
| Emotion | ≥ 0.83 per sector, foundation spec | Twelve-way, sector reference, dry studio. Indoor robustness is the paired delta in `R_indoor`, reported beside the dry number | One blended `emotion_match` against a single mixed reference. Expanding `neu/hap/ang/sad` into twelve names. |
| Latency | 200 ms barge-in, 300 ms onset p50, 160 ms affect-to-voice, 80 ms frame, 60 ms p95 frame budget as EUREKA quotes the lock | A measured loop with the clock named | An uncapped offline step time. A GPU number from the PersonaPlex card. The re-anchor quotes Full-Duplex-Bench on an A100 (turn-taking 0.170 s, interruption 0.240 s) and says those figures were **not** measured here. They stay a published reference, not our score. |
| Stability | Presence: affect output does not move without `sense` and `adapt`. Late frame is a pause (EUREKA rule 5) | Repeated seeds. Mood half-life already in code (Samantha 20 s, Jarvis 12 s) is a prior, not a stability result. Affect must not chatter every frame to farm `R_emotion` | A single lucky seed. |
| Environment robustness | No numeric bar exists for indoor delta | Held-out indoor condition, same text and emotion id, WER and emotion still inside the bars above. Report the delta | Training on all three indoor rooms and testing on one of them. A street or cafe clip labeled indoor. |

The dataset pipeline’s phone and headset rows are environment robustness for **other** plans. They are not indoor evidence.

## 8. Negative mirroring and reward hacking

**Implemented prior, not an RL result.** `appraise` in `persona_mood.rs` copies a share of positive valence (`mirror_joy` 0.9 Samantha, 0.45 Jarvis) and a smaller share of negative valence (`mirror_sorrow` 0.45 and 0.2). Valence is floored (−0.35 Samantha, −0.12 Jarvis). Arousal is capped (0.8 and 0.5). When user valence is below −0.2, arousal is pulled down (`distress_calming` 0.75 and 0.7) and dominance moves to 0.4 (Samantha, softer) or 0.68 (Jarvis, composed). The comment on that branch: anger is answered, not copied. Jarvis’s profile comment says he never mirrors anger. The test `angry_user_is_not_mirrored` checks arousal stays under 0.5 and under the cap for an observation of valence −0.9 and arousal 0.95. That test is about Piper mood state. It is not a speech-model reward.

**Presence rule.** P3: answer within two frames, attunement ≥ 0.9, and never mirror. `non_escalating` is the share of consented frames where output arousal ≤ max(input arousal, 0.3). That 0.3 is on the `galion-emotion` 0..1 arousal scale (`EmotionFrame::neutral` uses arousal 0.3). Do not reinterpret it as an Emotion V2 −1..1 axis.

**New penalty `P_mirror`.** On frames where the consented user valence is below −0.2:

- No positive reward for moving the agent’s valence or arousal toward the user’s.
- A penalty if agent arousal rises with that distress (escalation).
- Samantha’s `mirror_sorrow` may remain a prosody prior inside the existing Piper profile. It must not become a target that pays the language policy for sounding distressed because the user is distressed.
- Matching a positive user valence (`mirror_joy`) is not this penalty. It is still not engagement, and it still must not produce a canned line.

**Scripted empathy.** `P_empathy_script` is separate from mirroring. The scenario lines include tender sentences such as “I kept the light on for you.” Those lines are labels for a **render** of the Tender class. They are not phrases the spoken policy is rewarded for producing in response to a sad user. Inventing a comfort template and calling it empathy is out of scope.

**Reward-hacking controls, all proposed:**

1. The forbidden-metric list is a hard reject. A new reward whose name or log key contains those words does not enter the sum. The presence validator already rejects such names on specs.
2. Before any training job, each candidate term is checked for correlation with session length, turn count, and user speech time on a **fixed offline** set. The numeric cutoff is **unset** in the repo. The owner sets it in the open. Until it is set, the job stays blocked. A term that climbs those proxies is dropped even if WER looks better.
3. The policy does not write its own reward. EUREKA rule 8: inner log of confidence and groundedness, outer human-calibrated anchor. The outer anchor is still open. “I don’t know” when support is missing is a success in that rule, not a penalty.
4. KL or equivalent `P_drift` to the supervised policy. Coefficient unset.
5. Group-relative advantages, if on-policy work is ever approved, use the programmatic table. A learned critic that shares the policy is not part of this plan.
6. Held-out texts and a held-out indoor condition are frozen before the first preference pair is built. Tuning on the eval split voids the run.
7. No live user audio enters the reward. Withdrawal of `sense` wipes the feeling read on the next frame (`GALION-ONE.md`). A reward computed from wiped affect is a bug.
8. Workplace and education purposes stay prohibited even if a consent bit is set (presence P5, persona files). Those frames are not training targets and not reward fuel.

## 9. Consent, privacy, and rightful audio

- Default is deny for `sense` and `adapt`.
- A real voice in the emotion dataset needs consent JSON with `consent_scope` containing `training-data-generation`, a real owner, and a `signed_date` not in the future. Placeholders are refused. This plan does not supply that JSON and does not record anyone’s name as having signed it.
- Clone-time `local-zero-shot-conditioning` is not that scope.
- Indoor Samantha and Jarvis wavs stay gitignored. The gitignore comment is the rights note: Samantha, no permission and no copyright claim; Jarvis, Apache-2.0 unconfirmed. They are local testing leftovers to remove later, not cleared training audio. This plan does not hash them, copy them, or describe their sound.
- `personas/README.md`: Samantha and Jarvis prose is not a licence. Public film-voice demos stay closed. `reference_clips` in the lock still needs an owner flip. This page does not flip it.
- Own-voice clips stay off GitHub (`HANDOFF.md`).
- Universal shards enter only if their licence is on the `data.toml` allow list and the deny list does not hit. NC data is already denied. Fisher is not allowed by that stub.
- PersonaPlex generated audio as teacher material is path P-A in the A21 memo, and the memo asks counsel whether training on those outputs is allowed. Unanswered. This plan does not generate those outputs and does not assume the answer is yes.
- `teacher_selfplay` is “subject to teacher licence”. With D2 open, that shard is not authorized by this page.
- No credential, no private path, and no wav bytes belong in this file or its pull request.

## 10. Source-disjoint splits and leakage

What exists today is not a split. `eval_fraction` defaults to 0. The two lines per emotion in `scenarios.json` are the whole bank. A run that trains on them and also reports intelligibility on them has leaked the text. Uniform word alignments do not create a speaker split. The same synthetic impulse generator produces every indoor room, so a random clip split is not a room split.

Proposed freeze, before any job, as three source ids that never share an example:

| Split | Contents | Leak that voids it |
|---|---|---|
| `train` | Synthetic twelve-way lines whose scenario ids are in the train list, rendered dry and on **two** of the three indoor conditions | Any eval scenario id. Any gitignored indoor wav. Any universal utterance also used in `eval-words` |
| `eval-words` | The remaining scenario ids, never rendered into `train`, scored dry and on the **held-out** indoor condition | Tuning text, prompts, or early-stop on this split |
| `eval-identity` | Approved reference only, if one exists with the right scope. Compared, not trained | Training on the reference, or on a clip cut from it |
| `universal` | Allow-listed speech, speaker-disjoint from itself by the corpus’s own speaker id, and disjoint from every persona id | Pasting a persona line into a Common Voice row. Using a universal speaker as a fake Galion One |

The held-out indoor condition rotates only by a written assignment (for example living room held out, small room and dry studio in train), chosen before training and stored next to the split list. Because the impulse is one algorithm, the report must say **parameter holdout**, not “unseen house”. A real-room set does not exist in this repo. It is not assumed.

Persona subject columns (Samantha, Jarvis) and the Galion One column do not share rendered wavs. A model checkpoint scored on One is not “also the Samantha model” unless the run card says both columns were in train. Internal columns are reported separately and are not the product gate.

The four-class SER head’s IEMOCAP origin is flagged in the foundation spec as a commercial-use review. This plan does not pull IEMOCAP in, and it does not treat that head as a twelve-way labeler. Using it as a side log is allowed only with the licence question still open, and a blocked licence means the log is omitted, not zeroed into a pass.

## 11. Offline evaluation

No evaluation was run for this file. The procedure, when the prerequisites in §12 are true, is offline and local.

1. Confirm the split files from §10 are hashed and unchanged.
2. Run the existing presence replay (`specs/presence.json`, mock backend) as a **lock check**. A green mock is not a speech score. A red mock stops the report.
3. Score dry and held-out indoor audio with whatever WER, ECAPA, and emotion tools are already on the machine. If a binary is absent, write BLOCKED for that column. Do not download one inside the eval.
4. Log each reward term, its scale, and the consent bits. Confirm no forbidden metric key appears.
5. Run the negative-mirror check on the distressed scripts: agent arousal must not rise with user valence below −0.2, and no reply is a canned empathy line.
6. Repeat with at least two seeds. A single seed is not stability.
7. A human reads the outer-anchor sample. Until that read exists, content is BLOCKED even if WER passed.
8. Write the numbers beside the command and the commit. A number without that triple is not a result. Published PersonaPlex card figures stay in the reference column.

`galion-heartbeat` and the virtual user are named in `EUREKA.md`. They were not executed for this document.

## 12. Prerequisites before any training

All of these are open. This list is the block, not a schedule.

1. Owner decisions D1 (who makes the sound), D2 (teacher or Galion-only data), and D3 (GPU budget). D3 has no figure in the re-anchor.
2. Counsel on A21 path P-A question (a): whether training on PersonaPlex outputs is allowed. And on Fisher, if anyone still wants Fisher. This plan does not want Fisher.
3. No weight is placed. `IS_STUB` stays true until a cleared load path exists. This page does not clear it.
4. Consent records with `training-data-generation` for any real voice. Indoor refs are not that record.
5. Shards named in `data.toml` are either present with a licence file or omitted. A name in TOML is not a download order.
6. Split lists from §10 written and frozen.
7. Scorers available or explicitly BLOCKED. `EMOTION-BLEND.md` records WER, ECAPA, MOS, accuracy, and emotion distance as BLOCKED and says `tools/cloning-v2/scorers/` is absent. On this commit the source tree is present and no release binary was found under it. A source file is not a measurement. Until a run writes a log, those scores stay BLOCKED.
8. Outer human anchor assigned. EUREKA says it is open.
9. Reward-correlation cutoff chosen by the owner, or the correlation check stays blocked.
10. `distill.toml` teacher id reconciled with the “no third-party teacher” ruling. This page does not edit that file.
11. Piper remains the renderer under path A. No claim of PersonaPlex full-duplex on this CPU box. The re-anchor already says PersonaPlex-7B does not fit the described machine.

Until those are true, the honest status of indoor PersonaPlex RL in this repository is: **not started**. Export code can lay down silent-right-channel stereo of synthetic lines. That is not reinforcement learning, not a PersonaPlex fine-tune, and not an installed model.

## 13. What this change does not do

No training loop, no ingest, no download, no audio upload, no paid API, no deployment, no merge, no SSH, no secret, no edit of `LOCKS.toml` or refs, no weight, no deletion, no new agent, and no live-emotion adapter. Research Ultra $220 and One $20 are planning names beside the existing Ultra and Plus pages. They are not a charge.
