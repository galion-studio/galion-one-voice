# Persona scorer — interface

Opt-in sibling of `scorers/universal/`. The old scorer stays the default and is not edited. No deploy, merge or push.

This package (Agent 1) provides persona files, the detector hook, crying = emotion2vec+ sad, and the persona-aware 0.83 gate. Love/seductive *detection* is a plugin owned by Agent 2. Full rescore output is owned by Agent 3. Wiring into `pipeline/missing-emotion-targets` is owned by Agent 4.

## Layout

```
pipeline/personas/own-samantha.toml
pipeline/personas/own-jarvis.toml
scorers/persona/
  INTERFACE.md                 this file
  CRYING.md                    crying detector + probe false-positive counts
  detectors/                   Python detector package (the hook)
    base.py                    Detector protocol, DetectorScore, record helpers
    crying.py                  crying = e2v sad; EfficientAT crying logged only
    trained.py                 emotion2vec+ trained classes
    eat.py                     yelling + laughter (not crying)
    logic.py                   CLAP logic / reason
    legacy_clap_fallback.py    used only when no love_seductive plugin is present
    __init__.py                load_detectors(), discover_love_seductive()
  py/
    persona_lib.py             load TOML, n/a filtering, emotion_match()
    score_persona.py           file-scorer CLI
    persona_match.py           gate CLI (reuse cached universal JSON)
    test_persona.py            unit + SUMMARY.json fixture tests
  galion-persona/              Rust: load TOML, n/a, crying = e2v sad, gate math
```

## Persona files

Schema `galion-persona-emotion-targets/1`. A file lists only the emotions that voice is judged on. Anything not listed is **n/a**, never 0.

| Voice | Judged emotions | Explicitly out (n/a) |
|---|---|---|
| own-samantha | joyful (happy), aggressive, love, laughter, logic, neutral, seductive | yelling, crying, fear, surprise, disgust, … |
| own-jarvis | neutral, logic (reason) | **seductive**, love, joyful, laughter, yelling, crying, aggressive, … |

Seductive is Samantha-only. Gate = 0.83, presence = 0.20.

Target numbers come from the original ceiling window where the emotion is present, or from the best scorer-probe render where it was synthesized. love and seductive Samantha targets were measured with universal CLAP (`target_measured_by = "universal-clap"`); refresh them when the Agent 2 plugin lands. Assumptions and doc line citations live in the TOML files themselves.

`load_persona(path) -> Persona` (`py/persona_lib.py` and `galion_persona::load_persona`). Aliases: `happy` ↔ `joyful`, `reason` ↔ `logic`. `persona.in_set(category)`, `strength(category, rec, persona=...)` returns `"n/a"` outside the set.

## Detector hook (Agent 2)

Add either file:

- `scorers/persona/detectors/love_seductive.py`
- or any `scorers/persona/detectors/love_seductive*.py`

Export one of:

```python
from detectors.base import DetectorScore

class LoveSeductiveDetector:
    name = "love-seductive"
    categories = ("love", "seductive")
    def score(self, rec: dict) -> dict[str, DetectorScore]:
        # rec has e2v / clap / efficientat in score_universal OR scorer-probe SUMMARY shapes
        # helpers: detectors.e2v_probs(rec), clap_probs(rec), clap_cosine(rec), eat_max(rec)
        return {
            "love": DetectorScore(category="love", value=..., source="..."),
            "seductive": DetectorScore(category="seductive", value=..., source="..."),
        }

# or: def build_detector() -> Detector: ...
# or: DETECTOR = LoveSeductiveDetector()
```

`DetectorScore` fields: `category: str`, `value: float`, `source: str`, `used_for_decision: bool = True`, `extras: dict = {}`.

Discovery: `from detectors import discover_love_seductive, load_detectors, decision_scores`.

- `load_detectors(plugin_dir=None, include_legacy_love_seductive_fallback=True)`
- If at least one plugin loads, the legacy CLAP fallback is **not** used.
- Core order: crying, trained, eat (yelling/laughter), logic, then plugin-or-fallback. `EatDetector` does not emit `crying`, so it cannot overwrite emotion2vec+ sad.

Do not train on own-voice clips. MIT/Apache models only; weights stay local.

## Crying

Decision value = `e2v_probs(rec)["sad"]`. Source string: `"trained class (emotion2vec+ sad)"`. EfficientAT crying is stored as `extras["efficientat_crying_logged"]` and `extras["efficientat_crying_used_for_decision"] = False`. See `CRYING.md`.

## CLIs

Neither CLI is the default pipeline scorer.

### score_persona.py

```
scorers/persona/py/score_persona.py WAV [WAV ...] --out OUT.json
    [--persona own-samantha|own-jarvis|PATH.toml]
    [--threads 2] [--ref REF.wav]
    [--skip-clap] [--skip-e2v] [--skip-efficientat]
    [--reuse-json SCORE_UNIVERSAL.json]
    [--plugin-dir DIR]
```

`--reuse-json` overlays persona detectors on an existing `score_universal.py` JSON (no model reload). Output schema `galion-persona-emotion/1`. Each file gains `detectors` (all decision scores) and, if `--persona` is set, `persona_scores` (n/a outside the set).

### persona_match.py

```
scorers/persona/py/persona_match.py
    --persona own-samantha|own-jarvis|PATH.toml
    --out OUT.json
    [--reuse-windows base_windows_universal.json]
    [--reuse-lines name=lines_merge-v2.json]   # repeatable
    [--moments-json RESULTS.json]
    [--plugin-dir DIR]
```

If `--reuse-windows` is given without `--moments-json`, in-set moments are taken from those windows at presence 0.20. Output schema `galion-persona-match/1` with `BASE-self-ceiling` and `sets`.

Formula (same coefficients as universal, judged emotions only):

```
score_C = 0.55·max_i match_i + 0.45·mean_i match_i
category_blend = Σ strength_C · score_C / Σ strength_C     # in-set C only
emotion_match  = 0.70 · category_blend + 0.30 · emb_sim
pass           = emotion_match ≥ persona.gate (0.83)
```

Trained-class match is `max(0, 1 − RMS_9(e2v_line, e2v_moment))`. Other categories use `max(0, 1 − |decision_line − decision_moment|)`. `emb_sim` is mean-centred emotion2vec cosine against in-set moment windows.

## Python API

```python
import sys
sys.path.insert(0, "cloning-v2/scorers/persona")
sys.path.insert(0, "cloning-v2/scorers/persona/py")

from detectors import load_detectors, decision_scores, CryingDetector
from persona_lib import load_persona, resolve_persona_path, strength, apply_persona_scores, emotion_match, NA

persona = load_persona(resolve_persona_path("own-jarvis"))
assert strength("seductive", rec, persona=persona) == "n/a"
scored = decision_scores(rec)   # scored["crying"].value == e2v sad
```

## Rust API

Crate `scorers/persona/galion-persona` (`galion_persona`):

```rust
galion_persona::load_persona(path) -> Result<Persona, String>
persona.in_set("seductive")
galion_persona::e2v_sad_crying(&e2v_probs) -> f64
galion_persona::decision_strength(cat, e2v, clap, eat, Some(&persona)) -> NaOr::{Na, Value(f64)}
galion_persona::emotion_match(category_blend, emb_sim) -> f64   // 0.70 / 0.30
galion_persona::gate_pass(emotion_match, persona.gate) -> bool
galion_persona::GATE // 0.83
```

Agent 4 can depend on this crate additively. Do not change the default `--scorer` path (universal) until approved.

## Tests

```
python3 scorers/persona/py/test_persona.py
cargo test --offline --manifest-path scorers/persona/galion-persona/Cargo.toml
```
