#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""TTFA / streaming latency gate for live sectors.

Reads LOCKS.toml. Does not write it.

  ttfa_check.py frame
  ttfa_check.py presence
  ttfa_check.py gate [--write-eval]
  ttfa_check.py self-check
  GALION_TTFA_MEASURED=file.json ttfa_check.py gate

Stdout is one JSON object (schema galion-ttfa-gate/v1), except `frame` and
`presence`, which print their own objects. Exit 0 pass, 1 gate red, 2 usage
or IO.

A mock presence replay is kind "stub" and has no ttfa_ms. A wall-clock
ttfa_ms is accepted only from a measured file that also clears onset,
barge-in, and e_to_a.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    import tomllib
except ImportError:  # pragma: no cover
    sys.stderr.write("TTFA FAIL — python3 >= 3.11 (tomllib) required\n")
    sys.exit(2)

ROOT = Path(__file__).resolve().parents[4]
SYMPHONY = Path(__file__).resolve().parents[1]
LOCKS = ROOT / "LOCKS.toml"
LIVE = SYMPHONY / "live-sectors.json"
SELF_EVAL = SYMPHONY / "SELF-EVAL.md"
RECEIPT = SYMPHONY / "receipts" / "ttfa-stub.json"
HEARTBEAT = ROOT / "crates" / "galion-heartbeat" / "src" / "lib.rs"
PRESENCE_SPECS = (
    ("presence", ROOT / "specs" / "presence.json", "9f657f360c565dbf"),
    ("presence-wav", ROOT / "specs" / "presence-wav.json", "db7541ec9f6f42c7"),
)
FRAME_MS = 80
BARGE_CAP = 200
ONSET_CAP = 300
E2A_MS_CAP = 160
E2A_FRAMES_CAP = 2
BUDGET_P95_CAP = 60
STUB_ENGINES = {"mock", "stub", "protocol-stub", "unwired"}
FORBIDDEN = (
    "engagement",
    "retention",
    "session_length",
    "time_in_session",
    "dwell",
    "speech_elicited",
)
BEGIN = "<!-- TTFA-RECEIPT:BEGIN -->"
END = "<!-- TTFA-RECEIPT:END -->"


class GateError(Exception):
    def __init__(self, message: str, code: int = 1):
        super().__init__(message)
        self.code = code


def num(value, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise GateError(f"{label} must be a number")
    out = float(value)
    if out < 0 or out != out or out in (float("inf"), float("-inf")):
        raise GateError(f"{label} must be a finite number >= 0")
    return out


def integral(value: float):
    if value.is_integer():
        return int(value)
    return value


def dump(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))


def load_json(path: Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise GateError(f"missing {path}", 2) from exc
    except json.JSONDecodeError as exc:
        raise GateError(f"invalid JSON {path}: {exc}", 2) from exc


def load_locks() -> dict:
    if not LOCKS.is_file():
        raise GateError(f"missing {LOCKS}", 2)
    try:
        with LOCKS.open("rb") as fh:
            return tomllib.load(fh)
    except Exception as exc:  # noqa: BLE001 — surface the parse error
        raise GateError(f"cannot parse {LOCKS}: {exc}", 2) from exc


def locks_bytes() -> bytes:
    return LOCKS.read_bytes()


def forbidden_keys(obj, where: str) -> list[str]:
    found = []

    def walk(node):
        if isinstance(node, dict):
            for key, child in node.items():
                low = str(key).lower()
                if any(word in low for word in FORBIDDEN):
                    found.append(f"{where}: {key}")
                walk(child)
        elif isinstance(node, list):
            for child in node:
                walk(child)

    walk(obj)
    return found


def le(got, cap: float, label: str):
    if got > cap:
        raise GateError(f"{label} = {got} above {cap}")


def check_frame() -> dict:
    locks = load_locks()
    clock = locks.get("clock") or {}
    gates = locks.get("gates") or {}
    frame = clock.get("frame_ms")
    if frame != FRAME_MS:
        raise GateError(f"clock.frame_ms = {frame!r}, live-sector bar is {FRAME_MS}")
    if clock.get("frame_hz") != 12.5:
        raise GateError(f"clock.frame_hz = {clock.get('frame_hz')!r}, want 12.5")
    if clock.get("configurable") is not False:
        raise GateError("clock.configurable must stay false")
    budget = num(clock.get("frame_budget_p95_ms"), "clock.frame_budget_p95_ms")
    le(budget, BUDGET_P95_CAP, "clock.frame_budget_p95_ms")
    le(budget, FRAME_MS, "clock.frame_budget_p95_ms vs frame")

    barge = num(gates.get("barge_in_max_ms"), "gates.barge_in_max_ms")
    onset = num(gates.get("onset_p50_max_ms"), "gates.onset_p50_max_ms")
    e2a_ms = num(gates.get("affect_to_voice_max_ms"), "gates.affect_to_voice_max_ms")
    e2a_frames = num(gates.get("affect_to_voice_max_frames"), "gates.affect_to_voice_max_frames")
    le(barge, BARGE_CAP, "gates.barge_in_max_ms")
    le(onset, ONSET_CAP, "gates.onset_p50_max_ms")
    le(e2a_ms, E2A_MS_CAP, "gates.affect_to_voice_max_ms")
    le(e2a_frames, E2A_FRAMES_CAP, "gates.affect_to_voice_max_frames")
    if e2a_ms > frame * e2a_frames:
        raise GateError(
            f"gates.affect_to_voice_max_ms {e2a_ms} > frame_ms*frames {frame * e2a_frames}"
        )

    for name, path, _digest in PRESENCE_SPECS:
        spec = load_json(path)
        if spec.get("frame_ms") != FRAME_MS:
            raise GateError(f"{name} frame_ms = {spec.get('frame_ms')!r}, want {FRAME_MS}")
        targets = spec.get("targets") or {}
        for key, cap in (
            ("barge_in_max_ms", barge),
            ("onset_p50_max_ms", onset),
            ("affect_to_voice_max_ms", e2a_ms),
            ("affect_to_voice_max_frames", e2a_frames),
        ):
            got = num(targets.get(key), f"{name}.targets.{key}")
            if got > cap:
                raise GateError(f"{name}.targets.{key} = {got} looser than the lock {cap}")
        bad = forbidden_keys(spec, name)
        if bad:
            raise GateError("forbidden metric name: " + "; ".join(bad))

    text = HEARTBEAT.read_text(encoding="utf-8")
    needle = "pub const FRAME_MS: u32 = 80;"
    if needle not in text:
        raise GateError(f"heartbeat missing `{needle}`")

    sectors = load_json(LIVE)
    if sectors.get("schema") != "galion-ttfa-live-sectors/v1":
        raise GateError("live-sectors.json schema")
    if sectors.get("frame_ms") != FRAME_MS:
        raise GateError("live-sectors.json frame_ms")
    rows = sectors.get("sectors")
    if not isinstance(rows, list) or not rows:
        raise GateError("live-sectors.json has no sectors")
    ids = []
    for row in rows:
        if row.get("live") is not True:
            raise GateError(f"{row.get('id')}: live sector flag")
        if row.get("frame_bar_ms") != FRAME_MS:
            raise GateError(f"{row.get('id')}: frame_bar_ms must be {FRAME_MS}")
        ids.append(row.get("id"))
    for required in ("heartbeat", "barge-in", "onset", "e-to-a", "first-playable-frame"):
        if required not in ids:
            raise GateError(f"live sector missing: {required}")
    outside = sectors.get("outside_bar")
    if not isinstance(outside, list) or not outside:
        raise GateError("live-sectors.json outside_bar")

    return {
        "frame_bar": "pass",
        "frame_ms": FRAME_MS,
        "frame_hz": 12.5,
        "frame_budget_p95_ms": integral(budget),
        "barge_in_max_ms": integral(barge),
        "onset_p50_max_ms": integral(onset),
        "e_to_a_max_ms": integral(e2a_ms),
        "e_to_a_max_frames": integral(e2a_frames),
        "live_sectors": ids,
        "locks_edited": False,
    }


def _metric(report: dict, spec_id_prefix: str | None, key: str):
    out = []
    for spec in report.get("specs") or []:
        if spec_id_prefix and not str(spec.get("id", "")).startswith(spec_id_prefix):
            continue
        metrics = spec.get("metrics") or {}
        value = metrics.get(key)
        if value is None:
            continue
        out.append(num(value, f"{spec.get('id')}.{key}"))
    return out


def _must_status(report: dict, metric: str, scope: str) -> str:
    rows = []
    if scope == "run":
        rows = report.get("must") or []
    else:
        for spec in report.get("specs") or []:
            rows.extend(spec.get("must") or [])
    matched = [row for row in rows if str(row.get("assert", "")).split()[0] == metric]
    if not matched:
        return "absent"
    statuses = {row.get("status") for row in matched}
    if statuses == {"pass"}:
        return "pass"
    return "fail"


def check_presence_report(report: dict, name: str, digest: str, *, require_e2a: bool) -> dict:
    if report.get("backend") != "mock":
        raise GateError(f"{name}: backend {report.get('backend')!r} is not the mock; refusing to label it stub")
    if report.get("frame_ms") != FRAME_MS:
        raise GateError(f"{name}: frame_ms = {report.get('frame_ms')!r}")
    if report.get("verdict") != "green":
        raise GateError(f"{name}: verdict {report.get('verdict')!r}")
    if report.get("digest_fnv1a64") != digest:
        raise GateError(f"{name}: digest {report.get('digest_fnv1a64')} != {digest}")
    bad = forbidden_keys(report, name)
    if bad:
        raise GateError("forbidden metric name: " + "; ".join(bad))

    agg = report.get("aggregate") or {}
    onset = agg.get("onset_p50_ms")
    if onset is None:
        raise GateError(f"{name}: onset_p50_ms absent")
    onset_v = num(onset, f"{name}.onset_p50_ms")
    le(onset_v, ONSET_CAP, f"{name}.onset_p50_ms")
    if _must_status(report, "onset_p50_ms", "run") != "pass":
        raise GateError(f"{name}: onset_p50_ms must")

    barge_vals = _metric(report, None, "barge_in_ms")
    if not barge_vals:
        raise GateError(f"{name}: barge_in_ms absent")
    barge = max(barge_vals)
    le(barge, BARGE_CAP, f"{name}.barge_in_ms")
    if _must_status(report, "barge_in_ms", "spec") != "pass":
        raise GateError(f"{name}: barge_in_ms must")

    e2a_ms_vals = _metric(report, None, "e_to_a_ms")
    e2a_fr_vals = _metric(report, None, "e_to_a_frames")
    if require_e2a and (not e2a_ms_vals or not e2a_fr_vals):
        raise GateError(f"{name}: e_to_a absent")
    e2a_ms = max(e2a_ms_vals) if e2a_ms_vals else None
    e2a_fr = max(e2a_fr_vals) if e2a_fr_vals else None
    if e2a_ms is not None:
        le(e2a_ms, E2A_MS_CAP, f"{name}.e_to_a_ms")
    if e2a_fr is not None:
        le(e2a_fr, E2A_FRAMES_CAP, f"{name}.e_to_a_frames")
    if require_e2a:
        if _must_status(report, "e_to_a_ms", "spec") != "pass":
            raise GateError(f"{name}: e_to_a_ms must")
        if _must_status(report, "e_to_a_frames", "spec") != "pass":
            raise GateError(f"{name}: e_to_a_frames must")

    one_frame_rows = [
        row
        for spec in report.get("specs") or []
        for row in spec.get("should") or []
        if str(row.get("assert", "")).split()[:1] == ["barge_in_ms"]
    ]
    if not one_frame_rows:
        one_frame = "absent"
    elif all(row.get("status") == "pass" for row in one_frame_rows):
        one_frame = "pass"
    else:
        one_frame = "advisory"

    return {
        "spec": name,
        "kind": "stub",
        "measured": False,
        "backend": "mock",
        "frame_ms": FRAME_MS,
        "verdict": "green",
        "digest_fnv1a64": digest,
        "onset_p50_ms": integral(onset_v),
        "onset": "pass",
        "barge_in_ms": integral(barge),
        "barge_in": "pass",
        "barge_in_one_frame_should": one_frame,
        "e_to_a_ms": None if e2a_ms is None else integral(e2a_ms),
        "e_to_a_frames": None if e2a_fr is None else integral(e2a_fr),
        "e_to_a": "pass" if require_e2a else ("pass" if e2a_ms is not None else "absent"),
    }


def run_presence() -> dict:
    if not (ROOT / "crates" / "galion-presence").is_dir():
        raise GateError("crates/galion-presence missing", 2)
    runs = []
    for name, path, digest in PRESENCE_SPECS:
        proc = subprocess.run(
            [
                "cargo",
                "run",
                "--offline",
                "-q",
                "-p",
                "galion-presence",
                "--",
                str(path.relative_to(ROOT)),
                "--json",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        line = ""
        for candidate in reversed(proc.stdout.splitlines()):
            if candidate.startswith("{"):
                line = candidate
                break
        if proc.returncode not in (0, 1) or not line:
            tail = (proc.stderr or proc.stdout or "").strip().splitlines()
            detail = tail[-1] if tail else "no output"
            raise GateError(f"{name}: galion-presence rc={proc.returncode}: {detail}")
        try:
            report = json.loads(line)
        except json.JSONDecodeError as exc:
            raise GateError(f"{name}: report is not JSON: {exc}", 2) from exc
        if proc.returncode != 0:
            raise GateError(f"{name}: presence verdict red")
        runs.append(check_presence_report(report, name, digest, require_e2a=(name == "presence")))
    scripted = runs[0]
    return {
        "kind": "stub",
        "measured": False,
        "backend": "mock",
        "frame_ms": FRAME_MS,
        "onset_p50_ms": scripted["onset_p50_ms"],
        "onset": "pass",
        "barge_in_ms": max(row["barge_in_ms"] for row in runs),
        "barge_in": "pass",
        "barge_in_one_frame_should": (
            "pass"
            if all(row["barge_in_one_frame_should"] == "pass" for row in runs)
            else "advisory"
        ),
        "e_to_a_ms": scripted["e_to_a_ms"],
        "e_to_a_frames": scripted["e_to_a_frames"],
        "e_to_a": "pass",
        "runs": runs,
    }


def check_measured(path: Path) -> dict:
    doc = load_json(path)
    bad = forbidden_keys(doc, "measured")
    if bad:
        raise GateError("forbidden metric name: " + "; ".join(bad))
    if doc.get("schema") != "galion-ttfa-measured/v1":
        raise GateError("measured schema")
    if doc.get("kind") != "measured":
        raise GateError("measured kind must be measured")
    if doc.get("frame_ms") != FRAME_MS:
        raise GateError(f"measured frame_ms = {doc.get('frame_ms')!r}, want {FRAME_MS}")
    host = doc.get("host")
    if not isinstance(host, str) or not host.strip():
        raise GateError("measured host")
    engine = doc.get("engine")
    if not isinstance(engine, str) or engine.strip().lower() in STUB_ENGINES:
        raise GateError(f"measured engine {engine!r} is a stub label")
    ttfa = num(doc.get("ttfa_ms"), "ttfa_ms")
    onset = num(doc.get("onset_p50_ms"), "onset_p50_ms")
    barge = num(doc.get("barge_in_ms"), "barge_in_ms")
    e2a_ms = num(doc.get("e_to_a_ms"), "e_to_a_ms")
    e2a_fr = num(doc.get("e_to_a_frames"), "e_to_a_frames")
    le(ttfa, FRAME_MS, "ttfa_ms")
    le(onset, ONSET_CAP, "onset_p50_ms")
    le(barge, BARGE_CAP, "barge_in_ms")
    le(e2a_ms, E2A_MS_CAP, "e_to_a_ms")
    le(e2a_fr, E2A_FRAMES_CAP, "e_to_a_frames")
    claim = doc.get("product_claim", False)
    if not isinstance(claim, bool):
        raise GateError("product_claim must be a boolean")
    if claim and ttfa > FRAME_MS:
        raise GateError("product_claim requires ttfa_ms inside the frame bar")
    return {
        "present": True,
        "kind": "measured",
        "measured": True,
        "frame_ms": FRAME_MS,
        "host": host.strip(),
        "engine": engine.strip(),
        "ttfa_ms": integral(ttfa),
        "onset_p50_ms": integral(onset),
        "onset": "pass",
        "barge_in_ms": integral(barge),
        "barge_in": "pass",
        "e_to_a_ms": integral(e2a_ms),
        "e_to_a_frames": integral(e2a_fr),
        "e_to_a": "pass",
        "product_claim": claim,
    }


def receipt(frame: dict, presence: dict, measured: dict | None) -> dict:
    body = {
        "schema": "galion-ttfa-gate/v1",
        "program": "Galion Voice",
        "owner": "Maciej Grajczyk",
        "locks_edited": False,
        "frame_ms": FRAME_MS,
        "frame_bar": frame["frame_bar"],
        "kind": "measured" if measured else "stub",
        "stub": presence,
        "measured": measured if measured else {"present": False},
        "verdict": "pass",
    }
    encoded = dump(body)
    if measured is None and "ttfa_ms" in encoded:
        raise GateError("stub receipt carried ttfa_ms")
    return body


def write_eval(body: dict) -> None:
    pretty = json.dumps(body, indent=2, sort_keys=True) + "\n"
    block = f"{BEGIN}\n```json\n{pretty}```\n{END}"
    RECEIPT.parent.mkdir(parents=True, exist_ok=True)
    RECEIPT.write_text(pretty, encoding="utf-8")
    if SELF_EVAL.is_file() and BEGIN in SELF_EVAL.read_text(encoding="utf-8"):
        text = SELF_EVAL.read_text(encoding="utf-8")
        start = text.index(BEGIN)
        end = text.index(END)
        if end < start:
            raise GateError("SELF-EVAL markers out of order", 2)
        end = end + len(END)
        SELF_EVAL.write_text(text[:start] + block + text[end:], encoding="utf-8")
        return
    SELF_EVAL.write_text(
        "\n".join(
            [
                "# Symphony self-eval",
                "",
                "Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.",
                "Europe/Zagreb.",
                "",
                "A section is complete when its gate passes and the receipt below matches the script.",
                "`LOCKS.toml` is read by the gate and left unchanged.",
                "",
                "## G · Latency / TTFA",
                "",
                "Gate: `docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh`.",
                "The stub block is the mock presence replay. `measured.present` is false until a measured file is passed on a later run. `--write-eval` refuses while `GALION_TTFA_MEASURED` is set, so a host clock cannot overwrite this record.",
                "",
                block,
                "",
                "— Maciej Grajczyk",
                "2026-11-03 · Europe/Zagreb",
                "",
            ]
        ),
        encoding="utf-8",
    )


def emit(obj) -> int:
    sys.stdout.write(dump(obj) + "\n")
    return 0


def cmd_frame() -> int:
    return emit(check_frame())


def cmd_presence() -> int:
    return emit(run_presence())


def cmd_gate(write: bool) -> int:
    before = locks_bytes()
    frame = check_frame()
    presence = run_presence()
    raw = os.environ.get("GALION_TTFA_MEASURED", "").strip()
    measured = check_measured(Path(raw)) if raw else None
    if locks_bytes() != before:
        raise GateError("LOCKS.toml changed while the gate was running")
    body = receipt(frame, presence, measured)
    if write:
        if raw:
            raise GateError("--write-eval refuses GALION_TTFA_MEASURED so the stub record stays put", 2)
        write_eval(body)
    return emit(body)


def _expect_fail(fn, label: str):
    try:
        fn()
    except GateError:
        return
    raise GateError(f"self-check: {label} was accepted")


def cmd_self_check() -> int:
    before = locks_bytes()
    frame = check_frame()
    script_dir = Path(__file__).resolve().parent
    redirect = re.compile(r"(?:>>?|tee)\s+LOCKS\.toml")
    for path in sorted(script_dir.glob("*.sh")):
        if redirect.search(path.read_text(encoding="utf-8", errors="replace")):
            raise GateError(f"{path.name} writes LOCKS.toml")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)

        def write(name: str, doc: dict) -> Path:
            path = root / name
            path.write_text(json.dumps(doc), encoding="utf-8")
            return path

        good = {
            "schema": "galion-ttfa-measured/v1",
            "kind": "measured",
            "frame_ms": 80,
            "host": "self-check",
            "engine": "live-sector",
            "ttfa_ms": 80,
            "onset_p50_ms": 240,
            "barge_in_ms": 80,
            "e_to_a_ms": 80,
            "e_to_a_frames": 1,
        }
        ok = check_measured(write("good.json", good))
        if ok.get("ttfa_ms") != 80 or ok.get("kind") != "measured":
            raise GateError("self-check: good measured file")
        cases = {
            "frame": {**good, "frame_ms": 40},
            "ttfa": {**good, "ttfa_ms": 81},
            "onset": {**good, "onset_p50_ms": 301},
            "barge": {**good, "barge_in_ms": 201},
            "e2a-ms": {**good, "e_to_a_ms": 161},
            "e2a-frames": {**good, "e_to_a_frames": 3},
            "kind": {**good, "kind": "stub"},
            "engine": {**good, "engine": "mock"},
            "host": {**good, "host": "  "},
            "engagement": {**good, "engagement": 1},
        }
        for label, doc in cases.items():
            _expect_fail(lambda p=write(f"{label}.json", doc): check_measured(p), label)
    if locks_bytes() != before:
        raise GateError("LOCKS.toml changed during self-check")
    stub = receipt(frame, {"kind": "stub", "measured": False}, None)
    if "ttfa_ms" in dump(stub):
        raise GateError("self-check: stub receipt contained ttfa_ms")
    return emit({"self_check": "pass", "frame_bar": frame["frame_bar"], "frame_ms": FRAME_MS})


def main(argv: list[str]) -> int:
    cmd = argv[1] if len(argv) > 1 else "gate"
    write = "--write-eval" in argv[2:]
    if cmd in ("-h", "--help"):
        sys.stdout.write(__doc__ or "")
        return 0
    try:
        if cmd == "frame":
            return cmd_frame()
        if cmd == "presence":
            return cmd_presence()
        if cmd == "gate":
            return cmd_gate(write)
        if cmd == "self-check":
            return cmd_self_check()
    except GateError as exc:
        sys.stderr.write(f"TTFA FAIL — {exc}\n")
        return exc.code
    sys.stderr.write("usage: ttfa_check.py frame|presence|gate|self-check [--write-eval]\n")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
