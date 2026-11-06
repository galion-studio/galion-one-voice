"""Persona files, n/a filtering, and the 0.83 persona-aware gate.

Emotion ids match the universal catalog. Aliases: happy→joyful, reason→logic.
An emotion not listed on the persona is n/a (None), never 0.
"""
from __future__ import annotations

import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable, Mapping, Optional, Sequence

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from detectors import DetectorScore, decision_scores, load_detectors  # noqa: E402

ALIASES = {
    "happy": "joyful",
    "reason": "logic",
}
# reverse: catalog -> accepted aliases
GATE_DEFAULT = 0.83
PRESENCE_DEFAULT = 0.20
BLEND = 0.70
EMB = 0.30
NA = "n/a"

DEFAULT_PERSONAS = {
    "samantha": ROOT.parent.parent / "pipeline" / "personas" / "own-samantha.toml",
    "own-samantha": ROOT.parent.parent / "pipeline" / "personas" / "own-samantha.toml",
    "jarvis": ROOT.parent.parent / "pipeline" / "personas" / "own-jarvis.toml",
    "own-jarvis": ROOT.parent.parent / "pipeline" / "personas" / "own-jarvis.toml",
}


@dataclass
class EmotionSpec:
    id: str
    aliases: list[str] = field(default_factory=list)
    target: Optional[float] = None
    origin: Optional[str] = None
    detector: Optional[str] = None
    source_detail: Optional[str] = None
    docs: list[str] = field(default_factory=list)
    extra: dict[str, Any] = field(default_factory=dict)

    def names(self) -> set[str]:
        out = {self.id, *self.aliases}
        if self.id in ALIASES:
            out.add(ALIASES[self.id])
        for a, canon in ALIASES.items():
            if canon == self.id or a == self.id or canon in self.aliases or a in self.aliases:
                out.add(a)
                out.add(canon)
        return out


@dataclass
class Persona:
    schema: str
    id: str
    display_name: str
    gate: float = GATE_DEFAULT
    presence: float = PRESENCE_DEFAULT
    summary: str = ""
    docs_cited: list[dict[str, Any]] = field(default_factory=list)
    assumptions: list[dict[str, Any]] = field(default_factory=list)
    emotions: list[EmotionSpec] = field(default_factory=list)
    path: Optional[Path] = None

    def spec_for(self, category: str) -> Optional[EmotionSpec]:
        cat = ALIASES.get(category, category)
        for spec in self.emotions:
            if category in spec.names() or cat in spec.names():
                return spec
        return None

    def in_set(self, category: str) -> bool:
        return self.spec_for(category) is not None

    def judged_ids(self) -> list[str]:
        return [e.id for e in self.emotions]


def load_persona(path: str | Path) -> Persona:
    import tomllib

    p = Path(path)
    data = tomllib.loads(p.read_text())
    emotions = []
    for raw in data.get("emotions") or []:
        known = {
            "id", "aliases", "target", "origin", "detector",
            "source_detail", "docs", "target_measured_by",
        }
        extra = {k: v for k, v in raw.items() if k not in known}
        if raw.get("target_measured_by") is not None:
            extra["target_measured_by"] = raw["target_measured_by"]
        emotions.append(
            EmotionSpec(
                id=raw["id"],
                aliases=list(raw.get("aliases") or []),
                target=raw.get("target"),
                origin=raw.get("origin"),
                detector=raw.get("detector"),
                source_detail=raw.get("source_detail"),
                docs=list(raw.get("docs") or []),
                extra=extra,
            )
        )
    meta = data.get("persona") or {}
    return Persona(
        schema=data.get("schema", ""),
        id=data["id"],
        display_name=data.get("display_name", data["id"]),
        gate=float(data.get("gate", GATE_DEFAULT)),
        presence=float(data.get("presence", PRESENCE_DEFAULT)),
        summary=meta.get("summary", ""),
        docs_cited=list(data.get("docs_cited") or []),
        assumptions=list(data.get("assumptions") or []),
        emotions=emotions,
        path=p,
    )


def resolve_persona_path(name_or_path: str | Path) -> Path:
    raw = str(name_or_path)
    mapped = DEFAULT_PERSONAS.get(raw)
    if mapped is not None:
        return mapped
    p = Path(raw)
    if p.is_file():
        return p
    raise FileNotFoundError(f"persona not found: {name_or_path}")


def na_or_value(persona: Optional[Persona], category: str, value: Optional[float]) -> Any:
    """Return n/a if the category is outside the persona set."""
    if persona is not None and not persona.in_set(category):
        return NA
    return value


def strength(
    category: str,
    rec: Mapping[str, Any],
    persona: Optional[Persona] = None,
    detectors: Optional[Iterable[Any]] = None,
) -> Any:
    """Decision strength, or 'n/a' when the persona does not judge this emotion."""
    if persona is not None and not persona.in_set(category):
        return NA
    scores = decision_scores(rec, detectors)
    canon = ALIASES.get(category, category)
    sc = scores.get(category) or scores.get(canon)
    if sc is None:
        # try aliases listed on the persona
        if persona is not None:
            spec = persona.spec_for(category)
            if spec is not None:
                sc = scores.get(spec.id)
                if sc is None:
                    for a in spec.aliases:
                        sc = scores.get(a)
                        if sc is not None:
                            break
    if sc is None:
        return None
    return float(sc.value)


def apply_persona_scores(
    rec: Mapping[str, Any],
    persona: Optional[Persona] = None,
    detectors: Optional[Iterable[Any]] = None,
) -> dict[str, Any]:
    """Attach detectors[] and persona view (n/a outside the set)."""
    dets = list(detectors) if detectors is not None else load_detectors()
    scored = decision_scores(rec, dets)
    detectors_out = {k: v.to_dict() for k, v in scored.items()}
    persona_view = {}
    if persona is not None:
        # every judged emotion
        for spec in persona.emotions:
            val = strength(spec.id, rec, persona=persona, detectors=dets)
            persona_view[spec.id] = val
            for a in spec.aliases:
                persona_view[a] = val
        # and explicitly mark a few well-known out-of-set names as n/a
        for extra in ("seductive", "love", "crying", "yelling", "joyful", "happy",
                      "laughter", "aggressive", "logic", "reason", "neutral",
                      "sad", "fear", "surprise", "disgust"):
            if extra not in persona_view:
                persona_view[extra] = NA
    return {
        "detectors": detectors_out,
        "persona_scores": persona_view,
        "persona_id": persona.id if persona else None,
    }


def mean_centred_cosine(a, b) -> float:
    import numpy as np

    a = np.asarray(a, dtype=np.float64)
    b = np.asarray(b, dtype=np.float64)
    a = a - a.mean()
    b = b - b.mean()
    na = np.linalg.norm(a)
    nb = np.linalg.norm(b)
    if na < 1e-12 or nb < 1e-12:
        return 0.0
    return float(np.dot(a, b) / (na * nb))


def match_trained(a_probs: Mapping[str, float], b_probs: Mapping[str, float], labels: Sequence[str]) -> float:
    import numpy as np

    va = np.array([float(a_probs.get(l, 0.0)) for l in labels], dtype=np.float64)
    vb = np.array([float(b_probs.get(l, 0.0)) for l in labels], dtype=np.float64)
    d = float(np.sqrt(np.mean((va - vb) ** 2)))
    return float(max(0.0, 1.0 - d))


def match_scalar(a: float, b: float) -> float:
    return float(max(0.0, 1.0 - abs(float(a) - float(b))))


def category_score(matches: Sequence[float]) -> Optional[float]:
    if not matches:
        return None
    return 0.55 * max(matches) + 0.45 * (sum(matches) / len(matches))


def emotion_match(
    *,
    line_recs: Sequence[Mapping[str, Any]],
    moments: Sequence[Mapping[str, Any]],
    persona: Persona,
    detectors: Optional[Iterable[Any]] = None,
    e2v_labels: Sequence[str] = (
        "angry", "disgusted", "fearful", "happy", "neutral",
        "other", "sad", "surprised", "unknown",
    ),
) -> dict[str, Any]:
    """Persona-aware blend: only judged emotions enter category_blend.

    moments: list of {category, rec/window, strength?} already filtered or not;
    out-of-set categories are dropped (reported under na, not as 0).
    Formula (same coefficients as universal):
      score_C = 0.55*max_i match_i + 0.45*mean_i match_i
      category_blend = Σ strength_C · score_C / Σ strength_C   over in-set C
      emotion_match = 0.70 * category_blend + 0.30 * emb_sim
    """
    dets = list(detectors) if detectors is not None else load_detectors()
    leaderboard = []
    na_categories = []
    used_windows = []
    for moment in moments:
        cat = moment["category"]
        if not persona.in_set(cat):
            na_categories.append(cat)
            continue
        win = moment.get("window") or moment.get("rec") or moment
        m_strength = moment.get("strength")
        if m_strength is None:
            raw = strength(cat, win, persona=persona, detectors=dets)
            m_strength = 0.0 if raw == NA or raw is None else float(raw)
        matches = []
        best_id = None
        best_m = -1.0
        for i, ln in enumerate(line_recs):
            src = (decision_scores(win, dets).get(ALIASES.get(cat, cat)) or decision_scores(win, dets).get(cat))
            # match type: trained uses RMS over e2v 9-class; others scalar decision
            from detectors import e2v_probs
            from detectors.trained import E2V_TRAINED

            canon = ALIASES.get(cat, cat)
            if canon in E2V_TRAINED or cat in E2V_TRAINED:
                sc = match_trained(e2v_probs(ln), e2v_probs(win), list(e2v_labels))
            else:
                a = strength(cat, ln, persona=persona, detectors=dets)
                b = strength(cat, win, persona=persona, detectors=dets)
                if a == NA or b == NA or a is None or b is None:
                    continue
                sc = match_scalar(float(a), float(b))
            lid = ln.get("id") or ln.get("file") or f"line{i}"
            matches.append(sc)
            if sc > best_m:
                best_m = sc
                best_id = lid
        if not matches:
            continue
        score = category_score(matches)
        used_windows.append(win)
        src_name = None
        scored = decision_scores(win, dets)
        hit = scored.get(canon) or scored.get(cat)
        if hit is not None:
            src_name = hit.source
        leaderboard.append({
            "category": cat if persona.spec_for(cat) is None else persona.spec_for(cat).id,
            "source": src_name,
            "strength": float(m_strength),
            "score": score,
            "best_line": best_id,
            "best_match": best_m,
            "mean_match": sum(matches) / len(matches),
            "n": len(matches),
        })

    num = den = 0.0
    for row in leaderboard:
        w = max(float(row["strength"]), 1e-12)
        num += w * float(row["score"])
        den += w
    cat_blend = (num / den) if den > 0 else None

    emb_scores = []
    win_embs = []
    for w in used_windows:
        e2v = w.get("e2v") or {}
        emb = e2v.get("embedding")
        if emb:
            win_embs.append(emb)
    for ln in line_recs:
        e2v = ln.get("e2v") or {}
        emb = e2v.get("embedding")
        if not emb or not win_embs:
            continue
        emb_scores.append(max(mean_centred_cosine(emb, we) for we in win_embs))
    emb_sim = (sum(emb_scores) / len(emb_scores)) if emb_scores else None

    em = None
    if cat_blend is not None and emb_sim is not None:
        em = BLEND * cat_blend + EMB * emb_sim
    elif cat_blend is not None:
        em = cat_blend

    passed = None if em is None else bool(em + 1e-12 >= persona.gate)
    leaderboard.sort(key=lambda r: -r["score"])
    return {
        "persona_id": persona.id,
        "gate": persona.gate,
        "emotion_match": em,
        "category_blend": cat_blend,
        "emb_sim": emb_sim,
        "pass": passed,
        "leaderboard": leaderboard,
        "na_categories": na_categories,
        "judged": persona.judged_ids(),
        "formula": (
            "emotion_match = 0.70*category_blend + 0.30*emb_sim over persona target "
            "emotions only; out-of-set categories are n/a (dropped, not 0); "
            "crying decision = emotion2vec+ sad"
        ),
    }
