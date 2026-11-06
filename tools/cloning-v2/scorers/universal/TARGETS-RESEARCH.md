# Emotion-match targets: published research vs clone scores

Research only (2026-11-06). No clones or scorer files were changed.
Every numeric claim below was read from a page that was opened this turn.
**None of the sources publish a recommended per-emotion target for our metric.**

## Our metric (for mapping, not a literature target)

`emotion_match = 0.70 × category_blend + 0.30 × emb_sim`

- `emb_sim` = mean-centred emotion2vec cosine
- presence threshold 0.2
- overall rollout gate 0.83 (internal rule, not a published threshold)
- detectors: emotion2vec+ (trained classes), EfficientAT mn10_as (yelling / crying / laughter), CLAP (love / seductive / logic)

Overnight universal clone scores used below are exactly those supplied for this research:

| Voice / set | neutral | yelling | logic | laughter | aggressive | love |
|---|---:|---:|---:|---:|---:|---:|
| Samantha merge-v2 | 0.9689 | 0.9376 | 0.9248 | 0.8542 | 0.5555 | 0.3070 |
| Samantha coherent | 0.9646 | 0.9231 | 0.8973 | 0.9249 | 0.5288 | 0.2913 |
| Jarvis merge-v2 | 0.9650 | 0.9055 | 0.7932 | — | — | — |
| Jarvis coherent | 0.9823 | 0.9095 | 0.7301 | — | — | — |

Context supplied with the request (not re-derived here): Samantha ceiling/original love 0.7852; overall Samantha ceiling 0.9044; Jarvis ceiling 0.9475.
Crying and seductive were not scored in the overnight run (neither original contains them). Happy / sad / fear / surprise / disgust were not reported as clone categories in that run.

## What the literature actually reports

Papers do **not** give “a good yelling score is 0.X” on a 0.70/0.30 blend. They report one of:

1. **Average emotion2vec cosine (Emo_Sim / Emotion SIM / EMO-SIM)** between synthesised speech and a reference — usually raw cosine, often utterance-level, averaged across emotions. This is closest to our `emb_sim` term only, and even then not equivalent: we mean-centre; many papers do not.
2. **Emotion classification accuracy / recall / ECA** of generated speech (or of an SER model on human corpora). This is not a similarity score.
3. **AudioSet mAP** (mean average precision over 527 weakly labelled sound-event classes). This is detector quality, not TTS emotion-match.
4. **Zero-shot classification accuracy** (CLAP audio–text cosine used as a classifier). Again not a similarity target.

[The False Resonance (arXiv:2604.26347)](https://arxiv.org/html/2604.26347) is explicit that treating emotion2vec cosine as a zero-shot emotion-similarity metric is scientifically weak: even after mean-centring, speaker/linguistic-matched triplet accuracy is only about 60–70%; under linguistic distractors emotion2vec drops to 3.38% on CREMA-D; human-alignment of EMO-SIM is only 52.25–65.00%; uncalibrated raw cosines sit in a narrow cone of about 0.92–0.98.

## Scoreboard

“Published target” = a number the source itself presents as a recommended / target / threshold for *that emotion’s similarity*. If the source only reports average similarity, classification accuracy, or detector mAP, the target is **none published**.

Derived proxies are marked **DERIVED** and are not literature recommendations.

| Emotion | Published target | Source | What the source actually measures | Clone comparison |
|---|---|---|---|---|
| Neutral | none published | [emotion2vec](https://arxiv.org/html/2312.15185) reports overall IEMOCAP WA 71.79% (session-out) / 72.94% (speaker-out), and 74.48% / 77.64% under the starred protocol; [emotion2vec+ large card](https://huggingface.co/emotion2vec/emotion2vec_plus_large) lists `neutral` as class 4 of 9 but gives **no accuracy or similarity number**. | SER weighted accuracy on mixed emotions, not a neutral similarity target. Neutral is a trained emotion2vec+ class. | All four reported clones are high: Sam m-v2 0.9689, Sam coh 0.9646, J m-v2 0.9650, J coh 0.9823. Vs **DERIVED** internal gate 0.83: all meet (+0.1389, +0.1346, +0.1350, +0.1523). No published bar to miss. |
| Yelling | none published | [EfficientAT paper](https://arxiv.org/html/2211.04772v3): MN-KD pretrained (4.88M, the mn10_as-sized student) AudioSet **mAP 0.470**. [EfficientAT README](https://github.com/fschmid56/EfficientAT) lists **mn10_as mAP 47.1** (best checkpoint; paper reports run averages). Neither publishes per-class AP for Yell or Screaming. AudioSet does include those event labels. | Overall multi-label sound-event mAP over 527 classes, not a yelling similarity target. | Sam m-v2 0.9376, Sam coh 0.9231, J m-v2 0.9055, J coh 0.9095. Vs **DERIVED** gate 0.83: all meet (+0.1076, +0.0931, +0.0755, +0.0795). No published bar to miss. |
| Crying | none published | Same EfficientAT sources: overall mAP only. AudioSet label is “Crying, sobbing”. No per-class AP was printed in the paper HTML or README opened this turn. [PANNs](https://arxiv.org/html/1912.10211) says class-wise APs exist in its Fig. 12 but the figure is not numerically readable in the HTML; this note does **not** invent those values. | Detector mAP / class-wise AP for a sound-event tag, not a crying similarity target. | Not scored in the overnight clone run. No published bar. |
| Laughter | none published | Same EfficientAT / PANNs situation. AudioSet has a Laughter class; no numeric per-class AP extracted from the opened pages. | Detector quality, not TTS laughter-match. | Sam m-v2 0.8542, Sam coh 0.9249 (Jarvis not reported). Vs **DERIVED** gate 0.83: both meet (+0.0242, +0.0949). No published bar to miss. |
| Love | none published | No paper opened here evaluates “love” as an SER or TTS emotion class. CLAP papers evaluate broad audio–text retrieval or speech-emotion datasets (angry/happy/sad/neutral/etc.), not love. | — | Sam m-v2 0.3070, Sam coh 0.2913 (Jarvis not reported). Vs **DERIVED** gate 0.83: both fall short (−0.5230, −0.5387). Vs **DERIVED** own ceiling (Samantha original love 0.7852): merge-v2 −0.4782, coherent −0.4939. Both also sit far below published *average* raw Emo_Sim ranges in the next section (see caveat). |
| Seductive | none published | No opened paper treats “seductive” as a class or reports a similarity target. | — | Not scored overnight. No published bar. |
| Logic / reason | none published | No opened paper treats “logic” or “reason” as a speech-emotion class. | — | Sam m-v2 0.9248, Sam coh 0.8973 meet **DERIVED** gate 0.83 (+0.0948, +0.0673). Jarvis m-v2 0.7932 falls short of that gate by **0.0368**; Jarvis coh 0.7301 falls short by **0.0999**. This is an internal-gate comparison only. |
| Aggressive (anger) | none published as similarity | Closest *classification* numbers, not similarity: [Seed-TTS](https://arxiv.org/html/2406.02430) instruction-FT emotion-control **accuracy**: angry 1.0, happy 0.85, sad 1.0, surprise 0.98 (without instruction: 0.69 / 0.40 / 0.37 / 0.22). [CosyVoice](https://arxiv.org/html/2407.05407v2) instruct emotion-control accuracy via emotion2vec: angry 0.83±0.04 (base without instruction 0.59±0.03). emotion2vec+ lists `angry` as class 0. | Classification / control accuracy of generated speech, not cosine or blend similarity. | Sam m-v2 0.5555 and Sam coh 0.5288 vs **DERIVED** gate 0.83: fall short by **0.2745** and **0.3012**. Jarvis aggressive was not reported overnight. |
| Happy | none published as similarity | Seed-TTS IFT accuracy happy 0.85 (above). CosyVoice-instruct happy 1.00±0.00. emotion2vec+ class 3 = happy. EmoVoice recall is averaged over selected categories after dropping disgusted / fearful / surprised. | Classification accuracy / recall, not similarity. | Not reported as a clone category in the overnight run. |
| Sad | none published as similarity | Seed-TTS IFT sad 1.0; without instruction 0.37. CosyVoice-instruct sad 0.98±0.02 (base 0.45±0.05). emotion2vec+ class 6 = sad. | Classification accuracy, not similarity. | Not reported overnight. |
| Fear | none published as similarity | [EmoVoice](https://arxiv.org/html/2504.12867) explicitly **excludes** fearful, disgusted, and surprised from recall “due to insufficient training data and consequently low recognition accuracy of emotion2vec”. CosyVoice-instruct fearful 0.87±0.03 (base 0.88±0.01). emotion2vec+ class 2 = fearful. | The exclusion is itself evidence: emotion2vec is a weak fear detector. CosyVoice number is classification accuracy. | Not reported overnight. No published similarity target. |
| Surprise | none published as similarity | Seed-TTS IFT surprise 0.98 (without 0.22). CosyVoice-instruct surprised 0.64±0.03 (base 0.26±0.02). EmoVoice excludes surprised from recall for the same low-accuracy reason. emotion2vec+ class 7 = surprised. | Classification accuracy; EmoVoice warns the encoder is unreliable here. | Not reported overnight. |
| Disgust | none published as similarity | EmoVoice excludes disgusted from recall. CosyVoice-instruct disgusted 0.93±0.02 (base 0.46±0.06). emotion2vec+ class 1 = disgusted. | Classification accuracy; encoder reliability is contested. | Not reported overnight. |

## Closest published *average* emotion-similarity numbers (not per-emotion)

These are the numbers people quote as “good Emo-SIM”. They are **averages across emotions**, and they measure raw emotion2vec cosine against a reference, **not** `0.70 × category_blend + 0.30 × mean-centred cosine`. Do not treat them as per-emotion targets.

| Source | Number | What it is |
|---|---|---|
| [MaskGCT](https://arxiv.org/html/2409.00750v3) Table 5, ESD emotion imitation | GT **0.936**; CosyVoice 0.839; MaskGCT 0.822; VoiceBox 0.811; VoiceCraft 0.788; VALL-E 0.735 | Emotion SIM via [emotion2vec](https://github.com/ddlBoJack/emotion2vec) features between generated/GT speech and the emotion prompt. GT 0.936 is a same-corpus human ceiling for *that* cosine, not a recommended clone threshold. |
| [EmoVoice](https://arxiv.org/html/2504.12867) Table 2, EmoVoice-DB (English) | PromptStyle 0.8717; PromptTTS 0.8709; CosyVoice 0.8889; CosyVoice2 0.8647; EmoVoice 0.5B **0.9100**; EmoVoice 1.5B **0.9118**; GPT-4o-mini-tts 0.9168 | Utterance-level emotion2vec cosine vs ground-truth speech of the same item. Recall (separate metric): 0.313–0.456; they drop disgusted / fearful / surprised. System-level Spearman ρ of Emo_Sim vs human MOS: 0.9429; sentence-level only 0.4047. |
| [EmoVoice](https://arxiv.org/html/2504.12867) Table 4, Chinese Secap | EmoVoice-PP 0.7939; EmoVoice 0.7933; CosyVoice2 0.7811; GPT-4o-audio 0.6497; GPT-4o-mini-tts 0.6184 | Same cosine definition on a different language/test set — already ~0.12 lower than the English table. |
| [TTS-CtrlNet](https://arxiv.org/html/2507.04349v1) Tables 5, F5-TTS backbone | JVNV S2ST: F5-TTS Emo-SIM **0.684**, TTS-CtrlNet 0.751; EMO-Change: F5-TTS **0.692**, TTS-CtrlNet 0.724 | Frame-level Emotion2Vec similarity for *time-varying* emotion transfer (harder protocol → much lower numbers). Same name “Emo-SIM”, different setup. |
| [EmoSphere-TTS](https://arxiv.org/html/2406.07803) Table 1, ESD | ECA: proposed **94.02%**; GT speech only **85.67%** | Emotion *classification accuracy* of a pre-built classifier, not cosine similarity. GT below the model shows the classifier itself is imperfect on human ESD speech. |
| [EmergentTTS-Eval](https://arxiv.org/html/2505.23009) | no numeric emotion-similarity target | Emotions are judged by an LALM win-rate against a baseline (0–3 rubric), not emotion2vec cosine. |

**DERIVED loose band, labelled as such:** if one pretends the MaskGCT / EmoVoice English tables are a community “good average cosine” of about **0.82–0.92** (systems) with a human ceiling near **0.936**, then Samantha/Jarvis *neutral, yelling, and Samantha logic/laughter* would look competitive on that coarse band, while Samantha *aggressive* (0.5555 / 0.5288) and *love* (0.3070 / 0.2913) and Jarvis *logic* (0.7932 / 0.7301) would look low. That mapping is unofficial: those papers average across emotions and use a different formula than `emotion_match`.

## Detector papers (why some emotions have no target)

| Detector we use | Opened source | Relevant published number | Maps to our score? |
|---|---|---|---|
| emotion2vec / emotion2vec+ | [paper](https://arxiv.org/html/2312.15185), [large card](https://huggingface.co/emotion2vec/emotion2vec_plus_large), [seed card](https://huggingface.co/emotion2vec/emotion2vec_plus_seed) | IEMOCAP linear-probe WA up to 77.64% (starred protocol); 9 classes = angry, disgusted, fearful, happy, neutral, other, sad, surprised, unknown. Cards give **no** per-class F1. emotion2vec+ is **not** trained on yelling, crying, laughter, love, seductive, or logic. | Supplies `emb_sim` and trained-class category scores only. No recommended match threshold. |
| EfficientAT mn10_as | [paper](https://arxiv.org/html/2211.04772v3), [repo](https://github.com/fschmid56/EfficientAT) | Overall AudioSet mAP **0.470** (paper, MN-KD pretrained) / **47.1** (repo mn10_as). No per-class AP for Crying/sobbing, Laughter, Yell, or Screaming in those pages. | Detector quality, not a clone target. |
| CLAP (Microsoft) | [CLAP paper](https://arxiv.org/html/2206.04769) | Zero-shot emotion recognition: CREMA-D **0.1784**, RAVDESS **0.1599** (prompt “this person is feeling [class]”). Supervised finetune: CREMA-D 0.6834, RAVDESS 0.6436. Random baselines 0.1667 / 0.125. | Shows zero-shot CLAP is barely above chance on speech emotion. Love / seductive / logic are even further out of domain. A low love score can be detector failure, clone failure, or both. |
| LAION-CLAP | [LAION-CLAP paper](https://arxiv.org/html/2211.06687) | Zero-shot *audio event* classification (ESC-50, VGGSound, US8K, FSD50K) and retrieval (AudioCaps / Clotho). Best reported ESC-50 ZS 91.0% with keyword-to-caption aug. **No speech-emotion or love/seductive/logic results.** | Does not provide emotion-match targets. |
| Seed-TTS / CosyVoice | [Seed-TTS](https://arxiv.org/html/2406.02430), [CosyVoice](https://arxiv.org/html/2407.05407v2) | Emotion *control accuracy* (SER on generated speech), not embedding similarity. Seed-TTS does not report an emotion2vec cosine in the opened report. | Useful as a reminder that literature usually gates emotion on classification accuracy, not on 0.83 blend scores. |

## Which reported clone scores fall short

Against **published per-emotion targets**: none can be judged — those targets do not exist.

Against the only operational number we have, the **DERIVED internal overall gate 0.83** (not a published per-emotion recommendation):

Falls short:

- Samantha merge-v2 **aggressive 0.5555** (short by 0.2745) and **love 0.3070** (short by 0.5230)
- Samantha coherent **aggressive 0.5288** (short by 0.3012) and **love 0.2913** (short by 0.5387)
- Jarvis merge-v2 **logic 0.7932** (short by 0.0368)
- Jarvis coherent **logic 0.7301** (short by 0.0999)

Meets that same derived gate: all reported **neutral** and **yelling** scores; Samantha **logic** and **laughter**.

Additional **DERIVED** check using the supplied Samantha original/ceiling love score of 0.7852: both Samantha clones are far below their own original (0.3070 is −0.4782; 0.2913 is −0.4939). That is the strongest evidence that love is a real clone gap rather than “literature says 0.83”. It still does not tell us how much of the remaining ceiling-to-clone drop is cloning vs the CLAP love detector (zero-shot CLAP speech-emotion accuracy was only 0.1784 / 0.1599 in the CLAP paper).

Not comparable (no overnight clone score): crying, seductive, happy, sad, fear, surprise, disgust; Jarvis laughter / aggressive / love.

## Bottom line

No opened paper, model card, or benchmark publishes a recommended emotion-similarity target per emotion — not for neutral, yelling, crying, laughter, love, seductive, logic, or the Ekman extras. The closest citable figures are *average* emotion2vec cosines around 0.82–0.92 for strong English emotional TTS (MaskGCT ESD systems 0.735–0.839 with GT 0.936; EmoVoice-DB 0.8647–0.9168) and *classification* accuracies such as Seed-TTS IFT angry/happy/sad/surprise = 1.0 / 0.85 / 1.0 / 0.98. On that unofficial average-cosine band, and against our own 0.83 gate, Samantha’s aggressive (~0.53–0.56) and love (~0.29–0.31) scores are the clear misses; Jarvis logic (0.7932 / 0.7301) misses the internal gate only. Love in particular has no published target at all, and CLAP’s published zero-shot speech-emotion accuracy is near chance, so a 0.30 love score cannot be read as a clean clone failure without a separate detector check.

## Sources opened this turn

- https://arxiv.org/html/2312.15185 — emotion2vec
- https://huggingface.co/emotion2vec/emotion2vec_plus_large
- https://huggingface.co/emotion2vec/emotion2vec_plus_seed
- https://arxiv.org/html/2211.04772v3 — EfficientAT
- https://github.com/fschmid56/EfficientAT
- https://arxiv.org/html/1912.10211 — PANNs
- https://arxiv.org/html/2206.04769 — CLAP
- https://arxiv.org/html/2211.06687 — LAION-CLAP
- https://arxiv.org/html/2504.12867 — EmoVoice
- https://arxiv.org/html/2409.00750v3 — MaskGCT
- https://arxiv.org/html/2507.04349v1 — TTS-CtrlNet / F5-TTS Emo-SIM
- https://arxiv.org/html/2406.02430 — Seed-TTS
- https://arxiv.org/html/2407.05407v2 — CosyVoice
- https://arxiv.org/html/2406.07803 — EmoSphere-TTS
- https://arxiv.org/html/2505.23009 — EmergentTTS-Eval
- https://arxiv.org/html/2604.26347 — The False Resonance
