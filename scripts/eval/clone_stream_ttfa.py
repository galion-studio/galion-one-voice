#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""TTFA gate for the clone stream hook.

A protocol-stub clock is FAIL. It is not a pass and not a product claim.
Real Piper PCM is measured by ``piper_stream_ttfa.py``. This script does
not download weights and does not write a WAV.
"""

from __future__ import annotations

import json
import os
import select
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CLONE = ROOT / "scripts" / "clone"
WORKER = CLONE / "chatterbox_worker.py"
sys.path.insert(0, str(CLONE))

import nano_stream  # noqa: E402

WHOLE_UTT_FAIL_MS = 1500
GAP_MS = 30


def die(report: dict) -> None:
    sys.stdout.write(json.dumps(report, sort_keys=True) + "\n")
    raise SystemExit(1)


def ok(report: dict) -> None:
    sys.stdout.write(json.dumps(report, sort_keys=True) + "\n")
    raise SystemExit(0)


def source_gate() -> list:
    fails = []
    for rel in ("scripts/clone/chatterbox_worker.py", "scripts/clone/nano_stream.py"):
        text = (ROOT / rel).read_text(encoding="utf-8")
        if rel.endswith("chatterbox_worker.py"):
            # Merge #53+#54: whole-utterance generate lives only in render()
            # (engine chatterbox-nano, no stream path, never a claim here).
            # The stream path itself must not call it.
            body = text.split("\ndef render(", 1)[-1].split("\ndef ", 1)[0] if "\ndef render(" in text else ""
            outside = text.count(".generate(") - body.count(".generate(")
            stream = text.split("\ndef run_stream(", 1)[-1].split("\ndef ", 1)[0]
            if outside or ".generate(" in stream or "render(" in stream:
                fails.append(f"{rel} calls generate() outside render() or on the stream path")
        elif ".generate(" in text:
            fails.append(f"{rel} calls generate()")
        if "nano_stream" not in text and rel.endswith("chatterbox_worker.py"):
            fails.append("worker does not use the stream module")
        if "write_chunks" not in text:
            fails.append(f"{rel} has no chunk writer")
    hb = ROOT / "crates" / "galion-heartbeat"
    blob = "\n".join(p.read_text(encoding="utf-8") for p in hb.rglob("*.rs"))
    for needle in ("generate(", "chatterbox", "CloneFrameDrain"):
        if needle in blob:
            fails.append(f"galion-heartbeat contains {needle}")
    return fails


def logic_gate() -> list:
    fails = []
    if nano_stream.select_path(_GenerateOnly()) != "refuse":
        fails.append("generate-only model was not refused")
    if nano_stream.select_path(_StreamModel()) != "stream_api":
        fails.append("stream() was not selected")
    if nano_stream.select_path(_MeanflowModel()) != "chunked_meanflow":
        fails.append("chunked meanflow was not selected")
    called = _GenerateOnly()
    try:
        list(nano_stream.iter_pcm(called, "Hi.", None))
        fails.append("iter_pcm returned PCM for a generate-only model")
    except nano_stream.NoStreamHook:
        pass
    if called.calls:
        fails.append("generate-only model was called")
    if nano_stream.flush_due(4):
        fails.append("flush before lookahead+chunk")
    if not nano_stream.flush_due(5):
        fails.append("first flush is not at 5 tokens (80 ms + lookahead)")
    if nano_stream.flush_due(6):
        fails.append("unaligned token count flushed")
    if not nano_stream.flush_due(6, final=True):
        fails.append("final flush did not emit the tail")
    chunks = nano_stream.protocol_chunks("Hi.")
    if len(chunks) < 2 or len(chunks[0]) != 2:
        fails.append("protocol stub is not split into a first sample and a tail")
    pcm = _Mem()
    err = _Mem()
    count, total = nano_stream.write_chunks(
        pcm, chunks, engine="protocol-stub", persona="samantha", path="stream", err=err
    )
    text = err.text()
    if count < 2 or total != sum(len(c) for c in chunks):
        fails.append("chunk writer dropped bytes")
    chunk_at = text.find("galion-clone-chunk:")
    utt_at = text.find("galion-clone-utt:")
    if chunk_at < 0 or utt_at < chunk_at:
        fails.append("utt marker was written before a chunk marker")
    if "galion-clone-path: stream" not in text:
        fails.append("writer did not name the stream path")
    if judge({"path": "whole", "chunks": 0, "ttfa_ms": 10, "utt_ms": 2000, "hook": "whole"}) != "FAIL":
        fails.append("whole-utterance report was not failed")
    if judge({"path": "stream", "chunks": 1, "ttfa_ms": 1800, "utt_ms": 2400, "hook": "stream"}) != "FAIL":
        fails.append("1500 ms first-audio was not failed")
    if judge({"path": "stream", "chunks": 2, "ttfa_ms": 15, "utt_ms": 65, "engine": "protocol-stub"}) != "FAIL":
        fails.append("a 15 ms protocol-stub clock was not failed")
    if judge({"path": "stream", "chunks": 2, "ttfa_ms": 40, "utt_ms": 90, "engine": "piper", "real_pcm": True}) != "PASS":
        fails.append("real piper PCM was not accepted")
    return fails


class _GenerateOnly:
    def __init__(self):
        self.calls = 0

    def generate(self, *_a, **_k):
        self.calls += 1
        return [0.0]


class _StreamModel:
    def stream(self, text, **_k):
        yield _Chunk([0.0, 0.0])
        yield _Chunk([0.1, 0.1])


class _MeanflowModel:
    def __init__(self):
        self.t3 = object()
        self.s3gen = object()


class _Chunk:
    def __init__(self, audio):
        self.audio = [audio]
        self.sample_rate = 24000


class _Mem:
    def __init__(self):
        self.parts = []

    def write(self, data):
        if isinstance(data, str):
            data = data.encode()
        self.parts.append(data)
        return len(data)

    def flush(self):
        return None

    def text(self) -> str:
        return b"".join(self.parts).decode()


def judge(rep: dict) -> str:
    """PASS only for real Piper or Nano PCM on a stream path under 1500 ms.

    A protocol-stub clock is FAIL even when the pipe is fast.
    """
    engine = rep.get("engine") or ""
    path = rep.get("path")
    chunks = int(rep.get("chunks") or 0)
    ttfa = rep.get("ttfa_ms")
    real = bool(rep.get("real_pcm"))
    if engine == "protocol-stub" or not real:
        return "FAIL"
    if engine not in ("piper", "nano"):
        return "FAIL"
    if path != "stream" or chunks < 1 or ttfa is None or ttfa >= WHOLE_UTT_FAIL_MS:
        return "FAIL"
    return "PASS"


def measure(argv: list, env: dict, line: bytes, timeout: float = 5.0) -> dict:
    proc = subprocess.Popen(
        argv,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    assert proc.stdin and proc.stdout and proc.stderr
    t0 = time.perf_counter()
    proc.stdin.write(line)
    proc.stdin.close()
    first = None
    utt_at = None
    pcm = b""
    err = b""
    deadline = t0 + timeout
    while time.perf_counter() < deadline:
        readable, _, _ = select.select([proc.stdout, proc.stderr], [], [], 0.05)
        if proc.stdout in readable:
            block = os.read(proc.stdout.fileno(), 65536)
            if block:
                if first is None:
                    first = time.perf_counter()
                pcm += block
        if proc.stderr in readable:
            block = os.read(proc.stderr.fileno(), 65536)
            if block:
                err += block
                if utt_at is None and b"galion-clone-utt:" in err:
                    utt_at = time.perf_counter()
        if utt_at is not None and proc.poll() is not None:
            break
        if utt_at is not None and first is not None and time.perf_counter() - utt_at > 0.2:
            break
    if proc.poll() is None:
        proc.kill()
        proc.wait(timeout=2)
    else:
        proc.wait(timeout=2)
    text = err.decode(errors="replace")
    path = _last(text, "galion-clone-path:")
    engine = _last(text, "galion-clone-engine:")
    chunks = text.count("galion-clone-chunk:")
    ttfa = None if first is None else int((first - t0) * 1000)
    utt = None if utt_at is None else int((utt_at - t0) * 1000)
    return {
        "path": path,
        "engine": engine,
        "chunks": chunks,
        "ttfa_ms": ttfa,
        "utt_ms": utt,
        "pcm_bytes": len(pcm),
        "real_pcm": _real_pcm(pcm, engine),
        "stderr": text.strip().splitlines()[-8:],
    }


def _real_pcm(pcm: bytes, engine: str | None) -> bool:
    if engine in (None, "", "protocol-stub", "unwired"):
        return False
    if len(pcm) < 1920 * 2:
        return False
    import struct

    count = min(len(pcm) // 2, 4000)
    samples = struct.unpack("<" + "h" * count, pcm[: count * 2])
    peak = max(abs(s) for s in samples)
    return peak >= 100 and len(set(samples[:500])) >= 8


def _last(text: str, key: str):
    found = None
    for line in text.splitlines():
        if line.startswith(key):
            found = line[len(key) :].strip()
    return found


def whole_utt_fixture() -> dict:
    """A child that writes every sample only after the utterance is done."""
    code = (
        "import sys,time\n"
        "sys.stdin.buffer.readline()\n"
        "time.sleep(0.05)\n"
        "pcm=b'\\x01\\x00'*400\n"
        "sys.stderr.write('galion-clone-engine: protocol-stub\\n')\n"
        "sys.stderr.write('galion-clone-path: whole\\n')\n"
        "sys.stderr.flush()\n"
        "sys.stdout.buffer.write(pcm)\n"
        "sys.stdout.buffer.flush()\n"
        "sys.stderr.write('galion-clone-utt: %d\\n'%len(pcm))\n"
        "sys.stderr.flush()\n"
    )
    env = os.environ.copy()
    return measure([sys.executable, "-c", code], env, b'{"text":"Hi."}\n')


def stub_measure() -> dict:
    env = os.environ.copy()
    env["GALION_CLONE_STUB_PCM"] = "1"
    env.pop("GALION_CLONE_STREAM", None)
    env.pop("GALION_CLONE_LOCAL_REFS", None)
    return measure([sys.executable, str(WORKER)], env, b'{"text":"Hi.","persona":"samantha"}\n')


def refuse_measure() -> dict:
    env = os.environ.copy()
    env["GALION_CLONE_STUB_PCM"] = "1"
    env.pop("GALION_CLONE_LOCAL_REFS", None)
    proc = subprocess.run(
        [sys.executable, str(WORKER)],
        input=b'{"text":"Hi.","reference":"clip.wav"}\n',
        capture_output=True,
        env=env,
        timeout=5,
    )
    err = proc.stderr.decode(errors="replace")
    return {
        "code": proc.returncode,
        "stdout": len(proc.stdout),
        "rights": "rights hold" in err,
    }


def main() -> None:
    fails = source_gate() + logic_gate()
    refused = refuse_measure()
    if refused["code"] != 3 or refused["stdout"] != 0 or not refused["rights"]:
        fails.append(f"reference clip was not refused: {refused}")
    stub = stub_measure()
    gap = None
    if stub["ttfa_ms"] is not None and stub["utt_ms"] is not None:
        gap = stub["utt_ms"] - stub["ttfa_ms"]
    stub_judge = judge(stub)
    if stub_judge != "FAIL":
        fails.append(f"protocol stub clock was not failed: {stub}")
    if stub.get("chunks", 0) < 2:
        fails.append(f"protocol stream wrote fewer than 2 chunks: {stub}")
    if gap is None or gap < GAP_MS:
        fails.append(f"first PCM was not ahead of the utterance marker: gap={gap} stub={stub}")
    if stub.get("engine") != "protocol-stub":
        fails.append(f"stub engine changed: {stub.get('engine')}")
    whole = whole_utt_fixture()
    if judge(whole) != "FAIL":
        fails.append(f"whole-utterance fixture was not failed: {whole}")
    model_measured = False
    model = None
    if os.environ.get("GALION_CLONE_STREAM_EVAL") == "1":
        env = os.environ.copy()
        env["GALION_CLONE_STREAM"] = "1"
        env.pop("GALION_CLONE_STUB_PCM", None)
        model = measure(
            [sys.executable, str(WORKER)],
            env,
            b'{"text":"Hi.","persona":"samantha"}\n',
            timeout=120.0,
        )
        model_measured = model.get("engine") == "nano" and model.get("real_pcm") and model.get("path") == "stream"
        if model.get("path") == "whole" or (model.get("ttfa_ms") is not None and model["ttfa_ms"] >= WHOLE_UTT_FAIL_MS):
            fails.append(f"model clock is whole-utterance: {model}")
        if not model_measured and judge(model) == "FAIL" and model.get("engine") not in (None, "unwired"):
            fails.append(f"model stream eval failed: {model}")
    product_claim = bool(
        model_measured
        and model
        and model.get("engine") == "nano"
        and model.get("real_pcm")
        and judge(model) == "PASS"
    )
    if fails:
        gate = "FAIL"
    elif product_claim:
        gate = "PASS"
    else:
        gate = "FAIL"
    report = {
        "gate": gate,
        "hard_pass": gate == "PASS",
        "fails": fails,
        "path": None if not product_claim else model.get("path"),
        "engine": None if not product_claim else "nano",
        "chunks": None if not product_claim else model.get("chunks"),
        "whole_utt_fail_ms": WHOLE_UTT_FAIL_MS,
        "model_measured": model_measured and product_claim,
        "product_claim": product_claim,
        "stub_clock": "FAIL",
        "stub_engine": stub.get("engine"),
        "stub_gap_ms": gap,
        "heartbeat_step": "unchanged; late frame stays a pause; drain_frame is outside step",
        "piper_fallback": "SpeechStack still hands a dead clone to Piper",
        "whole_utt_fixture": judge(whole),
        "note": (
            "Protocol-stub clocks are FAIL. They are not ttfa_ms and not a product claim. "
            "Real Piper PCM is scripts/eval/piper_stream_ttfa.py."
        ),
    }
    if product_claim and model is not None:
        report["ttfa_ms"] = model.get("ttfa_ms")
        report["utt_ms"] = model.get("utt_ms")
    if model is not None and not product_claim:
        report["model_attempt"] = {
            "engine": model.get("engine"),
            "path": model.get("path"),
            "real_pcm": model.get("real_pcm"),
        }
    text = json.dumps(report, sort_keys=True)
    if fails:
        sys.stdout.write("FAIL\n" + text + "\n")
        raise SystemExit(1)
    if gate != "PASS":
        sys.stdout.write("FAIL\n" + text + "\n")
        raise SystemExit(2)
    sys.stdout.write("PASS\n" + text + "\n")
    raise SystemExit(0)


if __name__ == "__main__":
    main()
