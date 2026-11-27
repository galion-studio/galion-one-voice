# (41) Ranker research — human brain

Perpetual Open MIT License. (c) 2026 Galion Labs.

Research only. This file does not change a crate, a lock, a persona, or a config. No ranker was timed on this machine. A number below is either copied from a named source or marked as an estimate. Where the figure was not published, the cell says **I don't know.**

| Field | Value |
|---|---|
| Lane | How a brain picks one action, and the CPU-cheap ranker that copies the useful part |
| Question | Is there a better pick than XiaoIce's boosted tree, given work since 2018, that still abstains honestly? |
| Live budget | Owner rule, 2026-11-27: 13 ms per query is too slow. The ranker on the live turn must be well under 5 ms on a laptop CPU. |
| Miss line | Exactly `I don't know.` (`UNKNOWN_REPLY` in `horbm/crates/horbm/src/route.rs`) |
| Read | `main` at `c1264305` (2026-11-27). Code cited below was read in this tree. |

## The decision

Keep a margin test on the handful of sourced candidates HORBM already scored. Speak the winner only when it covers the question and leads the next candidate. Otherwise say `I don't know.` Leave the source id on the winner.

That is the part of the brain story that is both well measured and cheap enough. A boosted tree, a drift-diffusion simulator, and an iterative Hopfield loop are the wrong tools for this turn. The reasons are below, with the cost of each.

## What the live turn already does

A turn is a lookup. `LOCKS.toml` `[horbm]` sets `hot_path = "retrieval"`, `student_on_tick = false`, and `miss = "I don't know."`

Candidates are capped at four:

- `MEMORY_HIT_CAP` is 4 (`horbm/crates/horbm/src/retrieve.rs`).
- `SCAN_CAP` is 4 and `HIT_CAP` is 4 (`horbm/crates/horbm-brain/src/scan.rs`, `pick.rs`).

Two gates already refuse a guess:

- `covers` keeps a hit only when every content token of the ask is in the stored text (`retrieve.rs`).
- `pick` speaks a miss when the top two hits share a score and a word count (`horbm-brain/src/pick.rs`). The spoken miss is the stored persona line.

Two paths still speak on a weak lead:

- On the card path, `pick` breaks an equal score by preferring the shorter clause. Equal evidence with different lengths becomes a spoken answer.
- On the retrieval path, `Brain::hit_spoke` speaks `hits.first()` (`horbm/crates/horbm/src/brain.rs`). The scan order is the order. There is no lead test.

Layer 4 then picks a move with fixed integers (stop 100, cite 80, answer 60, offer topic 55, comfort 50, ask 10) and a mask (`horbm/crates/horbm/src/dialogue/policy.rs` `choose_held`). Answer and cite need a truth source. A research hit is never an answer. This layer does not choose among clause texts.

The unmerged R2 note is not on `main`. `docs/horbm/foundations/F2-retrieval-audit.md` records its intent: the pick replaces XiaoIce's GRU and boosted tree, and it runs beside the heartbeat.

## Budget, and the only timing this repo already published

The ranker must finish well under 5 ms. A design at 13 ms is out. "Well under" here means the estimate sits with the published scan, in tens of microseconds, and the ranker does not walk the pack again.

`docs/horbm/foundations/S54-diary-scan.md` timed `DiaryScan` once. Ignored test `timed_scan_of_one_thousand_rows`, release, minimum of 100 samples after 10 warmup queries. One query, top-k 4, 1,000 key rows of width 32 and 1,000 diary rows of width 256. Host: Linux, Xeon with AVX-512, rustc 1.85.0. This note did not re-run it.

| Build | Minimum | High median (`samples[50]`) |
|---|---|---|
| scalar | 58.425 µs | 58.740 µs |
| AVX-512 | 23.150 µs | 24.796 µs |

That is a scan of two thousand rows, not a ranker, and it is a minimum, not a 99th percentile. It is the scale the ranker has to beat by staying smaller than the scan.

`F2-retrieval-audit.md` also records release numbers from `horbm-memory` PR #16, not re-run in that note and not re-run here: selective p95 6.0 ms and broad p95 8.3 ms at 200,000 cards with a background writer. Those figures are retrieval over a large store. They are already over a 5 ms ranker budget. The live pack cited in F2 is 1,506 cards, not 200,000. I don't know the p99 of the 1,506-card path on this laptop.

R1's 5 ms p95, cited in F2, is marked there as a target. It is not a measurement.

## How a brain picks one action

Plain account. Each paragraph says what was measured, then what HORBM may copy. The cost line is an estimate from operation count, set next to the S54 scan. I did not time these designs.

### Basal ganglia: one channel, the rest held down

Redgrave, Prescott, and Gurney (1999) describe the basal ganglia as a central switch. Several systems want the same muscles or the same cognitive slot. One channel is released. The others stay suppressed. They also say selection happens in other places too. The basal ganglia are the switch for conflicts among systems that sit far apart.

Gurney, Prescott, and Redgrave (2001) turn that into a model. Each action has one number, the salience. Selection is an off-centre, on-surround: the chosen channel is boosted and the neighbours are pushed down. Dopamine changes how sharp the selection is. Part II reports that the model's outputs line up with pallidal recordings and with some animal selection behaviour under different dopamine levels. That is a model fit, not a proof that those equations run in the brain.

Bogacz and Gurney (2007) show that the same cortex–basal-ganglia loop has the shape of an optimal test for many alternatives (the multihypothesis sequential probability ratio test). Cortex accumulates evidence. The basal ganglia release an action when one option's probability crosses a threshold. They report that this test beats a simple race and the Usher–McClelland accumulator in their simulations, especially as the number of alternatives grows. The predictions are checked against existing recordings. The paper is a theory that matches anatomy and some data.

**HORBM.** The four candidates are the channels. Salience is the score the scan or the cover count already produced. Release the winner only when its score exceeds the runner-up. Leave every other clause unspoken. Do not splice two clauses into one sentence. Do not simulate striatum, subthalamic nucleus, or pallidum.

**Cost.** Four integer comparisons and one subtraction, on scores already in hand. No extra pass over the cards. Estimated cost is a small fraction of the S54 scan (23–58 µs minimum on 2,000 rows). **Fits well under 5 ms.** I don't know the measured nanoseconds.

**Status.** A central switch that matches basal-ganglia anatomy is the 1999 proposal, argued from the circuitry. The 2001 equations and the 2007 optimality proof are models checked against recordings and simulations. Using clause ids as channels is a design.

### Dopamine: a teaching signal, read later

Schultz, Dayan, and Montague (1997) review primate recordings in which midbrain dopamine neurons fire a short burst when an outcome is better than predicted, stay quiet when it matches the prediction, and pause when an expected reward is missing. The pattern matches the temporal-difference error used in reinforcement learning. The abstract also says the events are salient as well as rewarding.

Berke (2018) reviews what came after. The burst-as-error account of those spikes is supported, including later optogenetic work that those cells can teach. Dopamine release also tracks motivational value and movement on a sub-second scale. The tidy split "fast dopamine teaches, slow dopamine motivates" does not hold. A single scalar called dopamine is several jobs.

**HORBM.** The live turn does not learn (`student_on_tick = false`, learn consent deny). A prior of −1, 0, or +1 per clause id may be added to salience only if an offline pass wrote it, and only when learn is granted. The tick reads the table. The only honest outcome today is an explicit correction (`TurnIntent::Correct` in `dialogue/intent.rs`). Silence is not a reward. There is no +1 until a confirm exists. The prior is clamped so it cannot create a cover `covers` rejected and cannot overturn a real lead.

**Cost.** One table lookup and one add per candidate, at most four adds. **Fits well under 5 ms.** Training the prior is offline and is outside the live budget. I don't know a measured lookup time. The table does not exist yet.

**Status.** Phasic dopamine as a reward-prediction error in conditioning is one of the better replicated findings in systems neuroscience, with the limits Berke states. Using it as a clause prior in a voice ranker is speculative. Shipping it before a confirm signal exists would invent rewards.

### Prefrontal cortex: the goal biases the switch

Miller and Cohen (2001) argue that prefrontal cortex keeps the current goal active and sends a bias into the structures that actually compete. It does not replace those structures.

Padoa-Schioppa and Assad (2006) recorded orbitofrontal neurons in monkeys choosing between juice amounts. Those cells tracked the subjective value of the goods on offer and of the good that was chosen, separate from where the juice was placed and from the arm movement.

**HORBM.** This is layer 4, already written. `choose_held` holds the goal (stop, cite, answer, comfort, offer, ask) and `filter::mask` shuts illegal moves. The ranker proposes a clause. The mask decides whether that clause may be spoken as an answer. The integers in `policy.rs` are hand-set priorities. They are not the economic values Padoa-Schioppa measured.

**Cost.** Six integer comparisons, already on the turn. **Fits well under 5 ms.** Adding a value network would be a new model. I don't know its CPU time, and it is not needed.

**Status.** Goal bias and offer-value coding are measured. Calling the dialogue integers "OFC values" is speculative.

### Evidence, a threshold, and when to stop

Ratcliff and McKoon (2008) review the diffusion decision model. Noisy evidence drifts until it hits a bound. Accuracy, mean reaction time, and the shape of the reaction-time distribution come from the drift, the bound, and the start point. Telling a person to be fast moves the bound. Telling them to be accurate moves it the other way. The model is for two-choice lab tasks. It is fitted to those tasks across many papers in that review. I don't know a CPU time for the fit. The fit is offline.

Gold and Shadlen (2007) review the physiology. In the random-dot motion task, neurons in parietal cortex ramp up as evidence arrives and the animal commits around a threshold.

Bogacz and Gurney (2007), above, are the version for more than two options: stop when one posterior crosses a bound.

**HORBM.** The samples are the content tokens of the utterance, not spikes and not Gaussian noise. A candidate's evidence is how many of those tokens it contains. The commit bound stays the existing cover rule: every content token. A lower bound would speak a partial overlap. The second bound is the lead over the runner-up. End of turn with either bound unmet is `I don't know.`

Ratcliff and McKoon show that people drop the bound when speed is rewarded. HORBM's miss line is locked. The bound stays put when the user pauses. Copying the speed–accuracy collapse would manufacture answers.

A streaming form, if a partial transcript is already in hand, is four integer counters, incremented when a new token hits. Commit early only at a full cover plus a lead. I don't know whether the live host already passes partials into `Brain::turn`. The finished-line form is the one that matches today's call.

**Cost.** One increment per content token per candidate. A short utterance and four candidates is a few dozen adds, or one pass over four scores if the line is already finished. **Fits well under 5 ms.** A stochastic simulator (Euler steps, random draws, a 10 s cap) is a different program and stays off the turn. See the crate list.

**Status.** Bounded accumulation for two-choice perception is measured (behaviour and parietal ramps). A token counter over stored clauses is a design that uses the same shape. It is not a claim that parietal cortex stores galion-knowledge cards.

### Confidence: knowing you don't know

Kepecs, Uchida, Zariwala, and Mainen (2008) trained rats on an odour categorisation. Orbitofrontal firing matched a confidence quantity computed from the quality of the evidence, and rats waited longer for reward when that confidence was higher. They argue the computation is simple.

Kiani and Shadlen (2009) gave monkeys the chance to opt out of a motion judgment for a small sure reward. The same parietal neurons that formed the decision also tracked the certainty of it. The animals opted out when certainty was low.

Pouget, Drugowitsch, and Kepecs (2016) separate two words. Confidence is the probability that the decision is correct given the evidence. Certainty is the spread of a belief about the world. A high score with a near-tie is low confidence. The winning score alone is the wrong readout.

**HORBM.** Confidence is the lead (top score minus second score), stored on the result and not spoken. A unique argmax with a lead of zero is a miss, which `pick` already does when the word counts also match. Extend that refusal to a small lead, and stop using the shorter clause as a tie break. Two full covers at the same score are a conflict. The spoken line is still `I don't know.`

This number is not a calibrated probability. Pouget's definition needs a real P(correct). HORBM has no labelled set on which to measure that. Until `horbm-eval` scores the lead against known misses, the lead is an uncalibrated signal. Calling it calibrated would be a false claim.

**Cost.** One subtraction of two scores already computed. **Fits well under 5 ms.**

**Status.** Confidence signals that track accuracy, and opt-out when they are low, are measured in animals. A lead threshold on retrieval scores is the cheap analogue. Its calibration on HORBM cards is unknown. I don't know the false-commit rate.

### Tip of the tongue

Brown and McNeill (1966) put people in the state where a word feels available and will not come. Before recall, people could report some letters, the number of syllables, and the stress pattern. They called this generic recall. The closer the person was to getting the word, the more accurate those fragments were. The fragments were still not the word.

Maril, Wagner, and Schacter (2001) measured that state with fMRI and treated it as a failed semantic retrieval together with cognitive conflict.

**HORBM.** A third outcome, beside commit and abstain. Partial: some stored row shares tokens or a positive scan score, and no row covers the question. The spoken line remains `I don't know.` The near-miss id is metadata for the existing research mark (`SidebandMark` in `pick.rs`). It is not a sentence. Brown and McNeill's subjects often guessed the first letter correctly. Those guesses were still guesses. HORBM does not speak a fragment, a first word, or a blended clause.

**Cost.** A flag set while the four hits are already being scanned for the cover. No second search. **Fits well under 5 ms.**

**Status.** The tip-of-the-tongue state and partial access are measured in people. Using the near-miss id as a side-band hint is a design. Speaking from it would break the miss lock.

### Predictive coding

Rao and Ballard (1999) built a model of visual cortex in which higher areas send a prediction downward and lower areas send back the error. Trained on natural images, the model developed simple-cell-like fields, and the error units showed surround suppression. That is a model of one visual phenomenon.

Walsh, McGovern, Clark, and O'Connell (2020) review the later physiology. Expectation-related changes show up in many studies and fail to replicate in a substantial set of others. Coarse EEG, MEG, and fMRI have trouble separating a prediction from a prediction error. They conclude that support for each of the strong claims is incomplete. A full hierarchical predictive-coding cortex is not an established fact.

**HORBM.** One integer, not a hierarchy. When the dialogue hold flag is set, a candidate that shares no topic token with the held topic loses a fixed penalty of 1, in the same units as the cover count, and still cannot be spoken unless it covers. When the utterance is a greeting or a topic switch (`dialogue/cues.rs`), the penalty is zero, because the context is a weak prediction. No free-energy iteration. No generated sentence.

**Cost.** One topic-token check per candidate, at most four. **Fits well under 5 ms.**

**Status.** A predictive-coding account of some visual surround effects is a published model (Rao and Ballard). The general theory is unsettled (Walsh and colleagues). A one-point topic penalty is a design. It should not ship until the hold flag's effect is tested, because a wrong penalty abstains on a true topic change.

### Modern Hopfield networks (2020): memory as one lookup

Krotov and Hopfield (2016) state that the classical Hopfield network stores on the order of 0.14 patterns per neuron before memories smear, and they raise that with a higher-order energy so the store can hold more patterns than there are units. Demircigil, Heusel, Löwe, Upgang, and Vermet (2017) prove the exponential-interaction case: capacity grows exponentially with the number of neurons, and the basins stay large. Those are theorems about the model.

Ramsauer and colleagues (preprint 2020, ICLR 2021) give a continuous version. One update retrieves a stored pattern. Storage grows exponentially with the dimension of the pattern. The update is the same maths as transformer attention. The energy has three kinds of fixed point: an average of every pattern, an average of a subset (a metastable state), and a single stored pattern.

**HORBM.** The int8 dot in `DiaryScan` is already the similarity inside that update. At a high inverse temperature the softmax winner is the maximum dot, which the scan already returns, fused by reciprocal rank (`SCAN_RRF_K` 60). The ranker uses that order. It does not form a weighted sum of clause strings. Ramsauer's averages are exactly a blended sentence, which would be a fabricated answer.

A flat top (the winner would hold less than half the mass of a softmax over the four hits) is the signature of those averages. The integer lead test is that check without an exponential. Four `exp` calls would also fit the budget. They buy nothing over the subtraction.

**Cost.** Zero extra dots if the scan has run. One subtraction. **Fits well under 5 ms.**

An iterative energy descent over the whole pack does not fit the spirit of the budget. The `hopfield` crate's own example runs up to 100 iterations. I don't know that crate's wall time. Scaling the S54 minimum by 100 full passes is roughly 2–6 ms (100 × 23 µs, 100 × 58 µs). That scaling pretends each iteration costs one S54 scan. It is not a measurement. It is close enough to 5 ms, and far from "well under", that the loop stays off the turn. A 13 ms query is over the line on its face.

**Status.** The capacity and convergence results are proved for the mathematical model. The match to transformer attention is a derivation. That hippocampal pattern completion uses this 2020 update is not shown. Pattern completion in the hippocampus is a separate, older finding. This note does not rest the ranker on it.

## One function, after the scores exist

The mechanisms above are one pure function, not eight systems. It runs after `covers` and after the scan, on at most four hits, and writes into a slot the caller owns. No heap, no `exp`, no random number, no second embedding.

1. Drop uncovered hits from the commit race. Keep them only as the partial-state witness.
2. Optionally add the offline prior (±1) and the topic penalty (−1). Neither can create a cover.
3. Sort by that integer, then by id. Word count is not a key. Brevity stays where it is, in `StyleHook::cap_sentences`, after a unique winner exists.
4. No covered hit: partial if any witness shared a token or a positive score, otherwise abstain. Both speak `I don't know.`
5. Lead of the top over the second is zero, or below a floor of 1 in cover-count units: abstain. Two full covers at the same score are a conflict, not a tip-of-the-tongue.
6. Otherwise commit that hit. The source string stays `card:<id>`, `retrieve:<id>`, or `learned:<id>`, as `hit_spoke` already builds.

The floor of 1 means "any tie abstains", which is today's `pick` rule without the shorter-clause escape. Raising the floor later is a calibration change and needs labels. This note does not pick a fitted threshold.

## Options

Licence of a paper is the publisher's. The papers below are ideas. No code was copied from them.

| Option | Year | Source | Licence | What it does |
|---|---|---|---|---|
| Margin on the existing four hits | — | This tree: `pick`, `covers`, `DiaryScan` | MIT (this repo) | Speak one sourced clause when it covers and leads. Otherwise `I don't know.` |
| XiaoIce response ranker | 2020 (journal). Features and the tree are earlier. | Zhou, Gao, Li, Shum, Computational Linguistics 46(1). https://aclanthology.org/2020.cl-1.2.pdf | Publisher copyright. The system is Microsoft's. | A boosted tree scores candidates on cohesion, coherence, empathy, and retrieval match, then samples one candidate above a threshold. |
| LambdaMART, the tree XiaoIce cites | 2010 | Wu, Burges, Svore, Gao. https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/LambdaMART_Final.pdf | Publisher copyright. Algorithm only. | Boosted regression trees trained to a ranking measure. |
| Basal-ganglia channel selection | 1999, 2001, 2007 | Redgrave et al.; Gurney et al.; Bogacz and Gurney. Links in Sources. | Publisher copyright | Pick the salient channel and hold the others down, when one probability clears a bound. |
| Diffusion decision bound | 2008 review; physiology 2007 | Ratcliff and McKoon; Gold and Shadlen | Publisher copyright | Accumulate evidence until a bound. Speed instructions move the bound. |
| Confidence opt-out | 2008, 2009, 2016 | Kepecs et al.; Kiani and Shadlen; Pouget et al. | Publisher copyright | A second signal, from the balance of evidence, withholds the action. |
| Tip-of-the-tongue | 1966, 2001 | Brown and McNeill; Maril, Wagner, Schacter | Publisher copyright | Partial features of a missing word, without the word. |
| Predictive coding | 1999 model; 2020 review | Rao and Ballard; Walsh et al. | Publisher copyright | Score by prediction error, weighted by how reliable the prediction is. |
| Modern Hopfield lookup | 2016, 2017, 2020/2021 | Krotov and Hopfield; Demircigil et al.; Ramsauer et al. | ICLR paper and arXiv. Theorems in a journal. | One update retrieves a stored pattern. Averages of patterns are other fixed points. |
| `hopfield` crate | crate 0.3.0, repo created 2026 | https://github.com/arclabs561/hopfield | MIT (`LICENSE-MIT`, copyright 2026 Arc). README also says MIT OR Apache-2.0. | Energy descent or one-step retrieval over stored vectors. The README example uses up to 100 iterations. |
| `sinistra-core` | 0.1.0, 2026 | https://github.com/snehasish01/sinistra | MIT, as recorded on crates.io and the GitHub repo | Simulates and fits a two-boundary drift-diffusion model for reaction-time experiments. |
| LightGBM and `lightgbm3` | library current; crate 1.0.11 | https://github.com/lightgbm-org/LightGBM and https://crates.io/crates/lightgbm3 | Both MIT | Gradient-boosted trees, including ranking. `lightgbm3` is a Rust binding over the C++ library. |
| `hessboost` | repo created 2026 | https://github.com/brndnmtthws/hessboost | Apache-2.0, copyright 2026 Brenden Matthews. Fork of sequoia-boost, copyright 2026 Patrick Garrett, Apache-2.0. | Rust gradient boosting. The README lists LambdaMART ranking. |
| `dilawar/moden-hopfield-network` | 2024 | https://github.com/dilawar/moden-hopfield-network | LGPL-2.1 | A modern Hopfield network in Rust. The licence is not MIT or Apache. Left unused. |
| `HopfieldAssociativeMemory` | 2026 | https://github.com/adamgracikowski/HopfieldAssociativeMemory | MIT | A course project for the 1982 network (Hebb and Oja). Not the 2020 update. Left unused. |

## Numbers that were actually published

| Claim | Figure | Source | Seen here? |
|---|---|---|---|
| Diary scan, 2,000 rows, top-k 4, minimum | 23.150 µs AVX-512, 58.425 µs scalar | S54, ignored test, release, rustc 1.85.0, Xeon | Read the note. Not re-run. |
| Same test, high median | 24.796 µs AVX-512, 58.740 µs scalar | S54 | Read the note. Not re-run. |
| `horbm-memory` at 200,000 cards | selective p95 6.0 ms, broad p95 8.3 ms | F2, quoting PR #16, not re-run there | Read F2. Not re-run. |
| Ranker wall time on this laptop | I don't know. | — | Not measured. |
| XiaoIce ranker CPU latency | I don't know. The paper does not give it. | Zhou et al. 2020 | Read the paper. |
| XiaoIce ranker accuracy on a named IR benchmark | I don't know. The response ranker is trained on 50K manually labelled dialogues and judged by conversation-turns per session, not by a public ranking set. | Zhou et al. 2020, General Chat pilot | Read the paper. |
| XiaoIce expected CPS, sixth generation, as of July 2018 | 23 | Zhou et al. 2020 | A product engagement number, not a ranker score. |
| Topic-manager A/B | +0.5 expected CPS for Core Chat over one month, topic classifier and topic ranker trained on 50K labelled sessions | Zhou et al. 2020 | Not a latency, not NDCG. |
| Persona generator on a TV-script set, from Li et al. 2016 as cited by Zhou | perplexity 25.0 versus 27.3 for seq2seq (−8.4%); BLEU 1.9% versus 1.6% | Zhou et al. 2020, Table 1 | Generator numbers, not the ranker. |
| Label scale for the XiaoIce ranker | 0, 1, 2 | Zhou et al. 2020 | Design of their training set. |
| `hopfield` crate model size, CPU latency, benchmark accuracy | I don't know. | README and docs.rs, retrieved 2026-11-27 | No benchmark table in the README. |
| `sinistra-core` ranker latency | I don't know. It is not a ranker. | crates.io README | The published example simulates 100,000 trials to fit parameters. |
| LightGBM or `hessboost` latency on four HORBM hits | I don't know. | — | Not measured. |
| False-commit rate of a lead test on HORBM cards | I don't know. | — | No labelled set was scored. |

Zhou and colleagues also describe the miss path in words: when nothing clears the threshold they pick an editorial line, and they give examples that keep the chat going ("Hmmm, difficult to say. What do you think?", "Let us talk about something else."). HORBM's locked miss is the sentence `I don't know.`

## Crates, and why the live turn does not gain one

Searched crates.io and GitHub on 2026-11-27 for MIT or Apache Rust code for Hopfield memory, drift-diffusion, and boosted ranking.

The job on the live turn is a comparison of four integers. No crate does that job in less code than the comparison. Adding one would spend dependency weight on a simulator or a learner.

- **`hopfield` 0.3.0** (Arc, MIT). Rust 1.75 default, 1.82 if the `fynch` feature is on. Both are below the repo's 1.85 pin. `retrieve_dot_product` is Ramsauer's one update. The README example of `retrieve_memory` allows 100 gradient steps. Use the paper, not the crate, on the turn: `DiaryScan` has already computed the dots. The crate is the reference if a later offline experiment wants energy descent. It has no published CPU number. I don't know its wall time.
- **`sinistra-core` 0.1.0** (Snehasish Satpathy, MIT). Euler–Maruyama simulation, `dt` of 1 ms, a 10 s cap, EZ-diffusion fitting (Wagenmakers, van der Maas, and Grasman, 2007, as cited by the crate). Dependencies include `argmin`, `rand`, and `rayon`. It recovers drift and bound from choice and reaction time. HORBM has neither as training data. One fitted bound is an offline lab tool. The live rule is the integer threshold, which does not need this crate.
- **LightGBM** (MIT, Microsoft and the LightGBM developers) and **`lightgbm3`** (MIT, Dmitry Mottl). This is the maintained way to run a XiaoIce-style tree. It needs a C++ build and a labelled ranking set. HORBM does not have that set. The expensive half of XiaoIce is the DSSM features in front of the tree. I don't know the CPU time of those models. A design that adds them is over 5 ms until someone measures it under the budget on the laptop gate.
- **`hessboost`** (Apache-2.0, Brenden Matthews). Native Rust, LambdaMART in the README, conformal intervals as an extra. Four GitHub stars on the day of the search. Same blocker: labels. A tree that cannot be trained is dead weight on the turn. I don't know its inference latency.
- **LGPL Hopfield crate.** Not usable under the MIT/Apache rule. Not timed.
- **Course-project classical Hopfield.** MIT, and the wrong algorithm (1982, small capacity). Not timed.

`eam` (R, MIT) also simulates diffusion, race, and leaky accumulators. It is not Rust. Ideas only. It does not go on the turn.

## Fit for HORBM

Scores are judgments from the constraints, not measurements. **Good** means it meets the constraint with what the repo has today. **Weak** means it meets it only after new data or a new measurement. **Out** means it breaks a lock or the budget.

| Option | CPU latency | Data needed | Abstain | Source stays on the answer | Complexity |
|---|---|---|---|---|---|
| Margin on four hits | Good. Estimate: well under the 23–58 µs scan. | Good. No new labels. | Good. Tie and failed cover say `I don't know.` Uncalibrated until eval. | Good. Winner keeps its id. | Good. A sort of four integers. |
| Layer 4 mask (prefrontal) | Good. Already running. | Good. | Good for illegal moves. It does not rank clauses. | Good. | Good. Already in tree. |
| Offline ±1 prior (dopamine) | Good, if the read is one add. | Weak. Needs an explicit confirm or correction. Silence is not a label. | Weak. A prior can hide a tie if it is not clamped. | Good, if the id is unchanged. | Good, offline only. |
| Topic penalty (predictive coding) | Good. | Good. Uses the hold flag. | Weak until tested. A bad penalty abstains on a real topic change. | Good. | Good. |
| Partial / tip-of-the-tongue metadata | Good. | Good. | Good, if speech stays the miss line. | Good. The id is metadata. | Good. |
| One-step Hopfield, dots already computed | Good. | Good. | Good, if a flat top abstains. | Good, if the winner is one stored row. | Good. |
| Iterative Hopfield over the pack | Out on the estimate (order of milliseconds × iterations, not measured). | Good. | Weak. Averages fabricate text if spoken. | Out if clauses are blended. | Out for the live turn. |
| `sinistra-core` on the turn | Out. Simulator, not a 4-row pick. I don't know one-trial ms. The fitting example is 100,000 trials. | Out. Needs choice and reaction time. | The bound idea is Good. The crate is the wrong place to get it. | Good only if unused. | Out. |
| XiaoIce tree + DSSM features | Out until measured under 5 ms. I don't know the real ms. A ranker at 13 ms is over the owner line. | Out. 50K labelled dialogues in their pilot. We have none. | Out. Their miss is editorial chit-chat, and they sample at random above the threshold. | Weak. Generated candidates have no card source. | Out. |
| LightGBM or `hessboost` | Weak. I don't know ms. A small tree on four rows may be fine. The features may not. | Out today. | Weak. A threshold can abstain. Calibration needs a held-out set. | Good only if every row is a stored card. | Out for now. |

## Recommendation

**Now.** Do not add a crate. Do not train a tree. The ranker is the margin test in "One function" above, applied in both places that currently speak too early: `pick` (stop breaking ties by length) and `hit_spoke` (stop taking `hits.first()` with no lead). Speech is the stored clause or exactly `I don't know.` Confidence is the lead, logged, not spoken, not called a probability. Estimated cost is a few integer operations after scores that already exist, set against a published scan minimum of 23–58 µs. That is well under 5 ms. A 13 ms ranker is rejected.

Leave dopamine, the topic penalty, and the partial-state hint off until the margin itself is in and a test shows the tie behaviour. Layer 4 stays the goal mask. It is not a second ranker.

**Later.** After a labelled set of (ask, candidate, accept or reject) exists, sweep the lead floor on a held-out split and publish the false-commit and false-abstain counts. That is the first time the word "calibrated" is allowed. An offline ±1 prior may then be read on the turn if corrections are explicit. A one-step Hopfield softmax over the four dots is optional and should lose to the integer lead unless it changes those counts. LightGBM or `hessboost` earn a look only if that labelled set is large enough to train a ranker and a laptop-gate measurement shows inference well under 5 ms with sources still attached. Iterative Hopfield, DSSM features, and a diffusion simulator stay off the turn.

## What would change my mind

- A labelled HORBM set where the margin rule's false commits, or its false abstains, are worse than a tree trained on that set, on a held-out split, with every accepted answer still a stored clause, and with the tree measured well under 5 ms on the laptop gate.
- A measurement, on that gate, showing the four-integer test itself near 5 ms. I do not expect this. I have not run it. If it happened, the estimate in this note is wrong.
- The owner wanting the shorter clause to break a tie. The brain account treats that as a guess. The current `pick` does it. This note recommends stopping.
- Retrieval returning hundreds of candidates inside the turn. The four-cap is what makes the ranker cheaper than the scan. If the cap rises, re-estimate. A second full scan, or 100 Hopfield iterations, is over budget on the S54 scaling above until a real timing says otherwise.
- A published CPU time for XiaoIce's ranker-plus-DSSM well under 5 ms on a laptop, plus a licence we can use, plus labels we have. None of those three are in hand. I don't know the CPU time.

## Proven, model, or speculative

| Piece | Standing |
|---|---|
| Basal ganglia as a central switch | Proposed from the anatomy (Redgrave, Prescott, and Gurney 1999). |
| Gurney 2001 equations, Bogacz and Gurney 2007 optimality | Models, checked against some recordings and against simulations. |
| Phasic dopamine as reward-prediction error | Measured in conditioning (Schultz, Dayan, Montague 1997), with the limits in Berke 2018. |
| Dopamine as an on-tick clause learner | Speculative, and forbidden by `student_on_tick = false`. |
| Prefrontal goal bias; OFC offer value | Measured (Miller and Cohen 2001; Padoa-Schioppa and Assad 2006). |
| Dialogue integers as those values | Speculative. They are hand-set. |
| Diffusion to a bound; parietal ramps; speed moves the bound | Measured for laboratory two-choice tasks (Ratcliff and McKoon 2008; Gold and Shadlen 2007). |
| Token counts as that diffusion | A design. |
| Dropping the bound to go faster | Measured in people. Refused here, because the miss line is locked. |
| Confidence from the balance of evidence; opt-out | Measured (Kepecs et al. 2008; Kiani and Shadlen 2009; Pouget et al. 2016). |
| The lead being a calibrated P(correct) on HORBM | Unknown. I don't know. |
| Tip-of-the-tongue partial knowledge | Measured (Brown and McNeill 1966; Maril, Wagner, Schacter 2001). |
| Speaking a fragment | Refused. The fragments in the 1966 study were often right and were still guesses. |
| Rao and Ballard 1999 surround-suppression model | A model that fits a visual effect. |
| Cortex as predictive coding in general | Unsettled (Walsh, McGovern, Clark, O'Connell 2020). |
| One-point topic penalty | A design, untested. |
| Modern Hopfield capacity, one-step retrieval, attention equivalence | Proved or derived for the model (Demircigil et al. 2017; Ramsauer et al. 2020/2021; Krotov and Hopfield 2016). |
| The brain running that update | Not shown. |
| Blended Hopfield averages as a reply | A fixed point of the model, and a fabricated sentence if spoken. |
| S54 scan minimum 23–58 µs | A measurement in that note. Not re-run here. Not a ranker time. |
| This ranker well under 5 ms | An estimate from operation count against that scan. Not a measurement. |

## Sources

Papers and notes. No Wikipedia.

- Berke, J. D. (2018). What does dopamine mean? *Nature Neuroscience*. https://doi.org/10.1038/s41593-018-0152-y
- Bogacz, R., & Gurney, K. (2007). The basal ganglia and cortex implement optimal decision making between alternative actions. *Neural Computation*, 19(2), 442–477. https://doi.org/10.1162/neco.2007.19.2.442
- Brown, R., & McNeill, D. (1966). The "tip of the tongue" phenomenon. *Journal of Verbal Learning and Verbal Behavior*, 5(4), 325–337. https://doi.org/10.1016/S0022-5371(66)80040-3
- Demircigil, M., Heusel, J., Löwe, M., Upgang, S., & Vermet, F. (2017). On a model of associative memory with huge storage capacity. *Journal of Statistical Physics*. https://doi.org/10.1007/s10955-017-1806-y
- Gold, J. I., & Shadlen, M. N. (2007). The neural basis of decision making. *Annual Review of Neuroscience*, 30, 535–574. Cited from the reference list of Pouget, Drugowitsch, and Kepecs (2016).
- Gurney, K., Prescott, T. J., & Redgrave, P. (2001). A computational model of action selection in the basal ganglia. I. A new functional anatomy. *Biological Cybernetics*, 84, 401–410. https://doi.org/10.1007/PL00007984
- Gurney, K., Prescott, T. J., & Redgrave, P. (2001). A computational model of action selection in the basal ganglia. II. Analysis and simulation of behaviour. *Biological Cybernetics*, 84, 411–423. https://doi.org/10.1007/PL00007985
- Kepecs, A., Uchida, N., Zariwala, H. A., & Mainen, Z. F. (2008). Neural correlates, computation and behavioural impact of decision confidence. *Nature*. https://www.nature.com/articles/nature07200
- Kiani, R., & Shadlen, M. N. (2009). Representation of confidence associated with a decision by neurons in the parietal cortex. *Science*. https://doi.org/10.1126/science.1169405
- Krotov, D., & Hopfield, J. J. (2016). Dense associative memory for pattern recognition. arXiv:1606.01164. https://arxiv.org/abs/1606.01164
- Maril, A., Wagner, A. D., & Schacter, D. L. (2001). On the tip of the tongue: An event-related fMRI study of semantic retrieval failure and cognitive conflict. *Neuron*, 31, 653–660. Listed by the Stanford Memory Lab: https://memorylab.stanford.edu/publications/2000-2009
- Miller, E. K., & Cohen, J. D. (2001). An integrative theory of prefrontal cortex function. *Annual Review of Neuroscience*, 24, 167–202. https://doi.org/10.1146/annurev.neuro.24.1.167
- Padoa-Schioppa, C., & Assad, J. A. (2006). Neurons in the orbitofrontal cortex encode economic value. *Nature*. https://www.nature.com/articles/nature04676
- Pouget, A., Drugowitsch, J., & Kepecs, A. (2016). Confidence and certainty: distinct probabilistic quantities for different goals. *Nature Neuroscience*, 19, 366–374. https://doi.org/10.1038/nn.4240
- Ramsauer, H., Schäfl, B., Lehner, J., Seidl, P., Widrich, M., Adler, T., Gruber, L., Holzleitner, M., Pavlović, M., Sandve, G. K., Greiff, V., Kreil, D., Kopp, M., Klambauer, G., Brandstetter, J., & Hochreiter, S. (2021). Hopfield networks is all you need. *International Conference on Learning Representations*. Preprint 2020, arXiv:2008.02217. https://arxiv.org/abs/2008.02217
- Rao, R. P. N., & Ballard, D. H. (1999). Predictive coding in the visual cortex: a functional interpretation of some extra-classical receptive-field effects. *Nature Neuroscience*. https://www.nature.com/articles/nn0199_79
- Ratcliff, R., & McKoon, G. (2008). The diffusion decision model: theory and data for two-choice decision tasks. *Neural Computation*, 20(4), 873–922. https://pubmed.ncbi.nlm.nih.gov/18085991/
- Redgrave, P., Prescott, T. J., & Gurney, K. (1999). The basal ganglia: a vertebrate solution to the selection problem? *Neuroscience*, 89(4), 1009–1023. https://pubmed.ncbi.nlm.nih.gov/10362291/
- Schultz, W., Dayan, P., & Montague, P. R. (1997). A neural substrate of prediction and reward. *Science*, 275(5306), 1593–1599. https://doi.org/10.1126/science.275.5306.1593
- Walsh, K. S., McGovern, D. P., Clark, A., & O'Connell, R. G. (2020). Evaluating the neurophysiological evidence for predictive processing as a model of perception. *Annals of the New York Academy of Sciences*, 1464(1), 242–268. https://doi.org/10.1111/nyas.14321
- Wu, Q., Burges, C. J. C., Svore, K. M., & Gao, J. (2010). Adapting boosting for information retrieval measures. *Information Retrieval*. https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/LambdaMART_Final.pdf
- Zhou, L., Gao, J., Li, D., & Shum, H.-Y. (2020). The design and implementation of XiaoIce, an empathetic social chatbot. *Computational Linguistics*, 46(1). https://aclanthology.org/2020.cl-1.2.pdf

Repo notes cited as measurements or targets, not re-run here:

- `docs/horbm/foundations/S54-diary-scan.md`
- `docs/horbm/foundations/F2-retrieval-audit.md`
- `LOCKS.toml` `[horbm]`
