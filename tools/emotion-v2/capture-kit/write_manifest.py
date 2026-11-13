#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
"""Validate capture WAV format and write sha256 manifests.

This tool reads audio; it does not rewrite it, and it does not write a
consent record. Acoustic loudness, reverb, tilt, and the session seal stay
with tools/emotion-v2/refcheck.

Subcommands:

  check    Validate one WAV (48 kHz / 24-bit packed PCM / mono)
  session  Hash every WAV in a session directory and write MANIFEST.json
  refs     Add or update one line/emotion entry in the per-sector refs map
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path
from typing import Any

STANDARD = "galion-voice-v2-432"
CONSENT = "owner-own-recordings-only"
ORIGINALS = "read-only"
TOOL = "capture-kit"
REFS_SCHEMA = "galion-emotion-v2-per-sector-refs/1"
CONSENT_TEMPLATE_SCHEMA = "galion-emotion-v2-capture-consent/1"

LINES = ("samantha", "jarvis")
OFFICIAL_EMOTIONS = (
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
OPTIONAL_EMOTIONS = {
    "seductive": frozenset({"samantha"}),
}
KNOWN_EMOTIONS = OFFICIAL_EMOTIONS + tuple(OPTIONAL_EMOTIONS)

SAMPLE_RATE = 48000
BIT_DEPTH = 24
CHANNELS = 1
PACKED_BLOCK_ALIGN = 3  # mono 24-bit packed
PCM_FORMAT = 1
WAVE_FORMAT_EXTENSIBLE = 0xFFFE
PCM_SUBFORMAT = bytes(
    [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]
)

SESSION_ID_RE = re.compile(r"^\d{4}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*$")


class FormatError(ValueError):
    """The file is not a passing capture WAV."""


class ConsentTemplateError(ValueError):
    """An unfilled template, or any consent path, was passed in error."""


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _read_u16(data: bytes, off: int) -> int:
    return int.from_bytes(data[off : off + 2], "little")


def _read_u32(data: bytes, off: int) -> int:
    return int.from_bytes(data[off : off + 4], "little")


def _chunks(data: bytes):
    if len(data) < 12 or data[0:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise FormatError("not a little-endian RIFF/WAVE file")
    pos = 12
    end = 8 + _read_u32(data, 4)
    if end > len(data):
        end = len(data)
    while pos + 8 <= end:
        name = data[pos : pos + 4]
        size = _read_u32(data, pos + 4)
        start = pos + 8
        stop = start + size
        if stop > len(data):
            raise FormatError("WAV chunk overruns the file")
        yield name, data[start:stop]
        pos = stop + (size & 1)
        if pos > len(data):
            break


def parse_wav_format(data: bytes) -> dict[str, Any]:
    """Return format fields or raise FormatError."""
    fmt = None
    data_size = None
    for name, body in _chunks(data):
        if name == b"fmt ":
            fmt = body
        elif name == b"data":
            data_size = len(body)
    if fmt is None or len(fmt) < 16:
        raise FormatError("WAV is missing a fmt chunk")
    if data_size is None:
        raise FormatError("WAV is missing a data chunk")

    format_tag = _read_u16(fmt, 0)
    channels = _read_u16(fmt, 2)
    sample_rate = _read_u32(fmt, 4)
    byte_rate = _read_u32(fmt, 8)
    block_align = _read_u16(fmt, 12)
    bits = _read_u16(fmt, 14)
    if format_tag == WAVE_FORMAT_EXTENSIBLE:
        if len(fmt) < 40:
            raise FormatError("WAVE_FORMAT_EXTENSIBLE fmt chunk is too short")
        cb_size = _read_u16(fmt, 16)
        if cb_size < 22 or len(fmt) < 24 + 16:
            raise FormatError("WAVE_FORMAT_EXTENSIBLE subformat is missing")
        valid_bits = _read_u16(fmt, 18)
        subformat = fmt[24:40]
        if subformat != PCM_SUBFORMAT:
            raise FormatError("WAVE_FORMAT_EXTENSIBLE subformat is not PCM")
        if valid_bits not in (0, BIT_DEPTH) and valid_bits != bits:
            if valid_bits != BIT_DEPTH:
                raise FormatError(f"valid bits per sample is {valid_bits}, not 24")
    elif format_tag != PCM_FORMAT:
        raise FormatError(f"format tag {format_tag} is not integer PCM")

    errors: list[str] = []
    if sample_rate != SAMPLE_RATE:
        errors.append(f"sample_rate {sample_rate} != {SAMPLE_RATE}")
    if bits != BIT_DEPTH:
        errors.append(f"bit_depth {bits} != {BIT_DEPTH}")
    if channels != CHANNELS:
        errors.append(f"channels {channels} != {CHANNELS}")
    if block_align != PACKED_BLOCK_ALIGN * channels:
        errors.append(
            f"block_align {block_align} is not packed 24-bit "
            f"(expected {PACKED_BLOCK_ALIGN * channels}); "
            "24-bit samples in a 32-bit word fail this gate"
        )
    expected_rate = sample_rate * block_align
    if byte_rate != expected_rate:
        errors.append(f"byte_rate {byte_rate} != {expected_rate}")
    if errors:
        raise FormatError("; ".join(errors))
    return {
        "sample_rate": sample_rate,
        "bit_depth": bits,
        "channels": channels,
        "block_align": block_align,
        "format_tag": format_tag,
        "data_bytes": data_size,
    }


def inspect_wav(path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    info = parse_wav_format(data)
    info["path"] = str(path)
    info["bytes"] = len(data)
    info["sha256"] = sha256_hex(data)
    info["format_verdict"] = "PASS"
    return info


def emotion_of(rel: str) -> str | None:
    for part in Path(rel).parts:
        slug = part.lower()
        if slug in KNOWN_EMOTIONS:
            return slug
    return None


def role_of(rel: str) -> str:
    for part in Path(rel).parts:
        if part.lower() == "calibration":
            return "calibration"
    return "emotion"


def line_of_path(rel: str, explicit: str | None) -> str | None:
    if explicit:
        return explicit
    for part in Path(rel).parts:
        if part.lower() in LINES:
            return part.lower()
    return None


def session_id_of(dir_path: Path, explicit: str | None) -> str | None:
    if explicit:
        return explicit
    name = dir_path.name
    if SESSION_ID_RE.match(name):
        return name
    return None


def collect_wavs(root: Path) -> list[Path]:
    found: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        dirnames[:] = [d for d in sorted(dirnames) if not d.startswith(".")]
        base = Path(dirpath)
        for name in dirnames:
            pth = base / name
            if pth.is_symlink():
                raise FormatError(f"symlink is not an original ({pth})")
        for name in sorted(filenames):
            if name.startswith("."):
                continue
            if name.lower() == "manifest.json":
                continue
            pth = base / name
            if pth.is_symlink():
                raise FormatError(f"symlink is not an original ({pth})")
            if name.lower().endswith(".wav"):
                found.append(pth)
    found.sort()
    return found


def relative_to(root: Path, path: Path) -> str:
    rel = path.resolve().relative_to(root.resolve())
    parts = rel.parts
    if not parts or any(p in (".", "..") for p in parts):
        raise FormatError(f"bad relative path {rel}")
    return "/".join(parts)


def build_session_manifest(
    session_dir: Path,
    *,
    line: str | None,
    session_id: str | None,
    persona: str | None,
) -> dict[str, Any]:
    if not session_dir.is_dir():
        raise FormatError(f"{session_dir} is not a directory")
    wavs = collect_wavs(session_dir)
    if not wavs:
        raise FormatError(f"no WAV files under {session_dir}")
    files = []
    errors = []
    for wav in wavs:
        rel = relative_to(session_dir, wav)
        role = role_of(rel)
        emotion = emotion_of(rel)
        if role == "calibration":
            emotion = None
        elif emotion is None:
            errors.append(f"{rel}: path is not under a known emotion or calibration folder")
            continue
        elif emotion == "seductive" and (line or "").lower() == "jarvis":
            errors.append(f"{rel}: seductive is Samantha-only")
            continue
        try:
            info = inspect_wav(wav)
        except FormatError as exc:
            errors.append(f"{rel}: {exc}")
            files.append(
                {
                    "path": rel,
                    "sha256": sha256_hex(wav.read_bytes()),
                    "bytes": wav.stat().st_size,
                    "emotion": emotion,
                    "role": role,
                    "verdict": None,
                    "format_verdict": "FAIL",
                    "format_error": str(exc),
                }
            )
            continue
        files.append(
            {
                "path": rel,
                "sha256": info["sha256"],
                "bytes": info["bytes"],
                "emotion": emotion,
                "role": role,
                "verdict": None,
                "format_verdict": "PASS",
            }
        )
    resolved_line = line or line_of_path(str(session_dir), None)
    resolved_session = session_id_of(session_dir, session_id)
    if resolved_line and resolved_line not in LINES:
        raise FormatError(f"line must be samantha or jarvis, not {resolved_line!r}")
    if resolved_session and not SESSION_ID_RE.match(resolved_session):
        raise FormatError(
            f"session id {resolved_session!r} must be YYYY-MM-DD-<room> "
            "with a lowercase room slug"
        )
    persona_value = persona or resolved_line or ""
    manifest = {
        "tool": TOOL,
        "standard": STANDARD,
        "persona": persona_value,
        "consent": CONSENT,
        "originals": ORIGINALS,
        "line": resolved_line or "",
        "session_id": resolved_session or "",
        "files": files,
    }
    if errors:
        manifest["format_errors"] = errors
    return manifest


def load_json(path: Path) -> dict[str, Any]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise FormatError(f"{path} is not JSON: {exc}") from exc
    if not isinstance(raw, dict):
        raise FormatError(f"{path} must be a JSON object")
    return raw


def refuse_consent_template(path: Path) -> None:
    """Refuse to treat the unfilled template, or to write consent."""
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return
    if not isinstance(raw, dict):
        return
    if raw.get("schema") == CONSENT_TEMPLATE_SCHEMA or raw.get("consent_kind") == "template":
        raise ConsentTemplateError(
            f"{path} is the unfilled consent template, not a signed record. "
            "This tool does not write or accept consent files."
        )
    if "recording_consent" in raw and "training_consent" in raw:
        raise ConsentTemplateError(
            f"{path} looks like a consent file. This tool does not write or "
            "accept consent files."
        )


def refs_from_payload(payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    entries: dict[str, dict[str, Any]] = {}

    def add(obj: Any) -> None:
        if not isinstance(obj, dict):
            raise FormatError("ref entry must be an object")
        line = str(obj.get("line") or "").strip().lower()
        emotion = str(obj.get("emotion") or "").strip().lower()
        if not line or not emotion:
            return
        key = f"{line}/{emotion}"
        entry = {
            "line": line,
            "emotion": emotion,
            "path": str(obj.get("path") or ""),
            "sha256": str(obj.get("sha256") or "").lower(),
        }
        for extra in ("bytes", "optional", "note", "source_session", "source_take"):
            if extra in obj:
                entry[extra] = obj[extra]
        entries[key] = entry

    refs = payload.get("refs")
    if isinstance(refs, dict):
        for key, obj in refs.items():
            if key.startswith("_"):
                continue
            if isinstance(obj, dict):
                if "line" not in obj or "emotion" not in obj:
                    if "/" in key:
                        line, emotion = key.split("/", 1)
                        merged = dict(obj)
                        merged.setdefault("line", line)
                        merged.setdefault("emotion", emotion)
                        add(merged)
                        continue
                add(obj)
    elif isinstance(refs, list):
        for obj in refs:
            add(obj)
    extra_list = payload.get("refs_list")
    if isinstance(extra_list, list):
        for obj in extra_list:
            add(obj)
    return entries


def write_refs_payload(entries: dict[str, dict[str, Any]]) -> dict[str, Any]:
    keys: list[str] = []
    for line in LINES:
        for emo in OFFICIAL_EMOTIONS:
            keys.append(f"{line}/{emo}")
    keys.append("samantha/seductive")
    for key in entries:
        if key not in keys:
            keys.append(key)
    refs_map = {}
    refs_list = []
    for key in keys:
        if key not in entries:
            continue
        entry = entries[key]
        refs_map[key] = entry
        refs_list.append(entry)
    return {
        "schema": REFS_SCHEMA,
        "standard": STANDARD,
        "consent": CONSENT,
        "originals": ORIGINALS,
        "refs": refs_map,
        "refs_list": refs_list,
    }


def update_refs(
    out: Path,
    *,
    line: str,
    emotion: str,
    wav: Path,
) -> dict[str, Any]:
    line = line.lower()
    emotion = emotion.lower()
    if line not in LINES:
        raise FormatError(f"line must be samantha or jarvis, not {line!r}")
    if emotion not in KNOWN_EMOTIONS:
        raise FormatError(f"unknown emotion slug {emotion!r}")
    if emotion in OPTIONAL_EMOTIONS and line not in OPTIONAL_EMOTIONS[emotion]:
        raise FormatError(f"{emotion} is not recorded for {line}")
    if out.exists():
        refuse_consent_template(out)
        payload = load_json(out)
        if (
            payload.get("schema") not in (None, REFS_SCHEMA)
            and "refs" not in payload
            and "refs_list" not in payload
        ):
            raise FormatError(f"{out} is not a per-sector-refs file")
        entries = refs_from_payload(payload)
    else:
        entries = {}
    info = inspect_wav(wav)
    entry = {
        "line": line,
        "emotion": emotion,
        "path": str(wav),
        "sha256": info["sha256"],
        "bytes": info["bytes"],
    }
    if emotion in OPTIONAL_EMOTIONS:
        entry["optional"] = True
    entries[f"{line}/{emotion}"] = entry
    return write_refs_payload(entries)


def dump_json(path: Path, payload: dict[str, Any]) -> None:
    if path.exists():
        refuse_consent_template(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, ensure_ascii=False) + "\n"
    path.write_text(text, encoding="utf-8")


def cmd_check(args: argparse.Namespace) -> int:
    if args.consent:
        refuse_consent_template(Path(args.consent))
        raise ConsentTemplateError(
            "this tool does not read or write consent files"
        )
    path = Path(args.wav)
    if not path.is_file():
        raise FormatError(f"missing file: {path}")
    if path.is_symlink():
        raise FormatError(f"symlink is not an original ({path})")
    info = inspect_wav(path)
    report = {
        "tool": TOOL,
        "standard": STANDARD,
        "path": str(path),
        "role": args.role,
        "emotion": emotion_of(str(path)),
        "format_verdict": info["format_verdict"],
        "sample_rate": info["sample_rate"],
        "bit_depth": info["bit_depth"],
        "channels": info["channels"],
        "bytes": info["bytes"],
        "sha256": info["sha256"],
    }
    inferred = role_of(str(path))
    if args.role == "calibration" and inferred != "calibration":
        report["note"] = (
            "role is calibration, but the path is not under a calibration folder"
        )
    print(json.dumps(report, indent=2, ensure_ascii=False))
    return 0


def cmd_session(args: argparse.Namespace) -> int:
    if args.consent:
        refuse_consent_template(Path(args.consent))
        raise ConsentTemplateError(
            "this tool does not read or write consent files"
        )
    session_dir = Path(args.dir)
    out = Path(args.out) if args.out else session_dir / "MANIFEST.json"
    if out.exists():
        refuse_consent_template(out)
    manifest = build_session_manifest(
        session_dir,
        line=args.line,
        session_id=args.session_id,
        persona=args.persona,
    )
    dump_json(out, manifest)
    errors = manifest.get("format_errors") or []
    summary = {
        "tool": TOOL,
        "out": str(out),
        "files": len(manifest["files"]),
        "format_pass": sum(
            1 for f in manifest["files"] if f.get("format_verdict") == "PASS"
        ),
        "format_fail": len(errors),
    }
    print(json.dumps(summary, indent=2, ensure_ascii=False))
    return 1 if errors else 0


def cmd_refs(args: argparse.Namespace) -> int:
    if args.consent:
        refuse_consent_template(Path(args.consent))
        raise ConsentTemplateError(
            "this tool does not read or write consent files"
        )
    out = Path(args.out)
    payload = update_refs(
        out,
        line=args.line,
        emotion=args.emotion,
        wav=Path(args.wav),
    )
    dump_json(out, payload)
    key = f"{args.line.lower()}/{args.emotion.lower()}"
    summary = {
        "tool": TOOL,
        "schema": REFS_SCHEMA,
        "out": str(out),
        "updated": key,
        "sha256": payload["refs"][key]["sha256"],
        "entries": len(payload["refs"]),
    }
    print(json.dumps(summary, indent=2, ensure_ascii=False))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate Emotion V2 capture WAV format and write sha256 manifests."
    )
    sub = parser.add_subparsers(dest="cmd", required=True)

    common_consent = argparse.ArgumentParser(add_help=False)
    common_consent.add_argument(
        "--consent",
        help="Rejected on purpose: this tool does not read or write consent files.",
    )

    p_check = sub.add_parser("check", parents=[common_consent], help="Validate one WAV")
    p_check.add_argument("--wav", required=True, help="Path to a take or calibration WAV")
    p_check.add_argument(
        "--role",
        choices=("emotion", "calibration"),
        default="emotion",
    )
    p_check.set_defaults(func=cmd_check)

    p_session = sub.add_parser(
        "session", parents=[common_consent], help="Write a session MANIFEST.json"
    )
    p_session.add_argument("--dir", required=True, help="Session directory")
    p_session.add_argument("--out", help="Output path (default: DIR/MANIFEST.json)")
    p_session.add_argument("--line", choices=LINES, help="Voice line: samantha or jarvis")
    p_session.add_argument("--session-id", help="YYYY-MM-DD-<room>")
    p_session.add_argument("--persona", help="Persona field (defaults to --line)")
    p_session.set_defaults(func=cmd_session)

    p_refs = sub.add_parser(
        "refs",
        parents=[common_consent],
        help="Update one line/emotion entry in the per-sector refs map",
    )
    p_refs.add_argument("--line", required=True, choices=LINES)
    p_refs.add_argument("--emotion", required=True)
    p_refs.add_argument("--wav", required=True, help="Selected original take")
    p_refs.add_argument("--out", required=True, help="per-sector-refs.json path")
    p_refs.set_defaults(func=cmd_refs)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return int(args.func(args))
    except ConsentTemplateError as exc:
        print(f"consent: {exc}", file=sys.stderr)
        return 2
    except FormatError as exc:
        print(f"format: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
