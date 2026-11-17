#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
"""Original-ref Nano first-PCM and 80 ms first-playable-frame harness.

Measures only when the in-tree worker can emit real Chatterbox Nano PCM
from the t273 Samantha and Jarvis refs. Anything else is BLOCKED.

A protocol-stub clock is FAIL and is not given ``ttfa_ms``. Piper and
Flite are not this clock. Whole-utterance only at or above 1500 ms is
HARD-FAIL. ``claim_80ms`` is true only when a measured ``ttfa_ms`` (the
first playable 1920-sample frame) is at most 80.

This process does not edit LOCKS.toml or REFS.sha256, does not set
GALION_CLONE_REF_SHA, and does not call a paid model.

Exit 0: real original-ref Nano PCM was classified (read ``claim_80ms``).
Exit 1: FAIL or HARD-FAIL.
Exit 2: BLOCKED. Not a product TTFA. 80 ms is not claimed.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import selectors
import socket
import struct
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKER_PATH = ROOT / "scripts" / "clone" / "chatterbox_worker.py"
LOCKS = ROOT / "LOCKS.toml"
HEARTBEAT = ROOT / "crates" / "galion-heartbeat" / "src" / "lib.rs"

SCHEMA = "galion-nano-original-ref-ttfa/v1"
FRAME_FILE = "galion-nano-first-playable-frame/v1"
FRAME_MS = 80
SAMPLE_RATE = 24_000
FRAME_SAMPLES = 1_920
FRAME_BYTES = FRAME_SAMPLES * 2
WHOLE_UTT_FAIL_MS = 1500
STREAM_GAP_MS = 20
NANO_ENGINES = frozenset({"nano", "chatterbox-nano"})
STUB_ENGINES = frozenset({"", "protocol-stub", "stub", "mock", "unwired"})
SUBSTITUTE_ENGINES = frozenset({"piper", "flite", "a21", "a21-weights"})
CLOCK_KEYS = ("ttfa_ms", "first_pcm_ms", "galion_clone_first")
STRIP_ENV = (
    "GALION_CLONE_STUB_PCM",
    "GALION_CLONE_STUB_FRAMES",
    "GALION_CLONE_REF_SHA",
    "GALION_CLONE_REF_ALLOW",
    "GALION_CLONE_FETCH",
)
# Commented t273 rows in scripts/clone/REFS.sha256. Not an allowlist edit.
T273 = {
    "samantha": "bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693",
    "jarvis": "c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60",
}
REQUESTS = {
    "samantha": {
        "text": "Hi. I'm here, and I'm listening.",
        "model": "nano",
        "persona": "samantha",
        "device": "cpu",
        "denoise": False,
        "tags": True,
    },
    "jarvis": {
        "text": "Here. What do you need?",
        "model": "nano",
        "persona": "jarvis",
        "device": "cpu",
        "denoise": False,
        "tags": False,
    },
}
_IPV4 = re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")


def scrub(text: str) -> str:
    return _IPV4.sub("[redacted]", str(text))


def hostname() -> str:
    return scrub(socket.gethostname())


def varied_pcm(n: int = 4000) -> bytes:
    samples = [((i * 97) % 4001) - 2000 for i in range(n)]
    return struct.pack("<" + "h" * n, *samples)


def real_pcm(pcm: bytes, engine: str) -> bool:
    if engine not in NANO_ENGINES or len(pcm) < FRAME_BYTES:
        return False
    count = min(len(pcm) // 2, 4000)
    samples = struct.unpack("<" + "h" * count, pcm[: count * 2])
    peak = max(abs(sample) for sample in samples)
    return peak >= 100 and len(set(samples[:500])) >= 8


def load_worker():
    spec = importlib.util.spec_from_file_location("galion_chatterbox_worker_ttfa", WORKER_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("chatterbox worker is missing")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def locked_frame_ms(path: Path = LOCKS) -> int:
    try:
        import tomllib
    except ImportError as exc:  # pragma: no cover
        raise RuntimeError("python3 >= 3.11 is required") from exc
    with path.open("rb") as handle:
        doc = tomllib.load(handle)
    return int(doc["clock"]["frame_ms"])


def child_env(base: dict | None = None) -> dict[str, str]:
    """Stream Nano through the worker allowlist. No stub and no ref override."""
    env = dict(os.environ if base is None else base)
    for key in STRIP_ENV:
        env.pop(key, None)
    env["GALION_CLONE_LOCAL_REFS"] = "1"
    env["GALION_CLONE_STREAM"] = "1"
    env["GALION_CLONE_MODEL"] = "nano"
    env["GALION_CLONE_THREADS"] = "1"
    return env


def live_blockers(worker=None) -> list[str]:
    """Why a real original-ref measurement cannot start. Empty means it can."""
    worker = worker or load_worker()
    allow = worker.load_allowlist()
    blockers: list[str] = []
    for persona, digest in T273.items():
        if not worker.digest_allowed(persona, digest, allow):
            blockers.append(
                f"A0: {persona} t273 digest is not the active REFS.sha256 allowlist"
            )
    for persona in ("samantha", "jarvis"):
        path = worker.default_ref(persona)
        if not path:
            blockers.append(f"A1: {persona} original ref wav is absent")
            continue
        digest = worker.sha256_file(path)
        if digest != T273[persona]:
            blockers.append(f"A1: {persona} wav sha256 is not the t273 original")
    root = worker.weights_root()
    weights_ok = worker.weights_present() and worker.tensors_pinned(root)
    if importlib.util.find_spec("torch") is None:
        blockers.append("A1: torch is not importable")
    if importlib.util.find_spec("chatterbox") is None:
        blockers.append("A1: chatterbox is not importable")
    if not weights_ok:
        blockers.append("A1: pinned nano safetensors are absent or not pinned")
    if not worker.tokenizer_ready(root):
        blockers.append("A1: tokenizer tree is missing or not pinned")
    return blockers


def _last(text: str, key: str) -> str | None:
    found = None
    prefix = key if key.endswith(":") else key + ":"
    for line in text.splitlines():
        if line.startswith(prefix):
            found = line.split(":", 1)[1].strip().lower()
    return found


def observe(events: list[tuple[int, str, bytes]]) -> dict:
    """Clock a stream of (t_ms, 'out'|'err', bytes) events. No product claim."""
    pcm = bytearray()
    err = bytearray()
    first_pcm = None
    playable = None
    utt = None
    marker = None
    for t_ms, stream, data in events:
        if not data:
            continue
        if stream == "out":
            if first_pcm is None:
                first_pcm = t_ms
            pcm.extend(data)
            if playable is None and len(pcm) >= FRAME_BYTES:
                playable = t_ms
        else:
            err.extend(data)
            text = err.decode("utf-8", "replace")
            if marker is None and "galion-clone-first" in text:
                marker = t_ms
            if utt is None and "galion-clone-utt:" in text:
                utt = t_ms
    stderr = err.decode("utf-8", "replace")
    marker_before = marker is not None and (utt is None or marker < utt)
    return {
        "engine": _last(stderr, "galion-clone-engine"),
        "path": _last(stderr, "galion-clone-path"),
        "chunks": stderr.count("galion-clone-chunk:"),
        "pcm": bytes(pcm),
        "stderr": stderr,
        "first_pcm_ms": first_pcm,
        "first_playable_frame_ms": playable,
        "utt_ms": utt,
        "first_marker_before_utt": marker_before,
        "stub": False,
    }


def _row(cap: dict, gate: str, **fields) -> dict:
    engine = (cap.get("engine") or "").strip().lower() or None
    body = {
        "persona": cap.get("persona"),
        "gate": gate,
        "engine": engine,
        "stub": bool(fields.pop("stub", False)),
        "product_claim": False,
        "claim_80ms": False,
        "measurement": "not measured",
        "pcm_bytes": len(cap.get("pcm") or b""),
    }
    ref = cap.get("ref_sha256")
    if isinstance(ref, str) and ref:
        body["ref_sha256"] = ref.lower()
    body.update(fields)
    return body


def score_capture(cap: dict) -> dict:
    """Score one capture. Non-measurements do not carry ``ttfa_ms``."""
    engine = (cap.get("engine") or "").strip().lower()
    stub = bool(cap.get("stub")) or engine in STUB_ENGINES
    if stub:
        return _row(
            cap,
            "FAIL",
            stub=True,
            engine=engine or "protocol-stub",
            reason="protocol-stub clock is FAIL",
        )
    if engine in SUBSTITUTE_ENGINES:
        return _row(cap, "FAIL", reason=f"{engine} is not an original-ref nano clock")
    if engine not in NANO_ENGINES:
        return _row(
            cap,
            "FAIL",
            reason=f"engine {engine or 'none'} is not chatterbox-nano or nano",
        )
    persona = str(cap.get("persona") or "")
    ref = str(cap.get("ref_sha256") or "").lower()
    if persona not in T273 or ref != T273[persona]:
        return _row(cap, "FAIL", reason="reference sha256 is not the t273 original")
    stderr = cap.get("stderr") or ""
    pcm = cap.get("pcm") or b""
    if "not allowlisted" in stderr and len(pcm) == 0:
        return _row(cap, "BLOCKED", reason="reference sha256 is not allowlisted")
    if not real_pcm(pcm, engine):
        return _row(cap, "FAIL", reason="PCM is not real clone audio")
    first = cap.get("first_pcm_ms")
    playable = cap.get("first_playable_frame_ms")
    utt = cap.get("utt_ms")
    gap_ok = (
        isinstance(first, int)
        and isinstance(utt, int)
        and utt - first >= STREAM_GAP_MS
    )
    streaming = (
        cap.get("path") == "stream"
        and (bool(cap.get("first_marker_before_utt")) or gap_ok)
        and int(cap.get("chunks") or 0) >= 1
    )
    whole = utt if isinstance(utt, int) else playable if isinstance(playable, int) else first
    if not streaming:
        if isinstance(whole, int) and whole >= WHOLE_UTT_FAIL_MS:
            return _row(
                cap,
                "HARD-FAIL",
                measurement="whole-utterance",
                whole_utt_ms=whole,
                rejected_clock_ms=whole,
                reason=f"whole-utterance only, {whole} ms >= {WHOLE_UTT_FAIL_MS} ms",
            )
        return _row(
            cap,
            "FAIL",
            reason="no streaming first-PCM; a whole utterance is not this clock",
        )
    if not isinstance(first, int) or not isinstance(playable, int):
        return _row(cap, "FAIL", reason="first PCM byte or first playable frame was not observed")
    if playable < first:
        return _row(cap, "FAIL", reason="first playable frame is before the first PCM byte")
    if playable >= WHOLE_UTT_FAIL_MS or first >= WHOLE_UTT_FAIL_MS:
        clock = max(playable, first)
        return _row(
            cap,
            "HARD-FAIL",
            measurement="whole-utterance",
            whole_utt_ms=utt if isinstance(utt, int) else clock,
            rejected_clock_ms=clock,
            reason=f"first playable frame {playable} ms >= {WHOLE_UTT_FAIL_MS} ms",
        )
    claim = playable <= FRAME_MS
    reason = None
    if not claim:
        reason = (
            f"first playable frame {playable} ms is over {FRAME_MS} ms; 80 ms is not claimed"
        )
    return _row(
        cap,
        "MEASURED",
        measurement="measured",
        product_claim=True,
        claim_80ms=claim,
        stub=False,
        ttfa_ms=playable,
        first_pcm_ms=first,
        galion_clone_first=(
            f"galion-clone-first: {first} persona={persona} engine={engine}"
        ),
        reason=reason,
    )


def _engine_label(rows: list[dict]) -> str:
    engines = []
    for row in rows:
        engine = row.get("engine")
        if engine and engine not in engines:
            engines.append(engine)
    return "+".join(engines) if engines else "nano"


def _measured_file(host: str, engine: str, ttfa_ms: int) -> dict:
    return {
        "schema": FRAME_FILE,
        "kind": "measured",
        "frame_ms": FRAME_MS,
        "host": host,
        "engine": engine,
        "ttfa_ms": ttfa_ms,
        "stub": False,
        "refs": "t273",
        "t273_sha256": dict(T273),
        "product_claim": True,
        "note": (
            "80 ms first-playable frame on original-ref Nano. "
            "Onset, barge-in, and e-to-a were not measured. "
            "Not a galion-ttfa-measured/v1 file."
        ),
    }


def combine(rows: list[dict], *, host: str, blockers: list[str] | None = None) -> dict:
    host = scrub(host)
    order = {"MEASURED": 0, "BLOCKED": 1, "HARD-FAIL": 2, "FAIL": 3}
    gate = max(rows, key=lambda row: order[row["gate"]])["gate"]
    names = {row.get("persona") for row in rows}
    if gate == "MEASURED" and names != {"samantha", "jarvis"}:
        gate = "BLOCKED"
    report = {
        "schema": SCHEMA,
        "gate": gate,
        "host": host,
        "frame_bar_ms": FRAME_MS,
        "frame_samples": FRAME_SAMPLES,
        "sample_rate_hz": SAMPLE_RATE,
        "stub_clock": "FAIL",
        "locks_edited": False,
        "refs_sha256_edited": False,
        "measured_file": None,
        "t273_sha256_expected": dict(T273),
        "personas": {row["persona"]: dict(row) for row in rows if row.get("persona")},
        "blockers": list(blockers or []),
        "depends_on": ["A0", "A1"],
        "measured": False,
        "measurement": "not measured",
        "product_claim": False,
        "claim_80ms": False,
        "stub": False,
        "engine": None,
        "frame_bar": "not-measured",
    }
    if gate == "MEASURED":
        report["measured"] = True
        report["measurement"] = "measured"
        report["engine"] = _engine_label(rows)
        report["ttfa_ms"] = max(row["ttfa_ms"] for row in rows)
        report["first_pcm_ms"] = max(row["first_pcm_ms"] for row in rows)
        report["claim_80ms"] = all(row["claim_80ms"] for row in rows)
        report["product_claim"] = True
        report["frame_bar"] = "PASS" if report["claim_80ms"] else "FAIL"
        report["galion_clone_first"] = [row["galion_clone_first"] for row in rows]
        if report["claim_80ms"]:
            report["measured_file"] = _measured_file(
                host, report["engine"], report["ttfa_ms"]
            )
            report["note"] = (
                "Original-ref Nano first playable frame met 80 ms. "
                "stub is false. Onset, barge-in, and e-to-a were not measured."
            )
        else:
            report["note"] = (
                f"Original-ref Nano first playable frame is {report['ttfa_ms']} ms. "
                "80 ms is not claimed."
            )
    elif gate == "HARD-FAIL":
        clocks = [row["rejected_clock_ms"] for row in rows if "rejected_clock_ms" in row]
        report["measurement"] = "whole-utterance"
        report["frame_bar"] = "FAIL"
        if clocks:
            report["rejected_clock_ms"] = max(clocks)
        report["note"] = (
            "Whole-utterance only at or above 1500 ms is HARD-FAIL. "
            "80 ms is not claimed. This is not a first-PCM product clock."
        )
    elif gate == "FAIL":
        report["stub"] = any(row.get("stub") for row in rows)
        report["frame_bar"] = "FAIL"
        report["note"] = (
            "Not an original-ref Nano measurement. "
            "A protocol-stub, Piper, or Flite clock is FAIL. 80 ms is not claimed."
        )
    else:
        report["note"] = (
            "Original-ref Nano first-PCM was not measured. "
            + ("; ".join(report["blockers"]) + ". " if report["blockers"] else "")
            + "Piper, Flite, and a protocol-stub clock are not substitutes. "
            "80 ms is not claimed."
        )
    return report


def blocked_report(host: str, blockers: list[str]) -> dict:
    return {
        "schema": SCHEMA,
        "gate": "BLOCKED",
        "host": scrub(host),
        "frame_bar_ms": FRAME_MS,
        "frame_samples": FRAME_SAMPLES,
        "sample_rate_hz": SAMPLE_RATE,
        "stub_clock": "FAIL",
        "locks_edited": False,
        "refs_sha256_edited": False,
        "measured_file": None,
        "t273_sha256_expected": dict(T273),
        "blockers": list(blockers),
        "depends_on": ["A0", "A1"],
        "measured": False,
        "measurement": "not measured",
        "product_claim": False,
        "claim_80ms": False,
        "stub": False,
        "engine": None,
        "frame_bar": "not-measured",
        "note": (
            "Original-ref Nano first-PCM was not measured. "
            + "; ".join(blockers)
            + ". Piper, Flite, and a protocol-stub clock are not substitutes. "
            "80 ms is not claimed."
        ),
    }


def fail_report(note: str) -> dict:
    return {
        "schema": SCHEMA,
        "gate": "FAIL",
        "host": hostname(),
        "frame_bar_ms": FRAME_MS,
        "frame_samples": FRAME_SAMPLES,
        "sample_rate_hz": SAMPLE_RATE,
        "stub_clock": "FAIL",
        "locks_edited": False,
        "refs_sha256_edited": False,
        "measured_file": None,
        "t273_sha256_expected": dict(T273),
        "blockers": [],
        "depends_on": ["A0", "A1"],
        "measured": False,
        "measurement": "not measured",
        "product_claim": False,
        "claim_80ms": False,
        "stub": False,
        "engine": None,
        "frame_bar": "FAIL",
        "note": scrub(note),
    }


def validate_public(doc: dict) -> None:
    if doc.get("schema") != SCHEMA:
        raise ValueError("schema")
    if doc.get("gate") not in ("BLOCKED", "FAIL", "HARD-FAIL", "MEASURED"):
        raise ValueError("gate")
    if doc.get("locks_edited") or doc.get("refs_sha256_edited"):
        raise ValueError("locks or REFS were edited")
    if doc.get("frame_bar_ms") != FRAME_MS:
        raise ValueError("frame bar")
    if doc.get("stub_clock") != "FAIL":
        raise ValueError("stub clock must stay FAIL")
    if doc["gate"] != "MEASURED":
        for key in ("ttfa_ms", "first_pcm_ms", "galion_clone_first"):
            if key in doc:
                raise ValueError(f"top-level {key} on a non-measurement")
        if doc.get("product_claim") or doc.get("claim_80ms"):
            raise ValueError("claim on a non-measurement")
        if doc.get("measured_file"):
            raise ValueError("measured file on a non-measurement")
    else:
        ttfa = doc.get("ttfa_ms")
        if not isinstance(ttfa, int):
            raise ValueError("measured ttfa_ms")
        if doc.get("claim_80ms") and ttfa > FRAME_MS:
            raise ValueError("80 ms claimed over the frame")
        if doc.get("claim_80ms") and not doc.get("measured_file"):
            raise ValueError("80 ms claim has no measured file")
        if not doc.get("claim_80ms") and doc.get("measured_file"):
            raise ValueError("measured file without an 80 ms claim")
        if not doc.get("product_claim") or doc.get("stub"):
            raise ValueError("measured report is not real PCM")
    measured = doc.get("measured_file")
    if measured:
        if measured.get("schema") == "galion-ttfa-measured/v1":
            raise ValueError("symphony measured file is not written here")
        if measured.get("stub") or measured.get("ttfa_ms", 10**9) > FRAME_MS:
            raise ValueError("measured file")
        for forbidden in ("onset_p50_ms", "barge_in_ms", "e_to_a_ms", "e_to_a_frames"):
            if forbidden in measured:
                raise ValueError("invented presence clock")
    for persona in (doc.get("personas") or {}).values():
        if persona.get("gate") != "MEASURED":
            for key in CLOCK_KEYS:
                if key in persona:
                    raise ValueError("persona clock on a non-measurement")
            if persona.get("claim_80ms") or persona.get("product_claim"):
                raise ValueError("persona claim")
        elif persona.get("claim_80ms") and persona.get("ttfa_ms", 10**9) > FRAME_MS:
            raise ValueError("persona 80 ms claim")
    blob = json.dumps(doc)
    if _IPV4.search(blob):
        raise ValueError("ipv4 in report")


def fixture_cases() -> list[tuple[str, dict, str, dict]]:
    pcm = varied_pcm()
    samantha = T273["samantha"]

    def cap(**kwargs) -> dict:
        base = {
            "persona": "samantha",
            "engine": "chatterbox-nano",
            "path": "stream",
            "stub": False,
            "ref_sha256": samantha,
            "pcm": pcm,
            "chunks": 2,
            "first_pcm_ms": 40,
            "first_playable_frame_ms": 70,
            "utt_ms": 400,
            "first_marker_before_utt": True,
            "stderr": "",
        }
        base.update(kwargs)
        return base

    return [
        (
            "stub",
            cap(
                engine="protocol-stub",
                stub=True,
                pcm=b"\x01\x00" * 10,
                first_pcm_ms=15,
                first_playable_frame_ms=15,
                utt_ms=65,
            ),
            "FAIL",
            {"stub": True},
        ),
        (
            "piper",
            cap(engine="piper", first_pcm_ms=20, first_playable_frame_ms=20),
            "FAIL",
            {},
        ),
        (
            "t623u",
            cap(ref_sha256="e4bc60c33f697108cf5585dead213da1065979010f5be1d513574893584a4641"),
            "FAIL",
            {},
        ),
        (
            "whole",
            cap(
                path="whole",
                chunks=0,
                first_marker_before_utt=False,
                first_pcm_ms=1800,
                first_playable_frame_ms=1800,
                utt_ms=1800,
            ),
            "HARD-FAIL",
            {},
        ),
        (
            "fast",
            cap(),
            "MEASURED",
            {"claim_80ms": True, "ttfa_ms": 70, "first_pcm_ms": 40, "product_claim": True},
        ),
        (
            "slow",
            cap(first_pcm_ms=120, first_playable_frame_ms=400, utt_ms=900),
            "MEASURED",
            {"claim_80ms": False, "ttfa_ms": 400, "product_claim": True},
        ),
        (
            "short-whole",
            cap(
                path="whole",
                chunks=0,
                first_marker_before_utt=False,
                first_pcm_ms=200,
                first_playable_frame_ms=200,
                utt_ms=200,
            ),
            "FAIL",
            {},
        ),
    ]


def contract_error() -> str | None:
    for name, cap, gate, expect in fixture_cases():
        row = score_capture(cap)
        if row["gate"] != gate:
            return f"fixture {name} gate {row['gate']}"
        for key, value in expect.items():
            if row.get(key) != value:
                return f"fixture {name} {key}"
        if gate != "MEASURED":
            for key in CLOCK_KEYS:
                if key in row:
                    return f"fixture {name} leaked {key}"
    fast = score_capture(fixture_cases()[4][1])
    jarvis = dict(fast)
    jarvis["persona"] = "jarvis"
    jarvis["ref_sha256"] = T273["jarvis"]
    jarvis["galion_clone_first"] = (
        f"galion-clone-first: 40 persona=jarvis engine={fast['engine']}"
    )
    report = combine([fast, jarvis], host="contract")
    try:
        validate_public(report)
    except ValueError as exc:
        return f"combine {exc}"
    if not report["claim_80ms"] or report["ttfa_ms"] != 70:
        return "combine did not claim the frame the fixtures met"
    return None


def read_child(argv: list[str], env: dict, line: bytes, timeout: float) -> list[tuple[int, str, bytes]]:
    proc = subprocess.Popen(
        argv,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    assert proc.stdin and proc.stdout and proc.stderr
    start = time.monotonic()
    try:
        proc.stdin.write(line)
        proc.stdin.close()
    except BrokenPipeError:
        pass
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout.fileno(), selectors.EVENT_READ, "out")
    selector.register(proc.stderr.fileno(), selectors.EVENT_READ, "err")
    events: list[tuple[int, str, bytes]] = []
    while selector.get_map():
        if time.monotonic() - start > timeout:
            break
        ready = selector.select(timeout=0.05)
        for key, _ in ready:
            try:
                chunk = os.read(key.fd, 65536)
            except OSError:
                chunk = b""
            t_ms = int((time.monotonic() - start) * 1000)
            if not chunk:
                selector.unregister(key.fd)
                continue
            events.append((t_ms, key.data, chunk))
        if proc.poll() is not None and not ready:
            break
    if proc.poll() is None:
        proc.kill()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2)
    for stream in (proc.stdout, proc.stderr, proc.stdin):
        if stream is not None:
            stream.close()
    return events


def capture_persona(persona: str, spawn=None) -> dict:
    worker = load_worker()
    env = child_env()
    line = (json.dumps(REQUESTS[persona]) + "\n").encode()
    if spawn is None:
        events = read_child([sys.executable, str(WORKER_PATH)], env, line, 120.0)
    else:
        events = spawn(env, line)
    cap = observe(events)
    cap["persona"] = persona
    path = worker.default_ref(persona)
    cap["ref_sha256"] = worker.sha256_file(path) if path else ""
    cap["stub"] = False
    return cap


def measure_both(spawn=None) -> list[dict]:
    return [score_capture(capture_persona(persona, spawn)) for persona in ("samantha", "jarvis")]


def exit_code(report: dict) -> int:
    gate = report["gate"]
    if gate == "BLOCKED":
        return 2
    if gate == "MEASURED":
        return 0
    return 1


def emit(report: dict, code: int) -> None:
    try:
        validate_public(report)
    except ValueError as exc:
        report = fail_report(f"harness report failed its own check: {exc}")
        validate_public(report)
        code = 1
    sys.stdout.write(json.dumps(report, sort_keys=True) + "\n")
    raise SystemExit(code)


def main(blockers_fn=None, measure_fn=None) -> None:
    broken = contract_error()
    if broken:
        emit(fail_report(broken), 1)
    if locked_frame_ms() != FRAME_MS:
        emit(fail_report("clock.frame_ms is not 80; the harness does not move the clock"), 1)
    needle = "pub const FRAME_MS: u32 = 80;"
    if needle not in HEARTBEAT.read_text(encoding="utf-8"):
        emit(fail_report("heartbeat FRAME_MS is not 80; the harness does not move the clock"), 1)
    blockers = (blockers_fn or live_blockers)()
    if blockers:
        emit(blocked_report(hostname(), blockers), 2)
    rows = (measure_fn or measure_both)()
    report = combine(rows, host=hostname(), blockers=[])
    emit(report, exit_code(report))


if __name__ == "__main__":
    main()
