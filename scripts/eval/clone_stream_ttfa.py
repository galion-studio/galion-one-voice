#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""TTFA gate for the Chatterbox Nano stream hook.

PASS when the worker emits a stream path: a chunk marker, then PCM, then
later the utterance marker, and the source has no whole-utterance
``generate()`` call. FAIL when the only path is a finished utterance, when
``path`` is not ``stream``, or when the first PCM is at or after 1500 ms
on this protocol clock.

The protocol stub is not a model measurement. ``model_measured`` stays
false unless ``GALION_CLONE_STREAM_EVAL=1`` and a loaded worker reports
``engine=model``. This script does not download weights and does not
write a WAV.
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
TARGET_LO_MS = 80
TARGET_HI_MS = 160
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
        if ".generate(" in text:
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
    if judge({"path": "stream", "chunks": 2, "ttfa_ms": 40, "utt_ms": 90, "hook": "stream"}) != "PASS":
        fails.append("a short streamed clock was not passed")
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
    path = rep.get("path")
    chunks = int(rep.get("chunks") or 0)
    ttfa = rep.get("ttfa_ms")
    utt = rep.get("utt_ms")
    if rep.get("hook") not in (None, "stream") and path != "stream":
        return "FAIL"
    if path != "stream" or chunks < 1 or ttfa is None:
        return "FAIL"
    if ttfa >= WHOLE_UTT_FAIL_MS:
        return "FAIL"
    if utt is not None and utt - ttfa < 5 and utt >= WHOLE_UTT_FAIL_MS:
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
        "stderr": text.strip().splitlines()[-8:],
    }


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
    if stub_judge != "PASS":
        fails.append(f"protocol stream clock failed: {stub}")
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
        model_measured = model.get("engine") == "model" and model.get("path") == "stream"
        if model.get("path") == "whole" or (model.get("ttfa_ms") is not None and model["ttfa_ms"] >= WHOLE_UTT_FAIL_MS):
            fails.append(f"model clock is whole-utterance: {model}")
        if not model_measured and judge(model) == "FAIL" and model.get("engine") not in (None, "unwired"):
            fails.append(f"model stream eval failed: {model}")
    report = {
        "gate": "FAIL" if fails else "PASS",
        "fails": fails,
        "path": stub.get("path"),
        "engine": stub.get("engine"),
        "chunks": stub.get("chunks"),
        "ttfa_ms": stub.get("ttfa_ms"),
        "utt_ms": stub.get("utt_ms"),
        "gap_ms": gap,
        "target_ms": [TARGET_LO_MS, TARGET_HI_MS],
        "whole_utt_fail_ms": WHOLE_UTT_FAIL_MS,
        "model_measured": model_measured,
        "product_claim": False,
        "heartbeat_step": "unchanged; drain_frame is outside step and does not generate",
        "piper_fallback": "SpeechStack still hands a dead clone to Piper",
        "whole_utt_fixture": judge(whole),
        "note": (
            "Protocol clock only. Target 80-160 ms is the Nano chunk budget "
            "(2 speech tokens at 25 Hz, after a 3-token lookahead). "
            "A model TTFA is absent until GALION_CLONE_STREAM_EVAL=1 returns engine=model."
        ),
    }
    if model is not None:
        report["model"] = {
            "engine": model.get("engine"),
            "path": model.get("path"),
            "ttfa_ms": model.get("ttfa_ms"),
            "utt_ms": model.get("utt_ms"),
            "chunks": model.get("chunks"),
        }
    if fails:
        die(report)
    ok(report)


if __name__ == "__main__":
    main()
