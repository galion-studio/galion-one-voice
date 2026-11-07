#!/usr/bin/env python3
"""Python cross-check of the Rust love/warmth diagnostic.

The decision lives in pipeline/expression (galion-expression). This module
repeats the same thresholds and the same branch order so a drift fails the
cross-check. It does not load CLAP, emotion2vec, or any audio.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping, Optional, Sequence

GATE = 0.83
ALIGN_MIN = 0.20
CATEGORY_WEIGHT = 0.70
EMB_WEIGHT = 0.30
MARGIN_COLLAPSE = 0.04
COSINE_NEAR = 0.03
COSINE_COLLAPSE = 0.05
TILT_BRIGHTER_DB_OCT = 1.5
ENERGY_LOUDER_DB = 3.0
F0_FLATTER_ST = 0.8
SAD_GATE = 0.5
REF_LOVE_GATE_DECISION = 0.9390544380026881

EXCLUDED = ("crying", "yelling")


def margin(obs: Mapping[str, Any]) -> Optional[float]:
    love = obs.get("love_mean_cos")
    neu = obs.get("neutral_mean_cos")
    if love is None or neu is None:
        return None
    return float(love) - float(neu)


def category_blend(rows: Sequence[Mapping[str, Any]]) -> Optional[float]:
    num = den = 0.0
    for row in rows:
        w = max(float(row["strength"]), 1e-12)
        num += w * float(row["score"])
        den += w
    if den <= 0.0:
        return None
    return num / den


def emotion_match(rows: Sequence[Mapping[str, Any]], emb_sim: float) -> Optional[float]:
    cat = category_blend(rows)
    if cat is None:
        return None
    return CATEGORY_WEIGHT * cat + EMB_WEIGHT * float(emb_sim)


def attribute_love(rows: Sequence[Mapping[str, Any]], emb_sim: float, gate: float = GATE) -> dict:
    actual = emotion_match(rows, emb_sim)
    love = next(r for r in rows if r["category"] == "love")
    perfect = [
        {**r, "score": 1.0} if r["category"] == "love" else dict(r) for r in rows
    ]
    dropped = [r for r in rows if r["category"] != "love"]
    with_love = emotion_match(perfect, emb_sim)
    without = emotion_match(dropped, emb_sim)
    return {
        "emotion_match": actual,
        "pass": actual + 1e-12 >= gate,
        "emotion_match_if_love_matched": with_love,
        "pass_if_love_matched": with_love + 1e-12 >= gate,
        "emotion_match_without_love": without,
        "love_score": float(love["score"]),
        "love_strength": float(love["strength"]),
    }


def _prosody_state(reference: Mapping[str, Any], clone: Mapping[str, Any]):
    cues = []
    saw = False
    if reference.get("spectral_tilt_db_oct") is not None and clone.get("spectral_tilt_db_oct") is not None:
        saw = True
        if float(clone["spectral_tilt_db_oct"]) - float(reference["spectral_tilt_db_oct"]) > TILT_BRIGHTER_DB_OCT:
            cues.append("spectral_tilt_brighter")
    if reference.get("rms_db") is not None and clone.get("rms_db") is not None:
        saw = True
        if float(clone["rms_db"]) - float(reference["rms_db"]) > ENERGY_LOUDER_DB:
            cues.append("energy_louder")
    if reference.get("f0_sd_st") is not None and clone.get("f0_sd_st") is not None:
        saw = True
        if float(reference["f0_sd_st"]) - float(clone["f0_sd_st"]) > F0_FLATTER_ST:
            cues.append("f0_contour_flatter")
    if not saw:
        return "unknown", cues
    tilt = "spectral_tilt_brighter" in cues
    if not cues:
        return "held", cues
    if tilt or len(cues) >= 2:
        return "lost", cues
    return "partial", cues


def _resolve_decision(reference: Mapping[str, Any], clone: Mapping[str, Any]) -> float:
    if clone.get("decision") is not None:
        return float(clone["decision"])
    if clone.get("match_score") is not None and reference.get("decision") is not None:
        implied = float(reference["decision"]) - (1.0 - float(clone["match_score"]))
        if implied >= -1e-6:
            return max(0.0, implied)
    return float("nan")


def diagnose(reference: Mapping[str, Any], clone: Mapping[str, Any]) -> dict:
    reference_decision = float(reference.get("decision") if reference.get("decision") is not None else REF_LOVE_GATE_DECISION)
    clone_decision = _resolve_decision(reference, clone)
    if clone.get("match_score") is not None:
        match_score = float(clone["match_score"])
    elif clone_decision == clone_decision:
        match_score = max(0.0, 1.0 - abs(clone_decision - reference_decision))
    else:
        match_score = float("nan")
    margin_reference = margin(reference)
    margin_clone = margin(clone)
    if reference.get("prosody") and clone.get("prosody"):
        state, cues = _prosody_state(reference["prosody"], clone["prosody"])
    else:
        state, cues = "unknown", []
    hard_zero = clone_decision == clone_decision and clone_decision <= 1e-6 and reference_decision >= 0.5
    love_cos = clone.get("love_mean_cos")
    align_pass = love_cos is not None and float(love_cos) > ALIGN_MIN
    align_fail = love_cos is not None and float(love_cos) <= ALIGN_MIN
    margin_collapsed = (
        margin_reference is not None
        and margin_clone is not None
        and margin_reference - margin_clone > MARGIN_COLLAPSE
    )
    margin_held = (
        margin_reference is not None
        and margin_clone is not None
        and margin_clone + MARGIN_COLLAPSE >= margin_reference
    )
    cos_near = (
        reference.get("love_mean_cos") is not None
        and love_cos is not None
        and abs(float(reference["love_mean_cos"]) - float(love_cos)) <= COSINE_NEAR
    )
    cos_collapsed = (
        reference.get("love_mean_cos") is not None
        and love_cos is not None
        and float(reference["love_mean_cos"]) - float(love_cos) > COSINE_COLLAPSE
    )
    has_margin = margin_reference is not None and margin_clone is not None
    decision_gap = reference_decision - clone_decision if clone_decision == clone_decision else float("nan")
    sad = clone.get("e2v_sad")

    if sad is not None and float(sad) > SAD_GATE and align_pass and hard_zero:
        verdict, evidence, detail = (
            "detector_miss",
            ["sad_gate", "align_would_pass"],
            "CLAP still clears the 0.20 love-align floor, and emotion2vec+ sad multiplied the contrast down to zero.",
        )
    elif align_pass and hard_zero:
        verdict, evidence, detail = (
            "detector_miss",
            ["decision_zeroed_above_align"],
            "Mean love cosine is above 0.20, so the align gate would keep the contrast. The stored decision is zero.",
        )
    elif margin_held and hard_zero:
        verdict, evidence, detail = (
            "detector_miss",
            ["margin_held", "hard_zero"],
            "The love-minus-neutral prompt margin still matches the reference, and the decision was clipped to zero.",
        )
    elif cos_near and hard_zero:
        verdict, evidence, detail = (
            "detector_miss",
            ["cosine_near_reference", "hard_zero"],
            "Love-bank cosine stayed next to the reference cosine, and the decision was clipped to zero.",
        )
    elif state == "held" and (margin_collapsed or align_fail or cos_collapsed):
        verdict, evidence, detail = (
            "detector_miss",
            ["prosody_held"],
            "F0 contour, level, and spectral tilt still match the reference warmth. The prompt-bank score moved.",
        )
    elif (margin_collapsed or cos_collapsed) and state == "lost":
        verdict, evidence, detail = (
            "real_loss",
            ["clap_margin", "prosody"],
            "The love prompt margin collapsed and the voice colour moved off the reference (brighter tilt, louder level, or a flatter F0 contour).",
        )
    elif (margin_collapsed or cos_collapsed) and state == "partial":
        verdict, evidence, detail = (
            "mixed",
            ["clap_margin", "prosody_partial"],
            "The prompt margin collapsed and only one non-tilt prosody cue moved. That is not enough to call it purely acoustic or purely a detector clip.",
        )
    elif (margin_collapsed or cos_collapsed) and state == "unknown":
        verdict, evidence, detail = (
            "real_loss",
            ["clap_margin"],
            "The love prompt-bank margin collapsed against the reference. The align gate's hard zero follows that embedding gap. Prosody was not measured.",
        )
    elif state == "lost" and not has_margin:
        verdict, evidence, detail = (
            "real_loss",
            ["prosody"],
            "Voice colour left the reference warmth. Prompt-bank cosines were not measured.",
        )
    elif state == "held" and not has_margin and hard_zero:
        verdict, evidence, detail = (
            "detector_miss",
            ["prosody_held", "hard_zero"],
            "Prosody still matches the reference and the love decision is zero. Prompt-bank cosines were not measured.",
        )
    elif not has_margin and state == "unknown" and hard_zero:
        verdict, evidence, detail = (
            "insufficient_evidence",
            ["hard_zero"],
            "The clone love decision is 0 against a reference near 0.94, which is what the 0.20 align gate writes. Pass love/neutral cosines or prosody to separate a real loss from that clip.",
        )
    elif decision_gap == decision_gap and abs(decision_gap) <= 0.05:
        verdict, evidence, detail = (
            "held",
            ["decision_close"],
            "Clone love decision stays within 0.05 of the reference.",
        )
    else:
        verdict, evidence, detail = (
            "mixed",
            ["unclassified"],
            "The measurements disagree. Inspect the margin and the prosody cues together.",
        )
    if align_fail:
        evidence.append("align_gate")
    if hard_zero and "hard_zero" not in evidence:
        evidence.append("hard_zero")
    return {
        "label": clone.get("label"),
        "verdict": verdict,
        "evidence": evidence,
        "detail": detail,
        "reference_decision": reference_decision,
        "clone_decision": clone_decision if clone_decision == clone_decision else 0.0,
        "match_score": match_score if match_score == match_score else 0.0,
        "margin_reference": margin_reference,
        "margin_clone": margin_clone,
        "prosody_cues": cues,
    }


def channel_allowed(persona_id: str, judged: Sequence[str], emotion: str) -> bool:
    if emotion in EXCLUDED:
        return False
    if emotion == "seductive" and "seductive" not in judged:
        return False
    return emotion in judged and (emotion != "seductive" or persona_id == "own-samantha" or "seductive" in judged)


def load_persona_policy(path: Path) -> dict:
    import tomllib

    doc = tomllib.loads(path.read_text())
    judged = []
    for emo in doc.get("emotions") or []:
        judged.append(emo["id"])
        judged.extend(emo.get("aliases") or [])
        if emo["id"] in ("joyful", "happy"):
            judged.extend(["joyful", "happy"])
        if emo["id"] in ("logic", "reason"):
            judged.extend(["logic", "reason"])
    # unique, stable
    seen = []
    for name in judged:
        if name not in seen:
            seen.append(name)
    return {"id": doc["id"], "gate": float(doc.get("gate", GATE)), "judged": seen}
