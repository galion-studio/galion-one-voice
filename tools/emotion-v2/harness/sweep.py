# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Score frozen emotion-sweep clips against the Voice V2 rollout gate.

The clip set is the synthetic listening set already in
``docs/galion-voice-v2/evals/listening-set/emotion-sweep``. The manifest is
``docs/galion-voice-v2/evals/emotion-sweep/sweep.csv`` and the reply line is
the sentence in ``SWEEP.md``. This module reads those files. It does not
synthesize speech, clone a voice, download weights, or commit audio.

WER, ECAPA, and ``emotion_match`` are filled only when the corresponding
scorer actually ran. The CPU stub is not a recogniser and is never written
down as ``emotion_match``. A missing model is ``unmeasured``, which is
neither a pass nor a fail. ``rollout_claimed`` stays false.
"""

from __future__ import annotations

import csv
import hashlib
import json
import math
import os
import re
import tempfile
import wave
from dataclasses import dataclass, field
from pathlib import Path

from .audio import read_wav
from .backends import Analysis, BackendUnavailable, SuperbBackend, discover_superb_weights
from .config import ConfigError, Thresholds, load_thresholds, repo_root
from .guards import locate_scorer, score_ecapa, score_wer
from .metrics import cosine, ecapa_passes, l2_normalize, meets_min, softmax, wer_passes

SCHEMA = "galion-voice-v2-emotion-sweep-eval/1"
WER_MAX = 0.018
ECAPA_MIN = 0.7057
EMOTION_MIN = 0.83
EMOTION_FORMULA = "0.70*category_blend+0.30*emb_sim"
CATEGORY_W = 0.70
EMB_W = 0.30
REPLY_RE = re.compile(r'^Reply line for every persona case: "([^"]+)"', re.M)

CLIPS_REL = Path("docs/galion-voice-v2/evals/listening-set/emotion-sweep")
CSV_REL = Path("docs/galion-voice-v2/evals/emotion-sweep/sweep.csv")
SWEEP_MD_REL = Path("docs/galion-voice-v2/evals/emotion-sweep/SWEEP.md")

RELEASE_NOTE = (
    "A measured pass is not a release. Nothing merges or deploys without "
    "Slavik's written go-ahead."
)
STUB_REFUSAL = (
    "refusing to report the CPU stub as emotion_match. "
    "emotion_match is unmeasured."
)
WEIGHTS_ABSENT = (
    "superb/wav2vec2-base-superb-er weights are not on this machine. "
    "Refusing to download them. The CPU stub was not substituted. "
    "emotion_match is unmeasured."
)


class SweepError(ValueError):
    """The sweep manifest or the locked gate cannot be used."""


@dataclass
class Metric:
    name: str
    status: str
    value: float | None
    threshold: float
    comparison: str
    passed: bool | None
    detail: str
    extra: dict[str, object] = field(default_factory=dict)

    def as_dict(self) -> dict[str, object]:
        body: dict[str, object] = {
            "comparison": self.comparison,
            "detail": self.detail,
            "name": self.name,
            "pass": self.passed,
            "status": self.status,
            "threshold": self.threshold,
            "value": _round(self.value),
        }
        body.update(self.extra)
        return body


@dataclass
class ClipFact:
    role: str
    file: str
    path: Path
    persona: str | None
    user_emotion: str | None
    mood_history: str | None
    mood_label: str | None
    present: bool
    sha256: str | None
    sample_rate: int | None
    channels: int | None
    sample_width: int | None
    duration_s: float | None
    error: str | None

    def as_dict(self) -> dict[str, object]:
        return {
            "channels": self.channels,
            "duration_s": _round(self.duration_s),
            "error": self.error,
            "file": self.file,
            "mood_history": self.mood_history,
            "mood_label": self.mood_label,
            "path": _display_path(self.path),
            "persona": self.persona,
            "present": self.present,
            "role": self.role,
            "sample_rate": self.sample_rate,
            "sample_width": self.sample_width,
            "sha256": self.sha256,
            "user_emotion": self.user_emotion,
        }


@dataclass
class Candidate:
    id: str
    persona: str
    kind: str
    speaker_ref: str
    clips: list[ClipFact]
    wer: Metric
    ecapa: Metric
    emotion_match: Metric
    gate: dict[str, object]

    def as_dict(self) -> dict[str, object]:
        return {
            "clips": [clip.as_dict() for clip in self.clips],
            "ecapa": self.ecapa.as_dict(),
            "emotion_match": self.emotion_match.as_dict(),
            "gate": self.gate,
            "id": self.id,
            "kind": self.kind,
            "n_clips": len(self.clips),
            "persona": self.persona,
            "speaker_ref": self.speaker_ref,
            "wer": self.wer.as_dict(),
        }


@dataclass
class Report:
    clips_dir: Path
    csv_path: Path
    sweep_md: Path
    reply_line: str
    files: list[ClipFact]
    unexpected: list[str]
    candidates: list[Candidate]
    scorers: dict[str, dict[str, object]]
    require_scorers: bool

    @property
    def missing(self) -> list[str]:
        return [item.file for item in self.files if not item.present or item.error]

    def as_dict(self) -> dict[str, object]:
        measured = [row for row in self.candidates if row.gate["status"] == "measured"]
        passed = [row for row in measured if row.gate["pass"] is True]
        failed = [row for row in measured if row.gate["pass"] is False]
        unmeasured = [row for row in self.candidates if row.gate["status"] == "unmeasured"]
        errored = [row for row in self.candidates if row.gate["status"] == "error"]
        if self.missing:
            overall = "incomplete"
        elif errored:
            overall = "error"
        elif len(measured) == len(self.candidates) and self.candidates:
            overall = "pass" if len(passed) == len(self.candidates) else "fail"
        elif measured:
            overall = "partial"
        else:
            overall = "unmeasured"
        by_role: dict[str, int] = {}
        for item in self.files:
            by_role[item.role] = by_role.get(item.role, 0) + 1
        return {
            "audio_generated": False,
            "blockers": _blockers(self.scorers),
            "candidates": [row.as_dict() for row in self.candidates],
            "clips_dir": _display_path(self.clips_dir),
            "inventory": {
                "expected": len(self.files),
                "files": [item.as_dict() for item in self.files],
                "missing": list(self.missing),
                "persona_cases": by_role.get("persona-case", 0),
                "present": sum(1 for item in self.files if item.present and not item.error),
                "references": by_role.get("reference", 0),
                "unexpected": list(self.unexpected),
                "user_cues": by_role.get("user-cue", 0),
                "v1_baselines": by_role.get("v1-baseline", 0),
            },
            "manifest_csv": _display_path(self.csv_path),
            "prosody_note": (
                "Prosody columns in sweep.csv (f0, rms, rate) belong to that sweep. "
                "This report does not copy them and does not treat them as WER, "
                "ECAPA, or emotion_match."
            ),
            "release": False,
            "reply_line": self.reply_line,
            "require_scorers": self.require_scorers,
            "rollout_claimed": False,
            "rollout_gate": {
                "claimed": False,
                "ecapa_comparison": ">",
                "ecapa_min_exclusive": ECAPA_MIN,
                "emotion_formula": EMOTION_FORMULA,
                "emotion_match_comparison": ">=",
                "emotion_match_min": EMOTION_MIN,
                "note": RELEASE_NOTE,
                "wer_comparison": "<=",
                "wer_max": WER_MAX,
            },
            "schema": SCHEMA,
            "scores_fabricated": False,
            "scorers": self.scorers,
            "summary": {
                "gate": overall,
                "n_candidates": len(self.candidates),
                "n_error": len(errored),
                "n_failed": len(failed),
                "n_measured": len(measured),
                "n_passed": len(passed),
                "n_unmeasured": len(unmeasured),
            },
            "sweep_md": _display_path(self.sweep_md),
        }


def emotion_match_value(category_blend: float, emb_sim: float) -> float:
    """``0.70*category_blend+0.30*emb_sim``. The weights are locked."""
    return CATEGORY_W * float(category_blend) + EMB_W * float(emb_sim)


def histogram_intersection(left: list[float], right: list[float]) -> float:
    width = min(len(left), len(right))
    return float(sum(min(left[index], right[index]) for index in range(width)))


def as_prob(row: list[float]) -> list[float]:
    values = [float(item) for item in row]
    total = sum(values)
    if any(item < -1e-5 for item in values) or abs(total - 1.0) > 1e-2:
        return softmax(values)
    if total == 0.0:
        return [0.0] * len(values)
    return [item / total for item in values]


def pair_emotion(
    render_prob: list[float],
    render_emb: list[float],
    ref_prob: list[float],
    ref_emb: list[float],
) -> tuple[float, float, float]:
    category = histogram_intersection(as_prob(render_prob), as_prob(ref_prob))
    sim = cosine(l2_normalize(list(render_emb)), l2_normalize(list(ref_emb)))
    return category, sim, emotion_match_value(category, sim)


def judge_rollout(wer: Metric, ecapa: Metric, emotion: Metric) -> dict[str, object]:
    """Combine the three guards. Missing numbers do not become a pass."""
    metrics = (wer, ecapa, emotion)
    if any(item.status == "error" for item in metrics):
        status = "error"
        passed = None
        reason = "a scorer ran and failed; that is not a measurement and not a pass"
    elif any(item.status != "ok" for item in metrics):
        status = "unmeasured"
        passed = None
        missing = [item.name for item in metrics if item.status != "ok"]
        reason = (
            "unmeasured: "
            + ", ".join(missing)
            + ". No number was invented, and an absent scorer is not a pass."
        )
    else:
        status = "measured"
        passed = bool(wer.passed and ecapa.passed and emotion.passed)
        reason = "measured against the locked rollout gate" if passed else "measured; at least one guard missed the gate"
    return {
        "claimed": False,
        "pass": passed,
        "reason": reason,
        "status": status,
    }


def default_clips_dir() -> Path:
    return repo_root() / CLIPS_REL


def default_csv_path() -> Path:
    return repo_root() / CSV_REL


def default_sweep_md() -> Path:
    return repo_root() / SWEEP_MD_REL


def read_reply_line(sweep_md: Path) -> str:
    if not sweep_md.is_file():
        raise SweepError(f"missing sweep note: {sweep_md}")
    match = REPLY_RE.search(sweep_md.read_text(encoding="utf-8"))
    if match is None:
        raise SweepError(f"{sweep_md} has no reply line for the persona cases")
    line = match.group(1).strip()
    if not line:
        raise SweepError(f"{sweep_md} reply line is empty")
    return line


def execute(
    *,
    out_dir: Path,
    clips_dir: Path | None = None,
    csv_path: Path | None = None,
    sweep_md: Path | None = None,
    config: Path | None = None,
    backend: str | None = None,
    require_scorers: bool = False,
    wer_bin: Path | None = None,
    ecapa_bin: Path | None = None,
    emotion_analyse=None,
    emotion_measurement: bool | None = None,
    emotion_backend_name: str | None = None,
) -> tuple[int, dict[str, object]]:
    """Write ``report.json`` and ``summary.md``. Return ``(exit_code, report)``."""
    thresholds = load_thresholds(config)
    _locked_thresholds(thresholds)
    clips = clips_dir or default_clips_dir()
    csv_file = csv_path or default_csv_path()
    note = sweep_md or default_sweep_md()
    reply = read_reply_line(note)
    catalog = load_catalog(clips, csv_file)
    wer_path = locate_scorer("wer", wer_bin)
    ecapa_path = locate_scorer("ecapa", ecapa_bin)
    analyse, backend_name, emotion_detail, emotion_ready = _resolve_emotion(
        backend,
        emotion_analyse,
        emotion_measurement,
        emotion_backend_name,
    )
    scorers = {
        "ecapa": _scorer_status(ecapa_path, "galion-ecapa is not on this machine"),
        "emotion_match": {
            "backend": backend_name if emotion_ready else None,
            "detail": emotion_detail,
            "status": "ready" if emotion_ready else "absent",
        },
        "wer": _scorer_status(wer_path, "galion-wer is not on this machine"),
    }
    candidates = [
        _score_candidate(
            spec,
            catalog,
            reply,
            wer_path,
            ecapa_path,
            analyse if emotion_ready else None,
            emotion_detail,
            backend_name if emotion_ready else None,
        )
        for spec in catalog["candidates"]
    ]
    report = Report(
        clips_dir=clips,
        csv_path=csv_file,
        sweep_md=note,
        reply_line=reply,
        files=catalog["files"],
        unexpected=catalog["unexpected"],
        candidates=candidates,
        scorers=scorers,
        require_scorers=require_scorers,
    )
    body = report.as_dict()
    write_reports(out_dir, body)
    return _exit_code(report), body


def load_catalog(clips_dir: Path, csv_path: Path) -> dict[str, object]:
    if not clips_dir.is_dir():
        raise SweepError(f"missing emotion-sweep clips: {clips_dir}")
    rows = _read_rows(csv_path)
    personas: list[str] = []
    user_emotions: list[str] = []
    for row in rows:
        if row["persona"] not in personas:
            personas.append(row["persona"])
        if row["user_emotion"] not in user_emotions:
            user_emotions.append(row["user_emotion"])
    expected: list[ClipFact] = []
    for row in rows:
        expected.append(
            _fact(
                clips_dir,
                row["file"],
                role="persona-case",
                persona=row["persona"],
                user_emotion=row["user_emotion"],
                mood_history=row["mood_history"],
                mood_label=row["mood_label"],
            )
        )
    for persona in personas:
        expected.append(
            _fact(
                clips_dir,
                f"{persona}-reference-rest.wav",
                role="reference",
                persona=persona,
                user_emotion=None,
                mood_history="rest",
                mood_label=None,
            )
        )
        expected.append(
            _fact(
                clips_dir,
                f"{persona}-v1-baseline.wav",
                role="v1-baseline",
                persona=persona,
                user_emotion=None,
                mood_history=None,
                mood_label=None,
            )
        )
    for emotion in user_emotions:
        expected.append(
            _fact(
                clips_dir,
                f"user-{emotion}.wav",
                role="user-cue",
                persona=None,
                user_emotion=emotion,
                mood_history=None,
                mood_label=None,
            )
        )
    known = {item.file for item in expected}
    unexpected = sorted(
        path.name
        for path in clips_dir.iterdir()
        if path.is_file() and path.suffix.lower() == ".wav" and path.name not in known
    )
    candidate_specs = []
    for persona in personas:
        cases = [item for item in expected if item.role == "persona-case" and item.persona == persona]
        candidate_specs.append(
            {
                "clips": cases,
                "id": persona,
                "kind": "voice-v2",
                "persona": persona,
                "speaker_ref": f"{persona}-reference-rest.wav",
            }
        )
    for persona in personas:
        baselines = [item for item in expected if item.role == "v1-baseline" and item.persona == persona]
        candidate_specs.append(
            {
                "clips": baselines,
                "id": f"{persona}-v1",
                "kind": "v1-baseline",
                "persona": persona,
                "speaker_ref": f"{persona}-reference-rest.wav",
            }
        )
    return {"candidates": candidate_specs, "files": expected, "personas": personas, "unexpected": unexpected}


def render_markdown(report: dict[str, object]) -> str:
    gate = report["rollout_gate"]
    summary = report["summary"]
    inventory = report["inventory"]
    assert isinstance(gate, dict)
    assert isinstance(summary, dict)
    assert isinstance(inventory, dict)
    lines = [
        "# Voice V2 emotion-sweep gate",
        "",
        "Offline score of the frozen synthetic clips. No audio was generated "
        "and no voice was cloned.",
        "",
        f"Clips: `{report['clips_dir']}`. Manifest: `{report['manifest_csv']}`. "
        f"Reply line: \"{report['reply_line']}\"",
        "",
        f"Rollout gate (not a release): WER ≤ {gate['wer_max']}, "
        f"ECAPA > {gate['ecapa_min_exclusive']}, "
        f"emotion_match ≥ {gate['emotion_match_min']} "
        f"(`{gate['emotion_formula']}` against that persona's resting reference, "
        "not a per-sector capture-kit score). `rollout_claimed` is false. "
        f"{RELEASE_NOTE}",
        "",
        f"Gate: **{summary['gate']}**. "
        f"{summary['n_measured']}/{summary['n_candidates']} candidates measured, "
        f"{summary['n_passed']} passed, {summary['n_failed']} failed, "
        f"{summary['n_unmeasured']} unmeasured.",
        "",
        "| Candidate | Kind | Clips | WER | ECAPA | emotion_match | Gate |",
        "| --- | --- | ---: | --- | --- | --- | --- |",
    ]
    candidates = report["candidates"]
    assert isinstance(candidates, list)
    for row in candidates:
        assert isinstance(row, dict)
        lines.append(
            "| {cid} | {kind} | {n} | {wer} | {ecapa} | {emotion} | {gate} |".format(
                cid=row["id"],
                kind=row["kind"],
                n=row["n_clips"],
                wer=_metric_cell(row["wer"]),
                ecapa=_metric_cell(row["ecapa"]),
                emotion=_metric_cell(row["emotion_match"]),
                gate=_gate_cell(row["gate"]),
            )
        )
    lines.extend(["", "## Scorers", ""])
    scorers = report["scorers"]
    assert isinstance(scorers, dict)
    for name in ("wer", "ecapa", "emotion_match"):
        item = scorers[name]
        assert isinstance(item, dict)
        lines.append(f"- {name}: {item['status']}. {item['detail']}")
    blockers = report["blockers"]
    assert isinstance(blockers, list)
    if blockers:
        lines.extend(["", "## Blockers", ""])
        lines.extend(f"- {item}" for item in blockers)
        lines.append("")
        lines.append(
            "Unmeasured is not a pass and not a fail. "
            "The CPU stub was not used to fill emotion_match."
        )
    lines.extend(
        [
            "",
            "## Inventory",
            "",
            f"{inventory['present']}/{inventory['expected']} expected files present "
            f"({inventory['persona_cases']} persona cases, {inventory['references']} resting references, "
            f"{inventory['v1_baselines']} V1 baselines, {inventory['user_cues']} user cues). "
            "User cues are not candidates. V1 baselines use the same reply line and the same resting reference.",
            "",
        ]
    )
    missing = inventory["missing"]
    assert isinstance(missing, list)
    if missing:
        lines.append("Missing: " + ", ".join(f"`{name}`" for name in missing))
        lines.append("")
    unexpected = inventory["unexpected"]
    assert isinstance(unexpected, list)
    if unexpected:
        lines.append("Unexpected wavs were not scored: " + ", ".join(f"`{name}`" for name in unexpected))
        lines.append("")
    lines.append(str(report["prosody_note"]))
    lines.append("")
    return "\n".join(lines)


def write_reports(out_dir: Path, report: dict[str, object]) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (out_dir / "summary.md").write_text(render_markdown(report), encoding="utf-8")


def _score_candidate(
    spec: dict[str, object],
    catalog: dict[str, object],
    reply_line: str,
    wer_bin: Path | None,
    ecapa_bin: Path | None,
    analyse,
    emotion_detail: str,
    emotion_backend: str | None,
) -> Candidate:
    clips = spec["clips"]
    assert isinstance(clips, list)
    persona = str(spec["persona"])
    speaker_name = str(spec["speaker_ref"])
    files = catalog["files"]
    assert isinstance(files, list)
    speaker = next((item for item in files if item.file == speaker_name), None)
    speaker_path = speaker.path if speaker is not None and speaker.present and not speaker.error else None
    usable = [item for item in clips if isinstance(item, ClipFact) and item.present and not item.error]
    broken = len(usable) != len(clips) or speaker_path is None
    with tempfile.TemporaryDirectory(prefix="emotion-sweep-") as tmp:
        work = Path(tmp)
        if broken:
            detail = "clip set is incomplete; scorers were not run"
            wer = Metric("wer", "error", None, WER_MAX, "<=", False, detail)
            ecapa = Metric("ecapa", "error", None, ECAPA_MIN, ">", False, detail)
            emotion = _emotion_metric(None, detail, emotion_backend, status="error")
        else:
            text_dir = work / "text"
            wer_dir = work / "wer"
            ecapa_dir = work / "ecapa"
            text_dir.mkdir()
            wer_dir.mkdir()
            ecapa_dir.mkdir()
            for clip in usable:
                (text_dir / f"{Path(clip.file).stem}.txt").write_text(reply_line + "\n", encoding="utf-8")
            wer = _from_guard(
                score_wer(
                    wer_bin,
                    [clip.path for clip in usable],
                    text_dir,
                    WER_MAX,
                    wer_dir,
                )
            )
            ecapa = _from_guard(
                score_ecapa(
                    ecapa_bin,
                    [clip.path for clip in usable],
                    speaker_path,
                    ECAPA_MIN,
                    ecapa_dir,
                )
            )
            wer.extra = _copied_per_clip(wer_dir / "wer.json", "files")
            ecapa.extra = _copied_per_clip(ecapa_dir / "ecapa.json", "scores")
            emotion = _score_emotion(usable, speaker_path, analyse, emotion_detail, emotion_backend)
    return Candidate(
        id=str(spec["id"]),
        persona=persona,
        kind=str(spec["kind"]),
        speaker_ref=speaker_name,
        clips=list(clips),
        wer=wer,
        ecapa=ecapa,
        emotion_match=emotion,
        gate=judge_rollout(wer, ecapa, emotion),
    )


def _score_emotion(
    clips: list[ClipFact],
    speaker_ref: Path | None,
    analyse,
    absent_detail: str,
    backend_name: str | None,
) -> Metric:
    if analyse is None or speaker_ref is None:
        return _emotion_metric(None, absent_detail, None, status="absent")
    try:
        ref = _split_analysis(analyse(speaker_ref))
        per_clip = []
        categories = []
        sims = []
        for clip in clips:
            labels, posterior, embedding = _split_analysis(analyse(clip.path))
            if labels != ref[0]:
                raise SweepError(f"{clip.file} posterior labels differ from the resting reference")
            category, sim, score = pair_emotion(posterior, embedding, ref[1], ref[2])
            categories.append(category)
            sims.append(sim)
            per_clip.append(
                {
                    "category_blend": _round(category),
                    "emb_sim": _round(sim),
                    "emotion_match": _round(score),
                    "file": clip.file,
                }
            )
    except (BackendUnavailable, ConfigError, OSError, RuntimeError, SweepError, ValueError) as exc:
        return _emotion_metric(None, f"emotion recogniser failed: {exc}", backend_name, status="error")
    category_mean = sum(categories) / len(categories)
    sim_mean = sum(sims) / len(sims)
    score = emotion_match_value(category_mean, sim_mean)
    return Metric(
        name="emotion_match",
        status="ok",
        value=score,
        threshold=EMOTION_MIN,
        comparison=">=",
        passed=meets_min(score, EMOTION_MIN),
        detail="scored against the persona resting reference in this listening set",
        extra={
            "backend": backend_name,
            "category_blend": _round(category_mean),
            "emb_sim": _round(sim_mean),
            "formula": EMOTION_FORMULA,
            "per_clip": per_clip,
            "reference_role": "persona resting reference",
        },
    )


def _emotion_metric(value: float | None, detail: str, backend: str | None, status: str) -> Metric:
    return Metric(
        name="emotion_match",
        status=status,
        value=value,
        threshold=EMOTION_MIN,
        comparison=">=",
        passed=False if status == "error" else None,
        detail=detail,
        extra={
            "backend": backend,
            "formula": EMOTION_FORMULA,
            "reference_role": "persona resting reference",
        },
    )


def _resolve_emotion(backend, analyse, measurement, backend_name):
    if measurement is True:
        if analyse is None:
            raise SweepError("emotion measurement was requested without an analyser")
        return analyse, backend_name or "scripted", "scored by the supplied analyser", True
    if measurement is False:
        return None, None, STUB_REFUSAL, False
    selected = (backend or os.environ.get("EMOTION_V2_BACKEND") or "auto").strip().lower()
    if selected in {"stub", "fake", "cpu-stub", "sine"}:
        return None, None, STUB_REFUSAL, False
    if selected not in {"auto", "default", "superb", "superb/wav2vec2-base-superb-er"}:
        return None, None, f"unknown emotion backend {backend!r}. emotion_match is unmeasured.", False
    found = discover_superb_weights()
    if found is None:
        return None, None, WEIGHTS_ABSENT, False
    recogniser = SuperbBackend(found)

    def _analyse(path: Path) -> Analysis:
        samples, rate = read_wav(path)
        return recogniser.analyse(samples, rate)

    return _analyse, recogniser.name, f"local weights at {found}", True


def _from_guard(result) -> Metric:
    return Metric(
        name=result.name,
        status=result.status,
        value=result.value,
        threshold=result.threshold,
        comparison=result.comparison,
        passed=result.passed,
        detail=result.detail,
    )


def _copied_per_clip(path: Path, key: str) -> dict[str, object]:
    """Copy per-clip rows the scorer itself wrote. Absent files add nothing."""
    if not path.is_file():
        return {}
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    if not isinstance(payload, dict) or key not in payload:
        return {}
    return {"scorer_" + key: payload[key]}


def _split_analysis(result) -> tuple[tuple[str, ...], list[float], list[float]]:
    if isinstance(result, Analysis):
        return result.labels, list(result.posterior), list(result.embedding)
    labels, posterior, embedding = result
    return tuple(labels), list(posterior), list(embedding)


def _read_rows(csv_path: Path) -> list[dict[str, str]]:
    if not csv_path.is_file():
        raise SweepError(f"missing sweep manifest: {csv_path}")
    try:
        text = csv_path.read_text(encoding="utf-8")
    except OSError as exc:
        raise SweepError(f"could not read {csv_path}: {exc}") from exc
    reader = csv.DictReader(text.splitlines())
    required = ("persona", "user_emotion", "mood_history", "mood_label", "file")
    if reader.fieldnames is None or any(name not in reader.fieldnames for name in required):
        raise SweepError(f"{csv_path} is missing one of {required}")
    rows: list[dict[str, str]] = []
    seen: set[str] = set()
    for raw in reader:
        file_name = _safe_name(str(raw["file"] or ""))
        if file_name in seen:
            raise SweepError(f"duplicate clip in {csv_path}: {file_name}")
        seen.add(file_name)
        persona = str(raw["persona"] or "").strip()
        emotion = str(raw["user_emotion"] or "").strip()
        if not persona or not emotion:
            raise SweepError(f"{file_name} is missing persona or user_emotion")
        rows.append(
            {
                "file": file_name,
                "mood_history": str(raw["mood_history"] or "").strip(),
                "mood_label": str(raw["mood_label"] or "").strip(),
                "persona": persona,
                "user_emotion": emotion,
            }
        )
    if not rows:
        raise SweepError(f"{csv_path} has no persona cases")
    return rows


def _fact(
    clips_dir: Path,
    file_name: str,
    *,
    role: str,
    persona: str | None,
    user_emotion: str | None,
    mood_history: str | None,
    mood_label: str | None,
) -> ClipFact:
    safe = _safe_name(file_name)
    path = clips_dir / safe
    fact = ClipFact(
        role=role,
        file=safe,
        path=path,
        persona=persona,
        user_emotion=user_emotion,
        mood_history=mood_history,
        mood_label=mood_label,
        present=path.is_file(),
        sha256=None,
        sample_rate=None,
        channels=None,
        sample_width=None,
        duration_s=None,
        error=None,
    )
    if not fact.present:
        fact.error = "missing"
        return fact
    try:
        fact.sha256 = _sha256(path)
        with wave.open(str(path), "rb") as handle:
            fact.channels = handle.getnchannels()
            fact.sample_width = handle.getsampwidth()
            fact.sample_rate = handle.getframerate()
            frames = handle.getnframes()
        if fact.sample_rate is None or fact.sample_rate <= 0 or frames <= 0:
            fact.error = "empty or unusable wav header"
        else:
            fact.duration_s = frames / float(fact.sample_rate)
    except (OSError, wave.Error) as exc:
        fact.error = f"unreadable wav: {exc}"
    return fact


def _safe_name(name: str) -> str:
    if not name or name != Path(name).name or name.startswith("."):
        raise SweepError(f"refusing clip name {name!r}")
    return name


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _locked_thresholds(thresholds: Thresholds) -> None:
    if not math.isclose(thresholds.wer_max, WER_MAX, rel_tol=0.0, abs_tol=1e-12):
        raise SweepError(
            f"thresholds wer_max is {thresholds.wer_max}; the rollout gate is locked at {WER_MAX}"
        )
    if not math.isclose(thresholds.ecapa_min, ECAPA_MIN, rel_tol=0.0, abs_tol=1e-12):
        raise SweepError(
            f"thresholds ecapa_min is {thresholds.ecapa_min}; the rollout gate is locked at {ECAPA_MIN}"
        )
    if not math.isclose(thresholds.emotion_target, EMOTION_MIN, rel_tol=0.0, abs_tol=1e-12):
        raise SweepError(
            f"thresholds emotion target is {thresholds.emotion_target}; the rollout gate is locked at {EMOTION_MIN}"
        )


def _scorer_status(path: Path | None, absent_detail: str) -> dict[str, object]:
    if path is None:
        return {"detail": absent_detail, "path": None, "status": "absent"}
    if not path.is_file() or not os.access(path, os.X_OK):
        return {
            "detail": f"scorer is not executable: {path}",
            "path": _display_path(path),
            "status": "error",
        }
    return {
        "detail": "executable located; candidate rows say whether it scored",
        "path": _display_path(path),
        "status": "present",
    }


def _blockers(scorers: dict[str, dict[str, object]]) -> list[str]:
    found = []
    for name in ("wer", "ecapa", "emotion_match"):
        item = scorers[name]
        if item["status"] != "ready" and item["status"] != "present":
            found.append(f"{name}: {item['detail']}")
    return found


def _exit_code(report: Report) -> int:
    if report.missing:
        return 2
    statuses = [str(row.gate["status"]) for row in report.candidates]
    if any(status == "error" for status in statuses):
        return 1
    pairs = list(zip(statuses, report.candidates))
    if report.require_scorers and any(
        status != "measured" or row.gate["pass"] is not True for status, row in pairs
    ):
        return 1
    if any(status == "measured" and row.gate["pass"] is not True for status, row in pairs):
        return 1
    return 0


def _metric_cell(metric: object) -> str:
    assert isinstance(metric, dict)
    if metric["status"] == "ok" and isinstance(metric["value"], float):
        mark = "pass" if metric["pass"] else "fail"
        return f"{metric['value']:.4f} ({mark})"
    return str(metric["status"])


def _gate_cell(gate: object) -> str:
    assert isinstance(gate, dict)
    if gate["status"] == "measured":
        return "pass" if gate["pass"] else "fail"
    return str(gate["status"])


def _display_path(path: Path) -> str:
    resolved = path.resolve()
    try:
        root = repo_root().resolve()
    except ConfigError:
        return str(resolved)
    try:
        return resolved.relative_to(root).as_posix()
    except ValueError:
        return str(resolved)


def _round(value: float | None) -> float | None:
    if value is None:
        return None
    return round(float(value), 6)
