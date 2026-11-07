# Original blend versus merged-production clones

Measured 2026-11-05 Europe/Zagreb (CET). Original fp32 Rust emotion scores reused from `../A-global/original-emotion.json`; merged-production scores reused from `merge-v2/scores/*_merge_v2_emo.json`. Contour supplement is Rust YIN, the identical scorer tracker. No original altered.

**Caution:** neu/hap/ang/sad are classifier outputs, not a human emotional inventory. “Sweet” and “playful” are not classes. Mean probabilities measure evidence/uncertainty, not literal emotional percentages. 2.5s original windows versus full different-text lines confound duration/text; therefore do not call any drop proven temporal emotion collapse.

## Samantha

Original top labels: {'neu': 4, 'ang': 3}; clone: {'hap': 1, 'ang': 7, 'neu': 4}.

| Metric | Original 2.5s windows | Merged clone lines |
|---|---:|---:|
| label_mix_entropy_bits | 0.985228 | 1.280672 |
| mean_probability_entropy_bits | 1.376915 | 1.518458 |
| mean_within_window_entropy_bits | 0.736639 | 1.115658 |
| arousal, mean ± population SD | 0.467641 ± 0.059805 | 0.472020 ± 0.049854 |
| valence, mean ± population SD | 0.487911 ± 0.085809 | 0.511730 ± 0.088517 |
| dominance, mean ± population SD | 0.540049 ± 0.045805 | 0.562033 ± 0.046572 |
| f0_mean_hz, mean ± population SD | 193.071496 ± 20.033993 | 185.730979 ± 8.671955 |
| f0_sd_st, mean ± population SD | 2.699281 ± 0.622265 | 2.599183 ± 0.727692 |
| f0_range_st_p5_p95, mean ± population SD | 8.632577 ± 2.406688 | 7.973041 ± 2.231388 |
| energy_dr_db, mean ± population SD | 29.405535 ± 2.150720 | 28.114830 ± 0.922138 |
| pause_ratio, mean ± population SD | 0.154147 ± 0.099809 | 0.064326 ± 0.072571 |
| spectral_tilt_db_oct, mean ± population SD | -6.163155 ± 0.890946 | -6.362983 ± 0.399603 |
| speech_rate_syl_s, mean ± population SD | 2.905497 ± 0.632277 | 3.469181 ± 0.645013 |

Mean SER probabilities (neu/hap/ang/sad): original {'neu': 0.55709, 'hap': 0.08326, 'ang': 0.34731, 'sad': 0.01234}; clone {'neu': 0.43781, 'hap': 0.14278, 'ang': 0.40906, 'sad': 0.01036}

| Original seconds | top | neu | hap | ang | sad | A | V | D |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 0.00–2.50 | neu | 0.9461 | 0.0347 | 0.0120 | 0.0072 | 0.5262 | 0.4725 | 0.5862 |
| 2.50–5.00 | neu | 0.9452 | 0.0335 | 0.0064 | 0.0148 | 0.4378 | 0.5020 | 0.5149 |
| 5.00–7.50 | neu | 0.9073 | 0.0902 | 0.0024 | 0.0001 | 0.3997 | 0.5454 | 0.4826 |
| 7.50–10.00 | ang | 0.2907 | 0.0463 | 0.6447 | 0.0183 | 0.4821 | 0.4497 | 0.5336 |
| 10.00–12.50 | neu | 0.5337 | 0.3631 | 0.1018 | 0.0013 | 0.4050 | 0.6429 | 0.5060 |
| 12.50–15.00 | ang | 0.2553 | 0.0084 | 0.6933 | 0.0430 | 0.4475 | 0.4620 | 0.5313 |
| 15.00–17.41 | ang | 0.0212 | 0.0066 | 0.9705 | 0.0017 | 0.5752 | 0.3408 | 0.6258 |

## Jarvis

Original top labels: {'neu': 6}; clone: {'neu': 12}.

| Metric | Original 2.5s windows | Merged clone lines |
|---|---:|---:|
| label_mix_entropy_bits | -0.000000 | -0.000000 |
| mean_probability_entropy_bits | 0.818980 | 1.022445 |
| mean_within_window_entropy_bits | 0.594579 | 0.858751 |
| arousal, mean ± population SD | 0.540629 ± 0.043930 | 0.507467 ± 0.036420 |
| valence, mean ± population SD | 0.574295 ± 0.102426 | 0.503157 ± 0.083215 |
| dominance, mean ± population SD | 0.641753 ± 0.041040 | 0.598956 ± 0.036462 |
| f0_mean_hz, mean ± population SD | 134.766257 ± 6.135813 | 144.081291 ± 7.149343 |
| f0_sd_st, mean ± population SD | 3.086180 ± 0.521711 | 3.669978 ± 0.421947 |
| f0_range_st_p5_p95, mean ± population SD | 9.433536 ± 1.151544 | 11.890311 ± 1.604456 |
| energy_dr_db, mean ± population SD | 22.570176 ± 2.521582 | 24.305484 ± 2.123305 |
| pause_ratio, mean ± population SD | 0.109066 ± 0.085531 | 0.056997 ± 0.036352 |
| spectral_tilt_db_oct, mean ± population SD | -6.507887 ± 0.904531 | -5.948957 ± 0.288384 |
| speech_rate_syl_s, mean ± population SD | 2.167071 ± 0.571421 | 2.645304 ± 0.503184 |

Mean SER probabilities (neu/hap/ang/sad): original {'neu': 0.82723, 'hap': 0.05374, 'ang': 0.119, 'sad': 3e-05}; clone {'neu': 0.76753, 'hap': 0.09756, 'ang': 0.13339, 'sad': 0.00152}

| Original seconds | top | neu | hap | ang | sad | A | V | D |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 0.00–2.50 | neu | 0.9812 | 0.0187 | 0.0001 | 0.0000 | 0.5287 | 0.7453 | 0.6413 |
| 2.50–5.00 | neu | 0.7662 | 0.0088 | 0.2249 | 0.0001 | 0.6077 | 0.6484 | 0.7080 |
| 5.00–7.50 | neu | 0.9970 | 0.0011 | 0.0019 | 0.0000 | 0.5136 | 0.6061 | 0.6186 |
| 7.50–10.00 | neu | 0.7526 | 0.0783 | 0.1691 | 0.0000 | 0.5934 | 0.5117 | 0.6785 |
| 10.00–12.50 | neu | 0.9989 | 0.0006 | 0.0005 | 0.0000 | 0.5083 | 0.4554 | 0.5837 |
| 12.50–15.95 | neu | 0.4675 | 0.2150 | 0.3175 | 0.0000 | 0.4920 | 0.4788 | 0.6205 |

## Interpretation / immediate guidance for candidate worker

Read the tables rather than assuming the owner theory is established. The whole-recording Samantha score has P(sad)=0.318, yet 2.5s windows average P(sad)=0.012; this is strong context-length dependence, NOT evidence that sadness was removed by cloning. Original windows contain neutral/angry detector modes and one happy-evidence interval at 10–12.5s (P(hap)=0.363). Hard-label entropy rises, rather than collapses, in merged Samantha. Her between-window F0-mean SD falls 20.03→8.67 Hz and energy-DR SD falls 2.15→0.92 dB; arousal variance falls 0.003577→0.002485, while valence variance does not fall. Jarvis F0 ranges grow rather than flatten. Clones can shift toward angry detector evidence without losing all F0 movement. Jarvis is an essential negative control: neutral hard labels can conceal valence loss. A reference fix should preserve multiple source regions and their unnormalised relative dynamics, not force a hand-authored happy/angry mixture. Do not average discrete speech tokens or splice unrelated prompt tokens.

**Important duration check:** the same-window comparison below does NOT reproduce an arousal-variance collapse for Samantha (SD 0.059805→0.062946). The stronger line-level flattening estimates partly reflect averaging longer lines. Prefer the duration-matched table: F0 range 8.63→7.19 st, between-window F0-mean SD 20.03→15.96 Hz, pause ratio 0.154→0.070. Neither categorical entropy nor Samantha arousal variation collapses.
Root-cause audit and opt-in global implementation follow in `../global-fix/REPORT.md`.
## Duration-matched production-window cross-check

Additional fp32 same-method source: `../A-global/production-window-emotion.json`; ~2.5s splitting merges/redistributes short tails. Still different text and unequal numbers of windows.

### Samantha (23 merged windows)

| Metric | Original windows | Clone windows |
|---|---:|---:|
| label_mix_entropy_bits | 0.985228 | 1.208830 |
| mean_probability_entropy_bits | 1.376915 | 1.408459 |
| arousal, mean ± population SD | 0.467641 ± 0.059805 | 0.465667 ± 0.062946 |
| valence, mean ± population SD | 0.487911 ± 0.085809 | 0.504904 ± 0.109033 |
| f0_mean_hz, mean ± population SD | 193.071496 ± 20.033993 | 186.047949 ± 15.955991 |
| f0_range_st_p5_p95, mean ± population SD | 8.632577 ± 2.406688 | 7.191221 ± 2.212757 |
| energy_dr_db, mean ± population SD | 29.405535 ± 2.150720 | 27.766179 ± 1.927687 |
| pause_ratio, mean ± population SD | 0.154147 ± 0.099809 | 0.070117 ± 0.110642 |

### Jarvis (25 merged windows)

| Metric | Original windows | Clone windows |
|---|---:|---:|
| label_mix_entropy_bits | -0.000000 | 0.402179 |
| mean_probability_entropy_bits | 0.818980 | 0.950509 |
| arousal, mean ± population SD | 0.540629 ± 0.043930 | 0.510437 ± 0.034522 |
| valence, mean ± population SD | 0.574295 ± 0.102426 | 0.496818 ± 0.100335 |
| f0_mean_hz, mean ± population SD | 134.766257 ± 6.135813 | 143.474162 ± 12.060661 |
| f0_range_st_p5_p95, mean ± population SD | 9.433536 ± 1.151544 | 10.432262 ± 2.547100 |
| energy_dr_db, mean ± population SD | 22.570176 ± 2.521582 | 24.298126 ± 2.394051 |
| pause_ratio, mean ± population SD | 0.109066 ± 0.085531 | 0.050357 ± 0.060932 |

