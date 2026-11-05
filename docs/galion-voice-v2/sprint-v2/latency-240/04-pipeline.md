# 04 — First sentence & pipeline (latency-240)

Owner: agent 04. Branch `swarm/lat240-pipeline` (worktree `wt-04-pipeline`), commits `d78a688`, `5142b5b`, `716f930`. I haven't pushed anything or opened a PR.
Code is in `crates/galion-v2-live` on the branch. It's a tracked copy of the untracked `galion-v2-live/` loop; I didn't edit the original, which other lanes use. Diff vs original: `04-data/diff-vs-untracked-galion-v2-live.patch`.
Shipped code is all Rust. Python is only used in `04-data/summarize.py` and `.venv` for analysis.
I didn't touch the live voice.galion.app deployment.

## Measured A/B (flock BENCH.lock, 24 runs each, same 4 clips, same session, load 0.6–3)
Raw data: `04-data/runs/*/full-loop.ndjson`. Table: `04-data/summary.md`.
- **baseline**: exact copy of the old loop, including the old 5-word clauser.
- **latency-fixed**: `v2-latency` with the LLM sent at ASR final, in parallel with the backchannel, and the cap fix. Per-turn curl still.
- **fast**: `v2-pipeline240 --mode fast`, which adds:
  - the two-stage endpointer
  - the LLM started on the partial transcript
  - a warm HTTP/2 pool
  - the prebuilt prompt
  - the cached floor-hold line
- **fast-fin**: same as fast, but it also forces the ASR final at speculate time. **Rejected:** it adds 26 % WER (see below).

All values are ms, p50 / p95.

| stage (ms) | orig | baseline | latency-fixed | fast | fast-fin (rejected) |
|---|---|---|---|---|---|
| end-of-speech → LLM trigger | 130.2 / 140.3 | 130.2 / 140.4 | 130.2 / 140.3 | 154.2 / 290.4 (hits: 40) | 30.2 / 80.2 |
| ASR final on the critical path | 19.9 / 32.7 | 20.5 / 28.5 | 22.3 / 32.8 | 19.4 / 31.1 (0 on hits) | 0 |
| ASR → LLM request gap | 68.0 / 112.6 | 65.2 / 97.9 | **0.2 / 0.3** | **0.0 / 0.0** | 0.0 / 0.0 |
| **eos → LLM request (sum of my 3 stages)** | 216.9 / 245.6 | 221.6 / 237.4 | **153.8 / 169.8** | 154.2 / 290.4 | 30.2 / 80.2 |
| first token → first clause | 20.6 / 44.1 | 18.9 / 48.9 | 15.0 / 24.7 | **12.8 / 22.5** | 12.6 / 26.9 |
| first-clause words | 5 / 10.5 | 5 / 8 | **3 / 3** | 3 / 3 | 3 / 3 |
| LLM TTFT (lane 05; Groq, noisy) | 284.8 / 498.0 | 187.3 / 312.6 | 207.6 / 334.6 | 141.6 / 247.0 | 171.5 / 326.4 |
| Piper TTFA (lane 01) | 71.3 / 181.9 | 58.8 / 136.6 | 54.6 / 160.7 | 42.2 / 98.3 | 40.3 / 125.9 |
| **eos → first content audio** | 608.3 / 775.1 | 495.1 / 614.3 | 430.2 / 649.7 | **353.3 / 581.5** | 270.9 / 426.8 |
| eos → floor-hold audio | 186.1 / 212.9 | 189.1 / 205.7 | 198.8 / 278.6 | 140.2 / 260.3 | 130.2 / 140.3 |

The speculative start hit (the partial transcript equalled the final) on 6/24 turns in fast. In fast-fin it hit on 12/24, but that cost 26 % WER, which I rejected.
- **Why the fast p95 is high:** pdx-voice ends with "...my voice". At 160 ms of silence the lagging partial still read "...my", so the semantic hold kept the turn open to 500 ms.
- **Fix:** `716f930` adds `EpEv::Hold`, which forces the ASR final once and re-checks against the real text.
- **Status:** the fix passes its unit test. In an unlocked 4-run smoke test (timing not valid), pdx committed at about 100 ms. The locked 24-run measurement (`runs/fast-v2`) is queued but **not run yet**; see the blockers.

## What the 68 ms gap was
The old `latency.rs` full loop ran these steps before it sent the LLM request:
1. `say("Mm-hm.")`
2. wait for its first PCM (~30–35 ms)
3. the rest of the backchannel synth
4. `collect_until_done`'s 30 ms idle poll
5. `llm::stream`

`llm::stream` itself added more time per turn:
- a temp-JSON write
- a curl fork/exec
- DNS + TCP + TLS 1.3 on HTTP/1.1. I measured TLS at 27–40 ms and the first byte at 51–72 ms fresh vs ~18 ms reused. This part shows up inside "llm_ttft", not inside the gap.

What I changed in `latency.rs`:
- **`5142b5b`:** the LLM is sent first, at ASR final, and the backchannel is synthesized while the request is in flight. Gap: 65.2/97.9 → 0.2/0.3 measured.
- **`716f930`:** removed the 30 ms idle poll. The backchannel is streamed: its first PCM is the floor hold, and the first clause only waits for Piper (single process) to finish "Mm-hm." if it hasn't already.
- **`716f930`:** one pre-connected HTTP/2 curl replaces the per-turn curl. It finishes TLS and sends headers, then waits for the body on stdin. The key goes in through `--variable`/`--expand-header` and is never in argv or a file. `GALION_WARM=0` gives the old path.
- **Status of `716f930`:** built; the locked A/B run (`runs/latency-warm`) is queued and not run yet.

## First-clause cap bug
- **Bug:** `Clauser` only cut the first clause when the text ended in a space. Llama BPE tokens *start* with a space, so the cap never fired and clauses ran 5–10.5 words.
- **Fix:** `hard_cap` cuts at the start of word N+1. `Clauser::new` gets the fix, so `call.rs`'s `Clauser::new(3)` picks it up too. `Clauser::legacy` keeps the old behaviour for the A/B.
- **Tests:** unit tests with Llama-style tokens, all passing:
  - `capped(3)` gives "What a fascinating"
  - `new(2)` gives "Oh wow"
  - a comma before the cap still wins
  - `legacy(5)` reproduces the old 7-word clause
- **Measured:** 5/8 → 3/3 words, and first token → first clause 18.9/48.9 → 12.8/22.5.

## False-endpoint rate (endpoint scan)
Source: `04-data/scan/endpoint-scan.json`. The scan used 77 clips (4 samples + 73 LibriSpeech, CC-BY), 511 s of speech, with lockstep Vosk.
A "cut-off" is a commit followed by more speech.

| setting (spec/commit/hold ms) | clips cut off | cut-off events | false speculative starts / turn | eos→commit p50/p95 | eos→spec p50 |
|---|---|---|---|---|---|
| fixed 160 (baseline) | 72.7 % | 118 | – | 140 / 144 | – |
| **chosen 60 / 160 / 500 + "unfinished-word" hold** | **71.4 %** | **107** | **4.23** | 140 / 480 | 40 |
| 60 / 200 / 600 | 67.5 % | 90 | 4.25 | 180 / 580 | 40 |
| fixed 400 | 51.9 % | 56 | – | – | – |

- **How to read it:** this is read speech with long phrase pauses, so every silence-only endpointer cuts readers off a lot. The chosen setting is never worse than the 160 ms baseline (−9 % cut-off events), and it starts the LLM ~100 ms earlier when the partial is right. Speech resuming cancels the speculative call.
- **Wasted calls:** 102 calls in the 24 fast turns, estimated at ≈ $0.0005 (8b). Once V2 is fully local, that cost becomes llama.cpp CPU time: about 4 killed prefills per turn on read speech.
- **Partial lag:** the Vosk partial catches up to the final text a median 180 ms after end of speech (p90 968 ms). That's why the speculative hit rate is low.
- **Forcing the final at speculate time** fixes the hit rate but breaks recognition, because Vosk resets on every mid-turn pause. ASR WER vs baseline was 0.26 (for example "liberal vox" → "liberal boxster"). Rejected; it stays off behind `GALION_EP_FINALIZE_AT_SPEC=1`.

## Production checklist (pipeline sector)
PASS only counts when it's measured or tested.

| # | item | status |
|---|---|---|
| 1 | 68 ms gap: LLM sent at ASR final, in parallel with the backchannel (latency.rs) | PASS: gap 0.2/0.3 measured |
| 2 | Gap ≤ 5 ms budget | PASS |
| 3 | First-clause cap fires on leading-space tokens + unit tests | PASS: 3/3 words measured, tests pass |
| 4 | Prebuilt prompt (serialize once, escape only the user text) | PASS: 36 µs once, ~2 µs per turn |
| 5 | No WAV/JSON on disk on the hot path (PCM over pipes, body over curl stdin) | PASS (pipeline240 + warm latency.rs) |
| 6 | Prepared opening line cached in RAM ("Mm-hm." PCM) | PASS: floor hold 140 ms, no synth |
| 7 | Two-stage endpointer with a false-endpoint check, rate reported | PASS: 71.4 % vs 72.7 % baseline, 4.23 false spec/turn |
| 8 | No python shipped, own branch, no force-push, no secrets in argv/files/commits | PASS |
| 9 | latency.rs backchannel streamed, no 30 ms idle poll | PENDING: code in 716f930, locked run queued |
| 10 | Persistent HTTP/2 connection in latency.rs | PENDING: code in 716f930, locked run queued |
| 11 | Hold re-check removes the stale-partial p95 (290 ms) | PENDING: unit test passes, locked run queued |
| 12 | End-of-speech → LLM trigger ≤ 60 ms p50 | FAIL: 154 p50 (40 on hits, which were only 25 % of turns) |
| 13 | First token → first clause ≤ 10 ms p50 | FAIL: 12.8 |
| 14 | ASR final ≤ 15 ms | FAIL: 19.4 (0 on hits) |
| 15 | Speculative hit rate ≥ 50 % with no WER loss | FAIL: 25 % (Vosk partial lag) |
| 16 | Endpointer + warm client + clause-streaming Piper lane integrated into call.rs | FAIL: not done |
| 17 | Measured with the local LLM (V2 fully local, no Groq) | FAIL: the bench still uses OpenRouter/Groq. With localhost llama-server, TLS goes away, so the warm win shrinks to the curl spawn. Next step: an in-process keep-alive client |
| 18 | Validated on conversational (not read) speech | FAIL |

**Completion: 8 / 18 PASS = 44 %.** 3 items are pending one locked run, so it's 61 % if those pass.

## Contribution to the 240 ms total
- **My stages combined** (end of speech → LLM request, plus first token → clause):
  - baseline rerun: 221.6 + 18.9 = **240.5 ms p50**
  - latency-fixed: 153.8 + 15.0 = 168.8
  - fast: 154.2 + 12.8 = **167.0 ms p50** (measured)
  - That saves about **74 ms p50**. Budget for my four stages is 90 ms, so I'm still ~77 ms over.
- **End of speech → first content audio** (includes other lanes): 495 → 353 ms p50 measured. LLM TTFT varies with Groq, so the endpoint-to-request sum is the clean measure of my work.
- **What would close the rest:** a streaming ASR whose partial includes the last word within ~40 ms (for example a newer Vosk/Zipformer model, owned with ASR). That would make speculation hit most of the time and bring my stages to roughly 40 + 0 + 0 + 12 ≈ 55 ms (estimate).

## Blockers
- **BENCH.lock stuck since ~19:48 (UTC+1).** Another lane's run left an orphaned `llama-server` (port 18080, parent gone) plus a hung `curl`. They inherited the lock fd, so the lock stays held at load ~0.6 and every lane's queue is blocked. Under "do not stop other processes" I didn't kill them. My locked runs `latency-warm` and `fast-v2` are queued and will write to `04-data/runs/` when the lock frees.
- Vosk partial lag of 180 ms p50 limits speculation.
- Piper has no cancel and is one process.
- All results are on read speech only.
- Spend: ~$0.0007 measured on calls that completed, plus ~$0.001 estimated for killed speculative calls.
