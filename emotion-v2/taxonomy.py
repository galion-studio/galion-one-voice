# Galion Voice — Perpetual Open MIT License
# (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
"""Load and validate ``emotion-v2/emotions.toml``.

The file is Samantha's Emotion V2 inventory. This module checks the
structure the taxonomy depends on: twelve emotions, unique ids, and
valence/arousal coordinates inside [-1, 1]. It also checks the pitch
centre, the classifier labels, and the blend graph, so a later edit
cannot quietly retune 432 Hz to 440 Hz or point a category at a label
the cited model does not have.
"""

from __future__ import annotations

import math
import tomllib
from dataclasses import dataclass
from pathlib import Path

SCHEMA = "galion-emotion-v2/1"
PERSONA = "samantha"
PITCH_CENTRE_HZ = 432
PITCH_CENTRE_RATIO = "1/1"
FORBIDDEN_CENTRE_HZ = 440

# 5-limit just intervals used as bounds around the 432 Hz unison.
# Reciprocals are the same intervals below the centre.
ALLOWED_RATIOS = frozenset(
    {
        "1/1",
        "25/24",
        "24/25",
        "16/15",
        "15/16",
        "10/9",
        "9/10",
        "9/8",
        "8/9",
        "6/5",
        "5/6",
        "5/4",
        "4/5",
    }
)

REQUIRED_IDS = (
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

REQUIRED_NAMES = {
    "neutral": "Neutral",
    "calm": "Calm",
    "happy": "Happy",
    "amused": "Amused",
    "excited": "Excited",
    "tender": "Tender",
    "sad": "Sad",
    "disappointed": "Disappointed",
    "angry": "Angry",
    "fearful": "Fearful",
    "surprised": "Surprised",
    "disgusted": "Disgusted",
}

SECTORS = {
    "grounded": ("neutral", "calm"),
    "bright": ("happy", "amused", "excited", "tender"),
    "heavy": ("sad", "disappointed"),
    "sharp": ("angry", "fearful", "surprised", "disgusted"),
}

SUPERB_LABELS = ("neu", "hap", "ang", "sad")
EMOTION2VEC_LABELS = (
    "angry",
    "disgusted",
    "fearful",
    "happy",
    "neutral",
    "other",
    "sad",
    "surprised",
    "unknown",
)

# Classes the cited heads were actually trained to emit, under our ids.
SUPERB_IN_SET = {
    "neutral": "neu",
    "happy": "hap",
    "angry": "ang",
    "sad": "sad",
}
EMOTION2VEC_IN_SET = {
    "neutral": "neutral",
    "happy": "happy",
    "sad": "sad",
    "angry": "angry",
    "fearful": "fearful",
    "surprised": "surprised",
    "disgusted": "disgusted",
}

COORDINATE_BASES = frozenset({"circumplex-origin", "russell-1980-angle", "design"})
BLEND_BASES = frozenset(
    {
        "sector-neighbor",
        "cowen-2017-coload",
        "cowen-2019-adjacency",
        "design-adjacency",
    }
)

# Current ProsodyControl clamp. This file does not change it.
# Targets outside it would be clipped by the engine that already ships.
PACE_BOUNDS = (0.8, 1.2)
ENERGY_BOUNDS = (0.6, 1.2)

# Persona floor. A scenario text that contains one of these is a canned
# comfort line, which the register already refuses.
FORBIDDEN_PHRASES = (
    "i hear you",
    "that sounds so hard",
    "that sounds hard",
    "i'm here for you",
    "i feel that in my soul",
    "i know this is difficult",
    "i'm worried about you",
    "take a deep breath",
    "so what i'm hearing",
    "noted, that is hard",
    "i'm sorry for your loss",
    "let's slow down",
)

REQUIRED_SOURCES = (
    "ekman1992",
    "ekman1993",
    "russell1980",
    "cowen2017",
    "cowen2019",
    "posner2005",
    "superb_er",
    "emotion2vec_plus",
)

HERE = Path(__file__).resolve().parent
DEFAULT_PATH = HERE / "emotions.toml"


class TaxonomyError(ValueError):
    """``emotions.toml`` failed validation. ``errors`` is the full list."""

    def __init__(self, errors: list[str]):
        self.errors = list(errors)
        super().__init__("\n".join(self.errors))


@dataclass(frozen=True)
class Blend:
    id: str
    basis: str


@dataclass(frozen=True)
class Prosody:
    pace: float
    energy: float
    breathiness: float
    pitch_low_ratio: str
    pitch_high_ratio: str
    pitch_median_ratio: str

    def ratio_value(self, ratio: str) -> float:
        num, den = ratio.split("/")
        return int(num) / int(den)


@dataclass(frozen=True)
class Emotion:
    id: str
    name: str
    sector: str
    order: int
    valence: float
    arousal: float
    coordinate_basis: str
    russell_angle_deg: float | None
    superb: str
    superb_in_set: bool
    emotion2vec: str
    emotion2vec_in_set: bool
    intensities: tuple[str, ...]
    blends: tuple[Blend, ...]
    texts: tuple[str, ...]
    prosody: Prosody


@dataclass(frozen=True)
class Taxonomy:
    schema: str
    persona: str
    pitch_centre_hz: int
    pitch_centre_ratio: str
    intensity: dict[str, float]
    emotions: tuple[Emotion, ...]

    def by_id(self) -> dict[str, Emotion]:
        return {item.id: item for item in self.emotions}


def _number(value: object) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def _in_unit(value: object) -> bool:
    return _number(value) and -1.0 <= float(value) <= 1.0


def _expect_ratio(raw: object, label: str, errors: list[str]) -> str:
    if not isinstance(raw, str) or raw not in ALLOWED_RATIOS:
        errors.append(f"{label} must be one of the 5-limit ratios, got {raw!r}")
        return "1/1"
    return raw


def _check_russell(item: dict, where: str, errors: list[str]) -> float | None:
    basis = item.get("coordinate_basis")
    angle = item.get("russell_angle_deg", None)
    if basis == "russell-1980-angle":
        if not _number(angle):
            errors.append(f"{where}: russell-1980-angle requires russell_angle_deg")
            return None
        radians = math.radians(float(angle))
        expected = (round(math.cos(radians), 2), round(math.sin(radians), 2))
        got = (item.get("valence"), item.get("arousal"))
        if any(not _number(part) or abs(float(part) - want) > 1e-9 for part, want in zip(got, expected)):
            errors.append(
                f"{where}: coordinates {got} are not the unit-circle projection of {angle}° ({expected})"
            )
        return float(angle)
    if angle is not None:
        errors.append(f"{where}: russell_angle_deg is only valid when coordinate_basis is russell-1980-angle")
    if basis == "circumplex-origin":
        if item.get("valence") != 0 or item.get("arousal") != 0:
            errors.append(f"{where}: circumplex origin must be valence 0 and arousal 0")
    return None


def _parse_emotion(item: object, errors: list[str]) -> Emotion | None:
    if not isinstance(item, dict):
        errors.append("emotion entry is not a table")
        return None
    eid = item.get("id")
    where = f"emotion {eid!r}" if isinstance(eid, str) else "emotion"
    name = item.get("name")
    sector = item.get("sector")
    order = item.get("order")
    basis = item.get("coordinate_basis")
    if not isinstance(eid, str) or not eid:
        errors.append("emotion id must be a non-empty string")
        eid = ""
    if eid and name != REQUIRED_NAMES.get(eid) and eid in REQUIRED_NAMES:
        errors.append(f"{where}: name must be {REQUIRED_NAMES[eid]!r}")
    if sector not in SECTORS:
        errors.append(f"{where}: unknown sector {sector!r}")
    elif isinstance(eid, str) and eid not in SECTORS[sector]:
        errors.append(f"{where}: {eid} is not in sector {sector}")
    if not isinstance(order, int) or isinstance(order, bool):
        errors.append(f"{where}: order must be an integer")
        order = 0
    if not _in_unit(item.get("valence")) or not _in_unit(item.get("arousal")):
        errors.append(f"{where}: valence and arousal must be numbers in [-1, 1]")
    if basis not in COORDINATE_BASES:
        errors.append(f"{where}: unknown coordinate_basis {basis!r}")
    note = item.get("coordinate_note")
    if not isinstance(note, str) or not note.strip():
        errors.append(f"{where}: coordinate_note must be non-empty")
    angle = _check_russell(item, where, errors)

    superb = item.get("superb")
    if superb not in SUPERB_LABELS:
        errors.append(f"{where}: superb label {superb!r} is outside neu/hap/ang/sad")
    in_superb = item.get("superb_in_set")
    if not isinstance(in_superb, bool):
        errors.append(f"{where}: superb_in_set must be a boolean")
        in_superb = False
    if eid in SUPERB_IN_SET:
        if not in_superb or superb != SUPERB_IN_SET[eid]:
            errors.append(f"{where}: {eid} is a SUPERB class and must map to {SUPERB_IN_SET[eid]!r}")
    elif in_superb:
        errors.append(f"{where}: {eid} is not a SUPERB class; superb_in_set must be false")
    if not isinstance(item.get("superb_note"), str) or not str(item.get("superb_note")).strip():
        errors.append(f"{where}: superb_note must be non-empty")

    e2 = item.get("emotion2vec")
    if e2 not in EMOTION2VEC_LABELS:
        errors.append(f"{where}: emotion2vec label {e2!r} is outside the nine-class head")
    in_e2 = item.get("emotion2vec_in_set")
    if not isinstance(in_e2, bool):
        errors.append(f"{where}: emotion2vec_in_set must be a boolean")
        in_e2 = False
    if eid in EMOTION2VEC_IN_SET:
        if not in_e2 or e2 != EMOTION2VEC_IN_SET[eid]:
            errors.append(f"{where}: {eid} is an emotion2vec+ class and must map to {EMOTION2VEC_IN_SET[eid]!r}")
    elif in_e2:
        errors.append(f"{where}: {eid} is not an emotion2vec+ class; emotion2vec_in_set must be false")
    if not isinstance(item.get("emotion2vec_note"), str) or not str(item.get("emotion2vec_note")).strip():
        errors.append(f"{where}: emotion2vec_note must be non-empty")

    intensities = item.get("intensities")
    expected_levels = ["full"] if eid == "neutral" else ["low", "mid", "full"]
    if intensities != expected_levels:
        errors.append(f"{where}: intensities must be {expected_levels}")
        intensities = []

    blends_raw = item.get("blends")
    blends: list[Blend] = []
    if not isinstance(blends_raw, list):
        errors.append(f"{where}: blends must be a list")
    else:
        seen: set[str] = set()
        for blend in blends_raw:
            if not isinstance(blend, dict):
                errors.append(f"{where}: blend entry is not a table")
                continue
            bid = blend.get("id")
            basis_name = blend.get("basis")
            if not isinstance(bid, str) or bid not in REQUIRED_IDS:
                errors.append(f"{where}: blend id {bid!r} is not one of the twelve emotions")
                continue
            if bid == eid:
                errors.append(f"{where}: blend cannot name itself")
            if bid in seen:
                errors.append(f"{where}: duplicate blend {bid}")
            seen.add(bid)
            if basis_name not in BLEND_BASES:
                errors.append(f"{where}: blend basis {basis_name!r} is not allowed")
                basis_name = "design-adjacency"
            if basis_name == "sector-neighbor" and sector in SECTORS and bid not in SECTORS.get(sector, ()):
                errors.append(f"{where}: sector-neighbor blend {bid} is in another sector")
            blends.append(Blend(bid, str(basis_name)))

    texts = item.get("texts")
    clean_texts: list[str] = []
    if not isinstance(texts, list) or len(texts) != 3:
        errors.append(f"{where}: texts must be three strings")
    else:
        for text in texts:
            if not isinstance(text, str) or not text.strip():
                errors.append(f"{where}: each scenario text must be a non-empty string")
                continue
            lowered = text.casefold()
            for phrase in FORBIDDEN_PHRASES:
                if phrase in lowered:
                    errors.append(f"{where}: scenario text contains forbidden phrase {phrase!r}")
            clean_texts.append(text)

    prosody_raw = item.get("prosody")
    prosody = Prosody(1.0, 1.0, 0.0, "1/1", "1/1", "1/1")
    if not isinstance(prosody_raw, dict):
        errors.append(f"{where}: prosody table is missing")
    else:
        pace = prosody_raw.get("pace")
        energy = prosody_raw.get("energy")
        breath = prosody_raw.get("breathiness")
        if not _number(pace) or not (PACE_BOUNDS[0] <= float(pace) <= PACE_BOUNDS[1]):
            errors.append(f"{where}: pace must be inside the current rate clamp {PACE_BOUNDS}")
            pace = 1.0
        if not _number(energy) or not (ENERGY_BOUNDS[0] <= float(energy) <= ENERGY_BOUNDS[1]):
            errors.append(f"{where}: energy must be inside the current energy clamp {ENERGY_BOUNDS}")
            energy = 1.0
        if not _number(breath) or not (0.0 <= float(breath) <= 1.0):
            errors.append(f"{where}: breathiness must be inside [0, 1]")
            breath = 0.0
        low = _expect_ratio(prosody_raw.get("pitch_low_ratio"), f"{where} pitch_low_ratio", errors)
        high = _expect_ratio(prosody_raw.get("pitch_high_ratio"), f"{where} pitch_high_ratio", errors)
        median = _expect_ratio(prosody_raw.get("pitch_median_ratio"), f"{where} pitch_median_ratio", errors)
        prosody = Prosody(float(pace), float(energy), float(breath), low, high, median)
        values = (prosody.ratio_value(low), prosody.ratio_value(median), prosody.ratio_value(high))
        if values != tuple(sorted(values)):
            errors.append(f"{where}: pitch ratios must satisfy low <= median <= high")

    if not eid:
        return None
    return Emotion(
        id=eid,
        name=name if isinstance(name, str) else "",
        sector=sector if isinstance(sector, str) else "",
        order=int(order),
        valence=float(item["valence"]) if _number(item.get("valence")) else math.nan,
        arousal=float(item["arousal"]) if _number(item.get("arousal")) else math.nan,
        coordinate_basis=basis if isinstance(basis, str) else "",
        russell_angle_deg=angle,
        superb=superb if isinstance(superb, str) else "",
        superb_in_set=bool(in_superb),
        emotion2vec=e2 if isinstance(e2, str) else "",
        emotion2vec_in_set=bool(in_e2),
        intensities=tuple(intensities) if isinstance(intensities, list) else (),
        blends=tuple(blends),
        texts=tuple(clean_texts),
        prosody=prosody,
    )


def _check_header(doc: dict, errors: list[str]) -> None:
    if doc.get("schema") != SCHEMA:
        errors.append(f"schema must be {SCHEMA!r}")
    if doc.get("persona") != PERSONA:
        errors.append(f"persona must be {PERSONA!r}")
    if doc.get("pitch_centre_hz") != PITCH_CENTRE_HZ:
        errors.append(f"pitch_centre_hz must be {PITCH_CENTRE_HZ}, never {FORBIDDEN_CENTRE_HZ}")
    if doc.get("pitch_centre_ratio") != PITCH_CENTRE_RATIO:
        errors.append("pitch_centre_ratio must be the just unison 1/1")
    forbidden = doc.get("pitch_retune_forbidden_hz")
    if forbidden != [FORBIDDEN_CENTRE_HZ]:
        errors.append("pitch_retune_forbidden_hz must be [440]")
    intensity = doc.get("intensity")
    if not isinstance(intensity, dict) or set(intensity) != {"low", "mid", "full"}:
        errors.append("intensity must define low, mid, and full")
    elif any(not _number(intensity[key]) or not 0.0 < float(intensity[key]) <= 1.0 for key in intensity):
        errors.append("intensity scalars must be in (0, 1]")
    superb = doc.get("superb")
    if not isinstance(superb, dict) or superb.get("model") != "superb/wav2vec2-base-superb-er":
        errors.append("superb.model must be superb/wav2vec2-base-superb-er")
    elif list(superb.get("labels") or []) != list(SUPERB_LABELS):
        errors.append("superb.labels must be neu, hap, ang, sad in id2label order")
    e2 = doc.get("emotion2vec_plus")
    if not isinstance(e2, dict) or e2.get("model") != "emotion2vec/emotion2vec_plus_large":
        errors.append("emotion2vec_plus.model must be emotion2vec/emotion2vec_plus_large")
    elif list(e2.get("labels") or []) != list(EMOTION2VEC_LABELS):
        errors.append("emotion2vec_plus.labels must be the nine-class head in model-card order")
    legacy = doc.get("legacy_scorer")
    if not isinstance(legacy, dict) or legacy.get("role") != "not-a-target":
        errors.append("legacy_scorer.role must be not-a-target")
    sources = doc.get("sources")
    if not isinstance(sources, dict) or any(key not in sources for key in REQUIRED_SOURCES):
        errors.append("sources is missing a required citation")
    elif any(not isinstance(sources[key], dict) or not str(sources[key].get("citation") or "").strip() for key in REQUIRED_SOURCES):
        errors.append("each source needs a non-empty citation")


def _check_set(emotions: list[Emotion], errors: list[str]) -> None:
    if len(emotions) != 12:
        errors.append(f"expected 12 emotions, found {len(emotions)}")
    ids = [item.id for item in emotions]
    if len(ids) != len(set(ids)):
        errors.append(f"emotion ids are not unique: {ids}")
    if tuple(ids) != REQUIRED_IDS:
        errors.append(f"emotion ids must be {REQUIRED_IDS} in that order")
    orders = [item.order for item in emotions]
    if orders != list(range(1, 13)):
        errors.append("emotion order must be 1..12 in file order")
    texts = [text for item in emotions for text in item.texts]
    if len(texts) != len(set(texts)):
        errors.append("scenario texts must be unique across the twelve emotions")
    by_id = {item.id: item for item in emotions}
    for item in emotions:
        for blend in item.blends:
            other = by_id.get(blend.id)
            if other is None:
                continue
            back = [edge for edge in other.blends if edge.id == item.id]
            if len(back) != 1 or back[0].basis != blend.basis:
                errors.append(
                    f"blend {item.id}->{blend.id} ({blend.basis}) is not returned with the same basis"
                )


def load(path: Path | None = None) -> Taxonomy:
    """Read ``path`` (default ``emotion-v2/emotions.toml``) and validate it."""
    source = DEFAULT_PATH if path is None else Path(path)
    try:
        doc = tomllib.loads(source.read_text(encoding="utf-8"))
    except OSError as exc:
        raise TaxonomyError([f"cannot read {source}: {exc}"]) from exc
    except tomllib.TOMLDecodeError as exc:
        raise TaxonomyError([f"invalid TOML in {source}: {exc}"]) from exc
    if not isinstance(doc, dict):
        raise TaxonomyError(["taxonomy root must be a table"])

    errors: list[str] = []
    _check_header(doc, errors)
    raw = doc.get("emotions")
    emotions: list[Emotion] = []
    if not isinstance(raw, list):
        errors.append("emotions must be an array of tables")
    else:
        for item in raw:
            parsed = _parse_emotion(item, errors)
            if parsed is not None:
                emotions.append(parsed)
        _check_set(emotions, errors)
    if errors:
        raise TaxonomyError(errors)

    intensity = {key: float(doc["intensity"][key]) for key in ("low", "mid", "full")}
    return Taxonomy(
        schema=str(doc["schema"]),
        persona=str(doc["persona"]),
        pitch_centre_hz=int(doc["pitch_centre_hz"]),
        pitch_centre_ratio=str(doc["pitch_centre_ratio"]),
        intensity=intensity,
        emotions=tuple(emotions),
    )
