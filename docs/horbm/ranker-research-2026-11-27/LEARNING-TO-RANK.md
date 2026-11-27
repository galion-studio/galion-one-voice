# (41) Ranker research — learning to rank

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Lane | Learning to rank. LambdaMART and other boosted trees, neural listwise rankers, learning from clicks and bandits, and Rust crates that can run them. |
| Date | 2026-11-27 |
| Kind | Research note. No code, no lock, no persona file. |
| Base | `origin/main` at the branch point of `cursor/horbm-ranker-ltr-b491`. |

Other lanes cover neural rerankers (cross-encoders and late interaction), dialogue abstention, and the human brain. This note stays on learning to rank: a model that scores candidates that are already in hand, then sorts them.

Every number below is copied from a named paper, a library document, or a crate page. This session did not train a model and did not time one on a laptop. Where a latency, a byte size, or an accuracy is missing, the cell says **I don't know.**

## What the question is

Part (41) has to pick one sourced card, or say exactly `I don't know.` The current card idea, taken from XiaoIce, is a boosted-tree ranker in the LambdaMART family, with features for local cohesion, global coherence, empathy, and retrieval match, plus a score threshold when nothing clears the bar.

HORBM has to do that on a laptop CPU (8 cores), in Rust on the rustc 1.85 pin, offline, inside a live voice turn. The owner set the ranking budget on 2026-11-27: well under 5 ms per query on that CPU. A ranker at 13 ms per query is too slow and is out. There is little or no labelled training data today. The spoken line stays a stored clause, and the card id stays on the answer.

## What the code does today

The hot path is a lookup, not a learned ranker.

`KnowledgeCards` keeps a card only when every content token of the ask is in the card. The smallest unique haystack wins. A tie is a miss, and the miss line is `I don't know.` (`horbm/crates/horbm/src/cards.rs`, `horbm/crates/horbm/src/route.rs` `UNKNOWN_REPLY`).

The phase-4 pick does the same thing on a fixture: full token cover, then higher cover score, then fewer words, then id. Two hits that tie on score and word count are a miss (`horbm/crates/horbm-brain/src/pick.rs` `pick`, `HIT_CAP` 4). The retrieval cell holds at most four hits (`horbm/crates/horbm/src/retrieve.rs` `MEMORY_HIT_CAP`).

`OfflineBandit` counts a dialogue move only when it is switched on and learn consent is granted. `Dialogue::decide` does not call it. The optional `choose_steered` adds those raw counts onto move scores. That is a frequency bump, and it is not wired (`horbm/crates/horbm/src/dialogue/policy.rs`).

The foundations note records two design lines that are not measurements. R1's 5 ms p95 is marked a target. R2 says the pick replaces XiaoIce's GRU and boosted tree and runs beside the heartbeat (`docs/horbm/foundations/F2-retrieval-audit.md`).

## The XiaoIce ranker, in the paper's own terms

Zhou, Gao, Li, and Shum, *The Design and Implementation of XiaoIce, an Empathetic Social Chatbot*, Computational Linguistics 46(1), 2020. [ACL PDF](https://aclanthology.org/2020.cl-1.2.pdf). Also [arXiv:1812.08989](https://arxiv.org/abs/1812.08989).

The response-candidate ranker is a boosted tree, cited as Wu et al. 2010 (the LambdaMART paper). Each candidate gets a score from four feature groups: local cohesion and global coherence from DSSM similarity models, empathy match, and retrieval match (BM25, TF-IDF, and DSSM, and only for paired-database hits). The ranker is trained on dialogue-state–response pairs labelled 0, 1, or 2. The grades mean "likely to end the chat", "likely to keep it going", and "likely to drive it". The pilot that describes the ranker trains it on 50,000 manually labelled dialogues. The cohesion and coherence models in that same pilot are trained on 50 million human dialogues.

A response is chosen by sampling at random among candidates above a preset threshold. If nothing qualifies, an editorial line is used. The paper gives the example of avoiding "I don't know" in favour of a line that keeps the chat going.

The published success number is conversation-turns per session (CPS), the engagement metric the whole system is built to raise. The sixth generation, as of July 2018, has average CPS 23. The same section gives human conversations a CPS of 9. An A/B test of adding the neural generator and the unpaired retriever, on top of this same ranker, raises Core Chat CPS by 0.5 over two weeks.

I don't know the ranker's NDCG, its threshold, its tree count, its model bytes, or its CPU latency. The paper does not print them. The labels and the CPS number measure whether the chat continues. They do not measure whether the spoken sentence is true.

The DSSM features are neural similarity models. Scoring those is the reranker lane. A boosted tree in this lane is only the combiner that sits on top of whatever features already exist.

## Options

| Option | Year | Where | Licence | One sentence |
|---|---|---|---|---|
| LambdaMART, the method XiaoIce cites | 2010 | Wu, Burges, Svore, Gao, *Adapting boosting for information retrieval measures*, Information Retrieval 13. Overview: Burges, [MSR-TR-2010-82](https://www.microsoft.com/en-us/research/publication/from-ranknet-to-lambdarank-to-lambdamart-an-overview/) | I don't know a code licence for the 2010 paper. The public libraries below carry the algorithm. | Boosted trees whose pair weights follow the change in NDCG, so the sort tracks a ranking metric. |
| LightGBM `lambdarank` and `rank_xendcg` | Library 2017, ranking objective still documented | [lightgbm-org/LightGBM](https://github.com/lightgbm-org/LightGBM), [parameters](https://lightgbm.readthedocs.io/en/latest/Parameters.html) | MIT | Leaf-wise gradient boosting with a LambdaRank objective. Integer labels, larger means more relevant. `rank_xendcg` is documented as similar in quality and faster to train. |
| XGBoost `rank:ndcg`, `rank:map`, `rank:pairwise` | Library 2016, ranking tutorial current | [dmlc/xgboost](https://github.com/dmlc/xgboost), [learning to rank](https://xgboost.readthedocs.io/en/stable/tutorials/learning_to_rank.html) | Apache-2.0 | The same LambdaMART idea. `rank:ndcg` is the default ranking objective. `rank:pairwise` is RankNet loss with no NDCG scaling. `rank:ndcg` can also debias click position. |
| CatBoost YetiRank and YetiRankPairwise | Paper 2017–2018, ranking docs current | [catboost/catboost](https://github.com/catboost/catboost), [ranking losses](https://catboost.ai/docs/en/concepts/loss-functions-ranking), [boosting type](https://catboost.ai/docs/en/references/training-parameters/common) | Apache-2.0 | Gradient boosting with a ranking objective that approximates NDCG or PFound, plus an ordered boosting mode documented as stronger on small datasets and slower. |
| TF-Ranking | KDD 2019 | Pasumarthi et al., [arXiv:1812.00073](https://arxiv.org/abs/1812.00073). Code: [tensorflow/ranking](https://github.com/tensorflow/ranking) | Apache-2.0. The GitHub repository is archived. | A TensorFlow library of pointwise, pairwise, and listwise losses, including LambdaLoss and inverse-propensity weights. The scorer is a neural net. |
| allRank, self-attention listwise | SIGIR eCom workshop 2020 | Pobrotyn, Bartczak, Synowiec, Białobrzeski, Bojar, [arXiv:2005.10084](https://arxiv.org/abs/2005.10084). Code: [allegro/allRank](https://github.com/allegro/allRank) | Apache-2.0 | A Transformer encoder scores each candidate while looking at the rest of the list. Losses include LambdaLoss, ListNet, ListMLE, and ordinal loss. |
| DASALC | ICLR 2021 | Qin, Yan, Zhuang, Tay, Pasumarthi, Wang, Bendersky, Najork, [PDF](https://marc.najork.org/papers/iclr2021.pdf) | I don't know a code licence. The paper says a release was in progress. The neural runs use TF-Ranking. | A listwise neural ranker (data augmentation, self-attention, latent cross) built to close the gap with LightGBM LambdaMART on numeric LTR sets. |
| Propensity SVM-Rank | WSDM 2017 | Joachims, Swaminathan, Schnabel, [doi:10.1145/3018661.3018699](https://doi.org/10.1145/3018661.3018699) | I don't know a code licence for this paper. | Learns a ranker from clicks by weighting each click with the inverse of the chance it was seen, so position bias does not write the training signal. |
| PDGD, online learning to rank | CIKM 2018 | Oosterhuis and de Rijke, [doi:10.1145/3269206.3271686](https://doi.org/10.1145/3269206.3271686), [arXiv:1809.08415](https://arxiv.org/abs/1809.08415). Code: [HarrieO/OnlineLearningToRank](https://github.com/HarrieO/OnlineLearningToRank) | I don't know the repository licence. | After each shown list, click pairs update any differentiable scorer. The list shown to the user is sampled, so the live ranking explores. |

SetRank (Pang et al., SIGIR 2020) and DLCM (Ai et al., SIGIR 2018) are the attention and RNN listwise models included in the ICLR 2021 comparison below. They are not separate recommendations.

## Measured numbers

NDCG figures from Bruch et al. and Qin et al. are printed on a 0–100 scale. LightGBM's own experiment page prints NDCG on a 0–1 scale. They are not one shared test.

### Trees against neural listwise models

Bruch, Zoghi, Bendersky, and Najork, SIGIR 2019, [PDF](https://marc.najork.org/papers/sigir2019b.pdf). LightGBM LambdaMART, learning rate 0.1, 200 leaves, `min_data_in_leaf` 50, `min_sum_hessian_in_leaf` 100. Ten trials, mean and 95% interval, queries with no relevant document left out.

| Model | Web30K NDCG@10 | Yahoo NDCG@10 |
|---|---|---|
| RankLib LambdaMART | 46.46 (±0.03) | 74.58 (±0.05) |
| LightGBM LambdaMART | 51.05 (±0.02) | 78.40 (±0.10) |
| ApproxNDCG network | 47.31 (±0.10) | 76.77 (±0.06) |

Qin et al., ICLR 2021, fixed test sets. LightGBM LambdaMART against the neural models those papers had been comparing to RankLib.

| Model | Web30K NDCG@5 | Web30K NDCG@10 | Yahoo NDCG@10 | Istella NDCG@10 |
|---|---|---|---|---|
| RankLib LambdaMART | 44.59 | 46.46 | 74.58 | 65.91 |
| LightGBM LambdaMART | 49.66 | 51.48 | 78.02 | 76.07 |
| DLCM | 45.00 | 46.90 | 74.30 | 66.80 |
| SetRank, rerank | 45.15 | 46.96 | 74.53 | 68.34 |
| DASALC, one net | 50.92 | 52.88 | 77.66 | 75.30 |
| DASALC, ensemble of 3–5 | 51.72 | 53.73 | 77.97 | 76.44 |

On that split the ensemble is 4.37% relative above LightGBM at Web30K NDCG@10, and 0.06% relative below it at Yahoo NDCG@10. The neural sweep uses hidden widths up to 4096 and 3 to 6 layers. I don't know the parameter count, the model bytes, or any CPU latency for DASALC.

Training-set size in that paper: Web30K has 136 features, 18,919 training queries, and 2,270,296 training pairs. Yahoo has 700 features and 19,944 training queries. Istella has 220 features, 20,901 training queries, and 6,587,822 training pairs. Those labels are human grades from web search, on features such as BM25 of page sections. A model trained there does not score a HORBM card. The feature columns are different.

### allRank on the same public set, a different split

Pobrotyn et al. average five folds of MSLR-WEB30K. Their self-attention model with ordinal loss reaches NDCG@5 53.00 ± 0.35 and NDCG@10 54.88 ± 0.21. The same table's XGBoost `rank:pairwise` run reaches NDCG@5 46.8 and NDCG@10 49.17. Their Table 1 lists a literature LightGBM NDCGLoss 2++ result of NDCG@5 51.21. I did not open that LambdaLoss paper, so 51.21 stays a number allRank attributes to earlier work.

I don't know which of allRank and LightGBM LambdaMART wins on one shared test. Qin et al. do not include allRank. allRank's own prose also says a 2.27% gain that lands at 52.37, while Table 3 prints 53.00. This note uses Table 3.

Their ablation baseline is 950,000 parameters and NDCG@5 52.64. Dropping dropout on that baseline falls to 42.18. Shortening the list they attend over, the shortest length they print is 30, at NDCG@5 50.94, against 52.78 at length 120. HORBM's cell cap is 4. I don't know the number at length 4.

Section 5.6 says the scorer is O(n²) in the list length and that a latency-sensitive deployment would shorten the list, drop blocks, or distill. I don't know a CPU millisecond for it.

On their e-commerce click logs, self-attention with NDCGLoss 2++ is a 3.00% relative NDCG@60 gain over the production XGBoost model. Absolute NDCG is not printed. I don't know it.

### LightGBM's own training bench

[LightGBM Experiments](https://github.com/microsoft/LightGBM/blob/master/docs/Experiments.rst), updated 8 March 2020, Azure ND24s, two E5-2690 v4, 16 threads, 500 trees, LightGBM `num_leaves` 255. MS LTR is folds S1–S3 for train and S5 for test, 2,270,296 training rows, 137 features. Yahoo LTR is set 1.

The speed section says the ranking sets were trained with the regression objective, because the two libraries' ranking objectives differ. The settings list no ranking objective, and other parameters are defaults. I don't know whether the NDCG rows below are LambdaMART or regression-on-grades scored with NDCG.

| Data | LightGBM test NDCG@10 | Train time, LightGBM | Train time, XGBoost exact |
|---|---|---|---|
| MS LTR | 0.524252 | 70.417 s | 1251.27 s |
| Yahoo LTR | 0.793498 | 76.229 s | 674.322 s |

Those times are training. I don't know the inference time of this 500-tree model. Peak training RSS for MS LTR, LightGBM column-wise, is 0.940 GB. That is the training process, not the model file. I don't know the model file bytes.

### The 5 ms gate

The ranking call has to finish well under 5 ms on a laptop CPU. 13 ms per query fails. A published time counts only when the page prints the milliseconds, the machine, and what was scored. A chart without numbers in the text is **I don't know.**

| What was timed | Machine | Time | Against 5 ms |
|---|---|---|---|
| LightGBM C API 3.3.0, one row, one thread. 100 trees, 31 leaves, 10 columns. [Bench](https://microsoft.github.io/lightgbm-benchmark/results/inferencing/), header says do not trust yet. | Linux x86_64, 16 CPUs | 12.7087 μs | Well under, if the number holds. One row, not four cards. |
| Same bench, 1,000 trees, 10 columns | same | 95.4644 μs | Well under, same warning. |
| Same bench, 5,000 trees, 31 leaves, 1,000 columns | same | 580.791 μs | Still under 1 ms for one row. Same warning. |
| Treelite 1.3.0, 100 trees, 31 leaves, 10 columns, one row | same bench | 5.27241 μs | Well under, same warning. |
| LightGBM classifier, 500 trees, depth 8, 63 leaves, 50 features, one row, native Python. Not a ranker. [onnx_et_al](https://github.com/srepho/onnx_et_al), training shape in [train_models.py](https://github.com/srepho/onnx_et_al/blob/main/models/train_models.py) | Intel i7-8559U, 4 cores / 8 threads | median 1.535 ms, p95 2.073 ms | Under 5 ms. The call is Python. |
| Same LightGBM model through ONNX Runtime | same laptop CPU | median 0.059 ms, p95 0.081 ms | Well under. |
| XGBoost classifier, 500 trees, depth 8, 50 features, one row, native Python | same | median 8.196 ms, p95 9.664 ms | Fails. The write-up blames per-call overhead. |
| Same XGBoost model through ONNX Runtime | same | median 0.054 ms, p95 0.076 ms | Well under. |
| CatBoost classifier, 500 trees, depth 8, one row, native Python | same | median 3.407 ms, p95 4.407 ms | Median is under 5 ms. The p95 is not well under 5 ms. |
| Same CatBoost model through ONNX Runtime | same | median 0.037 ms, p95 0.069 ms | Well under. |
| ColumnTransformer plus the same LightGBM, one row, native and ONNX | same | median 10.187 ms native, 9.936 ms ONNX | Fails. Feature preparation is the time. |
| ONNX LightGBM behind a small feature encode, one prediction, including that encode | same project, MCP server note | about 150 μs (one printed sample is 152.3 μs) | Well under. Not a ranker. |
| LambdaMART, list of 150 items, then 300 items. [Metarank talk slide](https://metarank.github.io/datatalks-ltr-talk/) | I don't know the CPU or the tree count | 10 ms, then 20 ms | Fails at that list length. HORBM holds 4 hits. I don't know the 4-hit time. |

Delivery Hero timed LambdaMART on MSLR-WEB10K through JVM bindings, one thread, AMD Ryzen 7 2700, 300 trees. Single-thread CatBoost was 12 to 15 times faster than LightGBM and XGBoost, and still about 3 times faster than 16-thread LightGBM. [Source](https://deliveryhero.jobs/blog/is-catboost-faster-than-lightgbm-and-xgboost/). The milliseconds are in the charts. I don't know them, so that comparison does not enter the gate.

A 2025 cascade that puts LightGBM LambdaMART after a dense retriever prints end-to-end query time on two Xeon Silver 4314 CPUs, not the tree call alone ([arXiv:2510.16393](https://arxiv.org/abs/2510.16393)). I don't know the ranker slice. It is not used for the gate.

No CPU millisecond for allRank, DASALC, SetRank, DLCM, or TF-Ranking turned up. They are not in the under-5 ms set. I don't know their latency.

What the gate keeps: an in-process tree walk (C API, Treelite, ONNX, or a Rust loader) over a few hundred to a few thousand trees, scoring rows that are already numeric. The sourced one-row times for that shape are microseconds to about 1.5 ms. What it cuts: a Python `predict` that costs 8 ms before the trees matter, a feature pipeline that costs about 10 ms, a CatBoost Python p95 of 4.4 ms, a 10 ms list of 150 items, and a 13 ms call. Four HORBM candidates were not the thing those benches scored. I don't know that exact time.

### Clicks, without a judged label

Joachims, Swaminathan, and Schnabel field Propensity SVM-Rank on a live search engine and compare with balanced interleaving. Against the production ranker that logged the clicks: 87 wins, 48 losses, 83 ties, two-tailed binomial sign test p = 0.001, relative risk 0.71. Against a ranker trained on the same clicks with no propensity weight: 95 wins, 60 losses, 102 ties, p = 0.006, relative risk 0.77. I don't know the NDCG, the query count, or the CPU latency of that ranker.

Oosterhuis and de Rijke simulate users with a cascade click model. They do not run a live user study. After 10,000 impressions, under the perfect click model, linear PDGD reaches NDCG@10 0.427 (0.005) on MSLR-WEB10K, 0.736 (0.004) on Yahoo, and 0.573 (0.004) on Istella. DBGD linear on the same WEB10K cell is 0.331 (0.010). The paper's runs together are more than 90,000,000 simulated impressions. A neural scorer needs about 3,000 to 20,000 impressions to match the linear model, depending on click noise. Each impression displays 10 documents. The shown order is sampled, which is the exploration.

I don't know a HORBM-scale result for either method. HORBM has no click log. A click, or a user who keeps talking, is the reward these methods train on. That is the same family of signal as XiaoIce's CPS.

### TF-Ranking on Google click logs

Pasumarthi et al. train a 3-layer feed-forward net. Gmail search: about 250 million queries, 10% held out, clicks as labels, metrics weighted for position. Relative to pointwise sigmoid cross-entropy, listwise softmax cross-entropy is +1.80% MRR, +1.88% ARP, and +1.57% NDCG. Drive Quick Access: about 30 million instances, same listwise loss, +1.08% MRR and +1.05% NDCG. Absolute NDCG is not printed, because the models are proprietary. I don't know it. I don't know the CPU latency. The repository is archived.

## Rust crates

The hot path wants a crate that loads a finished model and scores a handful of numeric rows, offline, on rustc 1.85. Training can stay a separate offline tool.

| Crate | Licence | rustc 1.85 | What it actually does |
|---|---|---|---|
| [`gbdt` 0.1.3](https://crates.io/crates/gbdt), [gbdt-rs](https://github.com/mesalock-linux/gbdt-rs) | Apache-2.0 | I don't know the stated MSRV. The crate is pure safe Rust and has been on crates.io since 2018. | Trains regression and binary classification. Loads an XGBoost model for inference, and the README lists `rank:pairwise` among the objectives it accepts. The README says that load path was tested with XGBoost 0.81 and 0.82. I don't know whether a current XGBoost file still loads. I don't know an inference microsecond. |
| [`lightgbm3` 1.0.11](https://crates.io/crates/lightgbm3) | MIT | I don't know the MSRV. | Bindings to the LightGBM C library, which implements `lambdarank`. I don't know whether the Rust examples include a ranking call. Linking the C++ library is a new native dependency. |
| [`lightgbm-rust` 0.1.1](https://crates.io/crates/lightgbm-rust) | Apache-2.0 | MSRV 1.70, so the pin can compile it. | Downloads LightGBM binaries at build time. That build is not offline. |
| [`xgboost-rust` 0.1.0](https://crates.io/crates/xgboost-rust/0.1.0) | Apache-2.0 | MSRV 1.70. | Downloads an XGBoost wheel at build time. The README says Windows is not covered by that download. One release, December 2025. |
| [`catboost-rust` 0.4.0](https://crates.io/crates/catboost-rust) | Apache-2.0 | MSRV 1.85, equal to the pin. | The README says it downloads CatBoost binaries at runtime. A voice turn that fetches a library is not offline. |
| [`linfa-trees` 0.8.1](https://crates.io/crates/linfa-trees) and `linfa-ensemble` | MIT OR Apache-2.0 | linfa 0.7.1 has MSRV 1.71.1. linfa 0.8.0 raises MSRV to 1.87.0. linfa 0.9.0 raises it to 1.88.0 ([changelog](https://github.com/rust-ml/linfa/blob/master/CHANGELOG.md)). 0.8 and 0.9 do not build on the pin. | Single decision trees, then bagging, random forest, and AdaBoost. [Issue 411](https://github.com/rust-ml/linfa/issues/411) is still an open request for gradient boosting. There is no ranking loss. |

I don't know a pure-Rust LambdaMART. `gbdt` is the closest existing piece, and only as an inference loader for an older XGBoost pairwise model.

TF-Ranking and allRank are Python. I don't know a Rust port of either.

## Fit for HORBM

Five is a good fit for that column. One fights the constraint. The covering rule is the code that already runs. It is the bar a learned ranker has to clear.

Latency here is only the 5 ms gate. An option with no sourced time is not eligible, even if it is probably fast.

| Option | CPU latency | Data needed | Abstention | Source stays on the answer | Complexity |
|---|---|---|---|---|---|
| Covering rule, already in the crate | 5. No added model. I don't know a measured millisecond | 5 | 4 | 5 | 5 |
| LightGBM, in-process C API or ONNX, hundreds to a few thousand trees | 5. Sourced one-row times 0.013–1.5 ms. The C API page is marked untrusted. The 1.5 ms figure is a classifier, not LambdaMART | 2 | 2 | 5 | 3 |
| XGBoost in-process or ONNX | 5 on the 0.054 ms ONNX row. That model is a classifier | 2 | 2 | 5 | 3 |
| XGBoost native Python `predict` | 1. Median 8.196 ms on the laptop-class CPU. Fails | 2 | 2 | 5 | 3 |
| CatBoost native Python, 500 trees | 2. p95 4.407 ms is under 5 ms and not well under | 2 | 2 | 5 | 2 |
| CatBoost through ONNX | 5. Median 0.037 ms. Classifier, not YetiRank | 2 | 2 | 5 | 2 |
| Feature pipeline wrapped around the trees | 1. About 10 ms in the same bench. Fails | 2 | 2 | 5 | 2 |
| Neural listwise (allRank, DASALC, TF-Ranking) | 1. No sourced CPU time under 5 ms, so not eligible | 1 | 2 | 5 | 1 |
| Propensity SVM-Rank | I don't know a millisecond. Not eligible until timed | 2 | 2 | 5 | 3 |
| PDGD, online | 1. The shown list is a sample. No inference millisecond | 2 | 1 | 4 | 2 |
| `gbdt` inference of `rank:pairwise` | I don't know a microsecond. Not eligible until timed | 2 | 2 | 5 | 4 |

Why the middle columns are low for every learned option:

- **Data.** LambdaMART, YetiRank, and the neural listwise models need a grade per candidate. The public sets that separate them have thousands of queries and millions of pairs. HORBM does not have that set. CatBoost says ordered boosting usually scores better on a small dataset and is slower. The parameters page lists the CPU default as Plain, and lists Ordered as the GPU default for a non-multiclass set of at most 50,000 rows. The speed-up page says a small dataset defaults to Ordered. Either way the mode still needs labels. Fifty thousand XiaoIce dialogues are engagement grades, and they do not transfer to truth cards.
- **Abstention.** A LambdaMART score is a sort key. XiaoIce thresholds it and then samples. Sampling is exploration for CPS. A threshold that means "this card is true" has to be fit on held-out judgements of truth. I don't know a published calibration error for these scores. The covering rule abstains on a stated condition: no full cover, or a tie. That condition does not need a fitted threshold.
- **Source.** Any of these rankers can keep the source if the only thing it does is choose a stored clause. The card id is already on the hit. The ranker must not rewrite the sentence. XiaoIce's neural generator is a different component, and it does rewrite.
- **PDGD.** The unbiasedness proof is about click preferences, and the algorithm displays a sampled order. Speaking a sampled card in the voice turn is a guess on the turns where the sample is wrong. The miss line would not be stable.
- **List length.** The cell cap is 4. allRank's measured gains come from attending over lists of length 30 to 360. A listwise net is built for a problem HORBM's retriever does not have.

The one place a tree can still help later is inside those four hits, when more than one card covers the question and the shortest haystack is the wrong card. I don't know how often that happens on the real pack. `tests/pack_bench.rs` prints a time only when `GALION_HORBM_KNOWLEDGE` is present, and this session did not run it.

## Recommendation

**Now.** Leave the ranker unlearned. The covering rule already picks one stored clause or says `I don't know.`, and it does that with no labels and no score to mis-calibrate. Keep `OfflineBandit` as a counter. Do not let `choose_steered` pick a card. Do not copy XiaoIce's sample-above-threshold step, and do not copy the editorial line that avoids "I don't know." Both of those exist to raise CPS.

**Later, once a judged set exists, and only on a runtime that already has a sourced time well under 5 ms.** Train LightGBM `lambdarank` and score the four hits in process, through the C API, Treelite, or ONNX. The features stay the ones the turn already computed: token cover, haystack length, the lexical or RRF score, which pool the hit came from, and the card's claim type. Do not add a second feature pipeline inside the ranker call. The insurance bench spent about 10 ms on that kind of pipeline and missed the gate, while the tree alone stayed under 2 ms. Speak the chosen clause unchanged, with its existing source id. Abstain, with the exact miss line, when the top two scores tie or the top score is below a cut fit on a held-out slice of the same judgements. Do not sample.

XGBoost `rank:ndcg` is the same shape of model if it is scored in process or through ONNX. The native Python `predict` on a 500-tree model took 8.2 ms median on a 4-core laptop CPU and fails the gate. CatBoost YetiRank is in the same boat: use it only through a runtime whose printed time is well under 5 ms. The native Python p95 of 4.4 ms on a 500-tree classifier is not that. The `catboost-rust` crate that downloads binaries at runtime does not belong in the voice process.

Neural listwise rankers stay off the list until a CPU timing of that model, at four candidates, is well under 5 ms. None of the papers in this lane print that time. On Qin's split they tie or slightly beat LightGBM only as an ensemble of wide networks.

Online bandits are the wrong teacher. The reward they have published results for is a click or a continued session. HORBM's reward is "this clause is the true one, or say you do not know." If a real correctness signal is ever logged, train offline with inverse-propensity weights on those logs, and keep the live turn greedy.

## What would change this

- A judged HORBM set, truth grades rather than "did the chat continue", where a neural listwise model beats LightGBM LambdaMART on a frozen slice, and a CPU timing of that model at four candidates is well under 5 ms on a laptop.
- A count, on the real card pack, of how often two covering cards tie or the shortest haystack is the wrong fact. If that count is zero, a ranker has nothing to do.
- A correctness log large enough that an offline propensity-weighted ranker beats the covering rule on a frozen slice, with exploration left out of the voice turn.
- A pure-Rust LambdaMART, or a tested load of a current XGBoost `rank:ndcg` model in `gbdt`, whose held-out NDCG is close to LightGBM on a public set and whose inference time is measured. Until that load is tested, "close to LightGBM" is **I don't know.**

## Sources

- Zhou, Gao, Li, Shum. The Design and Implementation of XiaoIce, an Empathetic Social Chatbot. Computational Linguistics, 2020. https://aclanthology.org/2020.cl-1.2.pdf
- Burges. From RankNet to LambdaRank to LambdaMART: An Overview. Microsoft Research, MSR-TR-2010-82. https://www.microsoft.com/en-us/research/publication/from-ranknet-to-lambdarank-to-lambdamart-an-overview/
- LightGBM, MIT. https://github.com/lightgbm-org/LightGBM and https://lightgbm.readthedocs.io/en/latest/Parameters.html and https://github.com/microsoft/LightGBM/blob/master/docs/Experiments.rst
- LightGBM inferencing bench, marked work in progress. https://microsoft.github.io/lightgbm-benchmark/results/inferencing/
- srepho, onnx_et_al, single-row CPU timings on an Intel i7-8559U. https://github.com/srepho/onnx_et_al and https://github.com/srepho/onnx_et_al/blob/main/models/train_models.py
- Metarank, LambdaMART latency slide. https://metarank.github.io/datatalks-ltr-talk/
- Cascade that reports end-to-end time with a LightGBM LambdaMART second stage. The tree time is not isolated, so it is not a gate number. https://arxiv.org/abs/2510.16393
- XGBoost, Apache-2.0. https://github.com/dmlc/xgboost and https://xgboost.readthedocs.io/en/stable/tutorials/learning_to_rank.html
- CatBoost, Apache-2.0. https://github.com/catboost/catboost and https://catboost.ai/docs/en/concepts/loss-functions-ranking and https://catboost.ai/docs/en/references/training-parameters/common and https://catboost.ai/docs/en/concepts/speed-up-training
- Delivery Hero, CatBoost versus LightGBM versus XGBoost prediction latency. https://deliveryhero.jobs/blog/is-catboost-faster-than-lightgbm-and-xgboost/
- Bruch, Zoghi, Bendersky, Najork. Revisiting Approximate Metric Optimization in the Age of Deep Neural Networks. SIGIR 2019. https://marc.najork.org/papers/sigir2019b.pdf
- Qin et al. Are Neural Rankers still Outperformed by Gradient Boosted Decision Trees? ICLR 2021. https://marc.najork.org/papers/iclr2021.pdf
- Pobrotyn et al. Context-Aware Learning to Rank with Self-Attention. 2020. https://arxiv.org/abs/2005.10084 and https://github.com/allegro/allRank
- Pasumarthi et al. TF-Ranking: Scalable TensorFlow Library for Learning-to-Rank. KDD 2019. https://arxiv.org/abs/1812.00073 and https://github.com/tensorflow/ranking
- Joachims, Swaminathan, Schnabel. Unbiased Learning-to-Rank with Biased Feedback. WSDM 2017. https://doi.org/10.1145/3018661.3018699
- Oosterhuis and de Rijke. Differentiable Unbiased Online Learning to Rank. CIKM 2018. https://arxiv.org/abs/1809.08415 and https://github.com/HarrieO/OnlineLearningToRank
- gbdt crate, Apache-2.0. https://crates.io/crates/gbdt and https://github.com/mesalock-linux/gbdt-rs
- lightgbm3, MIT. https://crates.io/crates/lightgbm3
- lightgbm-rust, Apache-2.0. https://crates.io/crates/lightgbm-rust
- xgboost-rust, Apache-2.0. https://crates.io/crates/xgboost-rust/0.1.0
- catboost-rust, Apache-2.0. https://crates.io/crates/catboost-rust
- linfa. https://github.com/rust-ml/linfa/blob/master/CHANGELOG.md and https://crates.io/crates/linfa-trees and https://github.com/rust-ml/linfa/issues/411
