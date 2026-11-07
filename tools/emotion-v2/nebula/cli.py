# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Command line for the Nebula live feed.

``replay`` reads a JSON file of posteriors. ``feed`` reads the same objects
as newline-delimited JSON on stdin. Both write smoothed frames to stdout
and, when asked, to a loopback UDP port. Neither reads audio.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .feed import DEFAULT_TAU_MS, FeedError, NebulaFeed, Posterior, encode
from .replay import load_replay, play
from .udp import UdpSink, parse_udp


class _Stdout:
    def __init__(self, stream: object) -> None:
        self._stream = stream

    def write(self, line: str) -> None:
        self._stream.write(line + "\n")
        self._stream.flush()


class _Fanout:
    def __init__(self, sinks: list[object]) -> None:
        self._sinks = sinks

    def write(self, line: str) -> None:
        for sink in self._sinks:
            sink.write(line)

    def close(self) -> None:
        for sink in self._sinks:
            close = getattr(sink, "close", None)
            if close is not None:
                close()


def _sinks(args: argparse.Namespace) -> _Fanout:
    sinks: list[object] = []
    if args.stdout:
        sinks.append(_Stdout(args.output))
    if args.udp:
        host, port = parse_udp(args.udp)
        sinks.append(UdpSink(host, port))
    if not sinks:
        raise FeedError("choose stdout, a loopback UDP port, or both")
    return _Fanout(sinks)


def _add_output(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--tau-ms",
        type=float,
        default=None,
        help=f"EMA time constant in milliseconds (default {DEFAULT_TAU_MS:g})",
    )
    parser.add_argument(
        "--udp",
        default=None,
        help="loopback destination, for example 127.0.0.1:8765",
    )
    parser.add_argument(
        "--no-stdout",
        action="store_true",
        help="do not write newline-delimited JSON to stdout",
    )


def _build() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="nebula",
        description="Smooth Emotion V2 posteriors into Nebula live frames.",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    replay = sub.add_parser("replay", help="play a JSON file of posteriors")
    replay.add_argument("path", type=Path)
    replay.add_argument("--voice", default=None, help="required for an array or JSONL file")
    _add_output(replay)

    feed = sub.add_parser("feed", help="read posterior JSON lines from stdin")
    feed.add_argument("--voice", required=True, choices=("samantha", "jarvis"))
    _add_output(feed)
    return parser


def _cmd_replay(args: argparse.Namespace) -> int:
    replay = load_replay(args.path, voice_id=args.voice, tau_ms=args.tau_ms)
    sink = _sinks(args)
    try:
        play(replay, sink)
    finally:
        sink.close()
    return 0


def _cmd_feed(args: argparse.Namespace) -> int:
    tau = DEFAULT_TAU_MS if args.tau_ms is None else args.tau_ms
    feed = NebulaFeed(args.voice, tau)
    sink = _sinks(args)
    try:
        _feed_lines(args, feed, sink)
    finally:
        sink.close()
    return 0


def _feed_lines(args: argparse.Namespace, feed: NebulaFeed, sink: _Fanout) -> None:
    for index, line in enumerate(args.input, start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        try:
            raw = json.loads(stripped)
        except json.JSONDecodeError as exc:
            raise FeedError(f"stdin line {index} is not JSON: {exc}") from exc
        if not isinstance(raw, dict):
            raise FeedError(f"stdin line {index} must be an object")
        voice = raw.get("voice_id")
        if voice is not None and str(voice).strip().lower() != args.voice:
            raise FeedError(f"stdin line {index} voice_id does not match --voice")
        frame = feed.push(Posterior.from_mapping(raw))
        sink.write(encode(frame))


def main(argv: list[str] | None = None, *, input_stream: object | None = None, output_stream: object | None = None) -> int:
    parser = _build()
    args = parser.parse_args(argv)
    args.stdout = not args.no_stdout
    args.input = sys.stdin if input_stream is None else input_stream
    args.output = sys.stdout if output_stream is None else output_stream
    try:
        if args.command == "replay":
            return _cmd_replay(args)
        if args.command == "feed":
            return _cmd_feed(args)
    except FeedError as exc:
        print(f"nebula: {exc}", file=sys.stderr)
        return 2
    parser.error(f"unknown command {args.command}")
    return 2
