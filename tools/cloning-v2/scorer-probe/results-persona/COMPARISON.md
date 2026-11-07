# Persona-aware rescore comparison (Agent 3)

Generated: 2026-11-06T11:56:09.174460+01:00 (Europe/Zagreb (UTC+1)).
Gate: emotion_match ≥ 0.83. Presence: 0.2.
No deploy / push / merge. Opt-in persona scorer only.

## OLD column source (reused, not re-scored)

- Probe: `scorer-probe/results/SUMMARY.json` + `probe_universal_all.json`
- Ceilings + merge-v2 + coherent: `scorers/universal/results/{samantha,jarvis}-universal/RESULTS.json`
  (same wav dirs as share `pkg/renders/*`; t01 sha matched for merge-v2).

## NEW column source

- `scorers/persona/INTERFACE.md`; file scores via `score_persona.py`; gate via Agent-3 `results-persona/persona_match_cached.py` (same math as `persona_match.py`, path-cached detectors so love_seductive CLAP runs once per wav).
- Persona files: `pipeline/personas/own-{samantha,jarvis}.toml` (not edited).
- Crying decision: emotion2vec+ sad. Love/seductive: final `detectors/love_seductive.py` mtime **2026-11-06 11:17:17 CET** (Agent 2 final: gender-conditioned prompt bank vs neutral, softmax temp 20; seductive × (1 − e2v sad); love gated at mean cosine > 0.20).
- No earlier NEW artifacts existed; this is the first complete NEW run and it used that final plugin (no stale rerun needed).

## Gate table (emotion_match)

| voice | set | old emotion_match | new emotion_match | old gate | new gate |
|---|---|---|---|---|---|
| samantha | ceiling | 0.904367 | 0.865031 | PASS | PASS |
| samantha | merge-v2 | 0.773827 | 0.686945 | FAIL | FAIL |
| samantha | coherent | 0.750903 | 0.67137 | FAIL | FAIL |
| jarvis | ceiling | 0.94751 | 0.954435 | PASS | PASS |
| jarvis | merge-v2 | 0.880208 | 0.873069 | PASS | PASS |
| jarvis | coherent | 0.890082 | 0.879119 | PASS | PASS |

### Samantha 0.83 gate verdict

- **merge-v2**: new emotion_match=0.686945 → **FAIL** (old was 0.773827 / FAIL)
- **coherent**: new emotion_match=0.67137 → **FAIL** (old was 0.750903 / FAIL)

## samantha

Persona judged: `['joyful', 'aggressive', 'love', 'laughter', 'logic', 'neutral', 'seductive']`

### Set `ceiling` — per emotion

old em=0.904367 cat=0.863382 emb=1 | new em=0.865031 cat=0.82182 emb=0.965857

| emotion | status | old score | new score | old strength | new strength | old source | new source |
|---|---|---|---|---|---|---|---|
| aggressive | scored | 0.8203 | 0.8203 | 0.999 | 0.999 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| joyful | missing | — | — | — | — | — | — |
| laughter | scored | 0.9165 | 0.9165 | 0.2744 | 0.2744 | efficientat | efficientat |
| logic | scored | 0.8519 | 0.8519 | 0.4997 | 0.4997 | zero-shot text prompt (CLAP) | zero-shot text prompt (CLAP) |
| love | scored | 0.7852 | 0.6302 | 0.8109 | 0.9391 | zero-shot text prompt (CLAP) | love_seductive clap-prompt-ensemble (gender-conditioned mean-target vs mean-neutral, sad-gated; love also requires mean cosine > 0.20) |
| neutral | scored | 0.9623 | 0.9623 | 1 | 1 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| seductive | missing | — | — | — | — | — | — |
| yelling | n/a | 0.8614 | n/a | 0.6654 | n/a | efficientat | n/a |

### Set `merge-v2` — per emotion

old em=0.773827 cat=0.727911 emb=0.880965 | new em=0.686945 cat=0.613557 emb=0.858183

| emotion | status | old score | new score | old strength | new strength | old source | new source |
|---|---|---|---|---|---|---|---|
| aggressive | scored | 0.5555 | 0.5555 | 0.999 | 0.999 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| joyful | missing | — | — | — | — | — | — |
| laughter | scored | 0.8542 | 0.8542 | 0.2744 | 0.2744 | efficientat | efficientat |
| logic | scored | 0.9248 | 0.9248 | 0.4997 | 0.4997 | zero-shot text prompt (CLAP) | zero-shot text prompt (CLAP) |
| love | scored | 0.307 | 0.0609 | 0.8109 | 0.9391 | zero-shot text prompt (CLAP) | love_seductive clap-prompt-ensemble (gender-conditioned mean-target vs mean-neutral, sad-gated; love also requires mean cosine > 0.20) |
| neutral | scored | 0.9689 | 0.9689 | 1 | 1 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| seductive | missing | — | — | — | — | — | — |
| yelling | n/a | 0.9376 | n/a | 0.6654 | n/a | efficientat | n/a |

### Set `coherent` — per emotion

old em=0.750903 cat=0.716687 emb=0.830741 | new em=0.67137 cat=0.606734 emb=0.822185

| emotion | status | old score | new score | old strength | new strength | old source | new source |
|---|---|---|---|---|---|---|---|
| aggressive | scored | 0.5288 | 0.5288 | 0.999 | 0.999 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| joyful | missing | — | — | — | — | — | — |
| laughter | scored | 0.9249 | 0.9249 | 0.2744 | 0.2744 | efficientat | efficientat |
| logic | scored | 0.8973 | 0.8973 | 0.4997 | 0.4997 | zero-shot text prompt (CLAP) | zero-shot text prompt (CLAP) |
| love | scored | 0.2913 | 0.0609 | 0.8109 | 0.9391 | zero-shot text prompt (CLAP) | love_seductive clap-prompt-ensemble (gender-conditioned mean-target vs mean-neutral, sad-gated; love also requires mean cosine > 0.20) |
| neutral | scored | 0.9646 | 0.9646 | 1 | 1 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| seductive | missing | — | — | — | — | — | — |
| yelling | n/a | 0.9231 | n/a | 0.6654 | n/a | efficientat | n/a |

### Probe (original-voice synthesized lines) — samantha

| emotion | n | status | old mean primary | old detect@0.2 | new mean decision | new detect@0.2 |
|---|---|---|---|---|---|---|
| crying | 4 | n/a | 0.0955 | 0 | n/a | n/a |
| disgust | 2 | n/a | 0.0032 | 0 | n/a | n/a |
| fear | 2 | n/a | 0.4962 | 0.5 | n/a | n/a |
| joyful | 2 | scored | 1 | 1 | 1 | 1 |
| neutral | 2 | scored | 0.9995 | 1 | 0.9995 | 1 |
| sad | 2 | n/a | 0.6967 | 1 | n/a | n/a |
| seductive | 4 | scored | 0.1998 | 0.5 | 0.3961 | 1 |
| surprise | 2 | n/a | 0.1033 | 0.5 | n/a | n/a |

Per-line detail (crying / seductive / love / neutral):

| id | group | status | old score | old det | new score | new det | persona | new source |
|---|---|---|---|---|---|---|---|---|
| cry01 | crying | n/a | 0.1245 | False | 1 | True | n/a | trained class (emotion2vec+ sad) |
| cry02 | crying | n/a | 0.0727 | False | 1 | True | n/a | trained class (emotion2vec+ sad) |
| cry03 | crying | n/a | 0.1107 | False | 1 | True | n/a | trained class (emotion2vec+ sad) |
| cry04 | crying | n/a | 0.074 | False | 0 | False | n/a | trained class (emotion2vec+ sad) |
| sed01 | seductive | scored | 0.2124 | True | 0.3278 | True | 0.3278 | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| sed02 | seductive | scored | 0.0958 | False | 0.3862 | True | 0.3862 | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| sed03 | seductive | scored | 0.1567 | False | 0.2614 | True | 0.2614 | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| sed04 | seductive | scored | 0.3344 | True | 0.6089 | True | 0.6089 | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| neu01 | neutral | scored | 1 | True | 1 | True | 1 | trained class (emotion2vec+) |
| neu02 | neutral | scored | 0.9989 | True | 0.9989 | True | 0.9989 | trained class (emotion2vec+) |

## jarvis

Persona judged: `['neutral', 'logic']`

### Set `ceiling` — per emotion

old em=0.94751 cat=0.925014 emb=1 | new em=0.954435 cat=0.964427 emb=0.931122

| emotion | status | old score | new score | old strength | new strength | old source | new source |
|---|---|---|---|---|---|---|---|
| logic | scored | 0.9214 | 0.9214 | 0.8277 | 0.8277 | zero-shot text prompt (CLAP) | zero-shot text prompt (CLAP) |
| neutral | scored | 1 | 1 | 1 | 1 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| yelling | n/a | 0.836 | n/a | 0.8088 | n/a | efficientat | n/a |

### Set `merge-v2` — per emotion

old em=0.880208 cat=0.892812 emb=0.850799 | new em=0.873069 cat=0.88718 emb=0.840143

| emotion | status | old score | new score | old strength | new strength | old source | new source |
|---|---|---|---|---|---|---|---|
| logic | scored | 0.7932 | 0.7932 | 0.8277 | 0.8277 | zero-shot text prompt (CLAP) | zero-shot text prompt (CLAP) |
| neutral | scored | 0.965 | 0.965 | 1 | 1 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| yelling | n/a | 0.9055 | n/a | 0.8088 | n/a | efficientat | n/a |

### Set `coherent` — per emotion

old em=0.890082 cat=0.880792 emb=0.911759 | new em=0.879119 cat=0.868125 emb=0.904771

| emotion | status | old score | new score | old strength | new strength | old source | new source |
|---|---|---|---|---|---|---|---|
| logic | scored | 0.7301 | 0.7301 | 0.8277 | 0.8277 | zero-shot text prompt (CLAP) | zero-shot text prompt (CLAP) |
| neutral | scored | 0.9823 | 0.9823 | 1 | 1 | trained class (emotion2vec+) | trained class (emotion2vec+) |
| yelling | n/a | 0.9095 | n/a | 0.8088 | n/a | efficientat | n/a |

### Probe (original-voice synthesized lines) — jarvis

| emotion | n | status | old mean primary | old detect@0.2 | new mean decision | new detect@0.2 |
|---|---|---|---|---|---|---|
| aggressive | 2 | n/a | 0.9268 | 1 | n/a | n/a |
| crying | 4 | n/a | 0.1224 | 0 | n/a | n/a |
| disgust | 2 | n/a | 0.0004 | 0 | n/a | n/a |
| fear | 2 | n/a | 0 | 0 | n/a | n/a |
| joyful | 2 | n/a | 1 | 1 | n/a | n/a |
| laughter | 2 | n/a | 0.3114 | 1 | n/a | n/a |
| love | 2 | n/a | 0.0162 | 0 | n/a | n/a |
| neutral | 2 | scored | 1 | 1 | 1 | 1 |
| sad | 2 | n/a | 0.0001 | 0 | n/a | n/a |
| seductive | 4 | n/a | 0.0213 | 0 | n/a | n/a |
| surprise | 2 | n/a | 0 | 0 | n/a | n/a |

Per-line detail (crying / seductive / love / neutral):

| id | group | status | old score | old det | new score | new det | persona | new source |
|---|---|---|---|---|---|---|---|---|
| cry01 | crying | n/a | 0.1758 | False | 1 | True | n/a | trained class (emotion2vec+ sad) |
| cry02 | crying | n/a | 0.0695 | False | 0.3634 | True | n/a | trained class (emotion2vec+ sad) |
| cry03 | crying | n/a | 0.1651 | False | 0.0029 | False | n/a | trained class (emotion2vec+ sad) |
| cry04 | crying | n/a | 0.0792 | False | 0 | False | n/a | trained class (emotion2vec+ sad) |
| sed01 | seductive | n/a | 0.0208 | False | 0.2784 | True | n/a | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| sed02 | seductive | n/a | 0.0203 | False | 0.2442 | True | n/a | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| sed03 | seductive | n/a | 0.0366 | False | 0.3816 | True | n/a | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| sed04 | seductive | n/a | 0.0075 | False | 0.0593 | False | n/a | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| neu01 | neutral | scored | 1 | True | 1 | True | 1 | trained class (emotion2vec+) |
| neu02 | neutral | scored | 1 | True | 1 | True | 1 | trained class (emotion2vec+) |
| lov01 | love | n/a | 0.0132 | False | 0 | False | n/a | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| lov02 | love | n/a | 0.0193 | False | 0 | False | n/a | love_seductive clap-prompt-ensemble (gender-conditioned mean |
| lau01 | laughter | n/a | 0.3795 | True | 0.3795 | True | n/a | efficientat |
| lau02 | laughter | n/a | 0.2433 | True | 0.2433 | True | n/a | efficientat |

## Samantha love-target sensitivity (toml unchanged)

own-samantha.toml love target remains 0.8109382317899566 (measured with universal CLAP; target_measured_by=universal-clap). Toml was NOT edited. Agent 2 final love_seductive reports 0.9228583200845187 on Samantha original love window. Below: gate recomputed by swapping only the love strength weight in category_blend; per-line match scores unchanged.

- Toml love target: `0.8109382317899566`
- New-detector love window: `0.9228583200845187`
- As-run detector strength on same w03 window during this rescore: `0.9390544380026881` (reported alongside Agent 2's 0.9228583200845187; toml still 0.8109382317899566).
- Plugin methods: `{'love': 'gender_mean_vs_neu_mean_t20_sadgate_align', 'seductive': 'gender_mean_vs_neu_mean_t20_sadgate', 'detail': 'gender-conditioned prompt bank vs neutral bank, softmax temp 20; seductive × (1 - e2v sad); love gated at mean cosine > 0.20'}`

| set | as-run em (detector strengths) | as-run gate | em @ toml love 0.8109 | gate @ 0.8109 | em @ new-det 0.9228583200845187 | gate @ 0.9228 |
|---|---|---|---|---|---|---|
| ceiling | 0.865031 | PASS | 0.869825 | PASS | 0.865619 | PASS |
| merge-v2 | 0.686945 | FAIL | 0.700773 | FAIL | 0.68864 | FAIL |
| coherent | 0.67137 | FAIL | 0.685027 | FAIL | 0.673044 | FAIL |

## Notes

- Out-of-persona emotions are **n/a**, never scored as 0.
- Samantha probe is missing aggressive/love/laughter wavs (20 lines); Jarvis has 26.
- Old probe `primary_detect_rate` in SUMMARY.json for Samantha seductive said 0.0 but per-line CLAP caught sed01+sed04 (≥0.2); this comparison uses per-line primary scores.
- Clone emotion_match only includes emotions with a present moment on the original (≥ presence) that are also in the persona set.
