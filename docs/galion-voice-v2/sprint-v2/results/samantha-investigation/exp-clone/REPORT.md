# Samantha clone-similarity experiments (2026-11-04, 21:37–22:55 CET)
Worker-direct (wt-13 chatterbox_worker.py, GALION_EXP=1 seeded hook, 8 threads), every worker under flock BENCH.lock, one at a time.
Metric: ECAPA cosine, torchaudio loader (same as tools/spk_sim_cc0_t628u.py). Sentences S1 (the old benchmark line), S2, S3 (long, 7-9 s) in lib.sh.
Held-out: Samantha H1 = amy-src 29.70-37.95 s, H2 = 0.60-9.00 s; Jarvis H1 = alan-src 43.40-53.40, H2 = 54.20-61.00.
Ceiling (real speech, ref vs held-out): Samantha 0.787 / 0.767; Jarvis 0.882 / 0.810. Candidate 17.84-29.25 vs H1 0.821, H2 0.768.
Full table: summary.md (summarize.py). Raw: results.ndjson, results-own.ndjson. Proposed repo change: proposed-ref-swap.diff (local commit 5899146 on swarm/samantha-exp).
| config | voice | n | vs original ref | vs own ref | vs held-out H1 | vs held-out H2 | paired Δref vs base | paired ΔH(1+2)/2 vs base |
|---|---|---|---|---|---|---|---|---|
| a-base | samantha | 15 | 0.758±0.031 med 0.759 | 0.758±0.031 med 0.759 | 0.642±0.045 med 0.649 | 0.662±0.035 med 0.660 | — | — |
| a-base | jarvis | 15 | 0.773±0.041 med 0.788 | 0.773±0.041 med 0.788 | 0.747±0.033 med 0.750 | 0.705±0.041 med 0.719 | — | — |
| b-ref-c1784 | samantha | 15 | 0.687±0.032 med 0.696 | 0.793±0.027 med 0.802 | 0.643±0.039 med 0.653 | 0.645±0.051 med 0.644 | -0.071±0.007 | -0.008±0.012 |
| b-ref-c1844 | samantha | 9 | 0.676±0.042 med 0.663 | 0.776±0.040 med 0.779 | 0.636±0.033 med 0.633 | 0.639±0.045 med 0.661 | -0.080±0.016 | -0.010±0.012 |
| b-ref-c2306 | samantha | 9 | 0.647±0.033 med 0.652 | 0.767±0.037 med 0.785 | 0.649±0.038 med 0.659 | 0.650±0.056 med 0.670 | -0.109±0.007 | +0.002±0.012 |
| b-nl-off | samantha | 9 | 0.759±0.039 med 0.758 | 0.759±0.039 med 0.758 | 0.622±0.043 med 0.625 | 0.667±0.030 med 0.678 | +0.003±0.012 | -0.003±0.016 |
| b-temp0.6 | samantha | 9 | 0.752±0.033 med 0.734 | 0.752±0.033 med 0.734 | 0.649±0.047 med 0.666 | 0.657±0.039 med 0.654 | -0.005±0.011 | +0.006±0.008 |
| b-full-s3gen10 | samantha | 9 | 0.771±0.033 med 0.770 | 0.771±0.033 med 0.770 | 0.651±0.042 med 0.659 | 0.662±0.034 med 0.672 | +0.015±0.010 | +0.009±0.013 |
| b-multiref3 | samantha | 9 | 0.722±0.024 med 0.733 | 0.722±0.024 med 0.733 | 0.631±0.042 med 0.637 | 0.669±0.027 med 0.665 | -0.034±0.011 | +0.003±0.016 |
| b-temp0.5 | samantha | 9 | 0.750±0.010 med 0.748 | 0.750±0.010 med 0.748 | 0.628±0.030 med 0.630 | 0.651±0.035 med 0.669 | -0.006±0.013 | -0.008±0.013 |
| b-topp0.8 | samantha | 9 | 0.746±0.026 med 0.749 | 0.746±0.026 med 0.749 | 0.633±0.040 med 0.645 | 0.650±0.035 med 0.655 | -0.011±0.006 | -0.006±0.010 |
| b-rep1.1 | samantha | 9 | 0.754±0.026 med 0.760 | 0.754±0.026 med 0.760 | 0.638±0.040 med 0.662 | 0.652±0.037 med 0.655 | -0.002±0.008 | -0.002±0.012 |
| b-steps4 | samantha | 9 | 0.753±0.043 med 0.747 | 0.753±0.043 med 0.747 | 0.639±0.058 med 0.652 | 0.656±0.033 med 0.659 | -0.003±0.003 | +0.000±0.002 |
| b-ref-c1844dn | samantha | 9 | 0.678±0.037 med 0.688 | 0.771±0.051 med 0.761 | 0.623±0.035 med 0.616 | 0.646±0.074 med 0.664 | -0.079±0.012 | -0.013±0.016 |
| b-ref-w8-8.3s | samantha | 9 | 0.745±0.031 med 0.747 | 0.755±0.030 med 0.759 | 0.607±0.033 med 0.620 | 0.624±0.035 med 0.629 | -0.012±0.010 | -0.032±0.015 |
| b-ref-w12-11.85s | samantha | 9 | 0.751±0.033 med 0.768 | 0.753±0.033 med 0.776 | 0.645±0.039 med 0.654 | 0.667±0.031 med 0.676 | -0.005±0.008 | +0.009±0.013 |
| b-temp0.7 | samantha | 9 | 0.750±0.034 med 0.763 | 0.750±0.034 med 0.763 | 0.613±0.055 med 0.632 | 0.644±0.055 med 0.658 | -0.006±0.009 | -0.019±0.015 |
| b-nowm | samantha | 9 | 0.761±0.033 med 0.754 | 0.761±0.033 med 0.754 | 0.639±0.051 med 0.646 | 0.658±0.034 med 0.655 | +0.005±0.001 | +0.001±0.001 |
| f-full-s3gen10 | samantha | 15 | 0.771±0.027 med 0.770 | 0.771±0.027 med 0.770 | 0.649±0.038 med 0.650 | 0.658±0.038 med 0.672 | +0.013±0.006 | +0.002±0.009 |
| f-full-s3gen10 | jarvis | 15 | 0.779±0.034 med 0.784 | 0.779±0.034 med 0.784 | 0.742±0.022 med 0.751 | 0.700±0.034 med 0.706 | +0.006±0.004 | -0.005±0.004 |
| f-c1784-full-s3gen10 | samantha | 15 | 0.711±0.030 med 0.711 | 0.808±0.028 med 0.812 | 0.663±0.039 med 0.665 | 0.653±0.055 med 0.656 | -0.047±0.007 | +0.006±0.012 |

a-base samantha S1: 0.734±0.027 med 0.743 n=5
a-base samantha S2: 0.788±0.014 med 0.789 n=5
a-base samantha S3: 0.752±0.025 med 0.736 n=5
a-base jarvis S1: 0.807±0.017 med 0.808 n=5
a-base jarvis S2: 0.789±0.016 med 0.788 n=5
a-base jarvis S3: 0.724±0.022 med 0.718 n=5
