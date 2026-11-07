# Nebula scoring

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Nebula scoring is an **additional offline report**. A clip is a distribution over the twelve Emotion V2 emotions, plus Samantha-only `seductive`, together with valence, arousal, and dominance. It is not one label.

It does not replace `emotion_match`. The rollout formula and the gate are unchanged:

```text
emotion_match = 0.70 * category_blend + 0.30 * emb_sim
gate: emotion_match ≥ 0.83
```

`category_blend` is still the strength-weighted mean of the per-emotion scores. `emb_sim` is still the mean-centred embedding cosine. Out-of-set persona emotions stay `n/a`. They are not written in as 0. No nebula distance is a pass/fail bar. CI is not retargeted. `tools/emotion-v2/thresholds.toml` is not edited.

The code is `tools/cloning-v2/scorers/persona/py/nebula.py`. The persona scorer puts the object in the `nebula` field immediately after `emotion_match` (`persona_lib.emotion_match`). The universal scorer does the same on `BASE-self-ceiling` and on each render set (`universal_match.py`). `persona_match.py` accepts `--nebula-reference` for an explicit payload. None of those paths read a wav for this field, and none of them download a model.

## Support

The twelve names, in the inventory order of `emotion-v2/emotions.toml`:

`neutral`, `calm`, `happy`, `amused`, `excited`, `tender`, `sad`, `disappointed`, `angry`, `fearful`, `surprised`, `disgusted`.

Samantha's report adds `seductive` as the last bin, because her persona judges it. Jarvis and any other persona omit that name. A Jarvis payload that includes `seductive` is an error. It is not stored as zero and it is not folded into the twelve. A Samantha payload that omits the key is an error. A Samantha weight of zero is allowed: the name is present and the mass is zero.

Weights are non-negative and finite. They are normalised to sum to 1. Zero total mass is an error. The key set must be exactly the support. This scorer does not map the four-class SUPERB head or the nine-class emotion2vec head onto these names. Those heads stay on the existing `emotion_match` path. A nebula payload is a distribution the caller already has (a soft label, a calibrated posterior, or a synthetic vector in a test).

## Distances

All three are reported. None of them is mixed back into `emotion_match`.

| Field | What it compares | Definition |
|---|---|---|
| `js_divergence` | Reference distribution and rendered distribution | Jensen–Shannon divergence, log base 2. Identical distributions are 0. Two disjoint one-hot distributions are 1. |
| `wasserstein_1d` | The same two distributions | 1-D earth mover's distance on the inventory order. Bins sit at 0, 1, …, n−1. Adjacent names are distance 1. `W1 = Σ |CDF_ref − CDF_rendered|` over every bin except the last. |
| `vad_distance` | Valence, arousal, dominance | Euclidean distance between the per-clip means. Each clip counts once. |

The inventory order is the stated ground distance so the earth-mover number is defined. It is a reporting convention. It is not a claim that the twelve emotions lie on a perceptual line, and it is not Russell's circumplex. Moving all of the mass from Neutral to Calm costs 1. Moving it from Neutral to Disgusted costs 11. Seductive, when it is present, sits one step past Disgusted.

`vad_abs` is the absolute gap on each axis of those means. `vad_wasserstein_1d` is the 1-D earth mover's distance on each axis between the pooled ratings. Each rating counts once, so a clip with more ratings weighs more there than it does in `vad_distance`. A single `{valence, arousal, dominance}` triple is one rating. When `vad_samples` is also present, its mean must match `vad` within 1e-5.

Several clips are averaged with equal weight per clip, then compared. A side that has a payload on only some of its clips is not averaged. The field comes back `status=error` and the distances stay empty. If neither side has a payload, `status` is `absent` and `emotion_match` is computed exactly as before.

Design coordinates in `emotions.toml` are not a reference. The VAD in a nebula payload is the measured (or, in tests, synthetic) VAD of that clip.

## Report

```json
{
  "emotion_match": 0.0,
  "nebula": {
    "schema": "galion-emotion-nebula/1",
    "role": "offline-additional",
    "status": "scored",
    "replaces_emotion_match": false,
    "affects_emotion_match": false,
    "affects_gate": false,
    "js_divergence": 0.0,
    "wasserstein_1d": 0.0,
    "vad_distance": 0.0
  }
}
```

The zeros in that sketch are an identical synthetic pair. They are not a Samantha measurement. `status` is `scored`, `absent`, `reference-absent`, `render-absent`, or `error`. `affects_gate` is always false. A large nebula distance does not flip `pass`. A nebula distance of zero does not clear a failing `emotion_match`.

A clip payload looks like this:

```json
{
  "weights": {
    "neutral": 1.0,
    "calm": 0.0,
    "happy": 0.0,
    "amused": 0.0,
    "excited": 0.0,
    "tender": 0.0,
    "sad": 0.0,
    "disappointed": 0.0,
    "angry": 0.0,
    "fearful": 0.0,
    "surprised": 0.0,
    "disgusted": 0.0
  },
  "vad": {"valence": 0.0, "arousal": 0.0, "dominance": 0.0}
}
```

Add `"seductive": 0.0` (or any non-negative mass) for Samantha. On a scored record the same object may sit under the key `nebula`.

## Tests

Synthetic distributions only. No audio is written, and no probe JSON is regenerated.

```bash
python3 -m unittest discover -s tools/cloning-v2/scorers/persona/py -p 'test_nebula.py' -v
```

The persona scorer tests next to that file are unchanged and still apply:

```bash
python3 -m unittest discover -s tools/cloning-v2/scorers/persona/py -p 'test_persona.py' -v
```

## What this file does not do

It does not edit `LOCKS.toml`, `SOUL.md`, `config.yaml`, a persona TOML, `tools/emotion-v2/thresholds.toml`, or `docs/emotion-v2/capture-spec.toml`. It does not lower 0.83. It does not commit audio or generate a dataset. It does not merge a branch or switch a live voice. Nebula, here, is the name of this offline metric. It is not a claim that emotion is a cloud of particles in the brain.
