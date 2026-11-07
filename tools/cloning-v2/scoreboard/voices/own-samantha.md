# own-samantha

speaker: samantha | baseline: true | clips: 12

| metric | value | source |
|---|---|---|
| true peak dBTP | 0.17 | rust (galion-board) |
| clipped samples | 6 | rust (galion-board) |
| clicks | 0 | rust (galion-board) |
| f99 Hz (ref) | 2612.0 | rust (galion-board) |
| ΔF0 Hz | 17.82 | rust (galion-board) |
| ΔF0 sd st | 0.39 | rust (galion-board) |
| Δenergy DR dB | 3.05 | rust (galion-board) |
| rate wps | 3.49 | rust (galion-board) |
| articulation | `{"error":"scorer ran but produced no parseable JSON"}` | rust |
| emotion | `{"label_match":"3/12","ref_top":"neu"}` | python |
| hygiene_ext | `{"artifacts":{"clips_with_clicks":5,"clips_with_internal_gaps":7,"clips_with_peak_0dbfs":4,"leading_silence_s_mean":0.095,"peak_dbfs_mean":-1.13,"peak_dbfs_min":-3.94,"sample_rates":[24000],"total_clicks":32,"total_clipping_samples":9,"trailing_silence_s_mean":0.239}}` | python |
| mos | `{"ref_mos":4.3255,"squim_mos":3.7285}` | python |
| prosody_py | `{"clone":{"energy_dyn_range_db":16.38,"f0_median_hz":200.7,"f0_range_st":17.75,"f0_sd_st":3.74,"speaking_rate_wps":3.49},"ref":{"duration_s":17.407,"energy_dyn_range_db":22.31,"energy_sd_db":8.45,"f0_median_hz":177.7,"f0_range_st":18.3,"f0_sd_st":3.11,"n_words_asr":49,"pause_count":7,"pause_mean_s":` | python |
| similarity | `{"error":"scorer ran but produced no parseable JSON"}` | rust |
| wer | `{"wer_mean":0.05}` | python |

## Persona target

| target | clone | goal | distance |
|---|---|---|---|
| pitch Hz | 190.48 | 178.0 | 12.48 |
| pace w/s | 3.49 | 2.82 | 0.67 |
| emotion | `{"label_match":"3/12","result":null,"source":"python","target":"calm/neutral"}` | calm/neutral | – |
| mouth expression | `{"bin":"/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/articulation/galion-artic/target/release/galion-artic","error":"scorer ran but produced no parseable JSON","source":"rust"}` | match original | – |
| HF (f99, Samantha) | `{"baseline_f99_hz":2612,"dist_hz":-1388.0,"f99_hz":2612.0,"target_hz":4000}` | ≥4000 Hz | – |

| generation RTF / CPU | – / – | – |
