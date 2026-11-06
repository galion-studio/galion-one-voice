"""Persona detector registry and love/seductive plugin hook.

Discovery rule (Agent 2): any file matching
`scorers/persona/detectors/love_seductive*.py` is loaded. The module must
export one of:

    class LoveSeductiveDetector:  # instance constructed with no args
        name = "..."
        categories = ("love", "seductive")
        def score(self, rec) -> dict[str, DetectorScore]: ...

    def build_detector() -> Detector: ...

    DETECTOR: Detector

If at least one plugin loads, the legacy CLAP fallback is not used.
"""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from typing import Any, Iterable, List, Mapping, Optional, Sequence

from .base import DetectorScore, clap_cosine, clap_probs, e2v_probs, eat_max
from .crying import CryingDetector, SOURCE as CRYING_SOURCE
from .eat import EatDetector
from .legacy_clap_fallback import LegacyClapLoveSeductiveFallback
from .logic import LogicDetector
from .trained import TrainedDetector

PLUGIN_GLOB = "love_seductive*.py"
CORE_DETECTORS = (CryingDetector, TrainedDetector, EatDetector, LogicDetector)


def _load_module_from_path(path: Path):
    name = f"galion_persona_plugin_{path.stem}"
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load plugin {path}")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def plugin_detector_from_module(mod) -> Optional[Any]:
    if hasattr(mod, "LoveSeductiveDetector"):
        return mod.LoveSeductiveDetector()
    if hasattr(mod, "build_detector"):
        return mod.build_detector()
    if hasattr(mod, "DETECTOR"):
        return mod.DETECTOR
    return None


def discover_love_seductive(directory: Optional[Path] = None) -> List[Any]:
    """Load Agent 2 plugins. Missing files are not an error."""
    d = Path(directory) if directory is not None else Path(__file__).resolve().parent
    found = []
    for path in sorted(d.glob(PLUGIN_GLOB)):
        if path.name.startswith("."):
            continue
        mod = _load_module_from_path(path)
        det = plugin_detector_from_module(mod)
        if det is None:
            raise ImportError(
                f"{path} matches {PLUGIN_GLOB} but exports none of "
                "LoveSeductiveDetector, build_detector(), DETECTOR"
            )
        found.append(det)
    return found


def load_detectors(
    *,
    plugin_dir: Optional[Path] = None,
    include_legacy_love_seductive_fallback: bool = True,
    extra: Optional[Sequence[Any]] = None,
) -> List[Any]:
    dets: List[Any] = [cls() for cls in CORE_DETECTORS]
    plugins = discover_love_seductive(plugin_dir)
    if plugins:
        dets.extend(plugins)
    elif include_legacy_love_seductive_fallback:
        dets.append(LegacyClapLoveSeductiveFallback())
    if extra:
        dets.extend(list(extra))
    return dets


def decision_scores(
    rec: Mapping[str, Any],
    detectors: Optional[Iterable[Any]] = None,
) -> dict[str, DetectorScore]:
    """Merge detector outputs. Later detectors override earlier ones for the same key.

    Core order is crying, trained, eat, logic, then love/seductive plugin (or
    fallback). Crying is first and EatDetector does not emit crying, so
    EfficientAT cannot overwrite the emotion2vec+ sad decision.
    """
    dets = list(detectors) if detectors is not None else load_detectors()
    out: dict[str, DetectorScore] = {}
    for det in dets:
        scored = det.score(rec) or {}
        out.update(scored)
    return out


# Re-export record helpers so callers can `from detectors import e2v_probs`.
__all__ = [
    "CRYING_SOURCE",
    "CryingDetector",
    "DetectorScore",
    "PLUGIN_GLOB",
    "clap_cosine",
    "clap_probs",
    "decision_scores",
    "discover_love_seductive",
    "e2v_probs",
    "eat_max",
    "load_detectors",
]
