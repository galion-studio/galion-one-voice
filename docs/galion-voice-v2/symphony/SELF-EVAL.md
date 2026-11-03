# Symphony self-eval

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Gate ledger. A section is complete when its gate passes here.

This landing records **RC**, the Galion Research console plan. Other symphony sectors append their own sections.

## RC · Research console

Plan: [`PLAN-RESEARCH-CONSOLE.md`](PLAN-RESEARCH-CONSOLE.md).
Recorded: 2026-11-03 Europe/Zagreb.
Owner: Maciej Grajczyk.
`LOCKS.toml` was not edited.

The plan section is complete when RC-1 through RC-4 pass. RC-5 and RC-6 are the product gates. They stay open on purpose: this landing is the plan.

| Gate | Criterion | Result |
|---|---|---|
| RC-1 | Research is the async side band. The plan names the turn-boundary handoff, the 3 s voice cap, and the tests that already refuse a blocking offer. | **PASS** |
| RC-2 | Galion Voice stays one dot. The Research console is its own product surface: own process, own page, seven state words, no second AI panel on `v1-screen.html`. | **PASS** |
| RC-3 | CPU and RAM come from the t284u summary of HARDWARE.md. Higher tok/s is the optimistic end of the published LLM band (300 / 1 300), nominal planning point 250 / 1 000, floor 200 / 800 when the node is at the voice peak. Residual arithmetic is shown. No extra tok/s was invented. | **PASS** |
| RC-4 | This change does not edit `LOCKS.toml`. `paid_tiers` stays false. `[ui]` stays one dot. | **PASS** |
| RC-5 | A CPU benchmark of research tok/s on the async queue, written to the ledger against the published band. | **NOT RUN** |
| RC-6 | A console process a person can open, separate from the voice page. | **NOT RUN** |

### RC-1 witness

rustc 1.85.0. The workspace default on this image is 1.83.0, which refuses `galion-openmind` (`rust-version = "1.85"`). The run used the 1.85.0 toolchain.

```text
cargo +1.85.0 test -p galion-openmind --test handoff
test offline_hermes_is_honest_unreachable ... ok
test affect_is_clamped_before_it_reaches_hermes ... ok
test money_is_not_offered ... ok
test offer_never_blocks ... ok
# 4 passed; 0 failed; finished in 0.20s

cargo +1.85.0 test -p galion-sideband --test client
test unknown_turn_is_none ... ok
test null_transport_gives_honest_unreachable ... ok
test submit_and_poll_never_block ... ok
# 3 passed; 0 failed; finished in 0.20s
```

`offer_never_blocks` is six offers on a 200 ms transport with queue depth 1, finished in under 80 ms. `submit_and_poll_never_block` is the same shape and ends in `QueueFull`. Neither test is a `Heartbeat::step` trace. The plan's rule is stricter: `offer` is not called from `step`.

### RC-3 arithmetic

From the t284u summary only: 806 voice streams, ~190 GB, ~5 app nodes, 192 threads, 768 GB on the App/Voice/Research node.

| | |
|---|---|
| Voice vCPU per node | 806 / 5 = 161.2 |
| Thread residual | 192 − 161.2 = 30.8 |
| Voice share of threads | 84% |
| 70% of 192 | 134.4 threads |
| Voice RAM per node | 190 / 5 = 38 GB |
| RAM residual | 768 − 38 = 730 GB |
| 8 000 source tokens at 1 000 tok/s | 8.0 s, past the 3 s voice cap |
| 8 000 source tokens at 800 tok/s | 10.0 s |
| 8 000 source tokens at 1 300 tok/s | 6.2 s |
| 300 decode tokens at 250 tok/s | 1.2 s, inside the cap once prefill is done |

At the voice peak the higher tok/s queue does not share those app-node threads. It runs where the 70% band is free. The desk control plane fits in the ~31 thread residual. The token worker does not.

### RC-4 witness

The diff for this landing is the two files under `docs/galion-voice-v2/symphony/`. `LOCKS.toml` is untouched.

### Still open

RC-5 needs a `.gq` run on CPU, host and model id written down, compared with 200/800, 250/1000, and 300/1300. RC-6 needs the separate console process. Neither is claimed here.

— Maciej Grajczyk
