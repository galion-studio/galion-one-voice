# Emotion-match calibration (existing renders only)
Backend: `galion-emotion` (pluggable). No new model fetched. No new renders.
Happy/joyful reference window (Samantha): **10.00–12.50 s** (FINDINGS 2.5s grid; config also forces PROFILE 9.947–12.434).

## Formula
`match = max(0, 1 − RMS_7(v_line − v_moment))` on (neu,hap,ang,sad,A,V,D);
`score_C = 0.55·max_i match_i + 0.45·mean_i match_i`;
`emotion_match = Σ strength_C·score_C / Σ strength_C` over detected moments.

## Per-render-set

| Set | WER | ECAPA | emotion_match |
|---|---:|---:|---:|
| samantha/BASE-self-ceiling | — | — | 0.8808 |
| jarvis/BASE-self-ceiling | — | — | 0.9409 |
| samantha/merge-v2 | 0.01764 | 0.7057 | 0.8389377911048609 |
| samantha/global-fix-coherent | 0.01455 | 0.7081 | 0.8164308027865333 |
| samantha/refprep | 0.02566 | 0.6739 | 0.8393563326851327 |
| samantha/refprep-baseline | 0.05026 | 0.7072 | 0.8190748500170649 |
| jarvis/merge-v2 | 0.01852 | 0.7331 | 0.9053534483134232 |
| jarvis/global-fix-coherent | 0.02963 | 0.7303 | 0.8799814008958347 |

## Recommended blended threshold

**emo_min = 0.83**

merge-v2 (current prod pre-fix) blended emotion_match=0.8389; coherent=0.8164 (REGRESSION of 0.0225). Self-ceiling on original windows=0.8808. Recommended emo_min=0.83 ≈ pre-fix − 0.01 noise margin: a candidate must not meaningfully regress vs the voice it is trying to improve. Coherent PASSES WER (0.01455≤0.018) and ECAPA (0.7081>0.7057) but FAILS emotion_match at 0.83. Per-category: coherent improves aggressive/yelling/fear (angry-tail) but worsens joyful/love/seductive/logic/neutral — exactly the happy-moment concern. Joyful soft min 0.84 (pre 0.859 − slack). sad has no moment in Samantha short windows (max P(sad)≈0.043 < presence 0.28). surprise/disgust not measurable with SUPERB/MSP.

### Per-emotion soft mins (diagnostic, not hard gates yet)

- joyful: 0.84
- aggressive: 0.79
- yelling: 0.79
- fear: 0.79
- logic: 0.77
- love: 0.76
- seductive: 0.76
- neutral: 0.78

### Measurability

- Direct: joyful/happy, aggressive, sad, neutral
- Proxy: yelling, fear, logic, reason, love, seductive
- Not measurable: surprise, disgust

### Coherent candidate vs three gates (existing box renders)

| Gate | Threshold | Coherent | Pass? |
|---|---:|---:|---|
| WER | ≤ 0.018 | 0.01455 | YES |
| ECAPA | > 0.7057 | 0.7081 | YES |
| emotion_match | ≥ 0.83 | 0.8164 | **NO** |
