# Love and seductive detector (Agent 2)

Plugin for the persona scorer. Old universal CLAP (`scorers/universal/`) is unchanged and stays the default until Slavik approves the persona path. Nothing was deployed.

Interface: Agent 1 did not publish `INTERFACE.md`. The hook in `scorers/persona/detectors/__init__.py` loads any `love_seductive*.py` that exports `LoveSeductiveDetector` (no-arg constructor), `build_detector()`, or `DETECTOR`. This plugin exports all three. `score(rec)` returns `DetectorScore` values for `love` and `seductive`. The full method needs a wav on `rec["path"]` or `rec["file"]`; without a wav it degrades to a contrast of whatever legacy CLAP cosines are already on the record.

Persona lists (`pipeline/personas/*.toml`, read-only):

- `own-samantha.toml` judges **love** and **seductive** (among others).
- `own-jarvis.toml` judges only **neutral** and **logic**. Love is excluded (`exclude-love`). Seductive is excluded (`exclude-seductive`, Samantha-only). Jarvis love and seductive numbers below are info / detector tests, not persona-gate inputs.

The probe set (`scorer-probe/lines.json`) has no Samantha love lines (`lov01` / `lov02` are Jarvis-only).

## Chosen method

**Model:** local `laion/clap-htsat-fused` (Apache-2.0), same weights the universal scorer already uses. No new model, no Qwen.

**Prompts:** gender-conditioned banks in `love_seductive_prompts.json`. Female prompts for Samantha, male prompts (including male whisper / male tender) for Jarvis, plus shared lines. Neutral-speech prompts are the contrast bank, not a scored emotion.

**Score (temperature 20 pairwise softmax, same family as the universal scorer so the 0.2 presence threshold still applies):**

- `contrast = softmax2(mean_cosine(target_bank), mean_cosine(neutral_bank), temp=20)`
- **Seductive** = `contrast * (1 - emotion2vec+ sad)`. Crying/sad lines are not counted as seductive. Whisper-blend and max-prompt variants were measured and rejected: they pushed neutrals over 0.2 (when both banks match poorly, a max-prompt contrast sits near 0.5).
- **Love** = that same sad-gated contrast **only if** the mean cosine vs the love bank is greater than `LOVE_ALIGN_MIN = 0.20`. Otherwise love is `0` (no presence). This is the existing presence number applied as a minimum CLAP alignment check, not a knife-edge fit on the two-line probe. emotion2vec+ happy is not used as love: Jarvis `lov02` is e2v-neutral `1.0` (not happy), and happy fires on both joyful lines. Samantha is scored on both love and joyful, so an OR with happy would create joyful false presence.

Max-over-individual-prompts helped diagnostics (the prompts that actually fired are stored in extras) but was worse than the mean for presence decisions.

## Before / after on the original-voice probe

Presence threshold `0.2`. "Detected" means score `>= 0.2`. Margin is `mean(real) - mean(neutral)`. Gap is `min(real) - max(neutral)` (positive means the two groups do not overlap).

### Samantha seductive (scored)

The bug this was meant to fix: old CLAP caught 2 of 4 lines, only the stronger whispers (`sed01` `0.21242489922715546`, `sed04` `0.33438899274060635`), and missed `sed02` and `sed03`.

| line | old universal CLAP | new |
|---|---|---|
| sed01 | 0.21242489922715546 | 0.3308078013697434 |
| sed02 | 0.09575001362314822 | 0.3810774608811502 |
| sed03 | 0.15666967712967103 | 0.24670745637040328 |
| sed04 | 0.33438899274060635 | 0.6222369636550176 |
| neu01 | 0.09117801014913104 | 0.031638791639141184 |
| neu02 | 0.12049726236349802 | 0.09831994082453392 |

| | old | new |
|---|---|---|
| mean real | 0.1998083956801452 | 0.3952074205690786 |
| mean neutral | 0.1058376362563145 | 0.0649793662318376 |
| margin | 0.0939707594238307 | 0.3302280543372410 |
| gap | -0.0247472487403498 | 0.1483875155458694 |
| detected real @ 0.2 | 2/4 | **4/4** |
| detected neutral @ 0.2 | 0/2 | **0/2** |
| detected joyful @ 0.2 | 0/2 | 0/2 (`joy01` 0.041082219594264835, `joy02` 0.036629374786360656) |
| detected crying @ 0.2 | 1/4 (old `cry03` 0.261534740695) | **0/4** (sad-gate) |
| detected sad @ 0.2 | 0/2 | 0/2 (`sad02` 0.1540651832435958) |

### Jarvis seductive (info only — n/a for his persona)

Old CLAP: 0 of 4 (`0.007503466957353025` to `0.03655004821177384`), and neutrals scored higher than the reals.

| line | old universal CLAP | new |
|---|---|---|
| sed01 | 0.020788459704512822 | 0.2088836818973831 |
| sed02 | 0.020337040605279598 | 0.19561392273506772 |
| sed03 | 0.03655004821177384 | 0.30078544163380877 |
| sed04 | 0.007503466957353025 | 0.039147601929223924 |
| neu01 | 0.029218809756874332 | 0.14453286700121862 |
| neu02 | 0.02849043063559944 | 0.07965604074356052 |

| | old | new |
|---|---|---|
| mean real | 0.0212947538697298 | 0.1861076620488709 |
| mean neutral | 0.0288546201962369 | 0.1120944538723896 |
| margin | -0.0075598663265071 | 0.0740132081764813 |
| gap | -0.0217153427995213 | -0.1053852650719947 |
| detected real @ 0.2 | 0/4 | 2/4 (`sed01`, `sed03`; `sed02` is 0.19561392273506772) |
| detected neutral @ 0.2 | 0/2 | 0/2 |

Male-whisper prompts do lift Jarvis (margin goes from negative to positive; 0/4 → 2/4) but the groups still overlap. `joy02` scores 0.21656458252974847 and `cry03` scores 0.2590908043126387 under the new seductive method. Those false fires matter less because Jarvis is not scored on seductive.

### Jarvis love (info / male-tender test — n/a for his persona)

The owner note that emotion2vec+ rated both "I love you" lines happy `0.99999` is not what the probe file says:

- `lov01`: old CLAP love `0.013194709546592643`, emotion2vec+ happy `0.9999991655349731`
- `lov02`: old CLAP love `0.019290552229826268`, emotion2vec+ **neutral `1.0`** (happy `1.1429572843724145e-08`)

New male-tender prompts still do not positively align with either line (`love_mean` `-0.1155603816732764` and `-0.0446460712701082`). The sad-gated mean contrast without the alignment gate would be `0.165860674427` and `0.164695673331` — still below 0.2, and Jarvis `neu02` would be `0.165984292315`, slightly above both love lines. Raw max cosine does separate these two love lines from these two neutrals (love_max `0.117554053664` / `0.116378687322` vs `0.085314683616` / `0.040378190577`, gap `0.031064003706`) but the same max fires higher on `joy01` (`0.119456112385`), `cry01` (`0.23489569`) and several seductive lines, so it cannot be used as a 0.2 presence score.

The shipped alignment gate therefore reports love `0` on the Jarvis probe love and neutral lines (detected 0/2 vs 0/2, same as old). That is detector failure on synthesized male tender speech, not a claim that the lines contain no feeling: `lov01` is clearly happy according to emotion2vec+.

| line | old universal CLAP | new (shipped) | love_mean cosine |
|---|---|---|---|
| lov01 | 0.013194709546592643 | 0.0 | -0.1155603816732764 |
| lov02 | 0.019290552229826268 | 0.0 | -0.0446460712701082 |
| neu01 | 0.010308911502726876 | 0.0 | -0.0995819023810327 |
| neu02 | 0.015935718687990586 | 0.0 | -0.1189250219613314 |

| | old | new (shipped) |
|---|---|---|
| mean real | 0.0162426308882095 | 0.0 |
| mean neutral | 0.0131223150953587 | 0.0 |
| margin | 0.0031203157928507 | 0.0 |
| detected real @ 0.2 | 0/2 | 0/2 |
| detected neutral @ 0.2 | 0/2 | 0/2 |

### Samantha love (scored, but no probe lines)

No labeled Samantha love wavs in the probe. Checked the already-cut universal windows (not `original.wav` / `Samantha.wav`):

| window | role | old CLAP love | love_mean cosine | new love |
|---|---|---|---|---|
| w00 `0.000-2.500` | original neutral (e2v neutral 1.0) | 0.07693856890835132 | 0.04579303786158562 | 0.0 (below align min) |
| w03 `7.500-10.000` | original love moment (also e2v angry 0.9990363121032715) | 0.8109382317899566 | 0.27918557822704315 | 0.9228583200845187 |

So the new love detector does fire on her original love window and does not fire on her original neutral window. Agent 1's Samantha love target (`0.8109382317899566`, measured by legacy CLAP) should be refreshed to `0.9228583200845187` if this plugin is adopted; this file does not edit the persona toml.

## Files

- `cloning-v2/scorers/persona/detectors/love_seductive.py` — plugin + `--measure`
- `cloning-v2/scorers/persona/detectors/love_seductive_prompts.json` — prompt banks
- `cloning-v2/scorers/persona/detectors/love_seductive_separation.json` — per-line scores and every candidate method
- `cloning-v2/scorers/persona/detectors/love_seductive.md` — this note

Not edited: `scorers/universal/`, `pipeline/`, `scorer-probe/results-persona/`, Agent 1's other files under `scorers/persona/`.

## Tests run

- `py_compile` on `love_seductive.py`
- `discover_love_seductive()` / `load_detectors()`: plugin loads; `LegacyClapLoveSeductiveFallback` is skipped
- `LoveSeductiveDetector().score(rec)` without a wav (degraded path)
- `--measure` on the original-voice probe (30 clips: Samantha and Jarvis crying, seductive, neutral, joyful, sad; Jarvis love). Wall time `123.43534803390503` s on box CPU
- Candidate ranking of prompt-mean vs prompt-max, several contrast temperatures, sigmoid of the cosine delta, whisper acoustics, whisper blend, and `max(contrast, e2v happy)`
- Read-only rescore of existing Samantha window cuts `w00_0.000_2.500.wav` and `w03_7.500_10.000.wav`

No deploy, no push, no merge, no remote commit, no originals modified.
