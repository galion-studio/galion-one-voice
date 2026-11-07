# Universal emotion backend — report

Finished 2026-11-05 Europe/Zagreb. No git/deploy/prod voice writes. No new renders. Jarvis voice untouched.

## Models used (licences verified before download)

| Model | Role | Licence | URL | Notes |
|---|---|---|---|---|
| **emotion2vec+ base** | Trained 9-class SER + 768-d embedding | FunASR Model License **1.1** (commercial OK w/ attribution + keep model name) | https://huggingface.co/emotion2vec/emotion2vec_plus_base | ONNX: https://huggingface.co/ziyu12345/emotion2vec_plus_base_onnx (packaging MIT). Classes: angry, disgusted, fearful, happy, neutral, other, sad, surprised, unknown. |
| **EfficientAT mn10_as** | PRIMARY for yelling / crying / laughter | **MIT** | https://huggingface.co/Chris-Kode/efficientat-mn10-as-onnx | SHA `bec20620…`; 32 kHz; 527 AudioSet; Rust `ort` (`galion-efficientat`); windowed max/mean pooling |
| **LAION CLAP htsat-fused** | Zero-shot for love / seductive / logic / reason | **Apache-2.0** | https://huggingface.co/laion/clap-htsat-fused | Fallback allowed by brief. Yell/cry/laugh moved to EfficientAT. |
| microsoft/msclap 2023 | (intended, not runtime) | HF card **ms-pl** (commercial OK); code MIT | https://huggingface.co/microsoft/msclap | Blocked on this box: Python 3.13 has no `numpy<2` wheels; `msclap 1.3.4` pins `numpy>=1.23,<2` and `transformers<5`. See `scorers/universal/licenses/CLAP-CHOICE.md`. |

Weights live under `cloning-v2/scorers/universal/{emotion2vec,clap}/`.

## Implementation label (honest)

| Piece | How it runs tonight |
|---|---|
| emotion2vec+ ONNX | Python `onnxruntime` in `scorers/universal/py/score_universal.py` (same ONNX a Rust `ort` port will load). Registered as Rust `EmotionBackend` id=`universal` via subprocess. |
| EfficientAT ONNX | Rust `ort` binary `scorers/universal/bin/galion-efficientat` (windowed). Primary for yell/cry/laugh. |
| CLAP zero-shot | **Python cross-check** (`transformers.ClapModel`). love/seductive/logic/reason only. |
| Match / blend / gate math | Rust (`emotion_match.rs`: `mean_centred_cosine`, `blend_universal`) + Python batch `universal_match.py` for re-score. |
| Pitch/energy proxies | **Dropped from the gate**. Legacy SUPERB path may still report them as diagnostics only. |

## Formula (keep `emo_min = 0.83`)

```
score_C = 0.55·max_i match_i + 0.45·mean_i match_i
category_blend = Σ_C strength_C · score_C / Σ strength_C
emb_sim = mean_i [ max_j mean_centred_cosine(emb_line_i, emb_window_j) ]
          // EmoCtrlTTS-Eval / Emo-DPO: subtract per-vector mean, then cosine on emotion2vec 768-d
emotion_match = 0.70 · category_blend + 0.30 · emb_sim
```

- **Trained-class match:** `max(0, 1 − RMS_9(e2v_line, e2v_moment))`
- **Zero-shot match:** `max(0, 1 − |clap_C_line − clap_C_moment|)`
- Moment presence floor: **0.20** (e2v peaks are softer than SUPERB)

### Per-emotion source labels

| Emotion | Source |
|---|---|
| joyful / happy, aggressive, disgust, fear, sad, neutral, surprise | **trained class (emotion2vec+)** |
| love, seductive, logic, reason | **zero-shot text prompt (CLAP)** |
| yelling, crying, laughter | **efficientat** (AudioSet; PRIMARY) |

### Not measurable / not detected on these originals

- No open model has trained **love / seductive / logic / reason** classes — zero-shot only.
- On Samantha’s original 2.5 s windows, emotion2vec+ did **not** fire a joyful/fear/sad/surprise/disgust/yelling/seductive moment above presence 0.20 (happy window scores mostly *neutral* under e2v). Detected moments: **aggressive, neutral, love, logic**.
- Jarvis detected moments: **neutral, logic** only (fits the calm persona).

## Old vs new blended numbers (existing renders only)

### Samantha

| Set | Old (SUPERB blend) | New (universal) | @0.83 |
|---|---:|---:|---|
| BASE self-ceiling | 0.8808 | **0.9016** | PASS |
| merge-v2 (prod pre-fix) | 0.8389 | **0.7370** | **FAIL** |
| global-fix coherent | 0.8164 | **0.7098** | **FAIL** |

Coherent still sits below merge-v2 on the new scale (Δ −0.027). Absolute levels dropped vs SUPERB because e2v+CLAP see different structure (happy window ≠ happy class).

### Jarvis (full re-score, voice untouched)

| Set | Old (SUPERB blend) | New (universal) | @0.83 |
|---|---:|---:|---|
| BASE self-ceiling | 0.9409 | **0.9751** | PASS |
| merge-v2 | 0.9054 | **0.8763** | **PASS** |
| global-fix coherent | 0.8800 | **0.8812** | **PASS** |

## Per-emotion leaderboards (new)

### Samantha merge-v2
1. neutral 0.9689 [trained] best=t12  
2. logic 0.9248 [CLAP] best=t08  
3. aggressive 0.5555 [trained] best=t07  
4. love 0.3070 [CLAP] best=t05  

### Samantha coherent
1. neutral 0.9646 [trained] best=t03  
2. logic 0.8973 [CLAP] best=t07  
3. aggressive 0.5288 [trained] best=t07  
4. love 0.2913 [CLAP] best=t04  

### Jarvis merge-v2
1. neutral 0.9650 [trained] best=t09  
2. logic 0.7932 [CLAP] best=t03  

### Jarvis coherent
1. neutral 0.9823 [trained] best=t02  
2. logic 0.7301 [CLAP] best=t11  

Raw JSON: `scorers/universal/results/{samantha,jarvis}-universal/RESULTS.json`.

## Gate recommendation

- Keep **`emo_min = 0.83`** as ordered.
- Under the **universal** metric, **neither Samantha candidate passes** emotion_match at 0.83 (merge-v2 0.737, coherent 0.710). Both Jarvis sets **pass**.
- WER/ECAPA gates unchanged. Samantha coherent still passes WER+ECAPA on existing numbers but fails the new emotion gate harder than before.
- Owner may later re-calibrate `emo_min` on the universal scale; not done here.

## Tests

`galion-voicecheck`: **30 passed / 0 failed** (was 27; + `mean_centred_cosine`, `blend_universal`, `resolve("universal")`).

## Paths

- Scorer: `cloning-v2/scorers/universal/`
- Licences: `scorers/universal/licenses/`
- Backend trait: `B-check/galion-voicecheck/src/emo_backend.rs` (`UniversalBackend`)
- Config: `config/voices/own-samantha.json` → `"emotion_backend": "universal"`

---

## EfficientAT PRIMARY update (2026-11-05 Europe/Zagreb)

Model: `Chris-Kode/efficientat-mn10-as-onnx` (MIT). ONNX sha256 `bec20620ab8be5e41297b45845f6fb116fbe42cd4da339ad7d44f27630b7a504` (20,200,590 bytes). Attribution: EfficientAT mn10_as © 2022 Florian Schmid; ONNX export Chris-Kode.

Class indices (verified vs AudioSet `class_labels_indices.csv`):

| State | Indices | Notes |
|---|---|---|
| yelling | Shout=8, Yell=11, Screaming=14, Children shouting=13 | max over family; Battle cry=12 excluded |
| crying | Crying, sobbing=22 | Baby cry, infant cry=23 **excluded** (infant-specific; would confound adult Sam/Jarvis) |
| laughter | Laughter=16, Baby laughter=17, Giggle=18, Belly laugh=20, Chuckle, chortle=21 | max over subtypes |

Match: `max(0, 1 − |eat_C_line − eat_C_moment|)` with windowed max pooling (2.5 s / 1.25 s hop). Leaderboard source label: **`efficientat`**.

### Re-scored blended numbers (EfficientAT in blend; emo_min=0.83)

#### Samantha

| Set | Pre-EfficientAT universal | +EfficientAT | @0.83 |
|---|---:|---:|---|
| BASE self-ceiling | 0.9016 | **0.9044** | PASS |
| merge-v2 | 0.7370 | **0.7738** | **FAIL** |
| global-fix coherent | 0.7098 | **0.7509** | **FAIL** |

#### Jarvis (voice untouched)

| Set | Pre-EfficientAT universal | +EfficientAT | @0.83 |
|---|---:|---:|---|
| BASE self-ceiling | 0.9751 | **0.9475** | PASS |
| merge-v2 | 0.8763 | **0.8802** | **PASS** |
| global-fix coherent | 0.8812 | **0.8901** | **PASS** |

### Three-state scores (source=`efficientat`)

| Persona / set | yelling | crying | laughter |
|---|---:|---:|---:|
| Sam merge-v2 | **0.9376** (str 0.6654, best t03) | *(no moment ≥0.20; max window ≈0.16)* | **0.8542** (str 0.2744, best t09) |
| Sam coherent | **0.9231** (str 0.6654, best t12) | *(none)* | **0.9249** (str 0.2744, best t11) |
| Jarvis merge-v2 | **0.9055** (str 0.8088, best t10) | *(none)* | *(none ≥0.20)* |
| Jarvis coherent | **0.9095** (str 0.8088, best t06) | *(none)* | *(none ≥0.20)* |

Crying honestly absent above presence on both originals (adult speech content; baby-cry class deliberately not used).
