Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Companion to SAMANTHA-EMOTION-GAP-PLAN.md. The text below is the research note of 7 November 2026. It does not change a score, a gate, a recording, or a deployed voice.

# Emotion as a nebula: evidence and scoring proposals

Research date: 7 November 2026 (Europe/Zagreb). Docs and research only; no code, score, gate, merge or deployment changes.

## 1. Key findings

1. **Core affect is not the same thing as a discrete emotion.** Russell distinguishes felt valence from judgments about different aspects of a situation: a situation can have good and bad aspects even within a model treating core valence as bipolar. This supports separating dimensions, context and category judgments, not claiming every theory endorses simultaneous opposite core feelings. [Russell, 2017](https://journals.sagepub.com/doi/10.1177/1754073916639658).

2. **Constructed emotion is a situated interpretation, not a fixed emotion switch.** Barrett's theory describes emotion instances as categorizations using past experience, bodily regulation and current sensory/contextual information; it is a theoretical account, not settled proof of one brain implementation. [Barrett, 2017](https://pmc.ncbi.nlm.nih.gov/articles/PMC5390700/).

3. **Mixed positive and negative feelings need not cancel into neutrality.** Larsen and McGraw review evidence for happiness and sadness together in bittersweet events, as well as other opposite-valence combinations; their account differs from Russell's treatment of core valence. [Larsen & McGraw, 2014](https://onlinelibrary.wiley.com/doi/full/10.1111/spc3.12108).

4. **Emotional granularity concerns specificity, not simply stronger emotion.** People differ in how precisely they distinguish and label affect; an ERP study links granularity with differences in processing affective pictures, but does not establish that a speech detector measures those distinctions correctly. [Emotional Granularity Effects, 2017](https://pmc.ncbi.nlm.nih.gov/articles/PMC5364149/).

5. **Dimensions complement categories but do not replace them.** Speech-emotion research uses valence (negative–positive), arousal (calm–active), and dominance (weak/submissive–strong/in-control); a dimensional mean alone loses both label disagreement and potentially distinct mixtures. [Wu, Zhang & Woodland, 2023](https://arxiv.org/html/2306.06760).

6. **Voice emotion perception uses a distributed network, not a single emotion center.** Schirmer and Kotz describe multistage bilateral sensory, cognitive and emotional processing; a later voice-network study finds overlapping valence/arousal responses involving superior temporal voice areas, insula, orbitofrontal cortex and amygdalae rather than complete anatomical segregation. [Schirmer & Kotz, 2006 abstract](https://pubmed.ncbi.nlm.nih.gov/16321562/); [voice-network study, 2017, including Belin](https://pmc.ncbi.nlm.nih.gov/articles/PMC5597854/).

7. **Prosody and words can convey different emotional evidence.** A voice can sound angry while the words are neutral, so disagreement between audio and text is not necessarily a broken detector; AmbER2 explicitly models this modality ambiguity separately from disagreement between raters. [Wu et al., AmbER2, 2026 preprint](https://arxiv.org/html/2601.18010v1).

8. **Multiple-rater labels can be retained as distributions rather than reduced to one winner.** AmbER2 constructs categorical soft targets from vote proportions on IEMOCAP and MSP-Podcast, while DEER models the distribution and uncertainty of dimensional ratings on those corpora. [AmbER2](https://arxiv.org/html/2601.18010v1); [DEER](https://arxiv.org/html/2306.06760).

9. **An emotion embedding is useful evidence, not a calibrated mixture readout.** emotion2vec learns representations with frame- and utterance-level self-supervised objectives and evaluates downstream emotion tasks; its original paper does not establish a calibrated twelve-emotion mixture detector or a validated warmth/love score. [Ma et al., emotion2vec, 2023](https://arxiv.org/html/2312.15185).

## 2. Finding → what it means for the nebula scoring pipeline

All items below are engineering proposals motivated by the sources, not claims that neuroscience prescribes this exact architecture.

| Finding | What it means for the nebula scoring pipeline |
| --- | --- |
| Core affect and situated construction (1–2) | Represent a clip with a distribution over the **existing 12 Emotion V2 emotions**, alongside VAD dimensions, context, speaker/persona and delivery-style metadata. Keep persona/style distinct from emotion rather than treating them as biological particles. |
| Mixed feelings need not cancel (3) | Preserve positive and negative components. Do not let a middle valence score alone classify bittersweet speech as emotionally empty; retain per-emotion intensity ratings in addition to normalized label distributions. |
| Granularity (4) | Define each project label with listener examples and contrasts, especially warmth/love versus happiness/calmness. Allow other/uncertain annotations rather than forcing every perception into the project's ontology. |
| Dimensions plus categories (5) | Collect VAD ratings and retain each rater's values, not just their mean; report mean, dispersion and sample count alongside the category distribution. Dominance is a perceptual control/power dimension, not loudness. |
| Distributed voice perception (6) | Evaluate several kinds of evidence: prosodic contours, voice quality, temporal structure and listening judgments. A single classifier's zero is insufficient evidence that an emotion is absent. |
| Prosody/text conflict (7) | Run audio-only and audio-plus-context annotation separately; log their disagreement. For the laptop warmth check, use matched text and listening conditions to isolate vocal delivery from affectionate vocabulary. |
| Distributional labels (8) | Use soft-label targets from multiple independent raters. Keep raw votes and multi-label intensity ratings so listener disagreement can be distinguished from within-listener perception of a true blend. |
| Distributional evaluation (8) | Compare render versus reference by Jensen–Shannon divergence/distance over identically ordered, calibrated label distributions; compare VAD distributions too (e.g. Wasserstein distance). Report per-component differences and human paired-listening results; select weights and thresholds on held-out data, not to rescue one failing render. |
| Representation learning (9) | Use emotion2vec as a candidate feature encoder with separately trained/calibrated twelve-label and VAD heads. Validate on consented project recordings; do not rename generic output classes as love/warmth without evidence. |
| Combined implications | Record deliberate blends as references in addition to controlled single-emotion anchors: e.g. affectionate concern, warm frustration, bittersweet relief and excited uncertainty. These are proposed performance prompts, not claimed dataset labels or fixed numerical blend ratios. |

### Concrete proposed sequence

1. **Audit the laptop warmth check:** preserve model/checkpoint/version, label vocabulary, preprocessing, reference/render pair IDs and full raw outputs. Run original and clone through the same path; obtain blind independent audio-only ratings for warmth and anger before interpreting a detector zero.
2. **Capture references:** with consent and the project's existing capture/QC protocol, record the twelve anchor emotions and selected deliberate blends using matched scripts, multiple takes and contextual performance instructions. Keep clean masters; introduce environment/noise variants as a separate test factor.
3. **Annotate:** ask each listener which emotions coexist and how strongly, then VAD and confidence; retain raw ratings, language/context and provenance. Collect separate whole-utterance and shorter-span judgments to distinguish coexistence from rapid transitions.
4. **Re-score transparently:** calibrate on held-out speakers/sessions/scripts; compare clone versus reference distributions, VAD and paired listening. Publish uncertainty and subgroup/component failures, including warmth and anger, without silently replacing the current gate.
5. **Validate a candidate successor metric:** compare the nebula metric against the existing metric and human preference on unseen material. The existing **0.83 project gate is not validated or superseded by these papers**; no new score or pass claim is produced here.

## 3. Caveats

- **Nebula is a metaphor**, not an established anatomical mechanism or evidence that emotion literally consists of additive particles. Twelve labels are a project design choice, not twelve proven neural primitives.
- Felt emotion, expressed emotion, intended acting and listener-perceived emotion are different targets. Speech recognition primarily estimates perceived expression, not private mental state.
- A softmax/vote distribution describes relative label mass; it does not directly measure simultaneous independent intensities. Two people disagreeing angry versus sad differs from both people hearing anger and sadness together: preserve per-rater multi-label data to tell these apart.
- Whole-clip blends may be rapid alternation. Short-span annotations and time trajectories are required before claiming momentary coexistence; temporal resolution and rater response lag constrain conclusions.
- VAD averages can hide bimodality and opposite-valence mixtures. Gaussian uncertainty models are assumptions; inspect empirical distributions and consider mixture models where justified.
- Acoustic noise, microphone, compression, accent, language, semantics, identity and acting conventions can bias model outputs. Control these factors and validate intended listening environments; no universal score threshold follows from the cited studies.
- Warmth/love is not a universally available class in speech models. Do not remove a difficult category or modify targets merely to pass a quality gate, and do not infer missing emotion solely from a detector output.
- AmbER2 is cited as a 2026 preprint, not established consensus. Its research splits/results do not certify generalization to Samantha or the project's twelve labels; emotion2vec likewise requires downstream validation.
- New recording/training requires appropriate speaker consent and provenance; this research authorizes no recording, training, merge or deployment.

## 4. Sources and link checks

Links checked on 7 November 2026 (Europe/Zagreb). Wiley, SAGE and arXiv pages loaded through WebFetch; PMC pages returned HTTP 200 through a direct public-page request after WebFetch's provider returned 403. PubMed returned HTTP 203 to a public request and its abstract was available in web-search results (WebFetch rate-limited); this is an abstract citation, not a full-text claim. The Cell publisher URL was blocked, so it is not used as the review link.

- Russell (2017), *Mixed Emotions Viewed from the Psychological Constructionist Perspective*: https://journals.sagepub.com/doi/10.1177/1754073916639658
- Barrett (2017), *The theory of constructed emotion: an active inference account of interoception and categorization*: https://pmc.ncbi.nlm.nih.gov/articles/PMC5390700/
- Larsen & McGraw (2014), *The Case for Mixed Emotions*: https://onlinelibrary.wiley.com/doi/full/10.1111/spc3.12108
- *Emotional Granularity Effects on Event-Related Brain Potentials during Affective Picture Processing* (2017): https://pmc.ncbi.nlm.nih.gov/articles/PMC5364149/
- Schirmer & Kotz (2006), *Beyond the right hemisphere: brain mechanisms mediating vocal emotional processing*: https://pubmed.ncbi.nlm.nih.gov/16321562/
- *Effects of emotional valence and arousal on the voice perception network* (2017): https://pmc.ncbi.nlm.nih.gov/articles/PMC5597854/
- Wu, Zhang & Woodland (2023), *Estimating the Uncertainty in Emotion Attributes using Deep Evidential Regression*: https://arxiv.org/html/2306.06760
- Ma et al. (2023), *emotion2vec: Self-Supervised Pre-Training for Speech Emotion Representation*: https://arxiv.org/html/2312.15185
- Wu et al. (2026 preprint), *AmbER2: Dual Ambiguity-Aware Emotion Recognition Applied to Speech and text*: https://arxiv.org/html/2601.18010v1
