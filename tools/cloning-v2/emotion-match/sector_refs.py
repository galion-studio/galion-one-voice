# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Per-sector reference manifest for emotion_match.

A manifest maps each line's emotion to a reference clip path and its sha256.
Real reference audio is not in git. It comes from the Emotion V2 capture kit
(docs/emotion-v2/CAPTURE-PROTOCOL.md): five takes per shared sector, plus
Samantha's optional seductive overlay. This module only reads paths the
caller already has.

Schema: galion-emotion-v2-sector-refs/1
"""

from __future__ import annotations

import hashlib
import json
import math
from dataclasses import dataclass
from pathlib import Path

from scorelib import CATEGORY_W, EMB_W, FORMULA, GATE

SCHEMA = "galion-emotion-v2-sector-refs/1"

# The twelve shared Emotion V2 sectors (taxonomy #89). Names match the
# harness contract. "Sector" here is one emotion, as in the foundation
# spec §6 / §7, not the four family groupings (Grounded, Bright, Heavy, Sharp).
SHARED_SECTORS: tuple[str, ...] = (
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

# Samantha-only overlay. Required only when a scored line uses it.
OVERLAY_SECTORS: tuple[str, ...] = ("seductive",)

KNOWN_SECTORS: tuple[str, ...] = SHARED_SECTORS + OVERLAY_SECTORS


class SectorRefError(ValueError):
    """The manifest, a reference file, or a sha256 is not usable.

    This is a closed failure: the run must not invent a substitute reference
    and must not fall back to a single mixed clip.
    """


@dataclass(frozen=True)
class SectorRef:
    emotion: str
    path: Path
    sha256: str


@dataclass(frozen=True)
class Line:
    id: str
    emotion: str
    path: Path


@dataclass(frozen=True)
class Manifest:
    schema: str
    persona: str | None
    gate: float
    references: dict[str, SectorRef]
    lines: tuple[Line, ...]
    source: Path
    root: Path

    def ref_for(self, emotion: str, line_id: str | None = None) -> SectorRef:
        key = canonical_sector(emotion)
        if key not in self.references:
            where = f" (line {line_id})" if line_id else ""
            raise SectorRefError(
                f"missing sector reference for {key}{where}: "
                "every scored line needs a reference clip path and sha256 in the manifest"
            )
        return self.references[key]


def canonical_sector(name: str) -> str:
    key = name.strip().lower()
    if key not in KNOWN_SECTORS:
        known = ", ".join(KNOWN_SECTORS)
        raise SectorRefError(f"unknown emotion sector {name!r}; expected one of {known}")
    return key


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_ref(ref: SectorRef) -> SectorRef:
    """Confirm the file exists and its sha256 matches the manifest."""
    if not ref.path.is_file():
        raise SectorRefError(
            f"sector reference file not found for {ref.emotion}: {ref.path}"
        )
    got = file_sha256(ref.path)
    if got.lower() != ref.sha256.lower():
        raise SectorRefError(
            f"sha256 mismatch for {ref.emotion} reference {ref.path}: "
            f"expected {ref.sha256}, got {got}"
        )
    return ref


def resolve_ref(manifest: Manifest, emotion: str, line_id: str | None = None) -> SectorRef:
    return verify_ref(manifest.ref_for(emotion, line_id))


def load_manifest(path: Path, root: Path | None = None) -> Manifest:
    source = path.expanduser().resolve()
    if not source.is_file():
        raise SectorRefError(f"manifest not found: {source}")
    try:
        data = json.loads(source.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SectorRefError(f"manifest {source} is not valid JSON: {exc}") from exc
    if not isinstance(data, dict):
        raise SectorRefError(f"manifest {source} must be a JSON object")
    schema = str(data.get("schema") or "")
    if schema != SCHEMA:
        raise SectorRefError(
            f"manifest {source} has schema {schema!r}; expected {SCHEMA!r}"
        )
    _assert_locked_constants(data, source)
    base = (root or source.parent).expanduser().resolve()
    raw_refs = data.get("references")
    if not isinstance(raw_refs, dict) or not raw_refs:
        raise SectorRefError(
            f"manifest {source} needs a non-empty 'references' object "
            "mapping each emotion to {path, sha256}"
        )
    references: dict[str, SectorRef] = {}
    for name, body in raw_refs.items():
        emotion = canonical_sector(str(name))
        if not isinstance(body, dict):
            raise SectorRefError(
                f"reference for {emotion} must be an object with path and sha256"
            )
        ref_path = _resolve_path(base, body.get("path"), f"{emotion} reference")
        sha = _parse_sha256(body.get("sha256"), emotion)
        if emotion in references:
            raise SectorRefError(f"duplicate reference for sector {emotion}")
        references[emotion] = SectorRef(emotion=emotion, path=ref_path, sha256=sha)
    lines = _parse_lines(data.get("lines") or [], base, source)
    persona = data.get("persona")
    return Manifest(
        schema=schema,
        persona=str(persona) if persona is not None else None,
        gate=GATE,
        references=references,
        lines=tuple(lines),
        source=source,
        root=base,
    )


def infer_emotion(path: Path, manifest: Manifest | None = None) -> str:
    """Assign a sector from a parent directory, a filename prefix, or a line id."""
    parent = path.parent.name.strip().lower()
    if parent in KNOWN_SECTORS:
        return parent
    stem = path.stem.strip().lower()
    for sector in KNOWN_SECTORS:
        if stem == sector or stem.startswith(sector + "-") or stem.startswith(sector + "_"):
            return sector
    if manifest is not None:
        for line in manifest.lines:
            if line.id == path.stem or line.id == stem or line.path == path.resolve():
                return line.emotion
    raise SectorRefError(
        f"cannot assign a sector for {path}: put it in an emotion directory, "
        "name it {{emotion}}-line.wav, or list it under 'lines' in the manifest"
    )


def collect_jobs(
    manifest: Manifest,
    renders_dir: Path | None = None,
    clips: list[Path] | None = None,
) -> list[Line]:
    """Build the scored line list from the manifest, a render tree, and CLI clips."""
    jobs: list[Line] = []
    seen: set[Path] = set()

    def add(line: Line) -> None:
        resolved = line.path.resolve()
        if resolved in seen:
            return
        if not resolved.is_file():
            raise SectorRefError(f"missing render for line {line.id}: {line.path}")
        seen.add(resolved)
        jobs.append(Line(id=line.id, emotion=line.emotion, path=resolved))

    for line in manifest.lines:
        add(line)
    if renders_dir is not None:
        directory = renders_dir.expanduser().resolve()
        if not directory.is_dir():
            raise SectorRefError(f"renders directory not found: {directory}")
        for child in sorted(directory.iterdir(), key=lambda item: item.name.lower()):
            if not child.is_dir() or child.name.startswith("."):
                continue
            emotion = canonical_sector(child.name)
            wavs = sorted(
                (
                    path
                    for path in child.iterdir()
                    if path.is_file() and path.suffix.lower() in {".wav", ".wave"}
                ),
                key=lambda item: item.name,
            )
            if not wavs:
                raise SectorRefError(f"no render wavs in {child}")
            for wav in wavs:
                add(Line(id=f"{emotion}/{wav.stem}", emotion=emotion, path=wav))
    for clip in clips or []:
        path = clip.expanduser().resolve()
        emotion = infer_emotion(path, manifest)
        add(Line(id=path.stem, emotion=emotion, path=path))
    if not jobs:
        raise SectorRefError(
            "no renders to score: pass clips, --renders-dir, or lines in the manifest"
        )
    return jobs


def required_sectors(jobs: list[Line]) -> tuple[str, ...]:
    ordered: list[str] = []
    for sector in KNOWN_SECTORS:
        if any(job.emotion == sector for job in jobs) and sector not in ordered:
            ordered.append(sector)
    return tuple(ordered)


def _assert_locked_constants(data: dict, source: Path) -> None:
    if "gate" in data:
        try:
            gate = float(data["gate"])
        except (TypeError, ValueError) as exc:
            raise SectorRefError(f"manifest {source} has a non-numeric gate") from exc
        if not math.isclose(gate, GATE, rel_tol=0.0, abs_tol=1e-12):
            raise SectorRefError(
                f"manifest {source} sets gate={gate}; the emotion_match gate is locked at {GATE} "
                "and this tool will not lower or raise it"
            )
    if "formula" in data and str(data["formula"]) != FORMULA:
        raise SectorRefError(
            f"manifest {source} sets formula={data['formula']!r}; expected {FORMULA!r}"
        )
    weights = data.get("weights")
    if weights is not None:
        if not isinstance(weights, dict):
            raise SectorRefError(f"manifest {source} weights must be an object")
        category = weights.get("category_blend", weights.get("category"))
        emb = weights.get("emb_sim", weights.get("embedding"))
        try:
            category_w = CATEGORY_W if category is None else float(category)
            emb_w = EMB_W if emb is None else float(emb)
        except (TypeError, ValueError) as exc:
            raise SectorRefError(f"manifest {source} has non-numeric weights") from exc
        if not (
            math.isclose(category_w, CATEGORY_W, rel_tol=0.0, abs_tol=1e-12)
            and math.isclose(emb_w, EMB_W, rel_tol=0.0, abs_tol=1e-12)
        ):
            raise SectorRefError(
                f"manifest {source} sets weights category={category_w} emb={emb_w}; "
                f"the formula is locked at {FORMULA}"
            )


def _parse_sha256(value: object, emotion: str) -> str:
    if value is None:
        raise SectorRefError(
            f"missing sha256 for {emotion} reference: the manifest must pin the clip"
        )
    text = str(value).strip().lower()
    if text.startswith("sha256:"):
        text = text[7:]
    if len(text) != 64 or any(ch not in "0123456789abcdef" for ch in text):
        raise SectorRefError(
            f"sha256 for {emotion} must be a 64-character hex digest, got {value!r}"
        )
    return text


def _resolve_path(root: Path, value: object, label: str) -> Path:
    if not value or not isinstance(value, str):
        raise SectorRefError(f"{label} is missing a path")
    candidate = Path(value)
    if not candidate.is_absolute():
        candidate = root / candidate
    return candidate


def _parse_lines(raw: object, root: Path, source: Path) -> list[Line]:
    if raw is None:
        return []
    if not isinstance(raw, list):
        raise SectorRefError(f"manifest {source} 'lines' must be an array")
    lines: list[Line] = []
    for index, body in enumerate(raw):
        if not isinstance(body, dict):
            raise SectorRefError(f"manifest {source} lines[{index}] must be an object")
        emotion = canonical_sector(str(body.get("emotion") or body.get("sector") or ""))
        raw_id = body.get("id") or body.get("line") or body.get("path")
        if not raw_id:
            raise SectorRefError(f"manifest {source} lines[{index}] needs an id or path")
        line_id = str(raw_id) if body.get("id") or body.get("line") else Path(str(raw_id)).stem
        if "path" not in body and "file" not in body:
            raise SectorRefError(
                f"manifest {source} line {line_id} needs a path to the render wav"
            )
        path = _resolve_path(root, body.get("path") or body.get("file"), f"line {line_id}")
        lines.append(Line(id=str(line_id), emotion=emotion, path=path))
    return lines
