# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Offline nebula score. A clip is a distribution, not one label.

The support is the twelve Emotion V2 emotions, in inventory order.
Samantha adds ``seductive`` as the last bin. Other personas omit that
name. It is not entered as zero.

This module does not read audio, call a model, or choose a threshold.
It does not change ``emotion_match`` (``0.70 * category_blend + 0.30 * emb_sim``)
or the 0.83 rollout gate. Those stay on the persona and universal scorers.
"""

from __future__ import annotations

import math
from typing import Any, Mapping, Sequence

# Inventory order in emotion-v2/emotions.toml. Unit spacing on this order is
# the stated 1-D ground distance for earth-mover distance. It is a reporting
# convention. It is not a claim that the emotions lie on a perceptual line.
EMOTION_V2: tuple[str, ...] = (
    "neutral",
    "calm",
    "happy",
    "amused",
    "excited",
    "tender",
    "sad",
    "disappointed",
    "angry",
    "fearful",
    "surprised",
    "disgusted",
)
SEDUCTIVE = "seductive"
VAD_AXES: tuple[str, ...] = ("valence", "arousal", "dominance")
SCHEMA = "galion-emotion-nebula/1"
# Positions are 0, 1, ..., n-1. Adjacent inventory names are distance 1.
GROUND_DISTANCE = "emotion-v2 inventory order, unit spacing"


def labels_for(*, samantha: bool) -> tuple[str, ...]:
    if samantha:
        return EMOTION_V2 + (SEDUCTIVE,)
    return EMOTION_V2


def js_divergence(p: Sequence[float], q: Sequence[float]) -> float:
    """Jensen–Shannon divergence, log base 2. Range is [0, 1]."""
    if len(p) != len(q) or not p:
        raise ValueError("distributions must be the same non-empty length")
    mid = [0.5 * (float(a) + float(b)) for a, b in zip(p, q)]
    return 0.5 * _kl(p, mid) + 0.5 * _kl(q, mid)


def wasserstein_1d(p: Sequence[float], q: Sequence[float]) -> float:
    """1-D earth mover's distance on unit-spaced bins 0 .. n-1.

    W1 = Σ_i |CDF_p(i) − CDF_q(i)| for every bin except the last, where both
    CDFs are 1. Moving all mass from the first bin to the next costs 1.
    """
    if len(p) != len(q) or not p:
        raise ValueError("distributions must be the same non-empty length")
    acc_p = 0.0
    acc_q = 0.0
    distance = 0.0
    last = len(p) - 1
    for i, (a, b) in enumerate(zip(p, q)):
        acc_p += float(a)
        acc_q += float(b)
        if i < last:
            distance += abs(acc_p - acc_q)
    return distance


def wasserstein_1d_samples(left: Sequence[float], right: Sequence[float]) -> float:
    """1-D earth mover's distance between two equal-weight empirical samples."""
    if not left or not right:
        raise ValueError("each VAD side needs at least one rating")
    return _earth_mover_points(
        [(float(x), 1.0 / len(left), 0.0) for x in left]
        + [(float(y), 0.0, 1.0 / len(right)) for y in right]
    )


def vad_euclidean(left: Sequence[float], right: Sequence[float]) -> float:
    if len(left) != 3 or len(right) != 3:
        raise ValueError("VAD is valence, arousal, dominance")
    return math.sqrt(sum((float(a) - float(b)) ** 2 for a, b in zip(left, right)))


def score_nebula(
    reference: Mapping[str, Any],
    rendered: Mapping[str, Any],
    *,
    samantha: bool,
) -> dict[str, Any]:
    """Compare one reference payload with one rendered payload."""
    return score_nebula_set([reference], [rendered], samantha=samantha)


def score_nebula_set(
    references: Sequence[Mapping[str, Any]],
    rendered: Sequence[Mapping[str, Any]],
    *,
    samantha: bool,
) -> dict[str, Any]:
    """Mean the clip distributions (equal weight per clip) and compare them.

    VAD Euclidean distance uses the mean of the per-clip VAD means, so each
    clip counts once. Per-axis Wasserstein pools the ratings themselves, so
    each rating counts once.
    """
    if not references or not rendered:
        raise ValueError("reference and rendered each need at least one payload")
    labels = labels_for(samantha=samantha)
    ref_rows = [_parse_payload(item, labels) for item in references]
    ren_rows = [_parse_payload(item, labels) for item in rendered]
    ref_dist = _mean_rows([row[0] for row in ref_rows])
    ren_dist = _mean_rows([row[0] for row in ren_rows])
    ref_vad = _mean_rows([row[1] for row in ref_rows])
    ren_vad = _mean_rows([row[1] for row in ren_rows])
    ref_samples = [sample for row in ref_rows for sample in row[2]]
    ren_samples = [sample for row in ren_rows for sample in row[2]]
    per_axis_w = {
        axis: wasserstein_1d_samples(
            [sample[i] for sample in ref_samples],
            [sample[i] for sample in ren_samples],
        )
        for i, axis in enumerate(VAD_AXES)
    }
    report = _shell(samantha=samantha, status="scored")
    report.update(
        {
            "js_divergence": js_divergence(ref_dist, ren_dist),
            "wasserstein_1d": wasserstein_1d(ref_dist, ren_dist),
            "vad_distance": vad_euclidean(ref_vad, ren_vad),
            "vad_abs": {
                axis: abs(ref_vad[i] - ren_vad[i]) for i, axis in enumerate(VAD_AXES)
            },
            "vad_wasserstein_1d": per_axis_w,
            "reference_distribution": dict(zip(labels, ref_dist)),
            "rendered_distribution": dict(zip(labels, ren_dist)),
            "per_emotion_abs": {
                label: abs(a - b) for label, a, b in zip(labels, ref_dist, ren_dist)
            },
            "reference_vad": dict(zip(VAD_AXES, ref_vad)),
            "rendered_vad": dict(zip(VAD_AXES, ren_vad)),
            "n_reference": len(references),
            "n_rendered": len(rendered),
            "n_reference_ratings": len(ref_samples),
            "n_rendered_ratings": len(ren_samples),
            "error": None,
        }
    )
    return report


def field_for_clips(
    *,
    samantha: bool,
    rendered_recs: Sequence[Mapping[str, Any]] | None = None,
    reference_recs: Sequence[Mapping[str, Any]] | None = None,
    reference: Mapping[str, Any] | Sequence[Mapping[str, Any]] | None = None,
) -> dict[str, Any]:
    """Report field that sits beside ``emotion_match``.

    Missing payloads stay a report. They do not raise and they do not change
    the gated float. A bad payload is ``status=error`` with the distances empty.
    """
    try:
        ref_state, ref_payloads = _payloads_from_reference(reference, reference_recs)
        ren_state, ren_payloads = _payloads_from_recs(rendered_recs)
    except (TypeError, ValueError, KeyError) as exc:
        return _failed(samantha, str(exc))
    if ref_state == "partial" or ren_state == "partial":
        return _failed(
            samantha,
            "every clip in a scored side needs a nebula payload; a subset is not averaged",
        )
    if ref_state != "full" and ren_state != "full":
        return _shell(samantha=samantha, status="absent")
    if ref_state != "full":
        return _shell(samantha=samantha, status="reference-absent")
    if ren_state != "full":
        return _shell(samantha=samantha, status="render-absent")
    try:
        return score_nebula_set(ref_payloads, ren_payloads, samantha=samantha)
    except (TypeError, ValueError, KeyError) as exc:
        return _failed(samantha, str(exc))


def _kl(p: Sequence[float], q: Sequence[float]) -> float:
    total = 0.0
    for a, b in zip(p, q):
        aa = float(a)
        if aa == 0.0:
            continue
        bb = float(b)
        if bb <= 0.0:
            raise ValueError("Jensen-Shannon mixture has a zero where mass is positive")
        total += aa * math.log2(aa / bb)
    return total


def _earth_mover_points(events: Sequence[tuple[float, float, float]]) -> float:
    """Events are (position, reference mass, rendered mass). Each side sums to 1."""
    merged: list[list[float]] = []
    for pos, mass_p, mass_q in sorted(events, key=lambda item: item[0]):
        if merged and merged[-1][0] == pos:
            merged[-1][1] += mass_p
            merged[-1][2] += mass_q
        else:
            merged.append([pos, mass_p, mass_q])
    if len(merged) == 1:
        return 0.0
    cdf_p = 0.0
    cdf_q = 0.0
    distance = 0.0
    for i, (pos, mass_p, mass_q) in enumerate(merged[:-1]):
        cdf_p += mass_p
        cdf_q += mass_q
        distance += abs(cdf_p - cdf_q) * (merged[i + 1][0] - pos)
    return distance


def _parse_payload(
    payload: Mapping[str, Any], labels: tuple[str, ...]
) -> tuple[list[float], list[float], list[list[float]]]:
    if not isinstance(payload, Mapping):
        raise TypeError("nebula payload must be an object with weights and vad")
    weights = payload.get("weights")
    if not isinstance(weights, Mapping):
        raise ValueError("nebula payload needs a weights object")
    unknown = sorted(set(weights) - set(labels))
    missing = sorted(set(labels) - set(weights))
    if unknown or missing:
        raise ValueError(
            "weights must name exactly "
            + ", ".join(labels)
            + (f"; unknown: {', '.join(unknown)}" if unknown else "")
            + (f"; missing: {', '.join(missing)}" if missing else "")
        )
    raw = []
    for label in labels:
        value = float(weights[label])
        if not math.isfinite(value) or value < 0.0:
            raise ValueError(f"weight for {label} must be finite and >= 0")
        raw.append(value)
    total = sum(raw)
    if total <= 0.0:
        raise ValueError("distribution has no mass")
    dist = [value / total for value in raw]
    samples = _vad_samples(payload)
    mean = _mean_rows(samples)
    return dist, mean, samples


def _vad_samples(payload: Mapping[str, Any]) -> list[list[float]]:
    samples_raw = payload.get("vad_samples")
    point = payload.get("vad")
    if samples_raw is None and point is None:
        raise ValueError("nebula payload needs vad or vad_samples")
    samples: list[list[float]] = []
    if samples_raw is not None:
        if not isinstance(samples_raw, Sequence) or isinstance(samples_raw, (str, bytes)):
            raise TypeError("vad_samples must be a list of VAD ratings")
        if len(samples_raw) == 0:
            raise ValueError("vad_samples is empty")
        for item in samples_raw:
            samples.append(_vad_triple(item))
    if point is not None:
        triple = _vad_triple(point)
        if samples:
            mean = _mean_rows(samples)
            for axis, got, expected in zip(VAD_AXES, triple, mean):
                if abs(got - expected) > 1e-5:
                    raise ValueError(
                        f"vad.{axis} disagrees with the mean of vad_samples"
                    )
        else:
            samples.append(triple)
    return samples


def _vad_triple(raw: Any) -> list[float]:
    if isinstance(raw, Mapping):
        try:
            values = [raw[axis] for axis in VAD_AXES]
        except KeyError as exc:
            raise ValueError("vad needs valence, arousal, and dominance") from exc
    elif isinstance(raw, Sequence) and not isinstance(raw, (str, bytes)) and len(raw) == 3:
        values = list(raw)
    else:
        raise TypeError("vad must be {valence, arousal, dominance} or a length-3 list")
    out = []
    for axis, value in zip(VAD_AXES, values):
        number = float(value)
        if not math.isfinite(number):
            raise ValueError(f"vad.{axis} must be finite")
        out.append(number)
    return out


def _mean_rows(rows: Sequence[Sequence[float]]) -> list[float]:
    width = len(rows[0])
    count = float(len(rows))
    return [sum(row[i] for row in rows) / count for i in range(width)]


def _payloads_from_recs(
    recs: Sequence[Mapping[str, Any]] | None,
) -> tuple[str, list[Mapping[str, Any]]]:
    if not recs:
        return "none", []
    found: list[Mapping[str, Any]] = []
    for rec in recs:
        if not isinstance(rec, Mapping) or rec.get("nebula") is None:
            continue
        payload = rec["nebula"]
        if not isinstance(payload, Mapping):
            raise TypeError("nebula on a clip must be an object")
        found.append(payload)
    if not found:
        return "none", []
    if len(found) != len(recs):
        return "partial", found
    return "full", found


def _payloads_from_reference(
    reference: Mapping[str, Any] | Sequence[Mapping[str, Any]] | None,
    reference_recs: Sequence[Mapping[str, Any]] | None,
) -> tuple[str, list[Mapping[str, Any]]]:
    if reference is None:
        return _payloads_from_recs(reference_recs)
    if isinstance(reference, Mapping) and "weights" in reference:
        return "full", [reference]
    if isinstance(reference, Mapping) and "nebula" in reference:
        payload = reference["nebula"]
        if not isinstance(payload, Mapping):
            raise TypeError("nebula reference must be an object")
        return "full", [payload]
    if isinstance(reference, Sequence) and not isinstance(reference, (str, bytes)):
        payloads: list[Mapping[str, Any]] = []
        for item in reference:
            if not isinstance(item, Mapping):
                raise TypeError("nebula reference list items must be objects")
            if "weights" in item:
                payloads.append(item)
            elif isinstance(item.get("nebula"), Mapping):
                payloads.append(item["nebula"])
            else:
                raise ValueError("nebula reference item needs weights")
        if not payloads:
            raise ValueError("nebula reference list is empty")
        return "full", payloads
    raise TypeError("nebula reference must be a payload, a clip, or a list of those")


def _shell(*, samantha: bool, status: str) -> dict[str, Any]:
    return {
        "schema": SCHEMA,
        "role": "offline-additional",
        "status": status,
        "replaces_emotion_match": False,
        "affects_emotion_match": False,
        "affects_gate": False,
        "labels": list(labels_for(samantha=samantha)),
        "seductive": "included" if samantha else "omitted",
        "js_log_base": 2,
        "ground_distance": GROUND_DISTANCE,
        "vad_metric": "euclidean of per-clip VAD means",
        "js_divergence": None,
        "wasserstein_1d": None,
        "vad_distance": None,
        "error": None,
    }


def _failed(samantha: bool, message: str) -> dict[str, Any]:
    report = _shell(samantha=samantha, status="error")
    report["error"] = message
    return report
