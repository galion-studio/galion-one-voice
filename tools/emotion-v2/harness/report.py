# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Per-emotion JSON and one markdown table."""

from __future__ import annotations

import json
from pathlib import Path

from .score import Summary


def write_reports(out_dir: Path, summary: Summary) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    for row in summary.scores:
        _write_json(out_dir / f"{row.emotion}.json", row.as_dict())
    _write_json(out_dir / "summary.json", summary.as_dict())
    (out_dir / "summary.md").write_text(render_markdown(summary), encoding="utf-8")


def render_markdown(summary: Summary) -> str:
    targets = {row.target for row in summary.scores}
    if len(targets) == 1:
        target_sentence = f"Target {next(iter(targets)):.2f} on every emotion."
    else:
        target_sentence = "Targets are per emotion; see the Target column."
    lines = [
        "# Emotion V2 — Samantha",
        "",
        f"Tuning standard: {summary.tuning_hz} Hz. Backend: `{summary.backend}`. "
        "The emotion number is the categorical recogniser posterior compared "
        f"with that emotion's reference. {target_sentence} "
        "WER must be ≤ 0.018 and ECAPA must be > 0.7057 when those scorers ran. "
        "Absent means that scorer was not on this machine.",
        "",
        "| Emotion | Posterior vs reference | Target | Embedding similarity | WER | ECAPA | Result |",
        "| --- | ---: | ---: | ---: | ---: | ---: | --- |",
    ]
    notes: list[str] = []
    for row in summary.scores:
        lines.append(
            "| {display} | {posterior} | {target:.2f} | {embedding} | {wer} | {ecapa} | {result} |".format(
                display=row.display,
                posterior=_cell(row.posterior_vs_reference),
                target=row.target,
                embedding=_cell(row.embedding_similarity),
                wer=_guard_cell(row.wer.value, row.wer.status),
                ecapa=_guard_cell(row.ecapa.value, row.ecapa.status),
                result="pass" if row.passed else "fail",
            )
        )
        if row.error:
            notes.append(f"- {row.display}: {row.error}")
    if notes:
        lines.extend(["", "## Notes", ""])
        lines.extend(notes)
    lines.append("")
    return "\n".join(lines)


def _cell(value: float | None) -> str:
    if value is None:
        return "—"
    return f"{value:.4f}"


def _guard_cell(value: float | None, status: str) -> str:
    if status == "ok" and value is not None:
        return f"{value:.4f}"
    return status


def _write_json(path: Path, payload: dict) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
