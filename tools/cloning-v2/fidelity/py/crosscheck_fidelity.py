#!/usr/bin/env python3
"""Cross-check galion-fidelity closed-form numbers. Rust is the implementation.

Reads the JSON from `galion-fidelity self-check` and recomputes the embedding
average, segment score, WER, MOS proxy components, and persona emotion_match.
No audio, no weights, no network.
"""

from __future__ import annotations

import json
import math
import sys


def l2(v):
    return math.sqrt(sum(x * x for x in v))


def l2_normalize(v):
    n = l2(v)
    if n < 1e-12:
        return [0.0] * len(v)
    return [x / n for x in v]


def average_embeddings(vecs, weights):
    dim = next((len(v) for v in vecs if v), 0)
    if dim == 0:
        return []
    acc = [0.0] * dim
    wsum = 0.0
    for i, vec in enumerate(vecs):
        if len(vec) != dim:
            continue
        n = l2(vec)
        if n < 1e-12:
            continue
        w = 1.0 if weights is None else max(0.0, float(weights[i]))
        if w == 0.0:
            continue
        for k, x in enumerate(vec):
            acc[k] += w * (x / n)
        wsum += w
    if wsum == 0.0:
        return [0.0] * dim
    return l2_normalize([x / wsum for x in acc])


def segment_score(snr_db, voiced, coverage):
    snr_n = min(1.0, max(0.0, snr_db / 30.0))
    return 0.50 * snr_n + 0.30 * min(1.0, max(0.0, voiced)) + 0.20 * min(1.0, max(0.0, coverage))


def emotion_match(category_blend, emb_sim):
    # galion_persona::emotion_match: 0.70 * category_blend + 0.30 * emb_sim
    return 0.70 * category_blend + 0.30 * emb_sim


def mos_from_components(snr_db, voiced, flatness, clip_fraction):
    snr_n = min(1.0, max(0.0, (snr_db - 5.0) / 25.0))
    voiced_n = min(1.0, max(0.0, voiced))
    tonal = min(1.0, max(0.0, 1.0 - min(1.0, max(0.0, flatness))))
    clip_pen = min(1.0, max(0.0, clip_fraction * 20.0))
    raw = min(1.0, max(0.0, 0.40 * snr_n + 0.35 * voiced_n + 0.25 * tonal - 0.30 * clip_pen))
    return 1.0 + 4.0 * raw


def normalize_words(text):
    out = []
    for ch in text.lower():
        out.append(ch if ("a" <= ch <= "z") or ch in ("'", " ") else " ")
    return "".join(out).split()


def word_error_rate(reference, hypothesis):
    ref = normalize_words(reference)
    hyp = normalize_words(hypothesis)
    if not ref:
        return None
    n, m = len(ref), len(hyp)
    prev = list(range(m + 1))
    for i in range(1, n + 1):
        curr = [i] + [0] * m
        for j in range(1, m + 1):
            cost = 0 if ref[i - 1] == hyp[j - 1] else 1
            curr[j] = min(prev[j] + 1, curr[j - 1] + 1, prev[j - 1] + cost)
        prev = curr
    return prev[m] / n


def close(a, b, tol=1e-9):
    if isinstance(a, list):
        return len(a) == len(b) and all(close(x, y, tol) for x, y in zip(a, b))
    if a is None or b is None:
        return a is None and b is None
    return abs(float(a) - float(b)) <= tol


def main(path):
    with open(path, encoding="utf-8") as handle:
        data = json.load(handle)
    closed = data["closed_form"]
    errors = []

    def check(name, got, expect, tol=1e-9):
        if not close(got, expect, tol):
            errors.append(f"{name}: python {got} != rust {expect}")

    # Embedding averages are f32 in Rust; 1e-6 covers that rounding.
    check(
        "embedding_average",
        average_embeddings(closed["vectors"], closed["weights"]),
        closed["embedding_average"],
        1e-6,
    )
    check(
        "embedding_average_weighted",
        average_embeddings(closed["vectors"], closed["weighted_weights"]),
        closed["embedding_average_weighted"],
        1e-6,
    )
    check(
        "segment_score",
        segment_score(closed["segment_snr_db"], closed["segment_voiced"], closed["segment_coverage"]),
        closed["segment_score"],
    )
    check("emotion_match", emotion_match(closed["category_blend"], closed["emb_sim"]), closed["emotion_match"])
    check("wer", word_error_rate(closed["wer_ref"], closed["wer_hyp"]), closed["wer"])
    check("wer_exact", word_error_rate(closed["wer_exact_ref"], closed["wer_exact_hyp"]), closed["wer_exact"])
    check(
        "mos",
        mos_from_components(closed["mos_snr_db"], closed["mos_voiced"], closed["mos_flatness"], closed["mos_clip"]),
        closed["mos"],
    )
    if closed["ecapa_at_threshold_passes"] is not False:
        errors.append("ECAPA at 0.7057 must fail the strict gate")
    if closed["ecapa_just_above_passes"] is not True:
        errors.append("ECAPA 0.7058 must pass")
    if closed["wer_at_threshold_passes"] is not True:
        errors.append("WER 1.8% must pass")
    if closed["wer_above_threshold_passes"] is not False:
        errors.append("WER above 1.8% must fail")
    if closed["emotion_at_gate_passes"] is not True:
        errors.append("emotion_match at 0.83 must pass")
    if abs(closed["emotion_gate"] - 0.83) > 1e-15:
        errors.append("persona gate drifted from 0.83")
    if data.get("consent_evaluations") != 0:
        errors.append("fidelity self-check must not evaluate consent")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("cross-check ok")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("usage: crosscheck_fidelity.py SELF_CHECK.json", file=sys.stderr)
        sys.exit(2)
    sys.exit(main(sys.argv[1]))
