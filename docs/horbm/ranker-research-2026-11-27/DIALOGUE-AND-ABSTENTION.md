# Dialogue selection and abstention

Perpetual Open MIT License. (c) 2026 Galion Labs.

Lane of the 2026-11-27 ranker research. Research only. This file does not change code, locks, or personas. Read against `main` `9fa188d5` on 2026-11-27.

HORBM is the local CPU brain in `horbm/`. Part (41) is the ranker: many sourced candidates are already in hand, and the ranker picks one or abstains. A miss speaks exactly `I don't know.`

The live budget for this lane is the owner's line of 2026-11-27: 13 ms per query is too slow. The ranker on the live turn must finish well under 5 ms on a laptop CPU. This session did not time a ranker. Where a paper gives no laptop-CPU millisecond, the cell says **I don't know.** A method whose live step is a neural decode or a fresh encoder pass is out of the budget, because nothing read here shows that step finishing well under 5 ms on a laptop CPU.

## How to read a claim

| Tag | Meaning |
|---|---|
| **Fact** | Someone said it, and the source is named. |
| **Opinion** | A design choice of this note. Not a measurement. |
| **Unknown** | Not in the sources read here, and not measured here. |

Scores later in this note are judgments from those facts, on a scale of 5 (best fit for this laptop turn) to 1 (worst). They are not a new benchmark.

Other lanes own the tree-training recipe, the neural-encoder equations, and the neuroscience. This note stops at how a dialogue system chooses a line, and how it decides to say it does not know.

## What the tree already does

**Fact.** `LOCKS.toml` `[horbm]` records the 2026-11-24 decision: `hot_path = "retrieval"`, `student_on_tick = false`, `miss = "I don't know."` There is no student decode on the chat turn.

**Fact.** `UNKNOWN_REPLY` in `horbm/crates/horbm/src/route.rs` is the string `I don't know.` `MEMORY_HIT_CAP` in `horbm/crates/horbm/src/retrieve.rs` is 4. `covers` keeps a hit only when every content token of the ask is in the haystack. An empty token list misses.

**Fact.** The turn comment in `horbm/crates/horbm/src/lib.rs` says a retrieval `Hits` cell speaks the stored clause, and a `Miss` speaks `UNKNOWN_REPLY` and does not call research. Layer 4, in `horbm/crates/horbm/src/dialogue/mod.rs`, is a deterministic heuristic. An answer with no sourced card becomes ask, and the spoken miss stays `UNKNOWN_REPLY`. The bandit is off. That layer writes no file and opens no socket.

**Fact.** A search of `horbm/` on this commit found no boosted-tree ranker and no LambdaMART. The XiaoIce tree is a research idea. It is not code on this commit.

**Fact.** An older audit, `docs/horbm/foundations/F2-retrieval-audit.md`, quotes release numbers from PR #16 that it did not re-run: selective p95 6.0 ms and broad p95 8.3 ms for a 200,000-card search with a background writer. That is a search time, not a ranker time, and this session did not re-time it. It is context only. The ranker's own budget is well under 5 ms, on candidates that search has already returned.

**Unknown.** The millisecond cost of `covers` on four hits, on this laptop. The work is a token-set check. It is not a weight matrix. This session did not time it.

## What fits under 5 ms

**Opinion.** The live step has to be one of these: a token check, a compare of a score that is already stored, or a compare against a quantile that was computed before the turn. Anything that runs a network in order to choose the line misses the budget until someone times it well under 5 ms on a laptop CPU.

The methods that survive that cut are the covering rule already in the tree, a rank of at most four stored rows by a stored feature, and a selective or conformal threshold applied to that same feature. The generators below were read so the choice is on the record. They are not candidates for the live turn.

## Options

### 1. Covering rule, then one stored clause

**Fact.** This is the pick on `9fa188d5`, described above. Year in this repo: 2026. Licence of the crate: MIT, stated in `horbm/crates/horbm/src/lib.rs`.

One sentence: keep a candidate only when every content token of the ask sits in its text, speak that stored clause, and otherwise say `I don't know.`

Measured numbers: the cap is 4. The miss string is fixed. Latency on a laptop CPU: **I don't know.**

Rust: already in `horbm`. No new crate.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 5. The live step is a token check on at most 4 hits. Milliseconds not timed here. | 5. No labels. | 4. A miss is exactly the required sentence. It is not a risk bound. A paraphrase with none of the ask's tokens abstains. | 5. The spoken line is the stored clause. | 5. It is already the turn. |

### 2. XiaoIce ranker, 2018 preprint, journal 2020

**Fact.** Li Zhou, Jianfeng Gao, Di Li, and Heung-Yeung Shum, "The Design and Implementation of XiaoIce, an Empathetic Social Chatbot," *Computational Linguistics* 46(1), 2020. [Anthology PDF](https://aclanthology.org/2020.cl-1.2.pdf). Preprint [arXiv:1812.08989](https://arxiv.org/abs/1812.08989). No public implementation was found. Code licence: **I don't know.**

One sentence: three generators fill a candidate list, a boosted tree scores it, a reply is drawn at random from scores above a pre-set threshold, and a total failure speaks an editorial prompt rather than a refusal.

**Fact.** From that paper, read 2026-11-27. Paired retrieval held more than 30 billion pairs by May 2018, and 70% of responses were retrieved from XiaoIce's own chats. The paired index returns up to 400 candidates. The neural generator is a GRU, beam search, up to 20 candidates. Unpaired retrieval returns up to 400 sentences. The merged list is ranked by a boosted tree they cite as Wu et al. 2010. Features are local cohesion, global coherence, empathy match, and, for paired hits, retrieval match (BM25, TF-IDF, and DSSM). Labels are 0, 1, and 2. On the second pilot, three judges, the body says 4K sessions and the Table 3 caption in the same PDF says 5K:

| System | Average | Share of 0 | Share of 1 | Share of 2 |
|---|---:|---:|---:|---:|
| Retrieval only | 0.87 | 35.0% | 42.9% | 22.1% |
| Neural generator only | 0.40 | 66.6% | 27.3% | 6.1% |
| Hybrid | 1.09 | 23.1% | 44.6% | 32.3% |

The same section's two-week A/B test, against paired retrieval alone, raised expected conversation-turns per session of Core Chat by 0.5. They attribute the gain to coverage: distinct acceptable responses up 20% from the neural generator and 10% from the unpaired generator. Those figures are theirs, on their traffic.

**Fact.** On a total failure they reject a short refusal as bland, and they speak an editorial prompt that keeps the chat going. That is the opposite of `UNKNOWN_REPLY`.

**Unknown.** Laptop-CPU milliseconds for the tree, and for the DSSM features. Model size of the GRU. Whether the threshold was fit to an error rate. This note did not re-read Wu et al., so no speed figure is taken from that paper.

**Opinion.** A tree over features that are already columns, scoring four rows, can sit inside 5 ms. Filling those features with a DSSM at query time is a neural pass, and nothing read here puts that pass under 5 ms. The editorial prompt is not an allowed miss. There are no rank labels in the tree today, so the tree cannot be trained yet.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 3 for a tree on stored columns. 1 if a DSSM runs on the turn. Milliseconds: I don't know. | 2. Their pilot used manual labels on thousands of sessions, and feature models trained on millions of dialogues. | 1. The fallback is chit-chat, and the threshold is described as pre-set, not as a risk bound. | 2. Paired hits can keep a stored pair. The neural line has no span. | 2. A new model, new labels, and a fallback that the lock forbids. |

### 3. Cleverbot, ideas only

**Fact.** Closed. Rollo Carpenter. No source repository and no published scorer were found. A 2011 New Scientist report of a festival test says the audience rated Cleverbot 59.3% human and the humans 63.3% human, on 1,334 votes, and quotes Carpenter that the online system searched three times and the test build searched 42 times ([New Scientist, 6 September 2011](https://www.newscientist.com/article/1963356-software-tricks-people-into-thinking-it-is-human/)). A Live Science report the next day says it stores past conversations and answers with a past human reply, and gives about 65 million conversations as of that article ([Live Science, 7 September 2011](https://www.livescience.com/15940-cleverbot-computer-chats-human.html)). Meena's Figure 1, below, marks Cleverbot at 56% interactive SSA.

One sentence: search stored human conversations and speak a human line that overlapped this conversation.

**Unknown.** The scoring function, the licence, and any millisecond. A current conversation count. This note does not treat later retellings as a spec.

**Opinion.** The idea that survives is the one HORBM already uses: the spoken line is a line that was stored, not a fresh sentence. Searching an unbounded log of other people's chats on the turn is not a 5 ms plan, and "a stranger once typed this" is not a source. No code is taken.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 1. Many searches of a giant log. Milliseconds: I don't know. | 1. It needs the log. | 1. No published abstention. | 1. A past human line is not a sourced card. | 1. Closed. |

### 4. Meena sample-and-rank, 2020

**Fact.** Daniel Adiwardana and colleagues, "Towards a Human-like Open-Domain Chatbot," [arXiv:2001.09977](https://arxiv.org/abs/2001.09977). No weights were released with the paper. Code licence: **I don't know.**

One sentence: train one 2.6B-parameter seq2seq model on next-token loss, draw N samples, and speak the sample with the highest probability.

**Fact.** From that paper. Best model: 2.6B parameters, test perplexity 10.2, 8K BPE, context up to 7 turns. Sensibleness and Specificity Average (SSA): interactive 72% ± 1% for the base model, 79% ± 1% after the full decoding and repetition filter, 86% for humans. Figure 1 reports R² 0.93 between perplexity and interactive SSA. Section 4 uses sample-and-rank with N = 20 and temperature 0.88. Section 5 moves to top-k 40 and temperature 1.0, still N = 20, and reports the interactive SSA rising from 72% to 74%, then to 79% after a rule that drops cross-turn repetitions. N = 20 beat N = 1 by about 10 SSA points. N = 400 was worse on sensibleness. A safety classifier drops sensitive candidates. Training used TPU pods. Cleverbot is the 56% line on Figure 1.

**Unknown.** Laptop-CPU milliseconds for one sample, and for 20. The licence of any unreleased code.

**Opinion.** Twenty draws from a 2.6B model are not a 5 ms ranker. The safety filter is a classifier on top of those draws. There is no sourced span, and there is no "I don't know." The useful observation is narrower: once a model is good enough, picking the highest-probability sample among 20 beat taking one sample. HORBM has no such model on the turn, and the lock says a student decode stays off it.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 1. Twenty samples of a 2.6B model. Milliseconds: I don't know. | 1. They mined and filtered public social-media conversations, reported as 40B words. | 1. No abstention. A filter drops toxic samples and still speaks. | 1. The line is a new decode. | 1. A new runtime and weights the lock does not allow on the turn. |

### 5. Retrieve-and-refine, and BlenderBot 1

**Fact.** Jason Weston, Emily Dinan, and Alexander H. Miller, "Retrieve and Refine: Improved Sequence Generation Models For Dialogue," [arXiv:1808.04776](https://arxiv.org/abs/1808.04776), EMNLP workshop 2018. The generator is a 2-layer LSTM. The retriever is a Key-Value Memory Network. The retrieved line is appended to the context. They rerank the top 100 by similarity to the label before training. A later variant copies the retrieval exactly when word overlap with the generation is above 60%. Their code path is [ParlAI](https://github.com/facebookresearch/ParlAI), whose [LICENSE](https://github.com/facebookresearch/ParlAI/blob/main/LICENSE) is MIT. ParlAI is a Python training framework. It is not a laptop ranker.

One sentence: retrieve a candidate reply, then let a generator rewrite it.

**Fact.** From their Table 2, average words per response on that ConvAI2 setup: Seq2Seq 11.7, RetNRef 11.8, RetNRef++ 12.7, retrieval MemNet 13.1, human 13.0. The abstract says human judges preferred the combination to retrieval alone and to generation alone. The exact preference percentage: **I don't know.** It was not copied off a results table in the text read here. They also use "I don't know" as a name for a different failure: a short generic generation that fits many contexts and says little. That is not HORBM's miss.

**Fact.** Stephen Roller and colleagues, "Recipes for building an open-domain chatbot" (BlenderBot 1), [arXiv:2004.13637](https://arxiv.org/abs/2004.13637). Sizes they report: 90M, 2.7B, and 9.4B parameters. In their pairwise human comparison the best model beat Meena 75% to 25% on engagingness and 65% to 35% on humanness. They decode with beam search plus a minimum length, and they block repeated 3-grams. A knowledge variant retrieves a Wikipedia sentence and uses a transformer classifier to decide whether to retrieve on that turn. They say their own retrieve-and-refine trials on the failures they cared about (shallow knowledge, simple language, repeated phrases) were inconclusive. Code lives in ParlAI (MIT). Weight licence: **I don't know.**

**Unknown.** Laptop-CPU milliseconds for either generator, and for the "should I retrieve" classifier.

**Opinion.** Both systems do the choosing inside a generator. That misses 5 ms. The piece that matches this lane is the order: retrieve first, and only then consider a rewrite. The rewrite stays off the turn. The classifier that decides whether a knowledge lookup is needed is a second model call. A cheap stand-in for that decision is already `covers`: if nothing covers, do not invent a line.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 1. The live step is a generation. Milliseconds: I don't know. | 2. Public dialogue fine-tunes (persona, knowledge, empathy, blended skills) plus a large pretrain. | 2. BlenderBot 1 can skip a knowledge lookup. It still speaks a generated line. | 2. The retrieved sentence can be named. The spoken rewrite can leave it. | 1. ParlAI is MIT and is the wrong runtime for the voice turn. |

### 6. BlenderBot 2 and BlenderBot 3

**Fact.** Mojtaba Komeili, Kurt Shuster, and Jason Weston, "Internet-Augmented Dialogue Generation," [arXiv:2107.07566](https://arxiv.org/abs/2107.07566), 2021. The model writes a search query, reads the results, and generates with Fusion-in-Decoder. In their human evaluation of about 750 responses over nearly 100 conversations, the search model against a no-knowledge BART was knowledgeable 46.5% vs 38.6%, factually incorrect 5.3% vs 7.1%, consistent 76.1% vs 66.5%, engaging at utterance level 81.4% vs 69.9%, and 3.73 vs 3.64 at conversation level. Search cut the judged factual errors. It did not remove them.

**Fact.** Kurt Shuster and colleagues, "BlenderBot 3," [arXiv:2208.03188](https://arxiv.org/abs/2208.03188), 2022. One transformer, released at 3B, 30B, and 175B, runs a chain of modules by control codes: decide whether to search, write the query, decide whether to read long-term memory (`access memory` or `do not access memory`), then generate the line the person hears. A memory write can emit `no persona` and store nothing. The 175B and 30B models start from OPT. The deployment search engine they name is Mojeek. The abstract says human evaluations beat the predecessors. The exact percentages: **I don't know.** They were not transcribed here.

**Unknown.** Laptop-CPU milliseconds. A public MIT ranker crate for these modules. Both papers name the internet. HORBM's live turn is offline.

**Opinion.** The module that matters for this lane is the yes-or-no before any heavy work: do not search, do not open memory, do not generate. In BlenderBot 3 that yes-or-no is itself a forward pass of a model up to 175B. Under a 5 ms cap the same decision has to be the covering rule, not that forward pass. A generated line still has no card id.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 1. A multi-billion-parameter module chain, plus a network call in BlenderBot 2. Milliseconds: I don't know. | 1. Their fine-tunes plus the internet. | 3. They can decline a search or a memory read. The person still hears a generation. Factual error in the BlenderBot 2 study was 5.3%, not zero. | 2. A search snippet can be pointed at. The spoken sentence is new. | 1. Wrong machine, and not offline. |

### 7. RAG, DPR, and Fusion-in-Decoder

**Fact.** Patrick Lewis and colleagues, "Retrieval-Augmented Generation for Knowledge-Intensive NLP Tasks," [arXiv:2005.11401](https://arxiv.org/abs/2005.11401). A BART generator conditions on Wikipedia passages from a DPR index. RAG-Sequence uses one passage for the whole output. RAG-Token may switch passage at each token. On their Jeopardy question-generation human eval, 452 pairs, judges called RAG more factual than BART in 42.7% of pairs, BART more factual in 7.1%, and both factual in 17%.

**Fact.** Vladimir Karpukhin and colleagues, "Dense Passage Retrieval for Open-Domain Question Answering," [arXiv:2004.04906](https://arxiv.org/abs/2004.04906). Two BERT-base encoders, score is a dot product, search is approximate nearest neighbour. The abstract says top-20 retrieval accuracy beat Lucene-BM25 by 9 to 19 points absolute across their QA sets. The introduction states 65.2% vs 42.9% top-5 retrieval accuracy against BM25, and 41.5% vs 33.3% end-to-end accuracy against ORQA on open Natural Questions. The [DPR LICENSE](https://github.com/facebookresearch/DPR/blob/main/LICENSE) read 2026-11-27 is CC BY-NC 4.0. That code is not taken.

**Fact.** Gautier Izacard and Edouard Grave, "Leveraging Passage Retrieval with Generative Models for Open Domain Question Answering," [arXiv:2007.01282](https://arxiv.org/abs/2007.01282). Each passage is encoded alone, and the decoder attends over the concatenation, so cost grows linearly with the number of passages. Their Table 1 lists Fusion-in-Decoder large at 51.4 on Natural Questions, RAG at 44.5, and DPR at 41.5 in that column. They also report that raising the passage count from 10 to 100 gained 3.5 points on Natural Questions and 6 points on TriviaQA. The [FiD LICENSE](https://github.com/facebookresearch/FiD/blob/main/LICENSE) read 2026-11-27 is CC BY-NC 4.0. That code is not taken.

**Unknown.** Laptop-CPU milliseconds for a BERT-base query encode, for a FAISS probe, and for a FiD decode. The other three numbers in the FiD large row were not assigned to columns, because the PDF text run joined TriviaQA's two test sets with SQuAD Open.

**Opinion.** These papers are how a generator uses retrieved text. The generator and the query encoder miss 5 ms. CC BY-NC code stays out. The one slice that can fit, and only as a feature that retrieval has already paid for, is a dot product against vectors computed when the card was stored, over the four hits already returned. This note does not time that dot and does not specify the encoder. If the dot is not already in the candidate, it is not this ranker's job to encode it on the turn.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 2 for a dot of four cached vectors. 1 for the encoder or the decoder. Milliseconds: I don't know. | 3. DPR trains on question-passage pairs. The generator needs a seq2seq fine-tune. HORBM has little labelled data. | 2. They generate anyway. Provenance is the passage, not a refusal. | 4. The passage is an explicit input. The output can still leave it. | 1. Non-commercial reference code, and a runtime the turn does not have. |

### 8. Response selection: Ubuntu, DSTC7, poly-encoder

**Fact.** Ryan Lowe, Nissan Pow, Iulian V. Serban, and Joelle Pineau, "The Ubuntu Dialogue Corpus," [arXiv:1506.08909](https://arxiv.org/abs/1506.08909). Almost 1 million two-person dialogues, over 7 million utterances, 100 million words. The task is to pick the true next utterance from candidates. On the original set, 1-in-10 Recall@1 is TF-IDF 41.0%, RNN 40.3%, LSTM 60.4%. They warn that a later fixed release is not comparable to these numbers.

**Fact.** Chulaka Gunasekara, Jonathan K. Kummerfeld, Lazaros Polymenakos, and Walter S. Lasecki, "DSTC7 Task 1: Noetic End-to-End Response Selection," [Anthology PDF](https://aclanthology.org/W19-4107.pdf), 2019. Twenty teams. Subtask 1: 100 candidates, one correct. Subtask 4: the same, except 20% of examples had the correct line replaced by a wrong one, so the right act is to choose nothing. The official score is the mean of Recall@10 and MRR. They report that, on average, no metric moved by more than 0.016 relative to the subtask that always had a correct line. Their reading is that teams handled the extra case.

**Fact.** Samuel Humeau, Kurt Shuster, Marie-Anne Lachaux, and Jason Weston, "Poly-encoders," [arXiv:1905.01969](https://arxiv.org/abs/1905.01969). Candidate counts in their Table 1: ConvAI2 20, DSTC7 100, Ubuntu V2 10. With BERT-base fine-tunes they report bi-encoder DSTC7 R@1/100 of 66.8 and cross-encoder 67.4; a poly-encoder with 360 context codes reaches 68.9 and they say it beats that cross-encoder. On Ubuntu V2 they report bi-encoder R@1/10 80.6 and MRR 88.0, and poly-encoder 360 R@1/10 80.9 and MRR 88.1. They say a bi-encoder can cache every candidate and finish with a dot product, and that a cross-encoder must rerun the whole network for every candidate and is too slow for a large set. The speed remarks are about their GPU setup (training notes mention 8 V100s). Laptop-CPU milliseconds: **I don't know.**

**Opinion.** The benchmark usually hides the true reply in the set, then scores ranking. DSTC7 subtask 4 is the one that sometimes has no true reply, and the ranking metrics barely moved. A system can look strong on Recall@k and still never abstain. HORBM's miss has to be a separate decision, not a side effect of Recall@1. Under 5 ms the usable slice is the cheap one Lowe already measured as a baseline: a stored lexical score over a handful of candidates. A cross-encoder over 100 candidates is the slow path Humeau rejected, and it is worse under a 5 ms laptop cap. TF-IDF's 41.0% R@1 on 1-in-10 is a low ceiling, not a reason to spend the budget on an LSTM.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 5 for a stored lexical score on 4 rows. 1 for a cross-encoder. Milliseconds: I don't know. | 4 for the lexical score, which needs no new labels. 2 for a neural selector, which is trained on the candidate sets. | 2. Subtask 4 asked systems to pick nothing 20% of the time, and the headline metrics barely noticed. | 5. The chosen line is one of the candidates, so the card id can stay. | 4 for the lexical score. 2 for a new encoder. |

### 9. Selective prediction and a calibrated threshold

**Fact.** Yonatan Geifman and Ran El-Yaniv, "Selective Classification for Deep Neural Networks," [arXiv:1705.08500](https://arxiv.org/abs/1705.08500), 2017. Given a trained classifier, a desired risk, and a confidence level, they learn a threshold on a confidence score so that the error among the cases they do answer stays under that risk, with high probability. Coverage is the fraction they still answer. On ImageNet, with VGG-16 or ResNet-50, they report a 2% top-5 error guaranteed with probability 99.9% at almost 60% coverage. That number is images, not dialogue, and not a latency.

**Fact.** Amita Kamath, Robin Jia, and Percy Liang, "Selective Question Answering under Domain Shift," [arXiv:2006.09462](https://arxiv.org/abs/2006.09462), ACL 2020, [anthology](https://aclanthology.org/2020.acl-main.503/). A QA model that abstains from a softmax probability answers 48% of questions while holding 80% accuracy, on their half-and-half mix of in-domain and out-of-domain sets. A calibrator trained with some out-of-domain examples answers 56% at the same 80% accuracy. They say the raw softmax is overconfident out of domain.

**Fact.** Saurav Kadavath and colleagues, "Language Models (Mostly) Know What They Know," [arXiv:2207.05221](https://arxiv.org/abs/2207.05221), 2022. P(True) is the probability the model assigns to "this sample is correct," after it has proposed a sample. P(IK) is the probability it assigns to "I know," before a particular answer. They report useful in-domain behavior, and they report that P(IK) was poorly calibrated on tasks it was not trained on. The figure they discuss uses a 52B model. Laptop-CPU milliseconds: **I don't know.**

**Opinion.** The live step that fits is the threshold compare, after some other stage has already produced a score. Geifman's ImageNet net, Kamath's QA model, and Kadavath's 52B self-check do not fit inside 5 ms, and HORBM has no such model on the turn. The lesson that does fit: a raw score is not a probability, and a threshold fit in one domain goes wrong on another. Until there is a calibration set drawn from HORBM's own cards and asks, the covering rule remains the abstention. A softmax from a model we do not run would be the failure Kamath measured.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 5 for one compare, once a score exists. 1 if the score is a new network. | 3. Geifman needs labeled errors to set the threshold. Kamath needs out-of-domain examples or the threshold lies. HORBM has little labelled data today. | 5 when the threshold is fit to a stated error rate and a miss is the refusal. 2 if the score is an uncalibrated dot product. | 5 if the thing accepted or refused is a stored clause. 1 if the model wrote a new sentence. | 4 for a compare. 1 for a 52B self-check. |

### 10. Conformal risk control

**Fact.** Anastasios N. Angelopoulos and Stephen Bates, "A Gentle Introduction to Conformal Prediction," [arXiv:2107.07511](https://arxiv.org/abs/2107.07511). Given exchangeable calibration data, wrap any score so the prediction set contains the true answer with a probability the user chooses, such as 90%, with a finite-sample guarantee and no assumption on the model. A dialogue accuracy from that tutorial: **I don't know.**

**Fact.** Anastasios N. Angelopoulos, Stephen Bates, Adam Fisch, Lihua Lei, and Tal Schuster, "Conformal Risk Control," [arXiv:2208.02814](https://arxiv.org/abs/2208.02814). For a bounded loss that gets no worse as the threshold loosens, pick the tightest threshold whose calibration loss, with an O(1/n) pad, is at most α. The guarantee is on the expected loss of a fresh exchangeable point. Their worked examples include a token-level F1 bound. The F1 value in that example: **I don't know.** It was not transcribed.

**Fact.** Victor Quach and colleagues, "Conformal Language Modeling," [arXiv:2306.10193](https://arxiv.org/abs/2306.10193). For a generator, calibrate a stopping rule that keeps sampling until the set should contain one acceptable answer, and a rejection rule that drops bad samples. The user sets the probability, and they name 90% as an example. Tasks they name: open-domain QA, summarization, radiology reports. A measured coverage error from their tables: **I don't know.**

**Fact.** Yasin Abbasi-Yadkori and colleagues, "Mitigating LLM Hallucinations via Conformal Abstention," [arXiv:2405.01563](https://arxiv.org/abs/2405.01563). The abstention they describe is to say "I don't know" instead of a hallucinated answer. Samples are compared for agreement, and conformal risk control sets the threshold so the expected hallucination rate is at most α. They evaluate on TriviaQA and on Temporal Sequences, at α = 0.1 and α = 0.2. The abstract says the bound held, and that log-probability scores abstained more conservatively on the long-answer set. A single abstention percentage is not quoted here: the PDF text run did not keep row labels attached cleanly enough to trust one cell.

**Fact.** The Rust crate `risksieve` 0.2.2 ([repository](https://github.com/kent-tokyo/risksieve), [docs](https://docs.rs/risksieve/latest/risksieve/)) declares `license = "MIT OR Apache-2.0"`, `rust-version = "1.85"`, edition 2024, and dependencies `thiserror` plus optional `serde`. The MIT text was read at `LICENSE-MIT` on 2026-11-27. The authors say it implements the conformal risk control procedure above, and a selective deploy-or-abstain decision. This session did not compile it and did not time it. Latency: **I don't know.**

**Fact.** Two other crates were looked at and are not the recommendation. `anofox-ml-conformal` 0.6.1 is MIT OR Apache-2.0 and states MSRV 1.87 ([crates.io](https://crates.io/crates/anofox-ml-conformal)), which is above the workspace pin `rust-version = "1.85"` in `horbm/Cargo.toml`. It builds prediction intervals for that project's regressors. `conformal-prediction` 2.0.1 is MIT OR Apache-2.0 ([crates.io](https://crates.io/crates/conformal-prediction)) and its readme claims sub-2 ms operations. The readme names no CPU and no dataset. Those milliseconds are not used. The crate depends on `lean-agentic` and is written as a numeric-interval tool for trading examples.

**Opinion.** Quach's stopping rule and Abbasi-Yadkori's sample-and-compare both assume a generator that can be drawn many times. That misses 5 ms. The piece that fits is the threshold math alone: offline, on a calibration set of "this stored clause was wrong," compute one number; on the turn, compare the candidate's existing score with that number; below it, say `I don't know.` Exchangeability is the assumption. A new card pack or a new kind of ask voids the guarantee until the set is rebuilt. With a tiny calibration set the O(1/n) pad makes the test very strict, and almost every turn would abstain. That is a reason to wait for labels, not a reason to skip the miss.

| CPU under 5 ms | Data needed | Abstention | Sources stay on the line | Complexity |
|---|---|---|---|---|
| 5 for one compare against a quantile computed earlier. 1 for Quach's sampling loop. Milliseconds of `risksieve`: I don't know. | 3. Needs labeled errors from this card pack. Does not need a generator. | 5. The guarantee is an expected error rate, and the refusal is the miss sentence. It holds only while new asks look like the calibration asks. | 5 when the accepted object is a stored clause. | 4. One small crate, or a sort and a cutoff if that crate fails the pin. Not a new model. |

## Recommendation

**Opinion.** Now, on the live turn: keep the covering rule, keep the cap of 4, and speak the stored clause or exactly `I don't know.` Do not add XiaoIce's editorial prompt, a sample-and-rank decoder, a BlenderBot module, a RAG or FiD generator, a DPR or poly-encoder forward pass, or a 52B self-check. None of those has a laptop-CPU time under 5 ms in the sources read here, and several of them speak a new sentence with no card id. The rank among the rows that already passed `covers` stays a compare of a score the retrieval stage has already stored. If the scores tie, the current deterministic pick stands. This session did not change that pick.

**Opinion.** Later, and only after a calibration set exists: label held-out turns as "the spoken clause was the right card" or "it was wrong," offline. Fit one threshold with conformal risk control so the expected rate of speaking a wrong clause is at most a chosen α. On the turn, one compare. If no row clears it, `UNKNOWN_REPLY`. `risksieve` 0.2.2 is the MIT-or-Apache crate that already states this procedure and declares rustc 1.85. It is not wired here. It is not needed to ship the current miss. Do not copy the CC BY-NC DPR or FiD repositories. Do not port ParlAI into the voice process.

The neural reranker, if one is ever added, has to be paid for when the card is stored, not when the person is waiting. This note does not choose that encoder.

## What would change my mind

**Opinion.** A timed run on this laptop, well under 5 ms, for a method marked 1 on CPU above. No such run was found in the papers, and this session did not make one.

**Opinion.** A calibration set large enough that the conformal pad does not force almost every turn into `I don't know.` The size that does that for this card pack: **I don't know.** Angelopoulos et al. put the slack at O(1/n). They do not give an n for HORBM.

**Opinion.** Evidence that new asks are not exchangeable with the calibration asks. Kamath et al. already showed an in-domain confidence fails under domain shift. A new pack would mean a new fit, not a transferred threshold.

**Opinion.** `risksieve` failing to build on rustc 1.85. The online step would still be one compare. The crate is a convenience.

## Sources

- [Zhou, Gao, Li, and Shum, Computational Linguistics 2020, anthology PDF](https://aclanthology.org/2020.cl-1.2.pdf)
- [Zhou et al. preprint, arXiv:1812.08989](https://arxiv.org/abs/1812.08989)
- [New Scientist on Cleverbot, 6 September 2011](https://www.newscientist.com/article/1963356-software-tricks-people-into-thinking-it-is-human/)
- [Live Science on Cleverbot, 7 September 2011](https://www.livescience.com/15940-cleverbot-computer-chats-human.html)
- [Adiwardana et al., Meena, arXiv:2001.09977](https://arxiv.org/abs/2001.09977)
- [Weston, Dinan, and Miller, retrieve-and-refine, arXiv:1808.04776](https://arxiv.org/abs/1808.04776)
- [Roller et al., BlenderBot 1, arXiv:2004.13637](https://arxiv.org/abs/2004.13637)
- [ParlAI LICENSE (MIT)](https://github.com/facebookresearch/ParlAI/blob/main/LICENSE)
- [Komeili, Shuster, and Weston, BlenderBot 2, arXiv:2107.07566](https://arxiv.org/abs/2107.07566)
- [Shuster et al., BlenderBot 3, arXiv:2208.03188](https://arxiv.org/abs/2208.03188)
- [Lewis et al., RAG, arXiv:2005.11401](https://arxiv.org/abs/2005.11401)
- [Karpukhin et al., DPR, arXiv:2004.04906](https://arxiv.org/abs/2004.04906)
- [DPR LICENSE (CC BY-NC 4.0)](https://github.com/facebookresearch/DPR/blob/main/LICENSE)
- [Izacard and Grave, Fusion-in-Decoder, arXiv:2007.01282](https://arxiv.org/abs/2007.01282)
- [FiD LICENSE (CC BY-NC 4.0)](https://github.com/facebookresearch/FiD/blob/main/LICENSE)
- [Lowe et al., Ubuntu Dialogue Corpus, arXiv:1506.08909](https://arxiv.org/abs/1506.08909)
- [Gunasekara et al., DSTC7 Task 1, anthology PDF](https://aclanthology.org/W19-4107.pdf)
- [Humeau et al., poly-encoders, arXiv:1905.01969](https://arxiv.org/abs/1905.01969)
- [Geifman and El-Yaniv, selective classification, arXiv:1705.08500](https://arxiv.org/abs/1705.08500)
- [Kamath, Jia, and Liang, selective QA under domain shift, arXiv:2006.09462](https://arxiv.org/abs/2006.09462)
- [Kadavath et al., arXiv:2207.05221](https://arxiv.org/abs/2207.05221)
- [Angelopoulos and Bates, conformal prediction tutorial, arXiv:2107.07511](https://arxiv.org/abs/2107.07511)
- [Angelopoulos et al., conformal risk control, arXiv:2208.02814](https://arxiv.org/abs/2208.02814)
- [Quach et al., conformal language modeling, arXiv:2306.10193](https://arxiv.org/abs/2306.10193)
- [Abbasi-Yadkori et al., conformal abstention, arXiv:2405.01563](https://arxiv.org/abs/2405.01563)
- [risksieve 0.2.2, MIT OR Apache-2.0](https://github.com/kent-tokyo/risksieve)
- [anofox-ml-conformal on crates.io](https://crates.io/crates/anofox-ml-conformal)
- [conformal-prediction on crates.io](https://crates.io/crates/conformal-prediction)
