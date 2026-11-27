> **Research synthesis dated 2026-11-27 (Europe/Zagreb).** Desk synthesis only. It is not a measurement on the Galion box, not a LOCKS change, and not product code. Nothing here was downloaded, built, or timed on this machine.
>
> **LOCKS.toml overrides any conflicting sentence below.** `forbidden_runtimes` lists `candle`, `ort`, `tract`, `burn`, `llama`, and `ggml`. `candle` is `dev_oracle_only`. The only `ort` pin is the Piper sidecar (`formats.exceptions.a21_piper`); that pin says not to widen it. Agents do not edit `LOCKS.toml`.
>
> The shared lane brief (`/tmp/ranker-common.txt`) was not on disk when this note was written. Scope is the assignment that was: neural rerankers from 2018 onward that could run on CPU, and how their scores can support abstention.

# Neural rerankers on CPU (2018+)

**Owner:** Maciej Grajczyk · **Product:** Galion Voice · **Lane:** neural-rerankers
**Written:** 2026-11-27 · **Reads:** `LOCKS.toml` [D], `docs/galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md` §3 and §7.7 [D]

**How to read a number.** **[C]** is cited, with the link in §8. **[D]** is a design constant already in this tree. **[E]** is an estimate made in this note. An estimate is not a measurement. Third-party timings are other people's hardware; they are not Galion p50/p95.

---

## 0. What this lane decides

Galion memory retrieval, as planned, is a hybrid of a static embedder and FTS5 BM25, fused by reciprocal rank, with a read budget of ≤5 ms p95 at 100k items on the context worker [D]. The plan already names a neural reranker as the fallback if the recall gate fails, and it quotes a cost of 4–12 ms [D]. This note checks that quote against the models that actually exist.

The voice also has to abstain. A missing fact, a timed-out side band, or an empty memory is `NoKnowledge`, said in ordinary words [D]. A reranker is useful here only if its score can be turned into a threshold: speak the hit, or say the memory does not hold it. A sort key that is not a probability cannot be that threshold until something calibrates it.

Four families cover the 2018+ literature that can sit on a CPU. They are not interchangeable.

| Family | What is scored at query time | Document work can be done ahead | Score, before any calibration |
|---|---|---|---|
| Cross-encoder (BERT, MiniLM, TinyBERT, BGE, Jina) | One forward pass per query–passage pair | No | One logit. Sometimes squashed with a sigmoid |
| monoT5 / RankT5 | One encoder–decoder pass per pair, one new token | No | monoT5: softmax over the `true` and `false` logits. RankT5: a ranking score |
| ColBERT / ColBERTv2 | Encode the query, then MaxSim against stored token vectors | Yes | Sum of per-query-token max cosines. Grows with the query |
| SPLADE | Encode the query into a sparse vocab vector, then an inverted-index dot product | Yes | Sparse dot product |

---

## 1. Cross-encoders

### 1.1 BERT re-ranking (2019)

Nogueira and Cho feed the query as sentence A and the passage as sentence B into BERT, take the `[CLS]` vector, and map it with one linear layer to a relevance probability. Training is cross-entropy on BM25's top 1,000, with the query truncated at 64 tokens and the pair at 512. On MS MARCO passage, BERT-base reaches 34.7 MRR@10 on dev and BERT-large reaches 36.5 dev / 35.8 eval, against 16.7 for untuned BM25. The relative gain over the previous leaderboard entry is the 27% they report [C1].

That probability is a real training target: the loss is \(-\log s\) on relevant passages and \(-\log(1-s)\) on the rest [C1]. It is a probability for the MS MARCO label definition. It is not, by itself, a calibrated probability for a personal fact store. Section 5 is about that gap.

The cost is the reason later models exist. Khattab and Zaharia, re-ranking the official BM25 top 1,000 on one Tesla V100, measure BERT-base at 10,700 ms per query and BERT-large at 32,900 ms [C8]. A 340M-parameter cross-encoder is not a CPU reranker for a live call.

### 1.2 MiniLM and TinyBERT students

MiniLM (Wang et al., 2020) distils the teacher's last-layer self-attention into a smaller Transformer. A 6-layer, 384-wide student is about 11.7M embedding parameters plus 10.6M Transformer parameters [C2]. The MS MARCO cross-encoders shipped by sentence-transformers are that student, cut down further:

| Model | Params | NDCG@10, TREC DL 19 | MRR@10, MS MARCO dev | Docs/sec on a V100 |
|---|---:|---:|---:|---:|
| `cross-encoder/ms-marco-TinyBERT-L-2-v2` | 4.4M [C15] | 69.84 | 32.56 | 9,000 |
| `cross-encoder/ms-marco-MiniLM-L-2-v2` | — | 71.01 | 34.85 | 4,100 |
| `cross-encoder/ms-marco-MiniLM-L-4-v2` | — | 73.04 | 37.70 | 2,500 |
| `cross-encoder/ms-marco-MiniLM-L-6-v2` | 22.7M [C3] | 74.30 | 39.01 | 1,800 |
| `cross-encoder/ms-marco-MiniLM-L-12-v2` | 33.4M [C15] | 74.31 | 39.02 | 960 |

The docs/sec column is a GPU figure from the sentence-transformers table [C3]. It is not a CPU figure. L6 and L12 land on the same MS MARCO number; the extra six layers buy almost nothing on that set and cost about half the throughput. The Hub cards for TinyBERT-L2, MiniLM-L6, and MiniLM-L12 are Apache-2.0 [C16]. L6's context is 512 [C3].

The model card's own example prints raw scores `[8.607, -4.320]` for a relevant and an irrelevant pair [C3]. The library note says those raw values reasonably span about −10 to 10, and that passing `activation_fn=Sigmoid()` forces the output into (0, 1) [C4]. A sigmoid of an 8.6 logit is already 0.9998. That is a ranking signal. It is a poor abstention signal until the temperature is fit on Galion's own labels (section 5).

### 1.3 Later small cross-encoders

These are the 2023–2024 models people actually ship as "the reranker":

| Model | Size | Licence | Score | Why it matters on CPU |
|---|---|---|---|---|
| `BAAI/bge-reranker-base` | 278M, XLM-RoBERTa, 12×768, vocab 250,002 [C15][C18] | MIT. The card says the released models may be used commercially [C6] | Cross-entropy logit, explicitly **not** bounded to a range [C6] | Too wide for a live CPU rerank. About 69% of the parameters are the 250,002×768 word table (192M), which a pair barely multiplies [C15] |
| `BAAI/bge-reranker-v2-m3` | 568M [C17] | Apache-2.0 in a third-party licence table [C16]; confirm the card before any use | Same cross-encoder pattern | One third-party ONNX int8 service measured ~0.95 s per ~150-word document on an i5-8250U [C17] |
| `jinaai/jina-reranker-v1-tiny-en` | 33.0M, 4 layers, hidden 384 [C7] | Apache-2.0 [C7] | Logit, then sigmoid in the transformers.js example [C7] | Distilled from their base reranker. JinaBERT with ALiBi, context 8,192. Needs `trust_remote_code`. An ONNX int8 file of 33.4 MB is published [C19] |
| `jinaai/jina-reranker-v1-turbo-en` | 37.8M, 6×384 [C7] | Check the card; the tiny sibling is Apache-2.0 | Same | The tiny card's own BEIR average: tiny 48.54 NDCG@10, turbo 49.60, MiniLM-L6 48.64, bge-reranker-base 47.89, across 17 BEIR sets [C7]. Vendor table |
| `jinaai/jina-reranker-v2-base-multilingual` | 278M [C17] | CC-BY-NC-4.0 [C16][C17] | — | Non-commercial. Out of a shipped Galion path |
| `mixedbread-ai/mxbai-rerank-xsmall-v1` | 70.8M [C15] | Apache-2.0 in the same third-party table [C16] | Cross-encoder logit | On the PALDYN CPU bench it matched MiniLM and cost about 5× more [C15] |
| `Alibaba-NLP/gte-reranker-modernbert-base` | 149.6M [C15] | Confirm the card | Cross-encoder logit | The one model in that bench whose SciFact gain sat outside the others' interval. Also the slowest [C15] |

`bge-reranker-base` is the default reranker in `fastembed` 7.x [C20]. That crate is an `ort` program. Section 4 says why that does not ship inside Voice.

---

## 2. monoT5 and RankT5

monoT5 (Nogueira, Jiang, Pradeep, and Lin, Findings of EMNLP 2020) casts ranking as text generation. The input template is `Query: q Document: d Relevant:`. The model is fine-tuned to emit the single token `true` or `false`. At inference only those two logits are kept, a softmax is applied to the pair, and the probability of `true` is the relevance score. Softmax over the whole vocabulary, or the raw `true` logit alone, gave them retrieval metrics near zero [C5].

On MS MARCO dev, BM25 is 0.184 MRR@10, BM25 plus BERT-large is 0.372, plus T5-base is 0.363, plus T5-large is 0.383, plus T5-3B is 0.382. They note T5-3B had not finished converging [C5]. In a low-data probe, T5-base fine-tuned on 2,000 pairs beat BERT-base fine-tuned on the same pairs, and BERT-base at 2,000 pairs fell behind BM25 [C5]. The useful property for Galion is the score, not the MRR: the two-token softmax is a probability by construction. It still needs a calibration check on personal facts. The castorini `monot5-base-msmarco-10k` card states no licence [C16]. T5 itself is Apache-2.0; the fine-tune's terms are not written down. Treat the checkpoint as research-only until the card says otherwise.

T5-base is the encoder–decoder behind the 0.363 row [C5]. One new token is cheaper than a full generation, and the network is still far larger than MiniLM-L6. The paper publishes no CPU latency. **[E]** one T5-base pair on a single CPU core sits in the same hundreds-of-milliseconds band as `bge-reranker-base` on the benches in §6, which does not fit the context worker during a live call.

RankT5 (Zhuang et al., 2022) studies encoder–decoder and encoder-only T5 rankers trained with pairwise or listwise losses instead of pointwise classification. Listwise losses helped zero-shot ranking [C21]. A listwise loss optimises order. It does not produce a probability. RankT5 is the wrong head if the consumer of the score is an abstention threshold.

---

## 3. ColBERT and ColBERTv2

ColBERT (Khattab and Zaharia, SIGIR 2020) encodes the query and the passage with separate BERT passes, then scores them by late interaction. For each query token vector, take its maximum cosine against the passage token vectors, and sum those maxima (MaxSim) [C8]:

\[
s(q, d) = \sum_{i \in q} \max_{j \in d} \cos(E_q^{(i)}, E_d^{(j)})
\]

Document matrices can be stored ahead of the query. Re-ranking BM25's top 1,000, their BERT-base ColBERT reaches 34.9 MRR@10 on MS MARCO dev and eval, in 61 ms on a V100, against 10,700 ms for a BERT-base cross-encoder on the same GPU. The FLOPs per query drop from 97T to 7B [C8]. That 61 ms excludes nothing they forgot to mention: it is the re-rank, on a GPU, of 1,000 passages whose vectors are already in GPU memory. It is the existence proof that late interaction is the CPU-shaped architecture. It is not a CPU measurement.

ColBERTv2 (Santhanam et al., NAACL 2022) keeps MaxSim and adds two things: distillation from a cross-encoder ("denoised supervision"), and a residual compression. Each 128-d token vector is stored as a centroid id plus a 1-bit or 2-bit residual. They use 4 bytes for the centroid id and 16 or 32 bytes for the residual, so 20 or 36 bytes per token, against ColBERT's 256-byte fp16 vector. Official MS MARCO dev: 39.7 MRR@10, 86.8 R@50, 98.4 R@1k. Their search figure, on a Titan V plus two Xeon Gold 6132s, sits mostly under 150 ms and around 100 ms for the best quality, across MS MARCO (~9M passages) and two LoTTE sets [C9].

PLAID (Santhanam et al., CIKM 2022) is the engine, not a new model. It filters with a bag-of-centroids approximation before it decompresses residuals. On MS MARCO v1, PLAID ColBERTv2 at k=10 is 39.4 MRR@10 in 185.5 ms on 1 CPU thread, 31.5 ms on 8 threads, and 11.5 ms on a Titan V. The machine is 28 Xeon Gold 6132 cores at 2.6 GHz. They discard query-encoding time, on the argument that a distilled quantized encoder can make it small [C10]. For Galion that discarded term is the whole problem: a personal store of 10k short facts does not need PLAID's candidate filter, and the BERT query encode is what the core spends.

Two consequences for abstention.

- MaxSim is a sum. A longer query has a larger score with no change in relevance. Divide by the number of query tokens before any threshold [E]. The papers do not do this; they rank, and a monotone rescaling does not change a ranking.
- The residual quantisation was evaluated for ranking quality [C9]. It was not evaluated as a calibrated probability. A 2-bit residual changes the cosine. A threshold fit on fp32 MaxSim does not transfer to the compressed index without a new fit.

`answerai-colbert-small-v1` (Answer.AI, August 2024) is a 33.4M-parameter late-interaction model, Apache-2.0, trained with the JaColBERTv2.5 recipe from a weight-average of `gte-small` and `bge-small-en-v1.5`. The authors say it outperforms the 110M ColBERTv2 on the benchmarks they report, including LoTTE, which it did not train on, and that this size "can search through hundreds of thousands of documents in milliseconds, on CPU" [C11]. That sentence is their claim about search, not a table with a CPU sku, a passage length, or a recall. The model is the right size to prototype. The latency sentence is not a number Galion can plan on.

The ColBERT repository is MIT [C22]. The `colbert-ir/colbertv2.0` weights are listed as MIT in a third-party licence table [C16]; confirm the model card before a pin.

---

## 4. SPLADE

SPLADE (Formal, Piwowarski, and Clinchant, SIGIR 2021) predicts a weight for every BERT vocabulary term from the masked-language-model logits, applies \(\log(1+\mathrm{ReLU}(\cdot))\), and regularises with an \(\ell_1\) or a FLOPS term so the vector stays sparse enough for an inverted index. Ranking is a sparse dot product. On MS MARCO dev their \(\ell_1\) and FLOPS variants both reach 0.322 MRR@10 [C12]. SPLADE v2 adds distillation and hard negatives; SPLADE++ is the SIGIR 2022 short paper. ColBERTv2's table credits SPLADEv2 with 36.8 MRR@10 on the official MS MARCO dev set [C9].

This is a first-stage retriever that happens to be neural, not a cross-encoder. On a personal store it competes with FTS5, which Galion already has [D]. The efficiency paper (Lassance and Clinchant, SIGIR 2022) is the CPU one. On one Xeon Gold 6338 at 2.0 GHz, a DistilBERT query encoder costs 45.3 ms average (p99 57.6 ms) at sequence length 8, and a BERT-tiny query encoder costs 0.7 ms average (p99 1.1 ms). Their sparsest BERT-tiny SPLADE (BT-SPLADE-S) measures 7 ms end to end and 35.8 MRR@10 on MS MARCO, against BM25 at 4 ms and 19.7. The same sparsity costs them out-of-domain: BEIR average nDCG@10 falls from 47.0 for SPLADEv2-distil to 39.2 for BT-SPLADE-S. Adding BM25 back (their BEIR\* column) recovers 45.9 [C13].

The NAVER code and the usual `splade-cocondenser-ensembledistil` checkpoint are CC BY-NC-SA 4.0 [C14][C16]. That is a non-commercial, share-alike licence. It does not belong in shipped Voice. The design lesson still stands, and it is already the plan's lesson: a sparse lexical score plus a dense score, fused, beats either one when the neural model was trained far from the user's facts [C13][D].

---

## 5. Calibrated scores and abstention

### 5.1 What each family emits

| Family | Raw value | A monotone squash | Safe to threshold out of the box? |
|---|---|---|---|
| BERT cross-encoder, as trained by Nogueira and Cho | A cross-entropy probability [C1] | Already in (0, 1) if the head is a sigmoid | Only on data that looks like the training labels |
| sentence-transformers MS MARCO MiniLM | A logit, about −10 to 10 [C3][C4] | `Sigmoid()` maps to (0, 1) [C4] | No. An 8.6 logit becomes 0.9998. Almost every "somewhat relevant" pair saturates |
| `bge-reranker-*` | An unbounded cross-entropy logit [C6] | Sigmoid, if the caller adds it | No. The authors say the score is not a bounded relevance |
| monoT5 | \(P(\texttt{true})\) from a two-class softmax [C5] | Already a probability | Closer. Still fit on Galion labels. The checkpoint licence is unstated [C16] |
| RankT5 with a listwise loss | A ranking score [C21] | — | No. Order is the training target |
| ColBERT MaxSim | Sum of cosines [C8] | Divide by query length [E] | No. Length-normalise, then fit a calibrator |
| SPLADE | Sparse dot product [C12] | — | No. Same shape as BM25: a rank feature, not a chance |

Guo, Pleiss, Sun, and Weinberger (ICML 2017) showed that modern nets are miscalibrated and that temperature scaling, a single scalar \(T\) on the logits, matches or beats heavier post-hoc methods on the sets they tried. \(T\) is fit by negative log-likelihood on held-out data. Because every logit is divided by the same \(T\), the argmax does not change [C23]. For a one-logit cross-encoder the map is \(\sigma(z/T)\). For monoT5 it is a softmax of the two logits divided by \(T\). Si et al. (Findings of EMNLP 2022) repeat the point for question answering: temperature scaling changes confidence and leaves the prediction in place, and they use a sigmoid over a BERT reranker logit as one of their confidence features [C24].

Platt scaling is the two-parameter version (a slope and a bias on the logit). Isotonic regression is the non-parametric version. On a 200-question personal set, isotonic regression will overfit [E]. Temperature or Platt is the fit that matches the sample size.

### 5.2 A threshold is a dataset fact, not a model fact

CRAG (Yan et al., 2024) fine-tunes T5-large (0.77B) as a retrieval evaluator. Each retrieved document gets a score in \([-1, 1]\). Three actions follow: Correct if any document clears an upper threshold, Incorrect if every document sits under a lower threshold, Ambiguous in between. The thresholds are empirical and different per set: PopQA (0.59, −0.99), PubHealth and Arc-Challenge (0.5, −0.91), Biography (0.95, −0.91) [C25]. Copying 0.59 onto a MiniLM sigmoid would be meaningless. The pattern is the part that maps onto `NoKnowledge`:

- Correct → the memory hit may be spoken.
- Ambiguous → the voice says it is unsure, or asks, and does not invent the fact.
- Incorrect → `NoKnowledge`. The side band already has this outcome for timeout and for an unwired nerve [D].

Self-RAG's critic is an instruction-tuned 7B model [C25]. It is the same idea at a size the 8-core box cannot run beside ASR, the student, and Piper. CRAG's point is that a separate small evaluator beat a prompted large model at the relevance judgment [C25]. Galion's evaluator, if it needs one, is the small cross-encoder, not a second language model.

A max score is not sufficiency. Two memories can both score high and contradict each other. The abstention rule wants a second feature: the gap between the top score and the second score, after calibration. A small gap means the rank is unstable. That feature works for MaxSim and for BM25 as well, because it never treats the score as a probability [E].

### 5.3 A procedure that matches the gate already in the plan

The memory plan's recall gate is recall@5 ≥ 0.8 on a 200-question personal-fact set [D]. Use that same set, and add unanswerable questions (facts that were never stored, and facts stored under a different wording). Then:

1. Keep the first stage as it is: static embed, FTS5, reciprocal-rank fusion [D]. Log the top-8 texts and the fused rank. Do not threshold the fused score.
2. Score those eight with one pointwise model whose raw output is a logit (MiniLM-L6 or TinyBERT-L2). Record the logit, not a saturated sigmoid.
3. Fit \(T\) on a held-out half of the 200, by negative log-likelihood [C23]. Freeze \(T\).
4. On the other half, sweep two thresholds and report an abstain curve: fraction of questions the voice answers, and how often that answer is the right fact. Pick the operating point in that table. Do not ship 0.5.
5. Also abstain when the calibrated top-two gap is under a second constant, fit on the same half.
6. If int8 weights are the ones that will run, fit \(T\) on the int8 scores. A temperature fit on fp32 does not survive a quantiser that moves logits (section 6).

Expected errors, stated so the gate can see them. A model trained on Bing passages will over-score a lexical overlap with a personal fact and under-score a paraphrase that shares no content word [E]. That is why the fit is on Galion questions, and why a failure of the recall gate is allowed to change the model. It is not a reason to skip the fit.

---

## 6. CPU cost, int8, and what the 4–12 ms quote can mean

### 6.1 Published CPU timings

These rows are not comparable with each other. The hardware, the candidate count, the batch, and the precision all differ. They are listed so the spread is visible.

| Source | Setup | Result |
|---|---|---|
| PALDYN AI Lab, 2026-10-16 [C15] | Xeon 2.10 GHz, 4 vCPU, `torch.set_num_threads(4)`, fp32, batch 16, BEIR SciFact, 300 queries. The lead says five shared candidates; the retrieval script stores the cosine top 10 | TinyBERT-L2: 12.8 ms/query, p95 18.4, nDCG@10 0.6772 (no-rerank 0.6451). MiniLM-L6: 97.2 / p95 121.2 / 0.6808. MiniLM-L12: 180.4 / 216.2 / 0.6817. mxbai-xsmall: 485.4 / 558.6 / 0.6811. bge-reranker-base: 567.2 / 646.6 / 0.6791. gte-modernbert: 819.6 / 928.5 / 0.7045. A second install moved bge by 18%. Paired bootstrap: TinyBERT vs MiniLM-L6 is −0.0037 with an interval that includes 0 |
| A DEV Community post [C26] | "CPU only", default batch 32, median of 5. The CPU sku is not stated. Two runs on the same box differed by up to 2× | MiniLM-L6, k=5: 11–22 ms; k=20: 29–53 ms; k=50: 63–86 ms; k=100: 126–171 ms |
| clawRxiv preprint 2604.01082 [C27] | "Intel Xeon, AWS EC2", one CPU, 100 pair predictions, best of 3 | MiniLM-L12: 23.1 ms/pair. `bge-reranker-large`: 332 ms/pair. Bi-encoder MiniLM-L6: 5.83 ms/sentence. Preprint, not a venue paper, sku unstated |
| RAGatouille issue 190 [C28] | One user's CPU numbers. k and the CPU are not in the comment that was retrieved | MiniLM-L6 150 ms, MiniLM-L12 309 ms, ColBERT 620 ms. The maintainer's reply: L6 is ~22M, L12 ~35M, ColBERTv2 is 110M, and ColBERT can precompute document vectors |
| rahulnitb/bge-reranker-api [C17] | Windows, i5-8250U (4 cores / 8 threads, AVX2, no VNNI), ~150-word documents, p50, 3 requests per cell | `bge-reranker-v2-m3` ONNX int8: 0.95 s / 6.1 s / 33.8 s for 1 / 10 / 50 documents. A fused int8 `mmarco-mMiniLMv2-L12`: 40 ms / 0.40 s / 2.95 s. `jina-reranker-v2-base-multilingual` int8: 0.17 s for one document, and that model is CC-BY-NC-4.0 |
| tiebreak [C29] | One laptop CPU, Windows, ONNX Runtime, BM25 top 100, ms per pair | MiniLM-L6 fp32: 33–55 ms/pair depending on the set. The same model int8: 40.6 ms/pair on SciFact (fp32 55.4). `bge-reranker-base` int8: 110–175 ms/pair. `bge-reranker-v2-m3` int8: 492 ms/pair on 30 SciFact queries |
| PLAID [C10] | Xeon Gold 6132, MS MARCO v1, query encode excluded | 185.5 ms at 1 thread and 31.5 ms at 8 threads for k=10, at 39.4 MRR@10 |
| maxsim-cpu [C30] | The interaction only, ~1,000 documents, not the encoder | NumPy 50.3 ms, Numba 23.9 ms, their Rust kernel 3.36 ms |
| Efficient SPLADE [C13] | One Xeon Gold 6338 thread | BERT-tiny query encode 0.7 ms average. BT-SPLADE-S retrieval-plus-encode 7 ms. CC BY-NC-SA weights |
| ColBERT paper [C8] | One V100, re-rank of 1,000 | 61 ms. GPU. Included so it is not mistaken for a CPU number |

Read across the CPU rows and the stack plan's "4–12 ms" [D] lines up with one case: TinyBERT-L2 scoring a handful of short candidates on a few threads, in the PALDYN setup (12.8 ms for the query, p95 18.4) [C15]. MiniLM-L6 on that same box is about 100 ms. A 278M or 568M reranker is a large fraction of a second to several seconds. **[E]** on the planned 8-core Xeon, with the context worker on one core and the other cores busy with ASR, the student, and two Pipers, a MiniLM-L6 rerank of 8 short memory lines is tens of milliseconds and a TinyBERT-L2 rerank of those 8 lines can land near the low tens. Neither number has been measured here. Neither belongs on the partial-transcript path whose budget is ≤5 ms [D].

### 6.2 int8

ONNX Runtime's quantisation guide recommends dynamic int8 for Transformer models and static int8 for CNNs. Dynamic quantisation computes activation scales on the fly, which costs something and usually holds accuracy better than a static calibration. The conversion is not lossless; the guide says to compare tensors and to leave the worst layers in fp32, or to fall through to quantisation-aware training [C31].

The two small CPU checks that report quality, not just speed, disagree in a way that matters:

- tiebreak's MiniLM int8 kept the speedup (40.6 vs 55.4 ms/pair on SciFact) and moved logits by up to 0.74. One in ten top-10 lists stayed identical. They leave int8 opt-in for that reason [C29].
- The bge-reranker-v2-m3 service, on 10 handwritten queries × 4 documents, reports int8 Spearman 0.991 against PyTorch, the same top-1, and a max normalised-score gap of 0.027. The author calls it a sanity check [C17].

A ranking can survive a logit shift that an abstention threshold cannot. If the shipped weights are int8, the temperature and the thresholds in §5.3 are fit on those int8 scores. ColBERTv2's 2-bit residual is the quantisation its own paper measured for MRR [C9]. ONNX dynamic int8 of a cross-encoder is a different transform, and galion-tensor's symmetric per-row `gemv_i8` [D] is a third one. Each needs its own fit.

### 6.3 Rust crates, and the locks

| Crate | What it runs | Runtime | Shipped Voice |
|---|---|---|---|
| `fastembed` 7.x [C20] | `TextRerank` for `bge-reranker-base` (default), `bge-reranker-v2-m3`, `jina-reranker-v1-turbo-en`, `jina-reranker-v2-base-multilingual`. `SparseTextEmbedding` for SPLADE. `Bgem3Embedding` emits a ColBERT multi-vector. Optional `qwen3-text` reranker on candle, read as the yes-vs-no probability at the last position | `ort` 2.0.0-rc.15 by default. Candle is behind feature flags | **No.** `ort` is a forbidden runtime, and the Piper pin is the only exception [D] |
| `candding` 0.1.0 [C32] | Candle implementations, no ONNX. `TextCrossEncoder` lists MiniLM-L6, both BGE rerankers, jina tiny, jina v2 multilingual, and Qwen3 rerankers at 0.6B/4B/8B. `MultiVectorTextEmbedding` lists `bge-m3`, `colbertv2.0`, `answerai-colbert-small-v1`, and `jina-colbert-v2`. `SparseTextEmbedding` lists a SPLADE checkpoint | `candle-core` / `candle-nn` | **Dev oracle only.** `candle` is `dev_oracle_only` [D]. Not a shipped dependency |
| `maxsim-cpu` [C30] | The MaxSim kernel, Rust, called from Python | No neural runtime | The kernel idea is compatible with an in-tree dot product. The Python package is not a Voice dependency |

A native port is the path the stack plan already uses for the small embedder: a BERT-shaped network lowered onto `galion-tensor` `gemv_i8`, weights in a `.gq` table, no `ort` and no `candle` in the workspace [D]. The cross-encoder to port, if the recall gate fails, is TinyBERT-L2 first and MiniLM-L6 if that gate still fails. Both are Apache-2.0 and both are plain BERT. On the official MS MARCO table L6 matches L12 (39.01 vs 39.02 MRR@10) and beats MiniLM-L2 (34.85) [C3]. On the PALDYN SciFact bench, TinyBERT-L2 and MiniLM-L6 were not distinguishable from each other, and both beat the bi-encoder [C15]. The port starts at the model that has a published CPU time near the live-call budget. Jina tiny is a similar parameter count with a longer context, and it is a custom architecture behind remote code [C7], so it is a worse first port. `bge-reranker-base` is MIT and is the wrong size: 278M, and 567 ms/query on four threads in that bench [C15].

An out-of-process ONNX sidecar would be a new owner exception. The research brief's open decision O1 is the same question for other ONNX models, and the Piper pin says not to widen it [D]. This note does not recommend opening that exception for a reranker. A reranker that needs `ort` can wait until a `.gq` port exists, or it can stay a dev-oracle measurement.

---

## 7. Adopt / reject

| Option | Verdict | Why |
|---|---|---|
| TinyBERT-L2 or MiniLM-L6 cross-encoder, as a `.gq` int8 port, run on the context worker over the top 5–8 hits, and only when the recall gate fails | **The CPU candidate** | Apache-2.0, plain BERT, the only published CPU band that is even near a live-call budget. Fit \(T\) on the int8 logits. Not on the ≤5 ms partial path [D] |
| First-stage hybrid only, no neural reranker, abstain when the store has no hit | **Default until the gate fails** | The plan's static embedder plus FTS5 is already the cheap paraphrase-plus-lexical pair [D]. A reranker is the fallback, not the first design |
| `answerai-colbert-small-v1` late interaction, document tokens precomputed | **Evaluate beside the cross-encoder, dev oracle** | 33M, Apache-2.0, and MaxSim over a small personal store is the cheap part [C11][C30]. Query encode still costs a forward pass. Length-normalise, then Platt-scale. Do not ship candle or ort to get there |
| ColBERTv2 + PLAID as a first-stage index | **Reject for this store** | Built for millions to hundreds of millions of passages [C9][C10]. At 10k–100k short facts the exact int8 scan in the plan is the simpler index [D]. The 110M encoder is the wrong size |
| monoT5-base | **Research reference for the score, not the model** | The two-token softmax is the cleanest built-in probability [C5]. The model is large, CPU latency is unpublished, and the castorini card states no licence [C16] |
| RankT5 listwise | **Reject for abstention** | The loss optimises order [C21] |
| `bge-reranker-base`, `bge-reranker-v2-m3`, ModernBERT rerankers, Qwen3 rerankers | **Reject on CPU during a call** | Hundreds of milliseconds to seconds per query on the published CPU benches [C15][C17][C29]. Qwen is also excluded by the 2026-11-04 owner ruling [D] |
| Jina v2 multilingual, NAVER SPLADE weights | **Reject** | Non-commercial licences [C14][C16][C17] |
| `fastembed` / in-tree `ort` / in-tree `candle` | **Reject for shipped Voice** | Forbidden runtimes [D]. Candle may back a dev-oracle measurement. A new sidecar pin is an owner decision, and this lane does not ask for one |
| A fixed 0.5 cutoff on a sigmoid | **Reject** | MiniLM logits saturate [C3][C4]. CRAG's own thresholds differ by dataset [C25]. The cutoff comes from the abstain curve on the 200-question set |

---

## 8. Sources

1. Nogueira and Cho, *Passage Re-ranking with BERT*, arXiv:1901.04085 (2019). https://arxiv.org/abs/1901.04085
2. Wang et al., *MiniLM: Deep Self-Attention Distillation for Task-Agnostic Compression of Pre-Trained Transformers*, arXiv:2002.10957. https://arxiv.org/html/2002.10957v2
3. `cross-encoder/ms-marco-MiniLM-L6-v2` model card (22.7M, the MS MARCO table, the raw-score example). https://huggingface.co/cross-encoder/ms-marco-MiniLM-L6-v2
4. sentence-transformers cross-encoder pretrained-model docs (sigmoid note, raw range about −10 to 10, the same table). https://github.com/huggingface/sentence-transformers/blob/main/docs/cross_encoder/pretrained_models.md
5. Nogueira, Jiang, Pradeep, and Lin, *Document Ranking with a Pretrained Sequence-to-Sequence Model*, Findings of EMNLP 2020, arXiv:2003.06713. https://arxiv.org/abs/2003.06713
6. `BAAI/bge-reranker-base` README (`license: mit`, commercial use, unbounded cross-entropy score) and `config.json` (XLM-RoBERTa, 12 layers, hidden 768, vocab 250,002). https://huggingface.co/BAAI/bge-reranker-base
7. `jinaai/jina-reranker-v1-tiny-en` model card (Apache-2.0, 33.0M, 4×384, 8,192 tokens, BEIR table). https://huggingface.co/jinaai/jina-reranker-v1-tiny-en
8. Khattab and Zaharia, *ColBERT: Efficient and Effective Passage Search via Contextualized Late Interaction over BERT*, SIGIR 2020, arXiv:2004.12832. https://arxiv.org/abs/2004.12832
9. Santhanam, Khattab, Saad-Falcon, Potts, and Zaharia, *ColBERTv2: Effective and Efficient Retrieval via Lightweight Late Interaction*, NAACL 2022. https://aclanthology.org/2022.naacl-main.272.pdf
10. Santhanam et al., *PLAID: An Efficient Engine for Late Interaction Retrieval*, CIKM 2022. https://arxiv.org/html/2205.09707
11. Clavié, *Small but Mighty: Introducing answerai-colbert-small*, Answer.AI, 13 Aug 2024, and the model card (Apache-2.0, 33.4M). https://www.answer.ai/posts/2024-08-13-small-but-mighty-colbert.html · https://huggingface.co/answerdotai/answerai-colbert-small-v1
12. Formal, Piwowarski, and Clinchant, *SPLADE: Sparse Lexical and Expansion Model for First Stage Ranking*, SIGIR 2021, arXiv:2107.05720. https://arxiv.org/abs/2107.05720
13. Lassance and Clinchant, *An Efficiency Study for SPLADE Models*, SIGIR 2022. https://arxiv.org/html/2207.03834
14. NAVER SPLADE licence, CC BY-NC-SA 4.0. https://github.com/naver/splade/blob/main/LICENSE
15. PALDYN AI Lab, CPU reranker bench, measured 2026-10-16 on a 4-vCPU Xeon at 2.10 GHz. https://ailab.paldyn.com/articles/bench-reranker-models-cpu
16. Lasagna model-licence table (Apache-2.0 for the MS MARCO MiniLM and TinyBERT cards, MIT for `bge-reranker-base` and `colbertv2.0`, Apache-2.0 for `bge-reranker-v2-m3` and `mxbai-rerank-large-v1`, CC-BY-NC-SA for the NAVER SPLADE checkpoint, CC-BY-NC-4.0 for jina v2 multilingual, no licence stated for `castorini/monot5-base-msmarco-10k`). Third-party compilation; the model card is the authority. https://dolev31.github.io/lasagna/docs/model-licenses/
17. rahulnitb/bge-reranker-api README (i5-8250U ONNX int8 latencies, int8 Spearman sanity check, jina v2 non-commercial). https://github.com/rahulnitb/bge-reranker-api
18. `BAAI/bge-reranker-base` `config.json`. https://huggingface.co/BAAI/bge-reranker-base/raw/main/config.json
19. `jinaai/jina-reranker-v1-tiny-en` ONNX tree (`model_int8.onnx`, 33.4 MB). https://huggingface.co/jinaai/jina-reranker-v1-tiny-en/tree/main/onnx
20. `fastembed` 7.x README and docs (`TextRerank`, SPLADE, BGE-M3 ColBERT vectors, `ort` 2.0.0-rc.15, candle behind features). https://github.com/Anush008/fastembed-rs · https://docs.rs/fastembed/latest/fastembed/
21. Zhuang et al., *RankT5: Fine-Tuning T5 for Text Ranking with Ranking Losses*, arXiv:2210.10634 (2022). https://arxiv.org/abs/2210.10634
22. stanford-futuredata/ColBERT, MIT licence. https://github.com/stanford-futuredata/ColBERT/
23. Guo, Pleiss, Sun, and Weinberger, *On Calibration of Modern Neural Networks*, ICML 2017. https://proceedings.mlr.press/v70/guo17a.html
24. Si, *Re-Examining Calibration: The Case of Question Answering*, Findings of EMNLP 2022. https://aclanthology.org/2022.findings-emnlp.204.pdf
25. Yan, Gu, Zhu, and Ling, *Corrective Retrieval Augmented Generation*, arXiv:2401.15884. Thresholds in Appendix B.3 of the HTML version. https://arxiv.org/html/2401.15884
26. DEV Community, *Rerank your RAG results with a cross-encoder*, CPU timings for MiniLM-L6. The CPU sku is not stated. https://dev.to/m_t_ramkrushna/rerank-your-rag-results-with-a-cross-encoder-sentence-transformers-crossencoder-with-real-cpu-3i7c
27. *The Reranking Tax*, clawRxiv preprint 2604.01082. Single-CPU pair timings. Sku unstated; not a venue paper. https://clawrxiv.io/abs/2604.01082
28. AnswerDotAI/RAGatouille issue 190 (user CPU timings; maintainer on parameter count and precomputation). https://github.com/AnswerDotAI/RAGatouille/issues/190
29. zaidwhy/tiebreak README (ONNX fp32 vs int8, ms/pair, logit drift). https://github.com/zaidwhy/tiebreak
30. Mixedbread, *maxsim-cpu: Maximising Maxsim Efficiency* (Rust MaxSim kernel timings). https://www.mixedbread.com/blog/maxsim-cpu
31. ONNX Runtime, *Quantize ONNX models* (dynamic int8 recommended for Transformers). https://onnxruntime.ai/docs/performance/model-optimizations/quantization.html
32. `candding` 0.1.0 crate page (candle cross-encoders, late interaction, sparse, no ONNX). https://crates.io/crates/candding

**Not verified on the Galion box.** No model was downloaded. No latency, MRR, nDCG, or expected calibration error was measured here. The 200-question personal set does not exist yet, so no \(T\) was fit. Licence rows that cite the Lasagna table [C16] were not re-read from every model card; the cards cited directly are [C3], [C6], [C7], and the SPLADE licence file [C14]. `bge-reranker-v2-m3`'s Apache-2.0 line and `colbertv2.0`'s MIT line are from that table only.

— Maciej Grajczyk
