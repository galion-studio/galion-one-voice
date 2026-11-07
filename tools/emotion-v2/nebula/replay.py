# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Read a JSON file of posteriors and run them through the live feed.

No audio is opened. The file is numbers already produced by a scorer, or a
hand-written fixture for the unit tests.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from .feed import DEFAULT_TAU_MS, FeedError, NebulaFeed, Posterior, _tau, _voice, encode

REPLAY_SCHEMA = "galion-nebula-replay/1"


@dataclass(frozen=True)
class Replay:
    voice_id: str
    tau_ms: float
    posteriors: tuple[Posterior, ...]


def load_replay(
    path: Path,
    *,
    voice_id: str | None = None,
    tau_ms: float | None = None,
) -> Replay:
    """Load ``.json`` (one document) or ``.jsonl`` (one posterior per line)."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise FeedError(f"cannot read replay file: {exc}") from exc
    if path.suffix == ".jsonl":
        rows = _jsonl(text)
        if voice_id is None:
            raise FeedError("a JSONL replay needs --voice")
        posteriors = tuple(Posterior.from_mapping(row) for row in rows)
        return _finish(voice_id, tau_ms, posteriors)
    try:
        raw = json.loads(text)
    except json.JSONDecodeError as exc:
        raise FeedError(f"replay file is not JSON: {exc}") from exc
    if isinstance(raw, list):
        if voice_id is None:
            raise FeedError("a posterior array needs --voice")
        posteriors = tuple(Posterior.from_mapping(row) for row in raw)
        return _finish(voice_id, tau_ms, posteriors)
    if isinstance(raw, dict) and "posteriors" in raw:
        schema = raw.get("schema")
        if schema is not None and schema != REPLAY_SCHEMA:
            raise FeedError(f"unsupported replay schema {schema!r}")
        file_voice = raw.get("voice_id")
        if file_voice is not None and not isinstance(file_voice, str):
            raise FeedError("voice_id must be a string")
        chosen_voice = voice_id if voice_id is not None else file_voice
        if chosen_voice is None:
            raise FeedError("replay needs a voice_id")
        if file_voice is not None and voice_id is not None and _voice(file_voice) != _voice(voice_id):
            raise FeedError("voice_id does not match the replay file")
        file_tau = raw.get("tau_ms")
        if file_tau is not None and tau_ms is None:
            chosen_tau: float | None = _tau(file_tau)
        else:
            chosen_tau = tau_ms
        rows = raw["posteriors"]
        if not isinstance(rows, list):
            raise FeedError("posteriors must be an array")
        posteriors = tuple(Posterior.from_mapping(row) for row in rows)
        return _finish(chosen_voice, chosen_tau, posteriors)
    raise FeedError(
        "replay file must be a posterior array, JSONL, or an object with posteriors"
    )


def _finish(voice_id: str, tau_ms: float | None, posteriors: tuple[Posterior, ...]) -> Replay:
    if not posteriors:
        raise FeedError("replay has no posteriors")
    return Replay(
        voice_id=_voice(voice_id),
        tau_ms=DEFAULT_TAU_MS if tau_ms is None else _tau(tau_ms),
        posteriors=posteriors,
    )


def _jsonl(text: str) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for index, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        try:
            raw = json.loads(stripped)
        except json.JSONDecodeError as exc:
            raise FeedError(f"replay line {index} is not JSON: {exc}") from exc
        if not isinstance(raw, dict):
            raise FeedError(f"replay line {index} must be an object")
        rows.append(raw)
    return rows


class Sink:
    def write(self, line: str) -> None:
        raise NotImplementedError


class ListSink(Sink):
    def __init__(self) -> None:
        self.lines: list[str] = []

    def write(self, line: str) -> None:
        self.lines.append(line)


def play(replay: Replay, sink: Sink) -> int:
    """Push every posterior in order. Return the number of frames written."""
    feed = NebulaFeed(replay.voice_id, replay.tau_ms)
    count = 0
    for posterior in replay.posteriors:
        frame = feed.push(posterior)
        sink.write(encode(frame))
        count += 1
    return count
