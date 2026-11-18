# R2 — Ranking, generation, and Eureka Moment Pipelines

HORBM research note, agent R2 of 4. Research only.

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. Written 2026-11-18.

The spelling is **Galion**. The product is Galion Voice. HORBM here means
Heuristic Open Retrieval Brain Model: a CPU-first, Rust-first brain whose
knowledge lives in a compressed diary of sourced claims, and which learns
from later human reaction. This note covers one slice of that brain. It
replaces the two pieces XiaoIce used to pick and write a reply — the
GRU-RNN generator and the boosted-tree ranker — and it describes a
reasoning loop, the Eureka Moment Pipeline, that chains retrieved steps
and checks them. Memory layout, persona prose, and the sandbox transport
are other agents' areas. This note says what it needs from them and does
not invent their schemas.

This file is not [`docs/galion-voice-v2/symphony/EUREKA.md`](../../galion-voice-v2/symphony/EUREKA.md).
That page, dated 2026-11-03, is the symphony clock standard. Its champion
line is "A conversation is one heartbeat, not a pipeline." The pipeline
below runs beside that clock. It does not enter `Heartbeat::step`.

## How to read this note

Every claim is tagged.

| Tag | Meaning |
|---|---|
| **Fact** | Someone said it. The source, the date, and who said it are in the sentence or in §12. |
| **Opinion** | A design choice of this note, 2026-11-18, agent R2. It is not a measurement. |
| **Unknown** | Not established by the sources read here, and not measured here. |

This session did not run a benchmark, did not measure latency, did not
download a model or a dataset, and did not call a paid API. Web pages and
papers listed in §12 were read. Figures quoted from them are their
figures, on their data, on their hardware. None of them is a Galion
number.

`galion-tensor` says "No third-party inference runtimes" and that weights
will come from Galion's own `.gq` format
([`crates/galion-tensor/src/lib.rs`](../../../crates/galion-tensor/src/lib.rs),
lines 3–9, read 2026-11-18). The owner ruling of 2026-11-04 20:45 CET,
recorded in
[`SYMPHONY-STACK-V2-PLAN.md`](../../galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md),
keeps Qwen, Llama, llama.cpp, Groq, and any cloud LLM out of the
architecture. The reasoning slot is an in-house S0 student of about 0.5B
parameters, `.gq`, W8A8, on `galion-tensor`, with `CannedLlm` until
weights exist. The teacher is an open owner decision. This note does not
revoke that ruling. It also does not assume the student exists. The same
plan says the student weights were not in tree on 2026-11-03.
[`OPENMIND-NERVE-CORTEX.md`](../../galion-voice-v2/OPENMIND-NERVE-CORTEX.md)
§7, read 2026-11-18, still describes the student as a silent stub that
does not load weights, and says a later turn may repeat an exact line
from a local store the operator already placed. That stored line is the
text. It is not a student generation.

## 1. What XiaoIce actually ran

**Fact.** Zhou, Gao, Li, and Shum, "The Design and Implementation of
XiaoIce, an Empathetic Social Chatbot," arXiv:1812.08989, December 2018.
The journal version is *Computational Linguistics* 46(1), 2020 (ACL
Anthology `2020.cl-1.2`; MIT Press URL path `/coli/article/46/1/53/`).
Microsoft Research lists the same work as MSR-TR-2018-42, December 2018.
The quotes below are from the arXiv HTML read on 2026-11-18.

**Fact.** General Chat takes a dialogue state
`s = (Q_c, C, e_Q, e_R)` and answers in two stages: candidate generation,
then ranking. There are three generators.

1. **Paired retrieval.** Query–response pairs from the public web and, after
   the May 2014 launch, from XiaoIce's own chats. Zhou et al. write that
   by May 2018 this was more than 30 billion pairs, and that 70% of her
   responses were retrieved from her own past conversations. The index is
   Lucene. At runtime `Q_c` retrieves up to 400 candidates by keyword
   search and by semantic search.
2. **Neural generator.** A GRU-RNN in the speaker–addressee style they
   attribute to Li et al. 2016b. The query empathy vector and the response
   empathy vector are combined into a vector `v` and injected at every
   decoder step. They use beam search and keep up to 20 candidates. They
   describe this generator as the one that can answer topics missing from
   the paired store, and they describe its outputs as well-formed and
   often short. They cite the seq2seq line (Sutskever et al. 2014, Cho et
   al. 2014, and the dialogue papers they list) as the frame.
3. **Unpaired retrieval.** Sentences from lectures and news, filtered to
   the persona, indexed in Lucene. A knowledge-graph expansion adds
   related topics, those topics are themselves ranked by a boosted tree,
   and the expanded query retrieves up to 400 sentences. They describe
   these candidates as longer and more informative than the neural ones,
   and lower in quality than the paired hits.

**Fact.** The three lists are merged and ranked by a boosted tree. Zhou
et al. cite Wu et al. 2010. A reply is drawn at random from the candidates
whose score clears a threshold. Given `s`, each candidate gets four
feature groups:

- local cohesion between `Q_c` and the candidate, from DSSMs trained on
  conversation pairs (they cite Huang et al. 2013, Shen et al. 2014, Gao
  et al. 2014, and they define a DSSM as a model of semantic similarity
  of a pair);
- global coherence with `Q_c` and the context `C`, from DSSMs trained on
  full sessions, which they say matters when the query is bland ("OK",
  "why", "I don't know");
- empathy match between the candidate's empathy vector and the desired
  `e_R`;
- retrieval match, for paired hits only: BM25, TF-IDF, and DSSM between
  `Q_c` and the query side of the stored pair.

**Fact.** The ranker is trained on pairs `(s, R)` labeled on three levels:
0, likely to end the conversation; 1, acceptable and likely to keep it
going; 2, empathetic, in persona, and likely to drive it. In the hybrid
pilot they report, the ranker was trained on 50K manually labeled
dialogues, and the neural generator plus the feature models were trained
on 50 million human dialogues. If every generator fails, times out, or
the query is improper, an editorial response is chosen. Their examples of
editorial lines are stock phrases such as "Hmmm, difficult to say. What
do you think?"

**Fact.** Their second pilot, on 4K dialogue sessions, three human judges,
the same ranker for every system (Table 3 in the arXiv HTML):

| System | Average rating | Share of 0 | Share of 1 | Share of 2 |
|---|---:|---:|---:|---:|
| Retrieval only | 0.87 | 35.0% | 42.9% | 22.1% |
| Neural generator only | 0.40 | 66.6% | 27.3% | 6.1% |
| Hybrid (retrieval + neural) | 1.09 | 23.1% | 44.6% | 32.3% |

They write that the hybrid "significantly improves" the human rating over
retrieval alone. Their two-week A/B test, against a paired-retrieval
baseline, found that adding the neural generator and the unpaired
generator raised expected conversation-turns per session of Core Chat by
0.5, and they attribute the gain mainly to coverage: distinct acceptable
responses (ratings 1 or 2) up 20% from the neural generator and 10% from
the unpaired generator. Those percentages and the 0.5 CPS figure are
theirs, on their traffic. This note did not recompute them.

**Fact.** The boosted-tree citation is Wu, Burges, Svore, and Gao,
"Adapting boosting for information retrieval measures." The Springer page
read on 2026-11-18 dates the online publication 3 September 2009, in the
journal *Information Retrieval*. Zhou et al. cite it as 2010. The abstract
on that page says the algorithm uses boosted regression trees and is
"significantly faster in both train and test phases than the state of the
art, for comparable accuracy." This note did not re-read their speed
tables, so no speed factor from Wu et al. is repeated here. Burges's
overview "From RankNet to LambdaRank to LambdaMART" (Microsoft Research
technical report, 2010) is the paper Hofstätter et al. 2020 cite for the
same family. This note did not re-open that report. No figure is taken
from it. The name used below, LambdaMART, means that family: gradient
boosted trees trained for a ranking metric.

**Opinion.** The GRU is the piece to retire from the live path, and the
boosted tree is the piece to keep and extend. Zhou et al. already showed,
on their judges, that the neural generator alone was the weak system
(average 0.40) and that its value was coverage. A CPU-first diary can buy
coverage by storing more sourced spans. It cannot buy coverage by running
a free decoder inside an 80 ms clock. The hybrid's jump from 0.87 to 1.09
on their table is the best argument for keeping some generator. §9 faces
that argument. It is not waved away.

## 2. What this repository already fixes

**Fact.** [`EUREKA.md`](../../galion-voice-v2/symphony/EUREKA.md),
2026-11-03: one 80 ms clock; `Heartbeat::step` does not block and does not
allocate; the turn worker stays off that call.

**Fact.** The symphony plan, 2026-11-03, with the 2026-11-04 ruling quoted
in §0 of this note: core 0 is bus, heartbeat, audio, and a low-priority
retrieval worker; cores 2–5 are the in-house reasoning slot; Galion
Research is the side band and is never on the tick. The same plan's
memory sketch is SQLite FTS5 plus int8 vectors, and its default vector
scan is an exact brute-force int8 dot. `galion-tensor` implements
`gemv_i8` ([`crates/galion-tensor/src/gemv_i8.rs`](../../../crates/galion-tensor/src/gemv_i8.rs),
present in the tree on 2026-11-18). Whether that scan meets a turn budget
at diary scale is **unknown**. This note did not measure it.

**Fact.** The MnE contract README
([`tools/symphony/mne-contract/README.md`](../../../tools/symphony/mne-contract/README.md)):
an intent document already carries a persona id, a memory id, an optional
memory snippet, and a UTF-8 span into the utterance. The feeling is a
point (arousal 0..1, valence −1..1, dominance 0..1), not a sentence.

**Fact.** [`docs/galion-voice-v2/personas/README.md`](../../galion-voice-v2/personas/README.md),
the empathy standing rule: "Do not speak a scripted or canned empathy
line. The words stay on the matter. The feeling is in how you say them."
Galion One is the product voice. Galion Research is the research voice.

**Fact.** A search of this checkout on 2026-11-18 found no HORBM design
and no agent-tool protocol for a reasoning sandbox. The word "sandbox" in
`docs/` refers to systemd sandboxing and, in
[`docs/galion-context/2026-11-08/04-openmind-stack.md`](../../galion-context/2026-11-08/04-openmind-stack.md),
to a Galion OS ARM-sandbox plan whose status that page calls planning.
The API a live brain would call is **unknown**.

## 3. Recommended design

**Opinion.** HORBM keeps XiaoIce's split — many candidate sources, one
ranker, a threshold, a fallback — and changes what the candidates are
made of. A candidate is a claim, or a small chain of claims, not a
freshly decoded sentence. The live ranker is still a boosted tree. The
GRU does not run on the turn. Gaps in a retrieved reply are filled by
copying a marked slot from another claim. A neural rewriter is a later,
optional, off-tick proposer, and every sentence it proposes is rejected
unless each factual clause points at a claim the checker accepts.

The live turn, on the reasoning cores, after core 0 has prefetched:

1. Take the candidate set already in RAM: paired diary cases, unpaired
   notes, knowledge-graph neighbors, and any sandbox result that has
   already come back as a claim.
2. Score them with the boosted tree on stored columns plus query-time
   lexical scores (BM25 / FTS). This is the whole live rank.
3. If the margin between the top two scores is wide, speak the winner
   after the checks in §8. If the margin is thin, ask the side band for a
   mini cross-encoder opinion on that handful only. Do not wait inside
   the heartbeat. If the opinion misses the turn, speak the tree's winner
   if it passed §8, or speak a sourced holding clause.
4. Speak by slot filling. A factual clause is either a stored span or a
   frame around stored spans ("On {source_date}, {said_by} said {span}").
   Persona and feeling go out through the MnE fields that already exist:
   persona id, memory id = case or chain id, memory snippet = the shortest
   supporting span. This note does not compute the feeling point.
5. When the person reacts, log the candidate set, the selection policy,
   and the reaction. Training of the tree happens later, off the tick.

The Eureka Moment Pipeline (§7) is the side-band loop for questions one
retrieved reply cannot close. It searches over retrieved steps. It writes
a new intermediate sentence only when the optional rewriter is on, and
only as a proposal. Galion Research is where that loop sits, matching the
plan's rule that the cortex is never on the tick. Galion One speaks the
result on a later clause, or speaks the holding clause now.

**Opinion.** Knowledge lives in the diary, in spans and columns, which is
the compression this design actually uses: one stored episode, reused,
instead of a weight matrix that is hoped to remember it. Quantizing a
network (Jacob et al., CVPR 2018) is a different compression, of
arithmetic, and it applies only to the optional rewriter and the optional
reranker. It is not the diary.

## 4. Options

**Opinion**, except the "Reported by" column, which is fact.

| Option | Role | On the 80 ms tick? | Keeps source, date, speaker? | Third-party weights? | Reported by | Main limit |
|---|---|---|---|---|---|---|
| GRU-RNN, beam 20, empathy vector injected each step | XiaoIce neural generator | No. Theirs was a service, and our clock forbids it | No span. Persona is a hidden vector | Theirs, trained in-house at Microsoft | Zhou et al. 2018, §1 above | Their neural-only average rating was 0.40. Outputs often short. No provenance |
| Paired diary retrieval, lexical + stored semantic score, cap in the hundreds | Primary generator | The prefetch is off the tick. The lookup result is read by the turn worker | Yes, if the case record has them | No | Zhou et al.: up to 400 from Lucene; 70% of replies from her own chats by May 2018 | Misses topics the diary does not contain. BM25 misses paraphrases, which is why they also used DSSM |
| Unpaired spans + graph neighbors | Coverage generator | Same as paired | Yes, if author and date were stored | No | Zhou et al.: up to 400, KG expansion ranked by a boosted tree | They judged this stream lower quality than paired hits. A raw span can be unspeakable |
| Boosted tree (LambdaMART / LightGBM `lambdarank`) on cheap features | Live ranker, the successor of their tree | The scorer runs in the turn worker, off `Heartbeat::step` | The tree ranks records that already carry provenance. The tree does not invent text | Training can link LightGBM. Live inference should be a data file, not a new runtime inside `galion-tensor` | Wu et al., online 2009, cited by Zhou et al. as 2010; LightGBM: Ke et al., NIPS 2017; `lambdarank` is in the LightGBM docs fetched 2026-11-18 | Needs labels. Raw "user continued" is a biased label (§5) |
| Mini cross-encoder (MiniLM-class, int8), MS MARCO-style | Slow second opinion on a handful of pairs | No | It scores a pair. It does not record who said the passage | A public checkpoint is third-party. An in-house `.gq` encoder would not be | Wang et al., arXiv:2002.10957, submitted 2020-02-25; model card `cross-encoder/ms-marco-MiniLM-L6-v2`, read 2026-11-18; cost model in Khattab and Zaharia, SIGIR 2020 | One forward pass per pair. Trained on web passages, not on Galion turns or on persona |
| Late interaction or dual-encoder score, document side precomputed | A feature column, successor of their DSSM | The query side is one encode per turn, off the tick. The document side is computed at retain time | The column is a score. The span stays attached to the row | Same checkpoint question as the row above | Khattab and Zaharia, SIGIR, published 2020-07-25; Karpukhin et al., arXiv:2004.04906, 2020 | Wrong neighbors when the diary's language is far from the encoder's training text. Rate unknown here |
| Template plus marked-slot fill | The gap filler that replaces the GRU for schema-shaped replies | String work in the turn worker | Slots are copies of claim fields | No | XiaoIce editorial fallback is the in-family precedent for a written line when generators fail. The slot rule is this note | Breaks when the slot was never marked. Stock feeling lines conflict with the persona standing rule |
| Case reuse, with revise limited to marked slots | The CBR cycle | Off the tick for retain; reuse is the live fill | The past case is the source | No | Aamodt and Plaza, *AI Communications* 7(1), 39–59, 1994: retrieve, reuse, revise, retain | Revise is the unsolved part. Copying an unmarked entity into a new user's turn is how a diary lies |
| Tiny quantized rewriter, short, off the tick | Optional coverage tool for a residual gap | No | Only if the output is rejected unless every factual clause cites a claim | Open. `.gq` on `galion-tensor` matches the crate rule. candle, ort, and tract are third-party runtimes (§6) | Distillation: Hinton, Vinyals, and Dean, arXiv:1503.02531, 2015. int8: Jacob et al., CVPR 2018 | Distillation copies behavior. It does not add a source. Quantization results they report are on vision and, for Hinton, on MNIST and speech acoustics, not on dialogue truth |
| S0 student, about 0.5B, `.gq`, W8A8 | The plan's reasoning slot, if weights ever exist | No. Cores 2–5, off the heartbeat | Same citation rule as the rewriter | Teacher is an open owner decision. Qwen and Llama are excluded by the 2026-11-04 ruling | Symphony plan, 2026-11-03 and 2026-11-04 | Weights were not in tree. Quality at that size on this voice is **unknown** |
| Beam search over retrieved steps | The chain builder | Side band | Each step is a claim | No | Zhou et al. already use beam search inside the GRU (width 20). Asai et al., arXiv:1911.10470, v2 2020-02-14, retrieve a path and extract a span | Cannot invent a bridge sentence. Depth cap is a policy, not a theorem |
| MCTS / UCT over the same steps | A side-band search used when the beam is tied and nobody is waiting | Side band, idle only | Same | No | Algorithm name: Kocsis and Szepesvári, ECML 2006 (UCT); Coulom, Computers and Games 2006 (MCTS). PDFs not re-read this session. No number from them is used. Rust crates `mcts` and `mocats` on docs.rs expose a UCT policy; read 2026-11-18 | Needs a rollout value. A heuristic value is not a proof. Easy to burn CPU on a wide diary |
| NLI contradiction / entailment on a pair | Checker, not a generator | Side band, and only on the chosen chain | The premise is the source span | A public NLI checkpoint is third-party; an in-house one would not be | Williams et al., NAACL 2018, MultiNLI. Honovich et al., NAACL 2022, TRUE | Entailment of a span is not truth of the world. Sentence-pair models miss long chains. Rate on Galion speech is **unknown** |

**Opinion.** The row to ship first is: paired diary + unpaired spans +
boosted tree + slot fill + the §8 checker. The mini cross-encoder, the
rewriter, S0, and MCTS are later levers. Each one needs an owner decision
before a third-party weight or a third-party runtime enters the machine.

## 5. The ranker

### 5.1 Why the tree stays

**Fact.** Zhou et al. already ranked hundreds of candidates with a boosted
tree, in production, from 2014 onward, and they trained it on graded
human labels (0, 1, 2). Wu et al.'s abstract claims a test-time speed
advantage for boosted regression trees against the rankers of that time.
Ke, Meng, Finley, Wang, Chen, Ma, Ye, and Liu, "LightGBM: A Highly
Efficient Gradient Boosting Decision Tree," NIPS 2017, describe a GBDT
implementation whose contribution is training speed (GOSS and exclusive
feature bundling), not a dialogue result. The LightGBM parameter page
fetched on 2026-11-18 lists a `lambdarank` objective: integer labels,
larger means more relevant. That is the same shape as XiaoIce's 0/1/2
grades. The docs also list `rank_xendcg` and say it is faster than
`lambdarank` with similar performance. "Similar" is their claim. It was
not remeasured here.

**Fact.** A cross-encoder's cost is structural. Khattab and Zaharia,
"ColBERT," SIGIR 2020 (published 2020-07-25): ranking models that feed
each query–document pair through a deep network "increase computational
cost by orders of magnitude," and ColBERT's late interaction is their
answer, with document vectors precomputed. They report, on their passage
sets and hardware, about two orders of magnitude faster query processing
than the BERT rankers they compare, and up to four orders of magnitude
fewer FLOPs. Those factors are theirs. They are not a Galion CPU
measurement, and this note does not adopt them as a budget. Nogueira and
Cho, "Passage Re-ranking with BERT," arXiv:1901.04085, 2019, are the
cross-encoder result those later papers react to: BERT as a reranker,
then state of the art on MS MARCO passage ranking by their report.
Hofstätter, Althammer, Schröder, Sertkan, and Hanbury, arXiv:2010.02666,
submitted 2020-10-06, v2 2021-01-22, distill a concatenated BERT teacher
into cheaper students (TK, ColBERT, PreTT, and a CLS dot-product) with a
margin MSE loss, so the student keeps the teacher's pairwise margins.
They state that query latency depends on the architecture. Wang, Wei,
Dong, Bao, Yang, and Zhou, "MiniLM," arXiv:2002.10957, submitted
2020-02-25, distill a transformer's self-attention into a smaller
transformer. The Hugging Face card `cross-encoder/ms-marco-MiniLM-L6-v2`,
read 2026-11-18, is a MiniLM cross-encoder trained on MS MARCO passage
ranking. Its card publishes an NDCG / MRR table. The excerpt read here
does not state the machine behind the "docs/sec" column, so those rates
are not used.

**Opinion.** The live ranker is the tree. A MiniLM-class cross-encoder is
a second opinion on a handful of pairs when the tree's margin is thin,
run on the side band, scores treated as one more feature or as a veto,
never as the persona. An MS MARCO checkpoint does not know XiaoIce's
empathy grades, does not know a Galion persona, and does not know which
source is fresher. Using it as the only ranker would throw away the four
feature groups Zhou et al. found worth computing. A late-interaction or
dual-encoder score, with the document side stored when the case is
retained, is the honest successor of their DSSM feature: one query encode
per turn, then dots. Karpukhin et al., "Dense Passage Retrieval,"
arXiv:2004.04906, 2020, report that a dual encoder beat a Lucene BM25
baseline by 9 to 19 absolute points of top-20 accuracy on their
open-domain QA sets. That is a reason to keep a dense feature. It is also
their data. Zhou et al. kept BM25 and TF-IDF beside DSSM. **Opinion:**
keep the lexical index. The symphony plan already points at SQLite FTS5.
Tantivy (quickwit-oss/tantivy, MIT, repository created 2016-01-11, README
read 2026-11-18) is a Rust index in the Lucene style and states that it
scores with BM25. It is an alternative if a later note wants Lucene's
shape inside the process. This note does not move the store.

### 5.2 Features

**Opinion.** The four XiaoIce groups survive, with provenance added,
because the truth rule of this swarm is that every claim keeps its
source, its date, and who said it.

| Column | When it is computed | Ancestor |
|---|---|---|
| BM25 / FTS score of the case against the current query | Query time, from the postings | Their retrieval-match group |
| Precomputed late-interaction or dual-encoder score | Document side at retain time; query side once per turn | Their DSSM cohesion / coherence |
| Distance between the case's stored feeling point and the current MnE target | Query time, three subtractions | Their empathy-match group. The point is the contract's arousal, valence, dominance. This note does not set the target |
| Persona id match | Query time | Their persona filter on the unpaired store |
| Source present, speaker present, source date present, valid interval overlaps "now" | Stored on the claim | New. Their topic ranker had a separate freshness feature; the reply ranker as described did not |
| Historical reaction of this case, debiased (§5.3) | Stored, updated off the tick | Their topic ranker used an acceptance rate. The reply ranker used hand labels |
| Checker flags: slot clash, missing citation | Query time, on the shortlist only | New |

**Opinion.** Text is not a feature. The tree reads columns. The utterance
text is touched only for the case that will be spoken and for the handful
sent to the cross-encoder. That is the practical fix for "the diary is
huge": the live scorer's working set is rows of numbers, which is also
the sense in which the diary is compressed knowledge rather than a replay
of every past prompt.

### 5.3 Learning from the person

**Fact.** Zhou et al. train the reply ranker on hand labels, and they
sample at random among candidates above a threshold, which means the
spoken line is not always the single maximum. They also write, early in
the paper, that they ask users for explicit feedback when a new module is
under test, and that they A/B test on expected CPS. Joachims, Swaminathan,
and Schnabel, "Unbiased Learning-to-Rank with Biased Feedback," WSDM
2017, published 2 February 2017: clicks and dwell are biased, position
bias in particular, and training on raw clicks is the wrong risk. They
give a propensity-weighted ranking SVM and say it still applies when
queries do not repeat, provided a click model estimates the propensity.
The LightGBM advanced-topics page fetched 2026-11-18 describes a different
mechanism for the same worry: fit `s(x, pos) = f(x) + g(pos)` and keep
`f` as the unbiased score. That page attributes the idea to click-model
papers it names. Those papers were not re-read here.

**Opinion.** Copy the shape, not their web-search UI.

- The seed is a small set of 0/1/2 labels in Zhou et al.'s sense. Hand
  labels are the only unbiased seed this design trusts. How many Galion
  needs is **unknown**. Their 50K is a deployed-bot figure, not a minimum.
- The online signal is the reaction to the one spoken line: the person
  continues, corrects a slot, goes quiet, or gives an explicit rating if
  the product ever has one. Mapping those events onto 0/1/2 is a labeling
  policy. It is **unknown** whether "continued the conversation" means
  "good reply" or "polite."
- There is no rank position, because the voice speaks one line. Joachims's
  position propensity does not transfer as a formula. What does transfer
  is the idea of a propensity: log the candidate set and the sampling
  policy, and weight the update by the inverse probability that this
  candidate was the one spoken. Their threshold sample makes that
  probability computable. A proof of unbiasedness for this voice policy
  was not written here. It is a research item, not a property the system
  has on day one.
- Fit the tree off the tick. The live process loads a snapshot. A bad
  update must be reversible by putting the previous snapshot back.
- Do not let the rewriter's new sentences become training targets unless
  they passed §8. Otherwise the tree learns to prefer fluent unsupported
  text, which is the failure Zhou et al.'s neural-only column already
  shows on their judges.

**Fact.** The docs.rs crate `lightgbm`, read 2026-11-18, exposes
`Booster` and `Dataset` as wrappers. A Rust caller of that crate still
links the LightGBM library. The ONNX Runtime README, repository
`microsoft/onnxruntime`, MIT, created 2018-11-10, read 2026-11-18, says
the runtime can execute models from classical libraries including
LightGBM. **Opinion.** Train with LightGBM off the box or off the tick.
Ship the live scorer as a tree file walked by a small Rust loop, or as an
ONNX file if and only if the owner has already accepted ONNX Runtime for
something else. Do not add a C++ inference dependency to the turn worker
just for the ranker. This note did not re-read the LightGBM LICENSE file.
Confirm it before vendoring. A pure-Rust trainer that matches
`lambdarank` was not identified in this session. **Unknown** whether one
is mature enough to trust with the seed labels.

## 6. What replaces the GRU

**Fact.** Zhou et al. say the generator is a GRU-RNN, similar to the
speaker–addressee model of Li, Galley, Brockett, Gao, and Dolan, ACL 2016
(their reference Li et al. 2016b). They place it in the seq2seq frame of
Sutskever, Vinyals, and Le, NIPS 2014, and of Cho, van Merrienboer,
Bahdanau, and Bengio, SSST-8, October 2014, which is the Cho et al. 2014
entry in Zhou et al.'s own bibliography. This note did not re-open the
Sutskever or Cho papers. Zhou et al. are enough for the architectural
claim: a recurrent decoder, beam 20, persona injected as a vector, no
retrieved span in the output. **Opinion.** A GRU is already a small CPU model. The
reason to remove it is the quality and the missing source, not a belief
that a recurrent net is too heavy. Replacing it with a larger transformer
decoder would spend more CPU to keep the same provenance hole. Gholami,
Yao, Kim, Hooper, Mahoney, and Keutzer, "AI and Memory Wall,"
arXiv:2403.14123 (2024), write in the abstract that they analyze encoder
and decoder transformers and that memory bandwidth can dominate for
decoder models. Pope, Douglas, Chowdhery, and colleagues, "Efficiently
Scaling Transformer Inference," arXiv:2211.05102v1, 9 November 2022,
describe generative inference of very large transformers on TPU slices:
weights and the KV cache move from device memory every step, and at small
batch the weight load dominates. Their 29 ms/token figure is for PaLM
540B on TPU v4 with int8. It is not a CPU figure and it is not used as
one. **Opinion.** The qualitative warning still applies to a small
autoregressive decoder on a CPU: each new token rereads the weights. An
encoder reranker is a different regime, one pass per pair. This note did
not measure either regime on Galion hardware.

**Opinion.** The replacement, in order, is:

1. **Retrieve** a past case (Aamodt and Plaza's retrieve). The diary is
   the paired database, with the claim fields of §8 filled in. This is
   also what `OPENMIND-NERVE-CORTEX.md` already allows when it says a
   later turn can repeat an exact stored line.
2. **Reuse** by speaking a span, or a frame whose slots are copies.
   Substitution is allowed only for slots that were marked when the case
   was stored: names, dates, places, memory ids. An unmarked token is not
   a slot. Guessing it is the GRU's old job, and it is refused.
3. **Revise** only by the checker. If the filled reply clashes with a
   higher-priority claim, drop it and try the next case. Repair does not
   mean "ask a decoder to smooth it."
4. **Hold** if nothing clears the bar. The holding clause names the
   missing slot and, if relevant, that the source is missing or undated.
   It does not perform worry or warmth. The persona standing rule forbids
   a scripted empathy line. XiaoIce's "Hmmm, difficult to say" would fail
   that rule if the words were doing the feeling. The feeling, if the
   turn has one, is the MnE point, computed by the feeling path this note
   does not own.
5. **Retain** the spoken chain, the claims it used, and, when it arrives,
   the reaction (§5.3). Aamodt and Plaza also retain failures so the next
   similar failure is recognizable. **Opinion:** a rejected chain is
   stored as a failure case with the reason (clash, missing source,
   checker veto). It is not stored as a line the ranker may speak.

**Opinion.** An optional rewriter exists for one situation: every slot of
a good case is filled, the span is true and unspeakable (a fragment, a
list, the wrong language), and a grammatical wrapper would still be
entailed by the span. The rewriter may propose the wrapper. §8 must label
the proposal entailed by the cited spans. Neutral or contradictory
proposals are dropped and the raw span is spoken, or the holding clause
is spoken if the span itself cannot be said aloud.

Runtime for that rewriter, if the owner later allows one:

| Runtime | What it is | Fits `galion-tensor`'s "no third-party inference runtime"? | Note |
|---|---|---|---|
| `galion-tensor` `.gq` | In-tree CPU runtime, int8 GEMV, KV ring | Yes. This is the crate | Weights are future. The S0 teacher is an open owner decision |
| `huggingface/candle` | Rust ML library, Apache-2.0, repository created 2023-06-19. README read 2026-11-18 lists CPU, CUDA, and WASM backends, and lists Qwen and Llama among example models | No. It is a third-party runtime | Using the library is not shipping those examples. The 2026-11-04 ruling still excludes those models from the architecture |
| `pykeio/ort` | Rust bindings, Apache-2.0, repository created 2022-11-26, wrapping ONNX Runtime | No | ONNX Runtime itself is MIT, repository created 2018-11-10. The README of `ort` lists FastEmbed-rs as a user for local embeddings and reranking. That is an existence claim by the `ort` authors, not a Galion benchmark |
| `sonos/tract` | Rust ONNX / NNEF inference, repository created 2017-08-07. README read 2026-11-18 says MIT/Apache-2.0 and says Sonos uses it in production for wake-word and streaming speech recognition | No | GitHub's license field for that repository returned "Other" on 2026-11-18. Re-read the license files before any vendoring. The production sentence is Sonos's |

**Opinion.** Prefer no rewriter. If one is approved, prefer a `.gq` file
on `galion-tensor`, because that is the rule already written in the
crate. candle, ort, and tract belong in a side process, under an owner
exception of the same kind the symphony plan already uses for other
runtimes, and they stay off the tick. Jacob et al., CVPR 2018, report
that quantizing weights and activations to 8-bit integers cut the memory
footprint by about 4× versus 32-bit floats and was close to
floating-point accuracy on MobileNets for ImageNet and COCO, on ARM CPUs.
That is a vision result. It supports int8 as the arithmetic for a small
encoder or a small rewriter. It says nothing about whether the rewritten
sentence is true. Hinton, Vinyals, and Dean, arXiv:1503.02531, 2015, show
distillation of an ensemble into one model, on MNIST and on a commercial
acoustic model. Distillation is how a tiny rewriter would be trained from
a larger teacher. The teacher, if it is ever allowed, is the same open
owner decision as S0. Distillation transfers the teacher's outputs,
including the teacher's unsupported sentences, unless the §8 filter is
the training filter too.

## 7. Eureka Moment Pipelines

**Opinion.** The name is this note's, for a control loop. It is not a
published system. Graham Wallas, *The Art of Thought* (1926), is often
credited with four stages — preparation, incubation, illumination,
verification. That book was not re-read this session, and no psychological
claim is taken from it. The stages below are an engineering schedule that
happens to use those four words. A chain "illuminating" means a predicate
became true. It is not a feeling, and it must not be performed as one.

The published pattern it compresses is two papers, and it drops the part
of them that needs a large generator.

**Fact.** Trivedi, Balasubramanian, Khot, and Sabharwal, "Interleaving
Retrieval with Chain-of-Thought Reasoning for Knowledge-Intensive
Multi-Step Questions" (IRCoT), arXiv:2212.10509, submitted 2022-12-20, v2
2023-06-23. They alternate a generated chain-of-thought sentence with a
retrieval that uses that sentence as the next query. With GPT-3 they
report retrieval gains up to 21 points and QA gains up to 15 points on
HotpotQA, 2WikiMultihopQA, MuSiQue, and IIRC. They also report similar
gains with Flan-T5-large and no extra training. Both of those are
generative models. The next sentence is written, then used as a query.
**Fact.** Asai, Hashimoto, Hajishirzi, Socher, and Xiong, "Learning to
Retrieve Reasoning Paths over Wikipedia Graph for Question Answering,"
arXiv:1911.10470, submitted 2019-11-24, v2 2020-02-14. A recurrent
retriever picks the next paragraph conditioned on the paragraphs already
chosen. The reader ranks paths and extracts an answer span. They report
a gain of more than 14 points on HotpotQA over the previous best model
they compare. The reader extracts. It does not compose a new sentence.
Their retriever is still a neural sequence model.

**Opinion.** The non-LLM pipeline takes Asai's shape and uses XiaoIce's
retrievers as the action space.

1. **Preparation.** As soon as a partial transcript is stable, core 0
   retrieves, the way the symphony plan already asks the retrieval worker
   to prefetch while the person is talking. The query is the contextual
   query. Zhou et al. rewrite "him" to a name in their contextual-query
   unit. The rewrite rules are not in this checkout. **Unknown:** which
   coreference rules Galion will use. Until they exist, the query is the
   literal transcript plus the open memory id, and chains that depend on
   a resolved pronoun are held.
2. **Incubation.** On the side band, a beam of partial chains. A node is
   a retrieved claim, not a generated sentence. The next query is the text
   of the last claim, which is IRCoT's retrieval step with the generator
   removed. Branching factor is the shortlist the tree already scored,
   not the whole diary. Depth is a cap. The cap's right value is
   **unknown**. Start from a small cap (a few hops) because each extra
   hop multiplies checker cost and, past the evidence, adds places for a
   false join.
3. **Illumination.** A chain is accepted when all of the following hold:
   every hop has a source span, a speaker, and a date or is explicitly
   marked undated; every open slot of the question is filled by one of
   those spans; no two filled slots clash (§8); the tree score of the
   chain clears the threshold. The moment is that conjunction. Log it as
   a trace of claim ids. Do not log it as an emotion.
4. **Verification.** §8, on the accepted chain, before speech. If it
   fails, take the next chain on the beam. If the beam is empty, speak
   the holding clause that names the missing slot. Aamodt and Plaza's
   revise step is this rejection, plus the failure case retained
   afterward.
5. **Retain.** The accepted chain becomes a case. The next similar
   question can retrieve the chain as a single episode, which is how the
   diary gets shorter to use over time: a solved multi-hop question
   becomes one row. The row still points at the original claims. It does
   not replace them and does not merge their dates away.

**Beam search.** **Fact.** Zhou et al. keep a beam of 20 inside the GRU.
**Opinion.** Here the beam is over claim ids. Score of a partial chain =
tree score of the newest hop, plus a penalty for a missing date, plus a
veto (score driven to the bottom) on a slot clash. No learned value
function is required for the first version. That is deliberate. A learned
value function would be a second ranker, trained on a signal this note
has already called biased.

**MCTS.** **Fact.** UCT (Kocsis and Szepesvári, ECML 2006) treats each
tree node as a bandit; Coulom 2006 is the usual citation for the
Monte Carlo tree-search frame. Neither PDF was re-read. The docs.rs
pages for the Rust crates `mcts` and `mocats`, read 2026-11-18, both
expose a UCT policy a caller can run for a fixed number of playouts.
**Opinion.** Use that only on the side band, and only when the beam's top
scores are tied and the person is not waiting on this clause. The
rollout policy is "append the next retrieved claim at random from the
shortlist, score the finished chain with the same conjunction as
illumination." That is a heuristic search. It is not an optimal plan, and
it is not a reason to block the voice. If the playouts are still running
when the turn must speak, the beam's current leader is the candidate, and
it still has to pass §8. Wide diaries make the action space large; the
fix is to MCTS over the tree's shortlist, never over the raw index.

**Sandbox results.** **Opinion.** A sandbox is another retriever. Its
output enters the beam as a claim: `said_by` is the tool, `source_date`
is the run time, `source_locator` is the run id, `text_span` is the
slice of output a later checker may cite. Local execution does not raise
trust. A sandbox claim that clashes with a dated human claim is a clash
like any other. The transport is **unknown** (§2). This note does not
define an RPC.

**RAG, named carefully.** **Fact.** Lewis, Perez, Piktus, Petroni, and
colleagues, "Retrieval-Augmented Generation for Knowledge-Intensive NLP
Tasks," NeurIPS 2020. The ACM abstract read on 2026-11-18 says large
pretrained models make provenance and knowledge updates hard, and defines
RAG as a pretrained seq2seq model (parametric memory) plus a dense index
(non-parametric memory). They report gains on open-domain QA against
parametric seq2seq and against retrieve-and-extract systems, on their
tasks. **Opinion.** HORBM keeps their non-parametric memory and their
reason for it, which is provenance. HORBM does not keep the seq2seq
generator as the default reader. The reader is extractive, in Asai's
sense, plus templates. Calling this design "RAG" without that
qualification would smuggle the generator back in.

## 8. Verification and evidence

### 8.1 The claim

**Opinion.** This record is the unit of knowledge. Other notes may store
it inside their own tables. The fields are what this note requires.

| Field | What it holds |
|---|---|
| `claim_id` | Stable id |
| `text_span` | The words that may be spoken or cited. A span, not a paraphrase, unless `support = paraphrase` and a checker accepted it |
| `source_id` | Diary turn, unpaired document, knowledge-graph triple, sandbox run, or `unknown` |
| `source_locator` | URL, path, turn id, or run id |
| `said_by` | Person, system, tool, or `unknown` |
| `source_date` | When the source places the content, or `unknown`. This is not `retrieved_at` |
| `retrieved_at` | When HORBM stored the row |
| `valid_interval` | An interval, or `unknown` |
| `support` | `extract`, `user-said`, `template-slot`, `sandbox`, or `paraphrase` |
| `contradicts` | Other claim ids, once a check has said so |
| `trust_policy_id` | A pointer to a policy document. Not a hidden number presented as a fact |

**Fact.** Rashkin and colleagues, "Measuring Attribution in Natural
Language Generation Models," *Computational Linguistics* (MIT Press URL
path `/coli/article/49/4/777/`, read 2026-11-18), study whether a
generated text is attributable to an identified source. Honovich and
colleagues, "TRUE: Re-evaluating Factual Consistency Evaluation," NAACL
2022, build their labels around consistency with a grounding text. In the
PDF read this session they write that this notion of consistency follows
textual entailment in being judged against the given text, "irrespective
of contradiction with other world knowledge," and they point at Rashkin
et al. for the same stance. Kryscinski, McCann, Xiong, and Socher,
"Evaluating the Factual Consistency of Abstractive Text Summarization"
(FactCC), EMNLP 2020, train a checker on rule-corrupted sentences and
also extract a supporting span and an inconsistent span. They report that
the span tasks helped human verification. Their result is on summaries.
**Opinion.** The spoken clause inherits that standard, and not a stronger
one the literature does not grant: a clause may be spoken as fact when it
is attributable to identified claims. Attribution is not correctness. A
bad source can entail a false sentence. Who is trusted is a policy. This
note does not set it. What the note does set is that the source, the
date, and the speaker are on the row, and that a missing field is visible
rather than filled in.

### 8.2 Checks, in order

**Opinion.** Cheap checks first. Neural checks last, and only on the
chain about to be spoken.

1. **Presence.** A factual clause with no `claim_id` is not spoken. A
   claim with `source_id = unknown` may be spoken only inside a frame that
   says the source is unknown. Same for `said_by` and `source_date`.
2. **Slot clash.** Two claims with the same slot key, different values,
   and valid intervals that overlap: the chain is rejected. The interval
   test uses Allen's overlap relation and nothing else on the hot path.
   **Fact.** Allen, "Maintaining knowledge about temporal intervals,"
   *Communications of the ACM*, November 1983, DOI
   `10.1145/182.358434`: thirteen basic relations between intervals, plus
   constraint propagation. **Opinion.** The full propagator stays off the
   turn. Pairwise overlap on the shortlisted claims is the live test.
   Allen's paper is the vocabulary, not a license to run a solver in the
   voice path.
3. **Extractive pass.** If the spoken words are the span, skip NLI. There
   is nothing to entail. FactCC's useful idea, the supporting span, is
   already the whole utterance.
4. **Paraphrase pass.** Premise = span, hypothesis = spoken clause. The
   labels are MultiNLI's: entailment, neutral, contradiction (Williams,
   Nangia, and Bowman, "A Broad-Coverage Challenge Corpus for Sentence
   Understanding through Inference," NAACL 2018, DOI
   `10.18653/v1/n18-1101`). Entailment is required to speak the
   paraphrase. Neutral or contradiction: drop the paraphrase. This is
   stricter than using NLI as a metric. It is a policy.
5. **Date frame.** Undated claims stay undated in the words, not only in
   the log. The person hears "undated" because the truth rule is about
   the spoken system, not only the database.

**Unknown.** How often a sentence-pair NLI model catches a numerical or
temporal clash ("Tuesday" against "Wednesday") in Galion's own turns.
TRUE measures example-level consistency against a source across 11
datasets. It does not report a Galion rate, and this note did not run
one. Multi-sentence chains are outside the sentence-pair training task.
The slot-clash check exists because the NLI model is the wrong tool for a
key/value conflict, and because it is the more expensive tool.

**Opinion.** The ranker sees the flags as columns (§5.2). It does not
override a failed presence check or a slot clash. A high tree score is
not a source.

## 9. Where a non-LLM design stops

These are limits, not a backlog that a slightly larger tree will clear.

**Fact, then opinion.** On Zhou et al.'s Table 3 the hybrid beat
retrieval-only, 1.09 against 0.87, and they credit the neural generator
with a 20% coverage gain in the A/B test. Retiring the GRU gives up that
coverage unless the diary, the unpaired store, and the graph actually
contain the span. For a new topic with no stored sentence, this design
holds or asks. It does not invent a plausible paragraph. That is the
ceiling. It is accepted because an invented paragraph cannot carry
`said_by` and `source_date` without fiction, and fiction in those fields
breaks the truth rule harder than a short "I don't have a dated source."

**Fact.** IRCoT's published gains use a generative model to write the
next step, including in the Flan-T5-large setting. **Opinion.** A pipeline
that only retrieves cannot write the bridge sentence that makes the next
query good. Some multi-hop questions will stall with both hops present in
the diary and no chain found, because the query for hop two was never
phrased. Asai's recurrent retriever is one learned way to pick hop two.
Training that retriever needs multi-hop labels Galion does not have.
**Unknown:** how many real Galion questions are multi-hop. If almost none
are, the beam is idle complexity and should stay off.

**Fact.** Honovich et al. tie consistency to a grounding text, not to the
world. **Opinion.** The checker will bless a fluent paraphrase of a
wrong diary entry. Learning from the person (§5.3) can demote that case
later. It cannot detect the falsehood at speak time unless a second claim
clashes with it. Two sources that agree can still both be wrong. The
design records agreement. It does not call agreement truth.

**Fact.** Aamodt and Plaza include revise as a required task, and they
say there is no universal CBR method. **Opinion.** Marked-slot revise
fails on any reply whose varying part was not marked: jokes, advice,
hedges, and anything where the old user's name is baked into the
sentence. The failure mode is a confident, well-sourced, wrong-person
reply. The mitigation is mechanical: if entity spans were not marked at
retain time, the case is speakable only as a quotation with its speaker
and date, not as a fresh answer in Galion's voice. Quotation is the safe
reuse. Adaptation is the unsafe one.

**Opinion.** Templates and quotation will sound repetitive. The persona
rule says the feeling is in the delivery, which is the MnE path, so
repetition of wording is not supposed to be cured by adding emotional
stock phrases. Whether listeners accept a voice that often quotes is
**unknown**. No listening test was run.

**Opinion.** A tiny rewriter does not remove these limits. It moves the
failure into a sentence the checker might miss. Hinton et al. distill
behavior, not citations. A distilled model of a large teacher will
produce the teacher's unsupported connective tissue ("therefore", a
merged date, a guessed name). The filter in §8 drops a paraphrase that is
merely neutral. **Unknown:** the false-accept rate of that filter on
rewriter output. Until that rate is measured, the rewriter stays off by
default.

**Opinion.** MCTS does not add a world model. Searching a library of
steps finds combinations that are already implied by the library. It does
not discover a mechanism the diary never recorded. Presenting a searched
chain as a new insight, in the product voice, would over-claim. The chain
is a composition of other people's dated statements. The spoken form
should say so when the chain is more than one claim.

**Fact.** The symphony plan's S0 student is the acknowledged place a
learned generator could return, under an owner-chosen teacher, on
`galion-tensor`. **Opinion.** If those weights appear, they should sit in
the optional-rewriter seat of §6, under §8, and the diary should remain
the memory. Putting knowledge only in the 0.5B weights recreates the
provenance problem Lewis et al. state in the RAG abstract. If the weights
never appear, the brain described here still answers from cases, frames,
and refusals. `CannedLlm`, as the nerve document describes it, is that
state, not a temporary embarrassment.

## 10. Ranked CPU bottlenecks and fixes

Ranked by how directly they threaten the live turn under the constraints
in §2. The rank is **opinion**. The facts under each item are marked.
Nothing here was timed on a Galion machine.

### 1. An autoregressive decoder near the clock

**Fact.** `Heartbeat::step` does not block and does not allocate
(`EUREKA.md`, 2026-11-03). Gholami et al. 2024 say decoder transformers
can be limited by memory bandwidth. Pope et al. 2022 show, for large
models on TPU, that each decode step reloads weights and KV cache.
**Opinion.** Even a small GRU or a small transformer decoder is the wrong
shape: it wants a loop, it writes new words, and those words have no
source. It is ranked first because it breaks the clock rule and the truth
rule together. **Fix.** No decoder on the turn. The first spoken clause
is a span or a filled frame already in RAM when the turn opens. The
symphony plan's "1–2 word first clause" can be the start of a stored
span. A rewriter, if it exists, may replace a later clause, never the one
the clock is waiting on.

### 2. A cross-encoder over the full candidate list

**Fact.** Zhou et al. retrieve up to 400 paired and up to 400 unpaired
candidates. Khattab and Zaharia 2020: a pair-network computes one score
per forward pass, which is why they precompute document token vectors.
**Opinion.** Scoring hundreds of pairs with MiniLM on CPU is the live-path
cost most likely to miss a turn, if someone "upgrades" the tree by
replacing it. Not measured here. The model card's docs/sec column is
unused because the machine was not stated. **Fix.** The tree scores the
full set from columns and postings. A mini cross-encoder, int8, runs only
on a handful, only when the top margin is thin, only on the side band.
Document vectors for the dense feature are written at retain time (the
ColBERT / DPR split). Query-time work is one short encode plus the tree,
plus BM25 from the index. Jacob et al. 2018 support int8 as the
arithmetic, within the vision scope they actually measured.

### 3. Training the tree on the raw reaction

**Fact.** Joachims et al. 2017: raw implicit feedback is biased, and the
bias is enough to train the wrong ranker. Zhou et al. mix hand labels
with a random sample above a threshold. **Opinion.** This is the
highest-risk training bug. It is third, under the two latency bugs,
because a wrong tree still returns quickly while it slowly prefers
whatever the old tree already chose to say. **Fix.** Log candidates not
spoken, log the sampling probability, weight the update, keep the hand
seed, train off the tick, swap snapshots. Do not invent a position
feature the UI does not have. LightGBM's `g(pos)` term is the wrong tool
until a position exists. The open math is in §11.

### 4. Checking the whole diary with a neural entailment model

**Fact.** Williams et al. 2018 define NLI on sentence pairs. Honovich et
al. 2022: NLI-style metrics are strong checkers against a source and still
only checkers against a source. Each pair is a forward pass, so a chain
multiplies bottleneck 2. **Opinion.** Running that pass against every
stored sentence, every turn, will not fit, and a high entailment score
will be misread as "true" if the speaker and the date are dropped on the
floor. **Fix.** Presence and slot-clash first, which are column tests.
NLI only for a paraphrase, only on the chosen chain. Extractive replies
skip it. Cap the hop count so the side band cannot schedule an unbounded
number of passes. Speak the source and the date in the frame so the score
is not the only thing the person has.

### 5. Re-embedding the diary on the turn

**Fact.** Karpukhin et al. 2020 and Khattab and Zaharia 2020 both
precompute the document side. The symphony plan's default scan is exact
brute-force int8, and `galion-tensor` has that dot. **Unknown:** the
diary size at which that scan misses the turn budget. **Opinion.** Ranked
here because the failure, when it comes, is a cliff at retain time if
someone rebuilds vectors synchronously. **Fix.** Append the new case to a
small delta. Merge and embed on the side band. The turn reads the last
snapshot plus the delta. A new case can be missing from semantic search
for one merge interval and still be findable by the lexical index if the
postings are updated first. Staleness of the dense index is then a
bounded delay, not a stall. The acceptable delay is **unknown**.

### 6. Case adaptation as a hidden generator

**Fact.** Aamodt and Plaza treat revise as part of the cycle. Zhou et al.'s
hybrid gain (§9) is the evidence that pure reuse leaves coverage on the
table. **Opinion.** The tempting fix is a decoder "just for the rewrite."
That fix reintroduces bottlenecks 1 and 4. **Fix.** Quotation and marked
slots only, on the live path. The optional rewriter is off by default and
cannot block the first clause. Unmarked entities are not substituted.

### 7. MCTS with a wide action space

**Fact.** UCT spends its budget on playouts (algorithm: Kocsis and
Szepesvári 2006; not re-read). **Opinion.** Over the raw diary this is an
idle-CPU fire. It is last because the beam already searches the same
shortlist with a fixed bound, and MCTS is specified as an extra, and only
when idle. **Fix.** If it runs, it runs over the shortlist, with a
playout cap, on the side band, and its output re-enters §8. If the cap is
hit, the result is discarded rather than half-applied. No Galion number
for the right cap exists.

**Opinion, across all seven.** The CPU strategy is to move work to retain
time and to the side band, and to make the live turn a scan of columns
plus a string fill. That is the same split XiaoIce used when they put a
tree, not a second neural net, in the ranker seat. The upgrade is the
claim record and the refusal to decode, not a bigger model.

## 11. Open questions

1. **Case shape.** The symphony plan points at an Open Mind memory with
   tiers L0–L3. Those schema files were not in this checkout. Whether a
   claim row fits an existing tier is **unknown**. This note's fields are
   the requirement, not a migration.
2. **Coreference.** Zhou et al. depend on a contextual query rewrite.
   Galion's rewriter for "him" and "that" is **unknown**. Until it exists,
   chains that need it should hold.
3. **Label map.** What a continued conversation, a correction, a silence,
   and an explicit rating each mean on the 0/1/2 scale is unsettled.
   Whether "continued" is politeness is **unknown**.
4. **Propensity for a single spoken line.** Joachims et al. cover a ranked
   list. The inverse-probability update in §5.3 is an opinion that the
   same idea applies to a logged sampling policy. It is not a proof, and
   it has not been tried.
5. **Seed size.** How many hand labels the tree needs before online
   updates are allowed is **unknown**. 50K is XiaoIce's pilot figure, not
   a recommendation.
6. **Trust policy.** Which speaker overrides which, and whether a sandbox
   tool outranks a diary turn, is a policy decision. This note refuses to
   hide that decision inside a weight.
7. **Holding-clause wording.** A frame that names a missing source seems
   compatible with the empathy standing rule. A stock sympathetic line
   does not. The actual sentences need a wording review by whoever owns
   the persona files. This note does not write those sentences as canon.
8. **Hop cap and beam width.** Both are free parameters. No value is
   justified until multi-hop questions show up in real sessions. How often
   they show up is **unknown**.
9. **Dense-scan cliff.** At what diary size the plan's brute-force int8
   scan misses the turn is **unknown** because this note did not measure.
10. **Rewriter false-accept rate.** **Unknown**, and the reason the
    rewriter defaults to off.
11. **Checkpoint and runtime.** If a tiny model is ever allowed, the
    teacher and the file format are the open owner decision already
    recorded on 2026-11-04. candle, ort, and tract require an explicit
    exception to the `galion-tensor` rule. tract's SPDX should be re-read
    (GitHub said "Other"; the README said MIT/Apache-2.0). LightGBM's
    LICENSE file was not re-read.
12. **Other swarm notes.** R1, R3, and R4 were not in the tree on
    2026-11-18. If they define a different claim record or a sandbox RPC,
    this note should be reconciled rather than silently forked.
13. **Evaluation.** Not done. A later eval needs judges on a 0/1/2 scale
    of the kind Zhou et al. published, plus an attribution judgment of the
    kind Rashkin et al. define, on consented Galion dialogues. A public
    MS MARCO score would not answer either question.

## 12. Sources

Read in this session unless the entry says otherwise. Fetched 2026-11-18.
"Not re-read" means the work is named as the origin of an algorithm or a
citation and no number was taken from it.

### XiaoIce and the ranker it used

- Zhou, L., Gao, J., Li, D., and Shum, H.-Y. arXiv:1812.08989, December
  2018. Journal: *Computational Linguistics* 46(1), 2020, ACL Anthology
  `2020.cl-1.2`, MIT Press
  `https://direct.mit.edu/coli/article/46/1/53/93380/`. Microsoft Research
  MSR-TR-2018-42, December 2018,
  `https://www.microsoft.com/en-us/research/publication/the-design-and-implementation-of-xiaoice-an-empathetic-social-chatbot/`.
- Wu, Q., Burges, C. J. C., Svore, K. M., and Gao, J. "Adapting boosting
  for information retrieval measures." Zhou et al.'s bibliography, read
  in the arXiv HTML, gives *Information Retrieval* 13(3):254–270, 2010.
  The Springer page read this session dates online publication 3 September
  2009. `https://link.springer.com/article/10.1007/s10791-009-9112-1`.
- Burges, C. J. C. "From RankNet to LambdaRank to LambdaMART: An
  Overview." Microsoft Research technical report, 2010. Not re-read.
  Named because Hofstätter et al. cite it for this family.
- Ke, G., Meng, Q., Finley, T., Wang, T., Chen, W., Ma, W., Ye, Q., and
  Liu, T.-Y. "LightGBM: A Highly Efficient Gradient Boosting Decision
  Tree." NIPS 2017.
  `https://proceedings.nips.cc/paper_files/paper/2017/file/6449f44a102fde848669bdd9eb6b76fa-Paper.pdf`.
- LightGBM docs, `lambdarank` and the position-bias note, fetched
  2026-11-18.
  `https://lightgbm.readthedocs.io/en/latest/Parameters.html`,
  `https://lightgbm.readthedocs.io/en/v4.3.0/Advanced-Topics.html`.
- `lightgbm` crate, docs.rs, fetched 2026-11-18.
  `https://docs.rs/lightgbm/latest/lightgbm/`.

### Neural ranking, distillation, quantization, CPU runtimes

- Nogueira, R., and Cho, K. "Passage Re-ranking with BERT."
  arXiv:1901.04085, 2019. `https://arxiv.org/abs/1901.04085`.
- Khattab, O., and Zaharia, M. "ColBERT." SIGIR 2020, published
  2020-07-25. DOI `10.1145/3397271.3401075`.
- Hofstätter, S., Althammer, S., Schröder, M., Sertkan, M., and Hanbury, A.
  arXiv:2010.02666, submitted 2020-10-06, v2 2021-01-22.
  `https://arxiv.org/abs/2010.02666`.
- Wang, W., Wei, F., Dong, L., Bao, H., Yang, N., and Zhou, M. "MiniLM."
  arXiv:2002.10957, submitted 2020-02-25, v2 2020-04-06.
  `https://arxiv.org/abs/2002.10957`.
- `cross-encoder/ms-marco-MiniLM-L6-v2` model card, read 2026-11-18.
  Throughput column unused; hardware not stated in the excerpt.
  `https://huggingface.co/cross-encoder/ms-marco-MiniLM-L6-v2`.
- Karpukhin, V., Oğuz, B., Min, S., Lewis, P., Wu, L., Edunov, S., Chen,
  D., and Yih, W. "Dense Passage Retrieval for Open-Domain Question
  Answering." arXiv:2004.04906, 2020.
  `https://arxiv.org/abs/2004.04906`. Code they point at:
  `https://github.com/facebookresearch/DPR`.
- Hinton, G., Vinyals, O., and Dean, J. "Distilling the Knowledge in a
  Neural Network." arXiv:1503.02531, 2015.
  `https://arxiv.org/abs/1503.02531`.
- Jacob, B., Kligys, S., Chen, B., Zhu, M., Tang, M., Howard, A., Adam, H.,
  and Kalenichenko, D. "Quantization and Training of Neural Networks for
  Efficient Integer-Arithmetic-Only Inference." CVPR 2018.
  `https://openaccess.thecvf.com/content_cvpr_2018/html/Jacob_Quantization_and_Training_CVPR_2018_paper.html`.
- Pope, R., Douglas, S., Chowdhery, A., Devlin, J., Bradbury, J., Levskaya,
  A., Heek, J., Xiao, K., Agrawal, S., and Dean, J. "Efficiently Scaling
  Transformer Inference." arXiv:2211.05102v1, 9 November 2022.
  `https://arxiv.org/abs/2211.05102`. Large-TPU results are not reused as
  CPU numbers.
- Gholami, A., Yao, Z., Kim, S., Hooper, C., Mahoney, M. W., and Keutzer, K.
  "AI and Memory Wall." arXiv:2403.14123, 2024.
  `https://arxiv.org/abs/2403.14123`.
- `huggingface/candle`, Apache-2.0, repository created 2023-06-19. README
  read 2026-11-18. `https://github.com/huggingface/candle`.
- `pykeio/ort`, Apache-2.0, repository created 2022-11-26. README read
  2026-11-18. `https://github.com/pykeio/ort`.
- `microsoft/onnxruntime`, MIT, repository created 2018-11-10. README read
  2026-11-18. `https://github.com/microsoft/onnxruntime`.
- `sonos/tract`, repository created 2017-08-07. README claims
  MIT/Apache-2.0; GitHub's license field returned "Other" on 2026-11-18.
  `https://github.com/sonos/tract`.
- `quickwit-oss/tantivy`, MIT, repository created 2016-01-11. README read
  2026-11-18. `https://github.com/quickwit-oss/tantivy`.

### Reasoning, attribution, feedback, time

- Aamodt, A., and Plaza, E. "Case-Based Reasoning: Foundational Issues,
  Methodological Variations, and System Approaches." *AI Communications*
  7(1), 39–59, 1994. PDF read this session:
  `https://www.iiia.csic.es/~enric/papers/AICom.pdf`.
- Lewis, P., Perez, E., Piktus, A., Petroni, F., and colleagues.
  "Retrieval-Augmented Generation for Knowledge-Intensive NLP Tasks."
  NeurIPS 2020. ACM abstract read this session.
  `https://dl.acm.org/doi/abs/10.5555/3495724.3496517`.
- Trivedi, H., Balasubramanian, N., Khot, T., and Sabharwal, A. IRCoT.
  arXiv:2212.10509, submitted 2022-12-20, v2 2023-06-23.
  `https://arxiv.org/abs/2212.10509`. Code they point at:
  `https://github.com/stonybrooknlp/ircot`.
- Asai, A., Hashimoto, K., Hajishirzi, H., Socher, R., and Xiong, C.
  arXiv:1911.10470, submitted 2019-11-24, v2 2020-02-14.
  `https://arxiv.org/abs/1911.10470`.
- Joachims, T., Swaminathan, A., and Schnabel, T. "Unbiased
  Learning-to-Rank with Biased Feedback." WSDM 2017, published 2 February
  2017. `https://dl.acm.org/doi/10.1145/3018661.3018699`.
- Williams, A., Nangia, N., and Bowman, S. R. MultiNLI. NAACL 2018. DOI
  `10.18653/v1/n18-1101`.
- Honovich, O., Aharoni, R., Herzig, J., Taitelbaum, H., Cohen, V.,
  Kukliansky, D., Scialom, T., Szpektor, I., Hassidim, A., and Matias, Y.
  "TRUE: Re-evaluating Factual Consistency Evaluation." NAACL 2022.
  `https://aclanthology.org/2022.naacl-main.287.pdf`.
- Kryscinski, W., McCann, B., Xiong, C., and Socher, R. FactCC. EMNLP
  2020. `https://aclanthology.org/2020.emnlp-main.750/`.
- Rashkin, H., Nikolaev, V., Lamm, M., Aroyo, L., Collins, M., Das, D.,
  Petrov, S., Tomar, G. S., Turc, I., and Reitter, D. "Measuring
  Attribution in Natural Language Generation Models." *Computational
  Linguistics*, MIT Press URL `/coli/article/49/4/777/`, read 2026-11-18.
  That byline is the first author block on the page. A second role list
  on the same page did not repeat every name, so re-check the PDF byline
  before quoting it elsewhere.
  `https://direct.mit.edu/coli/article/49/4/777/116438/Measuring-Attribution-in-Natural-Language`.
- Allen, J. F. "Maintaining knowledge about temporal intervals."
  *Communications of the ACM*, November 1983. DOI
  `10.1145/182.358434`.
- Kocsis, L., and Szepesvári, C. "Bandit based Monte-Carlo Planning."
  ECML 2006. Not re-read. No figure used.
- Coulom, R. "Efficient Selectivity and Backup Operators in Monte-Carlo
  Tree Search." Computers and Games, 2006. Not re-read. No figure used.
- Rust crates `mcts` and `mocats`, docs.rs, UCT policy, read 2026-11-18.
  `https://docs.rs/mcts/latest/mcts/`,
  `https://docs.rs/mocats/latest/mocats/`.
- Wallas, G. *The Art of Thought*. 1926. Not re-read. Used only as the
  usual name for a four-stage schedule. No psychological claim.

### This repository, read 2026-11-18

- `docs/galion-voice-v2/symphony/EUREKA.md`, 2026-11-03. The clock. Not
  this pipeline.
- `docs/galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md`,
  2026-11-03, owner ruling 2026-11-04 20:45 CET.
- `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md`, §7. Stored line versus
  student generation.
- `docs/galion-voice-v2/personas/README.md`. Empathy standing rule. Galion
  One and Galion Research.
- `tools/symphony/mne-contract/README.md`. Persona id, memory id, memory
  snippet, feeling point.
- `crates/galion-tensor/src/lib.rs`. "No third-party inference runtimes."
  `.gq` weights are future.
- `docs/galion-context/2026-11-08/04-openmind-stack.md`. Galion OS sandbox
  is a plan. No agent protocol in this checkout.
