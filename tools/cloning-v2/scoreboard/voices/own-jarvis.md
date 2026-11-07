# own-jarvis

speaker: jarvis | baseline: true | clips: 12

| metric | value | source |
|---|---|---|
| true peak dBTP | 0.16 | rust (galion-board) |
| clipped samples | 3 | rust (galion-board) |
| clicks | 0 | rust (galion-board) |
| f99 Hz (ref) | 6044.0 | rust (galion-board) |
| ΔF0 Hz | 13.25 | rust (galion-board) |
| ΔF0 sd st | 0.93 | rust (galion-board) |
| Δenergy DR dB | -25.35 | rust (galion-board) |
| rate wps | 3.31 | rust (galion-board) |
| articulation | `{"error":"scorer ran but produced no parseable JSON"}` | rust |
| emotion | `{"label_match":"12/12","ref_top":"neu"}` | python |
| hygiene_ext | `{"artifacts":{"clips_with_clicks":8,"clips_with_internal_gaps":4,"clips_with_peak_0dbfs":1,"leading_silence_s_mean":0.062,"peak_dbfs_mean":-2.05,"peak_dbfs_min":-4.35,"sample_rates":[24000],"total_clicks":39,"total_clipping_samples":4,"trailing_silence_s_mean":0.207}}` | python |
| mos | `{"ref_mos":4.5481,"squim_mos":4.1418}` | python |
| prosody_py | `{"clone":{"energy_dyn_range_db":10.0,"f0_median_hz":143.3,"f0_range_st":14.53,"f0_sd_st":3.58,"speaking_rate_wps":3.311},"ref":{"duration_s":15.952,"energy_dyn_range_db":13.24,"energy_sd_db":6.46,"f0_median_hz":128.6,"f0_range_st":14.8,"f0_sd_st":3.31,"n_words_asr":42,"pause_count":6,"pause_mean_s":` | python |
| similarity | `{"error":"scorer ran but produced no parseable JSON"}` | rust |
| wer | `{"wer_mean":0.018}` | python |

## Persona target

| target | clone | goal | distance |
|---|---|---|---|
| pitch Hz | 138.9 | 129.0 | 9.9 |
| pace w/s | 3.31 | 2.63 | 0.68 |
| emotion | `{"label_match":"12/12","result":null,"source":"python","target":"calm/neutral"}` | calm/neutral | – |
| mouth expression | `{"bin":"/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/articulation/galion-artic/target/release/galion-artic","error":"scorer ran but produced no parseable JSON","source":"rust"}` | match original | – |
| HF (f99, Samantha) | `"n/a (not Jarvis's limit)"` | ≥4000 Hz | – |

| generation RTF / CPU | – / – | – |
