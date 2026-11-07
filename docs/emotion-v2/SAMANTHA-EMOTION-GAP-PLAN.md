# Samantha emotion gap plan

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 7 November 2026. Draft proposal. Docs only.

This file plans how Samantha’s persona `emotion_match` can reach the existing **0.83** gate. It does not merge, deploy, retune, record, or train. It does not edit `LOCKS.toml`, `SOUL.md`, `config.yaml`, a persona TOML, `tools/emotion-v2/thresholds.toml`, `docs/emotion-v2/capture-spec.toml`, or PR #86 (`swarm/symphony-b-expression-20261106`). It does not change a number that gates CI.

The research note is [`EMOTION-NEBULA-RESEARCH-20261107.md`](EMOTION-NEBULA-RESEARCH-20261107.md). The short account is in [Research basis](#research-basis).

## Emotion is a nebula

A line of Samantha is a **nebula**: a mixed cloud of particles — warmth, love, anger, calm, excitement, unrest, and the rest — spread across layers (words, prosody, voice quality), situations, the persona, and delivery style. It is not one solid label.

**Nebula is a metaphor.** It is not an anatomical claim, and it is not evidence that emotion is a sum of particles. The twelve Emotion V2 names are a project inventory. Listener disagreement is not, by itself, proof that one person heard a blend. Warmth and love stay in the account. Targets are not moved to clear 0.83.

Four places in this plan use that picture:

1. The laptop warmth check reports the **blend** (each particle’s intensity or posterior, and a mixture distance). A single love decision is one label on that evidence.
2. Controlled references keep one anchor per emotion and also keep **deliberate blends**, with the intended mix written as a target vector.
3. A later rescore compares a render and a reference as **distributions**. The 0.83 gate stays where it is.
4. The same cloud is the **live** emotion pipeline that influences Samantha and Jarvis while they speak, and that a separate visualiser draws. It is not only a training-set format. See [Nebula as the live emotion pipeline](#nebula-as-the-live-emotion-pipeline).

## Which number this plan is about

The number to move is the persona scorer’s `emotion_match` on Samantha’s clone sets. The formula in `tools/cloning-v2/scorers/persona/py/persona_lib.py`, repeated by PR #86, is:

```text
score_C = 0.55 * max_i match_i + 0.45 * mean_i match_i
category_blend = Σ (strength_C * score_C) / Σ strength_C
emotion_match = 0.70 * category_blend + 0.30 * emb_sim
```

The gate is **≥ 0.83**. Presence is **0.20**. An emotion outside the persona is `n/a`. It is not entered as 0. An in-set emotion with no original moment at or above presence is `missing`. It is not entered as 0.

Samantha’s judged set, from `tools/cloning-v2/scorer-probe/results-persona/COMPARISON.json`: `joyful`, `aggressive`, `love`, `laughter`, `logic`, `neutral`, `seductive`. Crying and yelling are excluded. Jarvis is judged on `neutral` and `logic` only. His merge-v2 `emotion_match` is **0.8730685436870271** and his coherent result is **0.8791185805790467**. Both pass. This plan does not retune him.

Published Samantha gate, same file (new persona scorer, 6 November 2026):

| Set | emotion_match | category_blend | emb_sim | Gate |
|---|---:|---:|---:|---|
| ceiling | 0.8650308818595598 | 0.8218196099645128 | 0.9658571829480033 | pass |
| merge-v2 | 0.6869450087862239 | 0.6135572718172585 | 0.8581830617138099 | fail |
| coherent | 0.6713695158531341 | 0.6067343694048821 | 0.822184857565722 | fail |

Gap of the failing sets to 0.83, by subtraction of those published values: merge-v2 **0.1430549912137761**, coherent **0.1586304841468659**.

Two other series live in the repo. They are different measurements. They are not averaged into the table above.

- Single-reference laptop scorer, `docs/emotion-v2/EMOTION-V2-FOUNDATION-SPEC.md` §7: Samantha **0.491** raw and **0.461** after room-mic normalization. Same 0.70 / 0.30 shape, compared with one mixed reference, not with the persona moments.
- Emotion V2 harness, `tools/emotion-v2/thresholds.toml`: posterior cosine against **that emotion’s** reference, target 0.83 on each of the twelve. No Samantha render has been scored that way in this repo. The result is **unknown**.

`docs/emotion-v2/HANDOFF.md` says Samantha’s `emotion_match` remains about 0.46–0.81. That sentence spans more than one series.

## Per-category gap

Source of every strength and score: `COMPARISON.json`, sets `merge-v2` and `coherent`, field `per_emotion`, `new_score` and `new_strength`. The same rows are copied in PR #86’s `tools/cloning-v2/pipeline/expression/fixtures/samantha_love_gate.json`.

`share` and `em_points_below_1` are **recomputed** from those published columns. They are not a new measurement.

```text
share = strength / Σ strength
em_points_below_1 = 0.70 * share * (1 - score)
```

Σ strength on both failing sets is the original-moment strengths of the five rows that entered the blend: **3.7121234978118727**. Joyful and seductive did not enter.

### merge-v2

| Category | Status | Strength | Score | Share (recomputed) | emotion_match points below a score of 1 (recomputed) |
|---|---|---:|---:|---:|---:|
| neutral | scored | 1.0 | 0.9689200045253754 | 0.269388 | 0.005861 |
| logic | scored | 0.49966762061927383 | 0.9247894142895605 | 0.134604 | 0.007087 |
| laughter | scored | 0.2743651270866394 | 0.8542200412601233 | 0.073911 | 0.007542 |
| aggressive | scored | 0.9990363121032715 | 0.5555290011531206 | 0.269128 | 0.083734 |
| love | scored | 0.9390544380026881 | 0.06094556199731194 | 0.252970 | 0.166287 |
| joyful | missing | — | — | — | unknown |
| seductive | missing | — | — | — | unknown |
| yelling | n/a | n/a | n/a | — | not in the blend |

Best lines in that file: neutral `t12`, logic `t08`, laughter `t09`, aggressive `t07` (best match 0.5742796720554101), love `t01` (best match 0.06094556199731194).

### coherent

| Category | Status | Strength | Score | Share (recomputed) | emotion_match points below a score of 1 (recomputed) |
|---|---|---:|---:|---:|---:|
| neutral | scored | 1.0 | 0.9646446600659391 | 0.269388 | 0.006667 |
| laughter | scored | 0.2743651270866394 | 0.9248757898807527 | 0.073911 | 0.003887 |
| logic | scored | 0.49966762061927383 | 0.8972519308363274 | 0.134604 | 0.009681 |
| aggressive | scored | 0.9990363121032715 | 0.5288252705675534 | 0.269128 | 0.088764 |
| love | scored | 0.9390544380026881 | 0.06094556199731194 | 0.252970 | 0.166287 |
| joyful | missing | — | — | — | unknown |
| seductive | missing | — | — | — | unknown |
| yelling | n/a | n/a | n/a | — | not in the blend |

Best lines: neutral `t03`, laughter `t11`, logic `t07`, aggressive `t07` (best match 0.5288265675646632), love `t01`.

Love’s strength is the original window’s decision, **0.9390544380026881**. Both clones’ love decision is **0**. The match `1 - |0 - 0.9390544380026881|` is **0.06094556199731194**. That zero is what the love plugin writes when the mean cosine against the love prompt bank is at or below **0.20**. The committed w03 measurement is love cosine **0.27918557822704315**, neutral cosine **0.15484974160790443**, margin **0.12433583661913872**. Clone prompt-bank cosines are not in the repo.

The same w03 moment has a second published decision, **0.9228583200845187** (the window file). `own-samantha.toml` still holds the legacy universal-CLAP love target **0.8109382317899566**. That file is not edited. Swapping only the love *strength* to either of those two figures, which `COMPARISON.json` already did, leaves merge-v2 at 0.7007728157652356 or 0.6886401489896614 and coherent at 0.6850265957629967 or 0.6730437267567072. All four still fail. The score, not the strength knob, is the love gap.

### What a perfect love row does, and what dropping it does

PR #86 `attribute_love` recomputes the blend with the published rows. Love’s score is set to **1.0**, or the love row is removed. `emb_sim` stays the published value. Those printed results:

| Set | Published emotion_match | If love score is 1 | If the love row is removed |
|---|---:|---:|---:|
| merge-v2 | 0.6869450087862239 | 0.8532315740464144 pass | 0.8179380447274447 fail |
| coherent | 0.6713695158531341 | 0.8376560811133247 pass | 0.8007452268534501 fail |

A perfect love match clears both sets. Removing love does not. Aggressive is the next heavy row (strength about 0.999, scores 0.5555290011531206 and 0.5288252705675534).

Further recomputations from the same rows and the same formula, not new measurements:

| Change, other published rows held | merge-v2 | coherent | Gate |
|---|---:|---:|---|
| aggressive score set to 1, love left at 0.06094556199731194 | 0.7706787173901027 | 0.7601339292753010 | both fail |
| emb_sim set to 1, category rows left as published | 0.7294900902720809 | 0.7247140585834175 | both fail |
| love score set to the ceiling set’s own love score, 0.6302473150364416 | 0.7877562389752144 | 0.7721807460421245 | both fail |
| love score set to 1 and aggressive score set to the ceiling aggressive score, 0.8202779419639838 | 0.9031075180800080 | 0.8925627299652062 | both pass |

Love score that would clear 0.83 with every other published row held: **0.8688065227883068** on merge-v2, **0.9567645351509508** on coherent. Aggressive cannot clear 0.83 by itself: the score that would be required is above 1 (1.3148862050196799 and 1.3708595111607793).

The ceiling set already passes with a love score of **0.6302473150364416**, not 1. Matching that ceiling love score on the clones is not enough, because the clones’ aggressive scores sit below the ceiling aggressive score and their `emb_sim` is lower.

### Rows that are not in the blend

Joyful and seductive are in the judged set and are `missing` on the ceiling, merge-v2, and coherent windows. Their contribution to these three `emotion_match` figures is unknown. They are not zeros.

The probe block in the same JSON is a different set: original-voice synthesized lines, not the clone gate. Samantha probe, new scorer: joyful n=2, mean decision **0.9999668002128601**; seductive n=4, mean decision **0.39605393525292687**. `COMPARISON.md` says the Samantha probe is missing aggressive, love, and laughter wavs. Those probe means do not enter the gate table.

### The other cloud, already measured

The single-reference posterior is a four-class cloud, and the repo already warns that it is not a target. Three published descriptions disagree with each other and must stay as three rows:

| Where | neu | hap | ang | sad | What the file says it is |
|---|---:|---:|---:|---:|---|
| Foundation spec §7.1 | 0.345 | 0.203 | 0.136 | 0.317 | Laptop single-reference posterior |
| `emotion-v2/emotions.toml` `legacy_scorer.known_context` | 0.35 | 0.20 | 0.14 | 0.32 | Rounded brief. Sums to 1.01. Not a distribution to renormalise |
| `emotions.toml` `in_repo_profile_example`, and the whole-recording P(sad) in `tools/cloning-v2/emotion-blend/analysis/FINDINGS.md` | 0.350 | 0.213 | 0.119 | 0.318 | In-repo profile example. FINDINGS: whole-recording P(sad) = 0.318 |

FINDINGS, 2.5 s original windows, mean probabilities: neu **0.55709**, hap **0.08326**, ang **0.34731**, sad **0.01234**. Merged clone lines: neu **0.43781**, hap **0.14278**, ang **0.40906**, sad **0.01036**. The same file says the whole-recording P(sad) of 0.318 against a window mean of 0.012 is context-length dependence, not proof that cloning removed sadness.

The foundation spec’s render description, which this plan does not remeasure: sad collapses to about **0.00–0.04**, the posterior leans angry **0.5–0.7**, and that mismatch caps the single-reference `emotion_match` near **0.80**. `emotions.toml` records `emotion_match_cap_known = 0.80` with `role = "not-a-target"`. Per-emotion render posteriors for the twelve sectors are **unknown**. They have not been recorded.

## What to measure first on the laptop

The missing evidence is the clone blend, not another edit of the gate. PR #86’s published verdict is `insufficient_evidence` because clone love and neutral cosines are absent and clone prosody was not stored. The hard zero alone is what the 0.20 align gate writes. It does not prove the warmth left the audio.

Run this on the PR #86 branch. Do not merge that branch to obtain the binary. Do not edit its fixture. One wav at a time. The card is an RTX 4060, 8 GB.

Constraints from that PR and from `score_universal.py` on main:

- One CLAP load, one file. Model named by the eval path: `laion/clap-htsat-fused`. Weights must already be on the machine. This plan does not download them.
- `GALION_CLAP_DEVICE=cuda` moves that one CLAP pass. Leave the variable unset when the job is to reproduce the published CPU scores. CUDA can move a cosine relative to the CPU figure the gate was built on.
- The eval path refuses CUDA when `--gpu-mem-gb` is below 6. Pass **8**.
- emotion2vec+ in `tools/cloning-v2/scorers/universal/py/score_universal.py` is ONNX `emotion2vec_plus_base` on `CPUExecutionProvider`. It does not go on the 4060. `emotion-v2/emotions.toml` names `emotion2vec/emotion2vec_plus_large`. That is a different id. The nine labels match (`angry`, `disgusted`, `fearful`, `happy`, `neutral`, `other`, `sad`, `surprised`, `unknown`). The posterior from the ONNX file is the base export, not a claim about the large model.
- Piper is not invoked. Qwen is not loaded. Do not start a second model in the CLAP process.
- The wav stays on the laptop. Do not commit audio, weights, or the observation JSON of a person’s voice.

Exact command from the PR #86 body, from `tools/cloning-v2`:

```bash
( cd pipeline/expression && cargo build --release )

GALION_CLAP_DEVICE=cuda \
pipeline/expression/target/release/galion-expression eval \
  --wav /path/to/consent-or-cc0.wav \
  --voice own-samantha \
  --emotion love \
  --persona pipeline/personas/own-samantha.toml \
  --case pipeline/expression/fixtures/samantha_love_gate.json \
  --out-dir /tmp/galion-expression-eval \
  --score --love-warmth \
  --device cuda --gpu-mem-gb 8
```

The PR README also prints a prosody-only eval (`--device cpu`, no `--score`) and:

```bash
pipeline/expression/target/release/galion-expression diagnose \
  --case pipeline/expression/fixtures/samantha_love_gate.json
```

`diagnose` on the committed fixture reprints `insufficient_evidence`. It does not fill the missing cosines.

The nine-class posterior is a second process, after CLAP has exited, using flags that exist on main and that the PR’s eval JSON names (`--skip-clap --skip-efficientat`):

```bash
python3 scorers/persona/py/score_persona.py \
  --persona pipeline/personas/own-samantha.toml \
  --skip-clap --skip-efficientat \
  --out /tmp/galion-expression-eval/e2v-cpu.json \
  /path/to/the-same.wav
```

Run the pair on the reference love window and on each clone line that entered the love and aggressive rows. One file, then the next.

### The report is the blend

`eval --score` writes `clap_observation.json`, `profile.json`, and `diagnostic.json`. The diagnostic of the **committed** case stays `insufficient_evidence`, because `--score` does not insert the new observation. `eval_obs.py` calls the love plugin with an empty emotion2vec map, so `e2v_sad` in that observation is the default from an empty map, not a measured posterior. The measured nine-class posterior is `e2v-cpu.json`.

Keep, for the reference window and for each clone, this blend. A single decision is stored beside it and is not the score.

| Particle | Where it is written today | If the file does not contain it |
|---|---|---|
| Love, neutral, and seductive mean cosines, and love − neutral | `clap_observation.json` after `--score` | unknown until that run |
| Love decision and seductive decision | same file | unknown until that run |
| Nine-class posterior, full map, not only the top label | `e2v-cpu.json` | unknown. Do not read the default sad value as a posterior |
| F0 SD in semitones, RMS dB, spectral tilt dB/octave | `profile.json` `prosody` | unknown until `analyze` or `eval` |
| Four-class SUPERB posterior | not produced by these commands | unknown. Run it only when the weights are already local. Do not download them for this pass |
| Mixture distance between reference and clone | not produced by these commands | unknown. Defined in the rescore section. Do not invent the distance |

Prosody on the clone alone cannot move the published verdict. The committed reference observation has cosines and no prosody. `diagnose` compares both sides. A local copy of the fixture under `/tmp` (not a commit, not an edit of the branch) is how the new fields go in:

- Reference `prosody`: `f0_sd_st`, `rms_db`, `spectral_tilt_db_oct` from the reference window’s `profile.json`.
- Each clone: `decision`, `love_mean_cos`, `neutral_mean_cos`, measured `e2v_sad` from `e2v-cpu.json`, and the same three prosody fields.

Then:

```bash
pipeline/expression/target/release/galion-expression diagnose \
  --case /tmp/samantha-love-case.json \
  --out /tmp/galion-expression-eval/diagnostic.json
```

Supply all three prosody pairs on both sides. In the PR code, if only one pair is present and it does not fire, prosody is treated as held. A one-cue “held” is not a full match.

Audio-only listening, warmth and anger, by more than one listener, is what the research note asks for before a detector zero is read as absence. Those ratings are not in the repo. They are **unknown**. Use the same sentence the render used, so the words are not the thing being scored.

### Can real_loss and detector_miss be decided?

**Not from the repo.** The published clone rows have a decision of 0, a match of 0.06094556199731194, and no love cosine, no neutral cosine, and no prosody. PR #86’s `diagnose` returns `insufficient_evidence` on that input. That verdict is correct for the committed evidence.

**Yes, once the local case above has the clone margin, or prosody on both sides, or both.** The branch order in `pipeline/expression/src/love.rs` on that PR (first match wins), with the constants it ships:

| Constant | Value |
|---|---:|
| Align floor | 0.20 |
| Margin collapse | reference margin − clone margin > 0.04 |
| Cosine near | absolute difference ≤ 0.03 |
| Cosine collapse | reference love cosine − clone love cosine > 0.05 |
| Brighter tilt | clone − reference > 1.5 dB/octave |
| Louder energy | clone − reference > 3.0 dB |
| Flatter F0 | reference F0 SD − clone F0 SD > 0.8 st |
| Sad gate | emotion2vec+ sad > 0.5 |

Prosody is **lost** when the tilt cue fires or at least two cues fire, **partial** when the only cue is energy or F0, **held** when every supplied pair is quiet, **unknown** when none of the three pairs exist on both sides.

| First matching evidence | Verdict |
|---|---|
| Sad > 0.5, love cosine still above 0.20, decision 0 | `detector_miss` |
| Love cosine above 0.20, decision 0 | `detector_miss` |
| Margin held within 0.04, decision 0 | `detector_miss` |
| Love cosine within 0.03 of the reference, decision 0 | `detector_miss` |
| Prosody held, and the margin or the cosine collapsed, or the align floor failed | `detector_miss` |
| Margin or cosine collapsed, and prosody lost | `real_loss` |
| Margin or cosine collapsed, and prosody partial | `mixed` |
| Margin or cosine collapsed, and prosody unknown | `real_loss` in the code |
| Prosody lost, no margin measured | `real_loss` |
| Prosody held, no margin, decision 0 | `detector_miss` |
| No margin, prosody unknown, decision 0 | `insufficient_evidence` (today) |
| Decision within 0.05 of the reference | `held` |
| Anything else | `mixed` |

The code will call **real_loss** on a collapsed margin when prosody was simply not recorded. Do not stop on that branch. Record the three prosody pairs in the same pass, then read the verdict. A detector zero remains one particle. The research note says a single classifier zero is not sufficient evidence that the emotion is absent. The blend table is the result that gets kept either way.

What each verdict would mean for the gate, using only the recomputed bounds above:

- `detector_miss`: the warmth may still be in the audio and the stored 0 is the align clip or the sad gate. A corrected love score has to reach **0.8688065227883068** (merge-v2) or **0.9567645351509508** (coherent) before these rows clear 0.83. Whether the corrected score does that is **unknown** until the cosines exist.
- `real_loss`: the prompt margin and the voice colour both left the reference. Changing the detector does not supply the missing warmth. The same score bounds are what the audio would have to reach. The new `emotion_match` after a re-render is **unknown**.
- `mixed` or a remaining `insufficient_evidence`: do not pick a repair. Measure the missing half.

## Controlled reference recordings

The session the checker can seal is `docs/emotion-v2/CAPTURE-PROTOCOL.md` and `docs/emotion-v2/capture-spec.toml`. The tool is `tools/emotion-v2/refcheck`. The foundation spec §3.2 names a different distance (15–20 cm), loudness (−18 LUFS ± 1), true peak (−1.5 dBTP), noise floor (−50 dBFS), and take count (8 lines plus 2 shared calibration lines). refcheck does not enforce those. This plan does not edit either document. Record to the checker.

### Consent

The voice is the owner’s own voice. No other person, no film line, no model output. The spec field stays `consent = "owner-own-recordings-only"`. The checker refuses any other value. A written consent record sits beside the archive before record is armed: the owner, this session, this purpose. Originals stay on the recording machine.

Training-data generation is a different act. `tools/emotion-v2/dataset` refuses a real voice unless a consent file has `voice_id`, `owner`, `signed_date`, and `consent_scope` containing `training-data-generation`. The clone scope `local-zero-shot-conditioning` does not authorize it. That record is the admin’s written consent. It is not implied by this plan, and the pipeline is not pointed at a person until it exists.

### Anchors

One session card, one room, one microphone, one gain, mouth mark at **30 cm**. The 432 Hz source sits on that mark for calibration. Recorder processing off. The sentence is one the owner chooses, in the owner’s words, and the **same sentence** is used for every anchor so the later comparison is the emotion. Same sentence again for the blends below, so affectionate or angry words are not the variable.

Calibration first: `calibration/take-01.wav`, `--role calibration`. Tone **432 Hz ± 2 Hz**. Duration 2.0–8.0 s. At least one PASS.

Then the twelve, five takes each, at least three PASS. A failed take stays in the folder.

| Emotion | Folder | Record | Passing minimum |
|---|---|---:|---:|
| Neutral | `neutral` | 5 | 3 |
| Calm | `calm` | 5 | 3 |
| Happy | `happy` | 5 | 3 |
| Amused | `amused` | 5 | 3 |
| Excited | `excited` | 5 | 3 |
| Tender | `tender` | 5 | 3 |
| Sad | `sad` | 5 | 3 |
| Disappointed | `disappointed` | 5 | 3 |
| Angry | `angry` | 5 | 3 |
| Fearful | `fearful` | 5 | 3 |
| Surprised | `surprised` | 5 | 3 |
| Disgusted | `disgusted` | 5 | 3 |

Acoustic pass, from `capture-spec.toml`, checked rather than edited: 48 kHz, 24-bit signed PCM, mono; emotion duration 3.0–12.0 s including **0.5 s** of quiet room before and after; integrated loudness **−20.0 LUFS ± 2.0**; true peak **≤ −1.0 dBTP**; quietest 400 ms **≤ −60 dBFS** RMS; DC **≤ −50 dBFS**; **0** clipped samples; Schroeder T20 **≤ 0.40 s**; spectral tilt **−6.0 dB/octave ± 2.0**.

Commands from the protocol:

```bash
SPEC=docs/emotion-v2/capture-spec.toml
DIR=originals/<session-id>

cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  check --wav path/to/take.wav --spec "$SPEC" --role emotion

cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  manifest --dir "$DIR" --spec "$SPEC" --out "$DIR/MANIFEST.json"

find "$DIR" -name '*.wav' -exec chmod a-w {} +

cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  verify-manifest --manifest "$DIR/MANIFEST.json" --spec "$SPEC" --require-session
```

`--require-session` demands the twelve folders and one passing calibration. It does not demand seductive or the blends. WAV files stay outside git. A manifest may be committed only when the hashes are meant to be public.

Seductive is Samantha-only. It is not one of the twelve and it is not a refcheck folder today. The foundation spec says she records it under the same protocol. Record `seductive/` the same way, five takes, three acoustic PASSes, on this card. The seal does not include it until the spec says so. Do not record it for Jarvis or for Galion One.

### Deliberate blends

After the twelve pass, on the same card, record blends. The owner speaks a mixture on purpose. The instruction is on the card. The sentence stays the sentence already used for the anchors.

Examples to perform, as prompts, not as fixed ratios and not as new dataset labels:

- tender + playful
- calm + concerned
- the research note’s prompts: affectionate concern, warm frustration, bittersweet relief, excited uncertainty

The foundation spec §2.3 already gives one authoring example, Tender 0.7 + Sad 0.3. That is a design example in that spec. It is not the weight this session must hit.

Words that are not sector ids stay words. In this inventory, playful is the foundation spec’s plain description of Amused, not an id. Concerned, unrest, and warmth are not ids. Love is a persona category, not one of the twelve. The owner may say those words. They are written in `spoken_words`. They are not forced into Happy, Angry, or Tender to make the vector look complete. `other` and `uncertain` are allowed.

Each blend take gets a sidecar, next to the wav, not inside it. Intensities are the owner’s intended strengths in \[0, 1\]. They do **not** have to sum to 1. A middle valence is not a reason to store an empty cloud. A normalized distribution is a second object, written later from raters, and it is not a substitute for the intensities. Do not renormalise the owner’s numbers. Do not fill a weight the owner did not speak. Until the take exists, every intensity is **unknown**.

```json
{
  "sentence_id": "same-as-anchors",
  "spoken_words": ["tender", "playful"],
  "unmapped_words": ["playful"],
  "intensities": {
    "neutral": null,
    "calm": null,
    "happy": null,
    "amused": null,
    "excited": null,
    "tender": null,
    "sad": null,
    "disappointed": null,
    "angry": null,
    "fearful": null,
    "surprised": null,
    "disgusted": null,
    "seductive": null
  },
  "delivery": [],
  "situation": "",
  "persona": "samantha"
}
```

`delivery` is where whisper, slower, or firmer goes. Those are not sectors. `situation` is the scene, not a thirteenth emotion.

Acoustic check of a blend wav is the same `check --role emotion`. This plan uses five takes and three acoustic PASSes so a blend file is held to the same room and gain as an anchor. That count is not in `capture-spec.toml`, and `--require-session` will not ask for it. Failed blend takes stay in the folder.

Listeners, when they are asked, say which emotions they hear together and how strongly, then valence, arousal, dominance, and confidence. Keep every rater’s raw marks. Two raters splitting angry versus sad is disagreement. Both raters marking anger and sadness together is a blend report. Those are different records. Whole-clip marks and shorter spans are both kept, because a clip can change its mind. None of that exists yet.

These originals do not, by themselves, change merge-v2’s 0.6869450087862239 or coherent’s 0.6713695158531341. Those numbers compare clone lines with original moments. The new references become the target cloud for the rescore below.

## Nebula rescore, beside the 0.83 gate

Proposal only. No new threshold. `tools/emotion-v2/thresholds.toml` stays at 0.83 per emotion. The persona gate stays at 0.83. CI is not retargeted.

A render and a reference are each a distribution, not an argmax.

- **Categories.** The twelve Emotion V2 names, plus Samantha-only `seductive`. Same order every time. Soft-label or posterior similarity (cosine of the two probability vectors) and Jensen–Shannon divergence over that order. Earth-mover distance belongs on this axis only when the category order has a stated ground distance. The twelve are not laid out on a line in the repo, so a categorical earth-mover distance is **not** defined here.
- **Dimensions.** Valence, arousal, and dominance. Compare the measured distribution on the render with the measured distribution on the reference. Wasserstein distance is the comparison the research note names. Report mean, dispersion, and how many ratings went in. Dominance is a control/power judgment, not loudness.
- **Intensities.** The unnormalised per-emotion strengths from the sidecar, kept beside any normalised posterior. A softmax is relative mass. It is not a reading of simultaneous strength.
- **Per-particle differences.** Warmth/love and anger are always printed on their own, including when the scalar `emotion_match` moves. Warmth is not removed from the report to help the gate.

Design coordinates are not the reference. `emotion-v2/emotions.toml` stores valence and arousal. The foundation spec sketches a different valence, arousal, and a dominance column, and it says those are design coordinates. A nebula score uses audio against audio. Distance to a design point may be logged and is not the gate.

emotion2vec, the base ONNX or the large model, is a feature encoder until a head is calibrated on consented project recordings. Its nine classes are not the twelve. They are not renamed into love or warmth. Calm, amused, tender, disappointed, and seductive are not classes of that head. A zero on a class the head does not have is not a measurement of that particle.

Relation to 0.83:

- The persona `emotion_match` keeps the formula and the gate printed above. PR #86’s attribution numbers stay the account of these two clone sets.
- The harness posterior-cosine target stays 0.83, one emotion at a time, once references exist. A single weak sector is not hidden inside one blended float. That rule is already in the foundation spec and this plan leaves it there.
- The nebula distances are a second report: JS and posterior cosine on the thirteen-way vector (twelve plus seductive), Wasserstein on VAD, and the per-particle table. They do not replace `emotion_match` in this proposal.
- Any weight inside a future combined score is chosen on held-out sessions, speakers, and scripts. It is not chosen so that merge-v2 or coherent passes. The papers do not validate 0.83 and do not replace it. No pass/fail claim is made for a nebula distance. A numeric bar for JS or Wasserstein is **unknown**.

## Nebula as the live emotion pipeline

The nebula is the real-time emotion pipeline Samantha and Jarvis are influenced by while a call is running. The training-data builder is a later reader of the same coordinates. It is not the reason the cloud exists. This section specifies that live path. It does not add a crate, does not change `LOCKS.toml`, and does not put a new model on the heartbeat.

`apps/nebula-voice-v2` is **not in this repository**. The admin’s visualiser is a separate standalone Rust program being built at that path. It reads newline-delimited JSON, one frame per line. This plan is the contract that program is being written against. The bytes are not checked against that tree, because that tree is not here.

### What a live frame carries

Each frame is one cloud, not one label:

- the twelve Emotion V2 names, as weights
- `seductive` only when `voice_id` is Samantha; the key is omitted for Jarvis and for Galion One
- valence, arousal, dominance
- energy
- a timestamp

Weights are intensities. They do not have to sum to 1. A mid valence does not blank the cloud. Warmth and love stay visible: love is carried as its own note beside Tender until a calibrated head has a love class, and it is not dropped to simplify the frame. Unmeasured fields stay null. They are not filled with 0.

Proposed line, key names proposed here because the visualiser tree is not in this repo:

```json
{"voice_id":"samantha","t_ms":0,"weights":{"neutral":null,"calm":null,"happy":null,"amused":null,"excited":null,"tender":null,"sad":null,"disappointed":null,"angry":null,"fearful":null,"surprised":null,"disgusted":null,"seductive":null},"vad":{"valence":null,"arousal":null,"dominance":null},"energy":null}
```

`t_ms` is milliseconds since the stream started, the same clock as `EmotionFrame.t_ms` in `galion-emotion`. A closed utterance appends one extra line of the same shape, with `t_ms` at the utterance end and the weights summarising that utterance. The visualiser can draw either density. The per-frame line is the live one.

Two clocks already exist and this plan does not add a third:

| Clock | Where | What it already is |
|---|---|---|
| Heartbeat | `LOCKS.toml` `[clock]` | `frame_ms = 80`, `frame_hz = 12.5`, shared by `audio`, `emotion`, `console`, `render`, `rika`, `os-scheduler`. Not configurable |
| Feeling wire | `LOCKS.toml` `[bands]` and `galion-emotion/src/band.rs` | `EmotionWire-v1-8B`, 8 bytes per frame per direction, 100 B/s at 12.5 Hz |

The 8-byte wire holds a sequence number, a present bit, a direction bit, a **3-bit** `Affect` label (eight names: Neutral, Calm, Joy, Excited, Sad, Tired, Angry, Anxious), valence as i8, arousal as u8, dominance as u8, confidence as u8, and four consent bits. It cannot carry twelve weights. The nebula line is a side stream. The wire stays the lock. This plan does not widen it.

`Affect` is derived from VAD inside `EmotionFrame::label`. It is a coarse label for the wire. It is not the nebula.

### Data flow

```text
utterance audio, and the intended sector for the line being spoken
        │
        ▼
per-frame estimate on the 80 ms heartbeat
12 weights + Samantha-only seductive + VAD + energy
        │
        ▼
EMA smooth, per weight, per VAD axis, and energy
(a missing estimate repeats the last smoothed outbound cloud;
 it does not invent a label)
        │
        ├──────────────────────────────► voice conditioning
        │                                 Samantha or Jarvis prosody /
        │                                 sector for the line in flight
        │
        └──────────────────────────────► local NDJSON
                                          apps/nebula-voice-v2
                                          one JSON object per line
```

Outbound is what that voice is expressing. `docs/galion-voice-v2/symphony/SAMANTHA-EMOTIONS.md` already says outbound affect is what she intended and does not need the caller’s consent to exist. That outbound cloud is what conditions her voice and what the visualiser draws.

Inbound is the caller. `EmotionWire` drops an inbound frame when the `sense` bit is off. `adapt` is the bit that may let inferred affect steer prosody. Both default to deny. Workplace and education purposes stay prohibited. The visualiser does not receive caller weights unless `sense` is on, and those weights do not leave the machine unless `share` is on. Withdrawing `sense` clears the inbound band on the next frame. The outbound cloud remains, back at a resting distribution the expressor already knows how to hold, not at a guess about the caller.

Energy on this frame is the expressed energy the voice conditioning consumes. `ProsodyControl` in `galion-emotion` already clamps that control to **0.6..1.2**. Dominance stays a control/power axis, not a second name for energy.

The smoother is the existing exponential moving average (`EmotionFrame::ema`). `GatedDetector` defaults its VAD coefficient to **0.5**. Rika’s arousal and tension averages use **0.08**, commented in `galion-emotion/src/rika.rs` as about 2 s at 12.5 Hz. Those coefficients belong to those paths. The coefficient for the twelve weights is **not set** in this plan. Choosing one is a later measurement. It is not chosen so a render passes 0.83.

### Latency budget

The budget is the one already locked. This plan adds no new millisecond figure.

| Lock | Value | Role for the nebula |
|---|---|---|
| `frame_ms` | 80 | A live frame is one heartbeat |
| `frame_budget_p95_ms` | 60 | Compute inside that frame |
| `affect_to_voice_max_ms` | 160 | Inbound affect to outbound audio |
| `affect_to_voice_max_frames` | 2 | Same hop, in frames |
| `affect_overhead_max_pct` | 3 | Emotion work as a share of the frame |
| `present_max_ms` | 1 | `present()` per frame |

`tools/cloning-v2/plan/CLONING-V2-PLAN.md` lists a separate target, emotion score latency **< 200 ms** per utterance, warm CPU. That line is the offline scoring target in that plan. It is not a measurement of a twelve-way live head, and it is not a replacement for the 80 ms heartbeat.

A wall-clock time for a twelve-way posterior on one 80 ms frame is **unknown**. emotion2vec+ in the persona scorer is an utterance ONNX session on `CPUExecutionProvider`. Nothing in the repo shows that session finishing inside 60 ms. Until that is measured on the laptop, the twelve-way head stays off the heartbeat. The frame that ships is the last smoothed outbound cloud, or an explicit absence when there has been no estimate yet. Absence is the wire’s existing `present = 0`. It is not a Neutral weight of 1 invented to keep the visualiser busy. The audio heartbeat is not blocked on the head.

The visualiser reads the side stream. It does not sit inside `frame_budget_p95_ms`. A slow draw drops a line. It does not delay `affect_to_voice`. Colour for a particle comes from the fixed element table below. That colour is a label. It is not an audio control.

### How the live cloud relates to the offline score and the gate

The live distribution is the cloud the voice was asked to inhabit, frame by frame, then smoothed. The offline score is a later comparison of a **finished** render with a reference.

| | Live nebula | Offline gate |
|---|---|---|
| When | During the call, one heartbeat at a time | After a render exists |
| Object | Intended weights, VAD, energy | Persona `emotion_match` on merge-v2 and coherent, and later a posterior against sector references |
| Bar | The lock table above | **0.83**, unchanged |
| Failure | Repeat the last smoothed outbound cloud, or send absence | The published sets fail. That result is not applied by deleting a live particle |

The same twelve names, the same Samantha-only seductive rule, and the same VAD axes are the coordinates of both. After a call, the utterance line and the rendered wav can be scored offline against each other and against the controlled reference: what was conditioned, what the audio did, what the reference holds. That comparison uses the rescore in the previous section. It does not run inside the 60 ms budget, and its distance does not become a live pass/fail.

A live frame is not a gate result. 0.83 is not consulted to accept or reject a heartbeat. The gate is not lowered because the visualiser needs a number. Warmth is not removed from the live weights because love is the weak offline row.

## Fixed element-to-emotion table (Nebula)

This table is hard-coded. The same rows are the colour key for the visualiser at `apps/nebula-voice-v2`, for the live emotion frames, and for the training labels. The emotion id is the key. All three consumers use this table. They do not each invent a line.

The gas line is a label on the emotion. It does not change acoustic output. Pitch, energy, tilt, the persona blend, and the 0.83 gate are untouched by the element, the wavelength, and the colour.

Ne I (Surprised) and Ar I (Sad) are lab lines that are weak in real nebulae. They stay in the table by admin choice.

| Emotion | Gas line | Wavelength (nm) | Colours |
|---|---|---:|---|
| Neutral | H-alpha | 656.3 | red |
| Calm | H-beta | 486.1 | blue-green |
| Tender | H-gamma | 434.0 | violet |
| Happy | He I | 587.6 | yellow |
| Excited | [O III] | 500.7 | teal-green |
| Amused | [O II] | 372.7 | violet (near-UV) |
| Surprised | Ne I | 640.2 | orange-red |
| Fearful | He II | 468.6 | hot blue |
| Sad | Ar I | 696.5 | deep red |
| Disappointed | [S II] | 673.1 | red-orange |
| Angry | [S III] | 631.2 | deep red |
| Disgusted | N II | 399.5 | violet-blue |
| Seductive (Samantha only) | [O I] | 630.0 | red-orange |

Seductive remains Samantha-only. Jarvis and Galion One do not take that row. The twelve shared emotions are the twelve rows above it.

## Ranked next steps

Impact on the published persona `emotion_match` is stated only where the rows above already determine it. Everywhere else the impact is unknown.

1. **Laptop blend, reference window and the clone lines, one file at a time.** Fills the cosines and the prosody that the verdict is waiting on. Expected change to 0.6869450087862239 and 0.6713695158531341: **unknown** until the files exist. This is the step that can decide `real_loss`, `detector_miss`, or `mixed`. Do not move the toml love target off 0.8109382317899566 to get there.

2. **Read the verdict only after margin and all three prosody pairs are in the local case.** A cosine-only `real_loss` is the code’s answer when prosody is missing, and it is not the last word. If the completed verdict is `detector_miss`, the love score still has to reach 0.8688065227883068 or 0.9567645351509508, with the other rows held, before these sets pass. If it is `real_loss`, the detector is the wrong repair and the same bounds are what a later render has to earn. The render’s new score is unknown.

3. **Leave love and warmth in the judged set.** Deleting the love row leaves 0.8179380447274447 and 0.8007452268534501, still under 0.83, and the research note forbids dropping the category to pass a gate. Aggressive set to a perfect 1 while love stays at 0.06094556199731194 still fails both sets. `emb_sim` set to 1 still fails both sets. Love is the row with the largest recomputed hole (0.166287 of `emotion_match` on both sets). Aggressive is second (0.083734 and 0.088764).

4. **Record the owner’s anchors and the deliberate blends** under the capture protocol, with the owner’s consent record, then seal with refcheck. This creates the target cloud. It does not rewrite the published clone scores. Impact on today’s persona float: **none**, until a render is scored against those takes.

5. **Score renders as distributions beside the existing gate.** Posterior cosine and Jensen–Shannon on the twelve plus seductive, Wasserstein on measured VAD, raw intensities kept, warmth and anger always visible. Thresholds and CI stay as they are. Impact on the gated float: **none** in this step.

6. **Listeners on held-out clips**, raw votes kept, disagreement stored separately from a within-listener blend. That is the check on whether a nebula distance agrees with people. No threshold comes out of the first session. Impact: unknown.

7. **Training data stays blocked.** The dataset stub may be read. A real voice or a Chatterbox render of a person waits on the admin’s written `training-data-generation` consent. Impact of a future corpus on `emotion_match`: unknown. This plan does not generate it. The live pipeline does not wait on that corpus.

8. **Keep the live cloud on the existing clock.** Specify the NDJSON side stream for `apps/nebula-voice-v2`. Leave `EmotionWire` at 8 bytes. Leave the twelve-way head off the heartbeat until a laptop measurement shows it fits `frame_budget_p95_ms` 60 and `affect_to_voice_max_ms` 160. Impact on the gated float: **none**.

## Research basis

Full note, with the link-check record: [`EMOTION-NEBULA-RESEARCH-20261107.md`](EMOTION-NEBULA-RESEARCH-20261107.md). The nine findings and the mapping below are that note. They are engineering motives. They are not a claim that neuroscience specifies this pipeline.

1. **Core affect is not a discrete emotion.** Russell separates felt valence from judgments about a situation, which can be good in one respect and bad in another, inside a model that still treats core valence as bipolar. [Russell, 2017](https://journals.sagepub.com/doi/10.1177/1754073916639658).
2. **Constructed emotion is a situated interpretation.** Barrett describes an instance as categorization from past experience, bodily regulation, and current context. That is a theory, not a settled brain implementation. [Barrett, 2017](https://pmc.ncbi.nlm.nih.gov/articles/PMC5390700/).
3. **Mixed positive and negative feelings need not cancel into neutrality.** Larsen and McGraw review happiness and sadness together in bittersweet events. Their account differs from Russell’s treatment of core valence. [Larsen & McGraw, 2014](https://onlinelibrary.wiley.com/doi/full/10.1111/spc3.12108).
4. **Granularity is specificity, not simply stronger emotion.** People differ in how finely they label affect. An ERP study links granularity with picture processing and does not show that a speech detector measures those distinctions. [Emotional Granularity Effects, 2017](https://pmc.ncbi.nlm.nih.gov/articles/PMC5364149/).
5. **Dimensions complement categories and do not replace them.** Valence, arousal, and dominance are the speech-emotion axes in use. A dimensional mean alone drops label disagreement and can hide a mixture. [Wu, Zhang & Woodland, 2023](https://arxiv.org/html/2306.06760).
6. **Voice-emotion perception is a distributed network.** Schirmer and Kotz describe multistage bilateral processing. A later study finds overlapping valence and arousal responses in superior temporal voice areas, insula, orbitofrontal cortex, and amygdalae. [Schirmer & Kotz, 2006, abstract](https://pubmed.ncbi.nlm.nih.gov/16321562/); [voice-network study, 2017](https://pmc.ncbi.nlm.nih.gov/articles/PMC5597854/).
7. **Prosody and words can carry different evidence.** A voice can sound angry while the words are neutral. AmbER2 models that modality split separately from rater disagreement. [Wu et al., AmbER2, 2026 preprint](https://arxiv.org/html/2601.18010v1).
8. **Rater labels can stay distributions.** AmbER2 builds soft targets from vote shares on IEMOCAP and MSP-Podcast. DEER models the distribution of dimensional ratings on those corpora. [AmbER2](https://arxiv.org/html/2601.18010v1); [DEER](https://arxiv.org/html/2306.06760).
9. **An emotion embedding is evidence, not a calibrated mixture readout.** emotion2vec trains frame- and utterance-level representations and evaluates downstream tasks. The paper does not establish a calibrated twelve-emotion mixture or a validated warmth or love score. [Ma et al., 2023](https://arxiv.org/html/2312.15185).

What that mapping asks of this plan:

| Findings | What the plan does with them |
|---|---|
| 1–2 | The clip carries a distribution over the existing twelve, plus VAD, situation, persona, and delivery. Persona and style stay metadata. They are not biological particles. |
| 3 | Positive and negative intensities are both kept. A mid valence does not blank a bittersweet line. Intensities sit beside any normalised distribution and do not have to sum to 1. |
| 4 | Warmth and love are defined against happiness and calm, with listener examples, once recordings exist. `other` and `uncertain` stay available. |
| 5 | VAD keeps each rater’s values, plus mean, dispersion, and count. Dominance is not loudness. |
| 6 | Prosody, voice quality, time, and listening all count. One classifier zero is not absence. |
| 7 | The laptop warmth check uses matched text. Audio-only ratings and audio-plus-context ratings are logged apart. |
| 8 | Soft labels come from independent raters. Raw votes and multi-label intensities are kept so disagreement is not stored as a blend. Jensen–Shannon compares render and reference on one label order. Wasserstein compares VAD. Weights and any future threshold are chosen on held-out data, not to rescue merge-v2 or coherent. |
| 9 | emotion2vec stays an encoder until a twelve-way head and a VAD head are calibrated on consented recordings. Generic classes are not renamed love or warmth. |

Caveats, carried from the note:

- **Nebula is a metaphor**, not an anatomical mechanism and not evidence that emotion is additive particles. Twelve labels are a project choice, not twelve neural primitives.
- Felt emotion, expressed emotion, intended acting, and what a listener hears are different targets. A speech scorer estimates perceived expression.
- **Listener disagreement is not proof of a blend.** A vote distribution is relative label mass, not simultaneous intensity. Two people splitting angry versus sad is not the same record as both people hearing anger and sadness together. Keep per-rater multi-label marks.
- A whole clip can be a fast change rather than a mixture. Short spans are required before a coexistence claim. Rater lag limits that claim.
- VAD averages can hide a mixture. A Gaussian uncertainty model is an assumption.
- Room, microphone, codec, accent, language, words, identity, and acting habits move model outputs. The capture protocol is how this project holds the room still. No universal threshold follows from the cited studies.
- **Do not drop warmth or love, and do not move a target, to pass the gate.** Do not infer a missing emotion from a detector output alone.
- AmbER2 is a 2026 preprint, not a consensus. Its splits do not certify Samantha or these twelve labels. emotion2vec needs the same downstream check.
- The existing **0.83 gate is not validated or replaced by these papers.**
- New recording and any training need the consent records named above. This research authorizes neither.

## What this document does not do

It does not merge PR #86, PR #88, or any other branch. It does not deploy a voice. It does not commit audio, weights, secrets, or a person’s consent file. It does not edit a lock, a soul file, a config, or a persona TOML. It does not lower 0.83, and it does not add a nebula number to CI. It does not add `apps/nebula-voice-v2`, and it does not widen `EmotionWire`.
