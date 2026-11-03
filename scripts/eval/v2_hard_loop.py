#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Hard loop for Galion Voice V2.

Penetrates the voice loop: reference hashes, Nano generate when the pinned
weights are actually on disk, streaming time-to-first-audio, persona HTTP,
the live one-dot, and late=pause on Heartbeat::step.

Overall verdicts are only STUB-PASS, HARD-PASS, or HARD-FAIL.
HARD-PASS is refused unless reference clips hash-match and a model engine
returns streaming PCM. A protocol stub is STUB-PASS for that pipe, never
HARD-PASS. This file does not download weights, does not write WAV or
safetensors into the repo, and does not read or edit LOCKS.toml.
"""

from __future__ import annotations

import hashlib
import http.client
import json
import os
import selectors
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlparse

REPO = Path(__file__).resolve().parents[2]
WORKER = REPO / "scripts" / "clone" / "chatterbox_worker.py"
PINS = REPO / "scripts" / "clone" / "PINS.sha256"
LOCKS = REPO / "LOCKS.toml"

NANO_RELS = (
    "shared/s3gen.safetensors",
    "shared/s3gen_meanflow.safetensors",
    "shared/ve.safetensors",
    "nano/t3_nano_v1.safetensors",
)
PERSONAS = ("samantha", "jarvis")
# Presence-eval fixtures already on main. They are not Samantha or Jarvis refs.
FIXTURE_WAVS = {
    "specs/fixtures/p7-turn-mono-8k16.wav",
    "specs/fixtures/p8-barge-stereo-8k16.wav",
}
TRACKED_AUDIO = {
    ".wav",
    ".wave",
    ".mp3",
    ".flac",
    ".ogg",
    ".opus",
    ".m4a",
    ".aac",
    ".mp4",
    ".mkv",
    ".mov",
    ".webm",
}
TRACKED_WEIGHTS = {".pt", ".onnx", ".safetensors"}
FORBIDDEN = (
    "i hear you",
    "that sounds so hard",
    "that sounds hard",
    "i'm here for you",
    "i feel that in my soul",
    "i know this is difficult",
    "i'm worried about you",
    "take a deep breath",
    "so what i'm hearing",
    "noted, that is hard",
    "i'm sorry for your loss",
    "let's slow down",
    "i'm sorry",
    "i am sorry",
)
KILL_MS = 1500
HEAR_MS = 160
STATUSES = ("met", "absent", "failed", "stub")


@dataclass
class Gate:
    name: str
    status: str
    detail: str

    def __post_init__(self) -> None:
        if self.status not in STATUSES:
            raise ValueError(self.status)


@dataclass
class WorkerProbe:
    returncode: int | None
    engine: str | None
    pcm_bytes: int
    ttfa_ms: int | None
    whole_ms: int | None
    streaming: bool
    stderr: str
    hung: bool


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_pins(text: str) -> dict[str, tuple[str, int]]:
    table: dict[str, tuple[str, int]] = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        rel, sha, size, *_ = line.split()
        table[rel] = (sha.lower(), int(size))
    return table


def inside_repo(repo: Path, path: Path) -> bool:
    try:
        path.resolve().relative_to(repo.resolve())
    except ValueError:
        return False
    return True


def evaluate_refs(repo: Path, flag: str, store: str) -> Gate:
    """Hash Samantha and Jarvis clips. Sidecars are the allowlist.

    LOCKS.toml is not an input. A mismatch is reported and the wav is left
    where it is.
    """
    if flag != "1" or not store.strip():
        return Gate(
            "ref-hashes",
            "absent",
            "GALION_CLONE_LOCAL_REFS=1 and GALION_CLIP_STORE are both required. "
            "LOCKS.toml is not read and reference_clips is not flipped. Not HARD-PASS.",
        )
    store_path = Path(store).expanduser()
    if not store_path.is_absolute():
        return Gate(
            "ref-hashes",
            "failed",
            "GALION_CLIP_STORE must be an absolute path outside the git work tree. Not HARD-PASS.",
        )
    store_path = store_path.resolve()
    if inside_repo(repo, store_path):
        return Gate(
            "ref-hashes",
            "failed",
            "GALION_CLIP_STORE is inside the git work tree. Reference clips stay outside git. "
            "Nothing was deleted. Not HARD-PASS.",
        )
    missing: list[str] = []
    bad: list[str] = []
    for persona in PERSONAS:
        wav = store_path / persona / "reference.wav"
        side = store_path / persona / "reference.wav.sha256"
        if not wav.is_file():
            missing.append(persona)
            continue
        if not side.is_file():
            bad.append(f"{persona} wav has no sha256 sidecar")
            continue
        token = side.read_text(encoding="utf-8", errors="replace").split()
        want = token[0].strip().lower() if token else ""
        if len(want) != 64 or any(c not in "0123456789abcdef" for c in want):
            bad.append(f"{persona} sidecar is not 64 hex characters")
            continue
        got = sha256_file(wav)
        if got != want:
            bad.append(f"{persona} sha256 mismatch")
    if bad:
        return Gate(
            "ref-hashes",
            "failed",
            "; ".join(bad) + ". The wav files were not deleted. Not HARD-PASS.",
        )
    if missing:
        return Gate(
            "ref-hashes",
            "absent",
            "missing reference.wav for " + ", ".join(missing) + ". Not HARD-PASS.",
        )
    return Gate(
        "ref-hashes",
        "met",
        "samantha and jarvis reference.wav match their sidecars outside the repo. "
        "The bytes were not committed.",
    )


def evaluate_weights(root: Path, pins: dict[str, tuple[str, int]]) -> Gate:
    present = []
    for rel in NANO_RELS:
        path = root / rel
        if path.is_file():
            present.append(rel)
    if not present:
        return Gate(
            "weight-pins",
            "absent",
            "Pinned Nano safetensors are not on disk. Fetch was not run. Not HARD-PASS.",
        )
    if len(present) != len(NANO_RELS):
        return Gate(
            "weight-pins",
            "failed",
            "Nano pack is incomplete (" + ", ".join(present) + "). Files were not deleted. Not HARD-PASS.",
        )
    bad = []
    for rel in NANO_RELS:
        path = root / rel
        sha, size = pins[rel]
        if path.stat().st_size != size:
            bad.append(f"{rel} size mismatch")
            continue
        if sha256_file(path) != sha:
            bad.append(f"{rel} sha256 mismatch")
    if bad:
        return Gate(
            "weight-pins",
            "failed",
            "; ".join(bad) + ". PINS.sha256 was not rewritten and the files were not deleted. Not HARD-PASS.",
        )
    return Gate(
        "weight-pins",
        "met",
        "Nano safetensors match scripts/clone/PINS.sha256. They are not committed by this harness.",
    )


def forbidden_tracked(path: str) -> bool:
    if path in FIXTURE_WAVS:
        return False
    suffix = Path(path).suffix.lower()
    if suffix in TRACKED_WEIGHTS or suffix in TRACKED_AUDIO:
        return True
    if path.startswith("weights/") and path != "weights/.gitignore":
        return True
    name = Path(path).name.lower()
    return name == "reference.wav"


def classify_hygiene(tracked: list[str], locks_same: bool) -> Gate:
    bad = [path for path in tracked if forbidden_tracked(path)]
    if bad:
        shown = ", ".join(bad[:8])
        return Gate(
            "tree-hygiene",
            "failed",
            f"tracked audio or weight blobs: {shown}. Not HARD-PASS.",
        )
    if not locks_same:
        return Gate(
            "tree-hygiene",
            "failed",
            "LOCKS.toml changed during the run. This harness does not edit it. Not HARD-PASS.",
        )
    return Gate(
        "tree-hygiene",
        "met",
        "No clone weight or reference clip is tracked. LOCKS.toml was not modified.",
    )


def classify_refuse(probe: WorkerProbe) -> Gate:
    if probe.hung:
        return Gate("ref-refuse", "failed", "worker hung on an unallowlisted reference. Not HARD-PASS.")
    held = (
        probe.returncode == 3
        and probe.pcm_bytes == 0
        and "rights hold" in probe.stderr
    )
    if held:
        return Gate(
            "ref-refuse",
            "met",
            "An unallowlisted reference exited 3 with no PCM. The rights hold holds. "
            "This is not a generate pass.",
        )
    return Gate(
        "ref-refuse",
        "failed",
        f"reference clip was not refused (code {probe.returncode}, pcm {probe.pcm_bytes}). Not HARD-PASS.",
    )


def classify_generate(
    probe: WorkerProbe,
    weights_met: bool,
    weights_failed: bool,
    sent_ref: bool,
    stub_env: bool,
) -> Gate:
    if stub_env or probe.engine == "protocol-stub":
        return Gate(
            "nano-generate",
            "stub",
            "GALION_CLONE_STUB_PCM protocol PCM. Engine is not model. Not HARD-PASS.",
        )
    if probe.hung:
        return Gate("nano-generate", "failed", "worker hung before PCM. Not HARD-PASS.")
    if "galion-clone-absent" in probe.stderr or probe.returncode == 2:
        return Gate(
            "nano-generate",
            "absent",
            "Nano weights are not on disk. Generate was not run. Not HARD-PASS.",
        )
    if sent_ref and probe.returncode == 3:
        return Gate(
            "nano-generate",
            "failed",
            "The allowlisted reference was refused (rights hold, exit 3, no PCM). "
            "Conditioned Nano generate is not available. Not HARD-PASS.",
        )
    if probe.engine == "model" and probe.pcm_bytes >= 2:
        how = "conditioned on the allowlisted reference" if sent_ref else "text only; no reference clip was sent"
        return Gate(
            "nano-generate",
            "met",
            f"engine model, {probe.pcm_bytes} PCM bytes, {how}.",
        )
    if weights_failed or weights_met:
        return Gate(
            "nano-generate",
            "failed",
            f"weights are on disk but the worker did not generate (engine {probe.engine or 'none'}). Not HARD-PASS.",
        )
    return Gate(
        "nano-generate",
        "absent",
        "generate is not wired and weights are absent. No PCM. Not HARD-PASS.",
    )


def classify_ttfa(probe: WorkerProbe, weights_met: bool, weights_failed: bool) -> Gate:
    if probe.engine == "protocol-stub":
        clock = f"{probe.ttfa_ms} ms" if probe.ttfa_ms is not None else "no clock"
        return Gate(
            "ttfa-streaming",
            "stub",
            f"protocol-stub {clock}. product_claim false. Not a streaming Nano result. Not HARD-PASS.",
        )
    if probe.hung:
        return Gate("ttfa-streaming", "failed", "worker hung. No TTFA number. Not HARD-PASS.")
    absent = "galion-clone-absent" in probe.stderr or probe.returncode == 2
    if absent or (probe.pcm_bytes == 0 and not weights_met and not weights_failed):
        return Gate(
            "ttfa-streaming",
            "absent",
            "no PCM, so no TTFA number. Not HARD-PASS.",
        )
    if probe.engine != "model" or probe.pcm_bytes < 2 or probe.ttfa_ms is None:
        if weights_met or weights_failed:
            return Gate(
                "ttfa-streaming",
                "failed",
                "weights are on disk and there is no model first-PCM time. No product TTFA. Not HARD-PASS.",
            )
        return Gate("ttfa-streaming", "absent", "generate is not wired. No TTFA number. Not HARD-PASS.")
    whole = probe.whole_ms if probe.whole_ms is not None else probe.ttfa_ms
    if not probe.streaming:
        if whole >= KILL_MS:
            return Gate(
                "ttfa-streaming",
                "failed",
                f"whole-utterance blocking path, {whole} ms >= {KILL_MS} ms. "
                "First PCM was not early. Not HARD-PASS.",
            )
        return Gate(
            "ttfa-streaming",
            "failed",
            f"no streaming evidence. Whole utterance {whole} ms is under {KILL_MS} ms "
            "and is still not a streaming path. Not HARD-PASS.",
        )
    if probe.ttfa_ms >= KILL_MS:
        return Gate(
            "ttfa-streaming",
            "failed",
            f"streaming TTFA {probe.ttfa_ms} ms >= {KILL_MS} ms. Not HARD-PASS.",
        )
    extra = ""
    if probe.ttfa_ms > HEAR_MS:
        extra = f" Over the {HEAR_MS} ms hearable budget; under the {KILL_MS} ms kill line."
    return Gate(
        "ttfa-streaming",
        "met",
        f"engine model, streaming, ttfa_ms {probe.ttfa_ms}, whole_ms {whole}.{extra}",
    )


def overall_verdict(gates: list[Gate]) -> str:
    if any(gate.status == "failed" for gate in gates):
        return "HARD-FAIL"
    if gates and all(gate.status == "met" for gate in gates):
        return "HARD-PASS"
    return "STUB-PASS"


def exit_code(verdict: str, require: bool) -> int:
    if verdict == "HARD-FAIL":
        return 1
    if verdict == "HARD-PASS":
        return 0
    if require:
        return 2
    return 0


def assert_no_bare_pass(text: str) -> None:
    scrubbed = text.replace("STUB-PASS", "").replace("HARD-PASS", "").replace("HARD-FAIL", "")
    if "PASS" in scrubbed or "\nOK\n" in f"\n{scrubbed}\n":
        raise RuntimeError("harness tried to print a bare PASS")


def render(gates: list[Gate], verdict: str) -> str:
    lines = [
        "Galion Voice hard loop",
        "STUB-PASS means the protocol checks finished and the hard loop did not.",
        "HARD-PASS is the overall verdict only when every gate below is met, "
        "including real Nano weights, reference hashes, and streaming TTFA.",
        "A gate status of met is that gate only. It does not by itself make the loop HARD-PASS.",
    ]
    for gate in gates:
        lines.append(f"gate {gate.name}: {gate.status} — {gate.detail}")
    lines.append(f"overall: {verdict}")
    if verdict == "HARD-PASS":
        lines.append("product_claim: true")
    else:
        lines.append("product_claim: false")
        lines.append("HARD-PASS of the loop: refused")
    text = "\n".join(lines) + "\n"
    assert_no_bare_pass(text)
    return text


def report_json(gates: list[Gate], verdict: str, probe: WorkerProbe | None) -> dict:
    body: dict = {
        "product": "Galion Voice",
        "overall": verdict,
        "hard_pass": verdict == "HARD-PASS",
        "product_claim": verdict == "HARD-PASS",
        "gates": [{"name": g.name, "status": g.status, "detail": g.detail} for g in gates],
    }
    ttfa_gate = next((g for g in gates if g.name == "ttfa-streaming"), None)
    if probe and probe.ttfa_ms is not None and ttfa_gate and ttfa_gate.status != "absent":
        body["ttfa_ms"] = probe.ttfa_ms
        body["whole_ms"] = probe.whole_ms
        if probe.engine == "protocol-stub":
            body["ttfa_kind"] = "protocol-stub"
        elif probe.engine == "model" and probe.streaming:
            body["ttfa_kind"] = "streaming-model"
        elif probe.engine == "model":
            body["ttfa_kind"] = "whole-utterance"
        else:
            body["ttfa_kind"] = probe.engine or "unknown"
    return body


def child_env(stub: bool) -> dict[str, str]:
    env = os.environ.copy()
    env.pop("GALION_CLONE_FETCH", None)
    env.pop("GALION_FLASH_EVAL", None)
    if stub:
        env["GALION_CLONE_STUB_PCM"] = "1"
    else:
        env.pop("GALION_CLONE_STUB_PCM", None)
    return env


def _engine_of(stderr: str) -> str | None:
    found = None
    for line in stderr.splitlines():
        if line.startswith("galion-clone-engine:"):
            found = line.split(":", 1)[1].strip()
    return found


def probe_worker(request: dict, stub: bool, timeout_s: float) -> WorkerProbe:
    env = child_env(stub)
    proc = subprocess.Popen(
        [sys.executable, str(WORKER)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        cwd=str(REPO),
    )
    assert proc.stdin and proc.stdout and proc.stderr
    payload = (json.dumps(request) + "\n").encode()
    start = time.monotonic()
    try:
        proc.stdin.write(payload)
        proc.stdin.close()
    except BrokenPipeError:
        pass
    selector = selectors.DefaultSelector()
    out_fd = proc.stdout.fileno()
    err_fd = proc.stderr.fileno()
    selector.register(out_fd, selectors.EVENT_READ, "out")
    selector.register(err_fd, selectors.EVENT_READ, "err")
    pcm = bytearray()
    err = bytearray()
    first_pcm: float | None = None
    utt_at: float | None = None
    first_marker: float | None = None
    hung = False
    while selector.get_map():
        if time.monotonic() - start > timeout_s:
            hung = True
            break
        events = selector.select(timeout=0.05)
        for key, _ in events:
            try:
                chunk = os.read(key.fd, 65536)
            except OSError:
                chunk = b""
            now = time.monotonic()
            if not chunk:
                selector.unregister(key.fd)
                continue
            if key.data == "out":
                if first_pcm is None:
                    first_pcm = now
                pcm.extend(chunk)
            else:
                err.extend(chunk)
                text = err.decode("utf-8", "replace")
                if first_marker is None and "galion-clone-first" in text:
                    first_marker = now
                if utt_at is None and "galion-clone-utt:" in text:
                    utt_at = now
        if proc.poll() is not None and not events and (time.monotonic() - start) > 0.3:
            break
    if hung or proc.poll() is None:
        proc.kill()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2)
    stderr = err.decode("utf-8", "replace")
    streaming = False
    if first_marker is not None and (utt_at is None or first_marker + 0.005 < utt_at):
        streaming = True
    elif first_pcm is not None and utt_at is not None and (utt_at - first_pcm) >= 0.020:
        streaming = True
    ttfa = int((first_pcm - start) * 1000) if first_pcm is not None else None
    whole = int((utt_at - start) * 1000) if utt_at is not None else None
    return WorkerProbe(
        returncode=proc.returncode,
        engine=_engine_of(stderr),
        pcm_bytes=len(pcm),
        ttfa_ms=ttfa,
        whole_ms=whole,
        streaming=streaming,
        stderr=stderr,
        hung=hung,
    )


def tracked_files() -> list[str]:
    out = subprocess.check_output(["git", "ls-files", "-z"], cwd=str(REPO))
    return [item.decode() for item in out.split(b"\0") if item]


def locks_digest() -> str:
    return sha256_file(LOCKS)


def cargo_late_cmd(offline: bool) -> list[str]:
    cmd = ["cargo", "test", "-p", "galion-heartbeat"]
    if offline:
        cmd.append("--offline")
    cmd += ["--test", "heartbeat", "--", "late_", "--test-threads=1"]
    return cmd


def run_late_pause() -> Gate:
    if os.environ.get("GALION_HARD_SKIP_CARGO") == "1":
        return Gate(
            "late-pause",
            "absent",
            "GALION_HARD_SKIP_CARGO=1, so Heartbeat::step was not executed. Not HARD-PASS.",
        )
    offline = cargo_late_cmd(True)
    ran = subprocess.run(offline, cwd=str(REPO), capture_output=True, text=True, timeout=900)
    text = ran.stdout + "\n" + ran.stderr
    if ran.returncode != 0 and "offline" in text.lower():
        ran = subprocess.run(cargo_late_cmd(False), cwd=str(REPO), capture_output=True, text=True, timeout=900)
        text = ran.stdout + "\n" + ran.stderr
    needed = (
        "late_frame_is_a_pause_no_extra_trigger_no_burst",
        "short_pcm_frame_is_treated_as_late_pause",
    )
    if ran.returncode == 0 and "test result: ok" in text and all(name in text for name in needed):
        return Gate(
            "late-pause",
            "met",
            "Heartbeat::step late frame is paused, me onset stays closed, and a short PCM frame pauses. "
            "No catch-up burst.",
        )
    tail = " ".join(text.strip().split())[-400:]
    return Gate("late-pause", "failed", f"late=pause test did not pass. {tail} Not HARD-PASS.")


def http_exchange(base: str, method: str, path: str, body: bytes | None = None, headers: dict | None = None):
    parsed = urlparse(base)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or 80
    conn = http.client.HTTPConnection(host, port, timeout=3)
    try:
        conn.request(method, path, body=body, headers=headers or {})
        resp = conn.getresponse()
        payload = resp.read()
        hdrs = {k.lower(): v for k, v in resp.getheaders()}
        return resp.status, hdrs, payload
    finally:
        conn.close()


def _json_has(blob: str, key: str, value: str) -> bool:
    return f'"{key}":"{value}"' in blob or f'"{key}": "{value}"' in blob


def persona_http(base: str) -> tuple[Gate, list[str]]:
    samples: list[str] = []
    try:
        status, _, health = http_exchange(base, "GET", "/health")
    except OSError as exc:
        return Gate("persona-http", "absent", f"no live server at {base} ({exc}). Not HARD-PASS."), samples
    if status != 200 or health.decode().strip() != "ok":
        return Gate("persona-http", "failed", f"/health was {status} {health!r}. Not HARD-PASS."), samples
    try:
        status, hdrs, _ = http_exchange(base, "GET", "/v2")
        state_code, _, state = http_exchange(base, "GET", "/v2/state")
    except OSError as exc:
        return Gate("persona-http", "failed", f"persona routes dropped ({exc}). Not HARD-PASS."), samples
    problems = []
    if status != 302 or hdrs.get("location", "") != "/":
        problems.append(f"/v2 was {status} location {hdrs.get('location')}")
    state_text = state.decode("utf-8", "replace")
    if state_code != 200 or '"rights":"hold"' not in state_text or '"film_voice":false' not in state_text:
        problems.append("/v2/state is not the rights hold")
    if '"clone_live":true' in state_text:
        problems.append("/v2/state claims clone_live")

    def say(text: str) -> str:
        raw = json.dumps({"text": text}).encode()
        code, _, payload = http_exchange(
            base,
            "POST",
            "/v2/say",
            body=raw,
            headers={"Content-Type": "application/json", "Content-Length": str(len(raw))},
        )
        if code != 200:
            problems.append(f"/v2/say {text!r} status {code}")
            return ""
        return payload.decode("utf-8", "replace")

    grief = say("My mother died this morning.")
    if not _json_has(grief, "mood", "grief") or "Your mother died this morning." not in grief:
        problems.append("Samantha grief did not stay on the fact")
    if not _json_has(grief, "persona", "samantha") or '"film_voice":false' not in grief:
        problems.append("Samantha grief persona or film flag")
    joy = say("I got the job. I can't stop smiling.")
    if not _json_has(joy, "mood", "joy"):
        problems.append("joy mood missing")
    vuln = say("I don't think anyone actually likes me.")
    if not _json_has(vuln, "mood", "vulnerability"):
        problems.append("vulnerability mood missing")
    quiet = say("You don't have to say anything. Just stay.")
    if '"silence":true' not in quiet or not _json_has(quiet, "mood", "quiet"):
        problems.append("quiet did not go silent")
    jarvis = say("be Jarvis")
    if not _json_has(jarvis, "persona", "jarvis") or "Here. What do you need?" not in jarvis:
        problems.append("Jarvis pin did not stick on the switch line")
    jarvis_grief = say("My mother died this morning.")
    if not _json_has(jarvis_grief, "persona", "jarvis") or not _json_has(jarvis_grief, "mood", "grief"):
        problems.append("pinned Jarvis left the grief fact")
    blob = " ".join((grief, joy, vuln, quiet, jarvis, jarvis_grief)).lower()
    for phrase in FORBIDDEN:
        if phrase in blob:
            problems.append(f"canned empathy {phrase!r}")
    try:
        _, _, dot = http_exchange(base, "GET", "/dot")
        dot_text = dot.decode("utf-8", "replace")
        samples.append(dot_text)
    except OSError as exc:
        problems.append(f"/dot after say failed ({exc})")
        dot_text = ""
    if not _json_has(dot_text, "persona", "jarvis") or not _json_has(dot_text, "mood", "grief"):
        problems.append("/dot did not carry the pinned persona and grief mood")
    if problems:
        return Gate("persona-http", "failed", "; ".join(problems) + ". Not HARD-PASS."), samples
    return (
        Gate(
            "persona-http",
            "met",
            f"Live {base} answered Samantha and Jarvis without a canned empathy line. "
            "film_voice is false. This HTTP floor is not a Nano generate.",
        ),
        samples,
    )


def _num(blob: str, key: str) -> int | None:
    token = f'"{key}":'
    idx = blob.find(token)
    if idx < 0:
        return None
    rest = blob[idx + len(token) :].lstrip()
    digits = []
    for ch in rest:
        if ch.isdigit():
            digits.append(ch)
        else:
            break
    return int("".join(digits)) if digits else None


def live_one_dot(base: str, launched: bool) -> Gate:
    samples: list[str] = []
    deadline = time.monotonic() + 8.0
    try:
        while time.monotonic() < deadline:
            _, _, payload = http_exchange(base, "GET", "/dot")
            samples.append(payload.decode("utf-8", "replace"))
            time.sleep(0.08)
    except OSError as exc:
        return Gate("live-one-dot", "absent", f"/dot was not reachable ({exc}). Not HARD-PASS.")
    def quoted(sample: str, key: str) -> str | None:
        mark = f'"{key}":"'
        at = sample.find(mark)
        if at < 0:
            return None
        return sample[at + len(mark) :].split('"', 1)[0]

    times = [n for n in (_num(s, "t_ms") for s in samples) if n is not None]
    frames = [n for n in (_num(s, "frames") for s in samples) if n is not None]
    pairs = [(quoted(s, "mode"), quoted(s, "state")) for s in samples]
    modes = [mode for mode, _state in pairs if mode]
    live = False
    if len(times) >= 2 and max(times) > min(times):
        live = True
    if len(frames) >= 2 and max(frames) > min(frames):
        live = True
    if not live:
        return Gate(
            "live-one-dot",
            "failed",
            "the /dot feed did not advance t_ms or engine frames. Not a live one-dot. Not HARD-PASS.",
        )
    if not any('"persona":' in s for s in samples):
        return Gate("live-one-dot", "failed", "/dot has no persona field. Not HARD-PASS.")
    spoke = [state for mode, state in pairs if mode == "speaking"]
    other = [mode for mode in modes if mode != "speaking"]
    speak_wins = bool(spoke) and bool(other) and all(state == "speaking" for state in spoke)
    if speak_wins:
        return Gate(
            "live-one-dot",
            "met",
            "Live /dot advanced with the heartbeat. Speaking was observed and the state was speaking "
            "(speak wins). A non-speaking sample was also observed.",
        )
    if launched:
        return Gate(
            "live-one-dot",
            "failed",
            "synthetic demo did not show speaking winning over another mode. Not HARD-PASS.",
        )
    return Gate(
        "live-one-dot",
        "absent",
        "The attached /dot feed is moving, and speaking-wins was not observed. "
        "Launch with GALION_HARD_HTTP=1 so the harness runs the synthetic room. Not HARD-PASS.",
    )


def launch_demo(addr: str) -> subprocess.Popen | None:
    built = subprocess.run(
        [
            "cargo",
            "build",
            "-q",
            "--offline",
            "-p",
            "galion-console",
            "--features",
            "dot-viz-web",
            "--example",
            "dot_console_demo",
        ],
        cwd=str(REPO),
        capture_output=True,
        text=True,
        timeout=900,
    )
    if built.returncode != 0 and "offline" in (built.stderr + built.stdout).lower():
        built = subprocess.run(
            [
                "cargo",
                "build",
                "-q",
                "-p",
                "galion-console",
                "--features",
                "dot-viz-web",
                "--example",
                "dot_console_demo",
            ],
            cwd=str(REPO),
            capture_output=True,
            text=True,
            timeout=900,
        )
    binary = REPO / "target" / "debug" / "examples" / "dot_console_demo"
    if built.returncode != 0 or not binary.is_file():
        sys.stderr.write(built.stderr[-800:] + "\n")
        return None
    log_dir = REPO / "target" / "hard-loop"
    log_dir.mkdir(parents=True, exist_ok=True)
    log = (log_dir / "demo.log").open("w", encoding="utf-8")
    env = os.environ.copy()
    env["GALION_ASR"] = "stub"
    env["GALION_LLM"] = "canned"
    env.pop("GALION_CLONE_FETCH", None)
    return subprocess.Popen(
        [
            str(binary),
            "--headless",
            "--realtime",
            "--synthetic",
            "--no-png",
            "--feed",
            "stub",
            "--web",
            addr,
            "--seconds",
            os.environ.get("GALION_HARD_SECONDS", "20"),
        ],
        cwd=str(REPO),
        stdout=log,
        stderr=subprocess.STDOUT,
        env=env,
    )


def wait_health(base: str, proc: subprocess.Popen | None, seconds: float) -> bool:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if proc is not None and proc.poll() is not None:
            return False
        try:
            status, _, body = http_exchange(base, "GET", "/health")
            if status == 200 and body.decode().strip() == "ok":
                return True
        except OSError:
            pass
        time.sleep(0.1)
    return False


def weight_root() -> Path:
    raw = os.environ.get("GALION_CLONE_WEIGHTS", "").strip()
    if raw:
        return Path(raw)
    return REPO / "weights" / "clone"


def collect() -> tuple[list[Gate], WorkerProbe | None]:
    before = locks_digest()
    pins = parse_pins(PINS.read_text(encoding="utf-8"))
    weights = evaluate_weights(weight_root(), pins)
    refs = evaluate_refs(
        REPO,
        os.environ.get("GALION_CLONE_LOCAL_REFS", ""),
        os.environ.get("GALION_CLIP_STORE", ""),
    )
    refuse = probe_worker({"text": "hi", "reference": "clip.wav"}, stub=True, timeout_s=10)
    stub_env = os.environ.get("GALION_CLONE_STUB_PCM") == "1"
    weights_met = weights.status == "met"
    sent_ref = refs.status == "met" and weights_met and not stub_env
    request: dict = {"text": "Hi.", "model": "nano", "persona": "samantha", "tags": True}
    if sent_ref:
        store = Path(os.environ["GALION_CLIP_STORE"]).expanduser().resolve()
        request["audio_prompt_path"] = str(store / "samantha" / "reference.wav")
    timeout = float(os.environ.get("GALION_HARD_GEN_TIMEOUT", "180" if weights_met else "15"))
    generated = probe_worker(request, stub=stub_env, timeout_s=timeout)
    hygiene = classify_hygiene(tracked_files(), locks_digest() == before)
    gates = [
        hygiene,
        classify_refuse(refuse),
        refs,
        weights,
        classify_generate(generated, weights_met, weights.status == "failed", sent_ref, stub_env),
        classify_ttfa(generated, weights_met, weights.status == "failed"),
    ]
    base = os.environ.get("BASE", "").strip()
    launched = False
    proc = None
    http_recorded = False
    http_flag = os.environ.get("GALION_HARD_HTTP") == "1"
    if not base:
        base = "http://127.0.0.1:7878"
    reachable = False
    try:
        status, _, body = http_exchange(base, "GET", "/health")
        reachable = status == 200 and body.decode().strip() == "ok"
    except OSError:
        reachable = False
    if not reachable and http_flag:
        addr = os.environ.get("GALION_HARD_ADDR", "127.0.0.1:7879")
        base = f"http://{addr}"
        proc = launch_demo(addr)
        launched = proc is not None
        reachable = proc is not None and wait_health(base, proc, 60)
        if proc is not None and not reachable:
            proc.terminate()
            gates.append(Gate("persona-http", "failed", "demo process did not serve /health. Not HARD-PASS."))
            gates.append(Gate("live-one-dot", "failed", "demo process did not serve /dot. Not HARD-PASS."))
            proc = None
            http_recorded = True
    if not reachable and http_flag and proc is None and not http_recorded:
        gates.append(Gate("persona-http", "failed", "GALION_HARD_HTTP=1 but the demo binary did not build. Not HARD-PASS."))
        gates.append(Gate("live-one-dot", "failed", "GALION_HARD_HTTP=1 but the demo binary did not build. Not HARD-PASS."))
        http_recorded = True
    if proc is None and not reachable and not http_recorded:
        why = f"no live server at {base}"
        gates.append(Gate("persona-http", "absent", why + ". Not HARD-PASS."))
        gates.append(Gate("live-one-dot", "absent", why + ". Not HARD-PASS."))
    elif reachable:
        try:
            dot = live_one_dot(base, launched)
            persona, _samples = persona_http(base)
            gates.extend((persona, dot))
        finally:
            if proc is not None:
                proc.terminate()
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    proc.kill()
    gates.append(run_late_pause())
    if locks_digest() != before:
        gates[0] = Gate(
            "tree-hygiene",
            "failed",
            "LOCKS.toml changed during the run. This harness does not edit it. Not HARD-PASS.",
        )
    return gates, generated


def logic_self_check() -> None:
    stub = WorkerProbe(0, "protocol-stub", 402, 4, 5, False, "galion-clone-engine: protocol-stub\n", False)
    absent = WorkerProbe(2, None, 0, None, None, False, "galion-clone-absent\n", False)
    model_stream = WorkerProbe(0, "model", 48000, 90, 400, True, "galion-clone-engine: model\n", False)
    model_block = WorkerProbe(0, "model", 48000, 1800, 1800, False, "galion-clone-engine: model\n", False)
    model_fast_block = WorkerProbe(0, "model", 800, 200, 210, False, "galion-clone-engine: model\n", False)
    unwired = WorkerProbe(0, "unwired", 0, None, None, False, "galion-clone-unwired\n", False)
    refused = WorkerProbe(3, None, 0, None, None, False, "galion-clone-refuse: reference clips are on the rights hold\n", False)

    assert classify_refuse(refused).status == "met"
    assert classify_generate(stub, False, False, False, True).status == "stub"
    assert classify_generate(absent, False, False, False, False).status == "absent"
    assert classify_generate(model_stream, True, False, True, False).status == "met"
    assert classify_generate(unwired, True, False, False, False).status == "failed"
    assert classify_generate(refused, True, False, True, False).status == "failed"
    assert classify_ttfa(stub, False, False).status == "stub"
    assert "ttfa" not in classify_ttfa(absent, False, False).detail.lower() or "no TTFA" in classify_ttfa(absent, False, False).detail
    assert classify_ttfa(absent, False, False).status == "absent"
    assert "ttfa_ms" not in classify_ttfa(absent, False, False).detail
    assert classify_ttfa(model_stream, True, False).status == "met"
    assert classify_ttfa(model_block, True, False).status == "failed"
    assert "whole-utterance" in classify_ttfa(model_block, True, False).detail
    assert classify_ttfa(model_fast_block, True, False).status == "failed"
    assert classify_ttfa(model_stream, True, False).detail.find("90") >= 0

    with tempfile.TemporaryDirectory(prefix="galion-hard-refs-") as tmp:
        root = Path(tmp)
        assert not inside_repo(REPO, root)
        for persona in PERSONAS:
            folder = root / persona
            folder.mkdir()
            wav = folder / "reference.wav"
            wav.write_bytes(b"galion-ref-fixture")
            side = folder / "reference.wav.sha256"
            side.write_text(sha256_file(wav) + f"  reference.wav\n", encoding="utf-8")
        good = evaluate_refs(REPO, "1", str(root))
        assert good.status == "met", good.detail
        (root / "samantha" / "reference.wav.sha256").write_text("ab" * 32 + "\n", encoding="utf-8")
        bad = evaluate_refs(REPO, "1", str(root))
        assert bad.status == "failed", bad.detail
        assert (root / "samantha" / "reference.wav").is_file()
        assert evaluate_refs(REPO, "", str(root)).status == "absent"
        assert evaluate_refs(REPO, "1", "").status == "absent"

    inside = evaluate_refs(REPO, "1", str(REPO / "docs"))
    assert inside.status == "failed"
    assert not (REPO / "docs" / "samantha").exists()

    pins = parse_pins(PINS.read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="galion-hard-weights-") as tmp:
        empty = evaluate_weights(Path(tmp), pins)
        assert empty.status == "absent"
        nano = Path(tmp) / "nano"
        nano.mkdir()
        (nano / "t3_nano_v1.safetensors").write_bytes(b"nope")
        partial = evaluate_weights(Path(tmp), pins)
        assert partial.status == "failed"
        assert (nano / "t3_nano_v1.safetensors").is_file()

    assert classify_hygiene(["docs/readme.md", *sorted(FIXTURE_WAVS)], True).status == "met"
    assert classify_hygiene(["weights/clone/nano/t3_nano_v1.safetensors"], True).status == "failed"
    assert classify_hygiene(["voice/reference.wav"], True).status == "failed"
    assert classify_hygiene(["notes/extra.wav"], True).status == "failed"
    assert classify_hygiene(["docs/readme.md"], False).status == "failed"
    assert cargo_late_cmd(True) == [
        "cargo",
        "test",
        "-p",
        "galion-heartbeat",
        "--offline",
        "--test",
        "heartbeat",
        "--",
        "late_",
        "--test-threads=1",
    ]

    full = [
        Gate(name, "met", "ok-for-test")
        for name in (
            "tree-hygiene",
            "ref-refuse",
            "ref-hashes",
            "weight-pins",
            "nano-generate",
            "ttfa-streaming",
            "persona-http",
            "live-one-dot",
            "late-pause",
        )
    ]
    assert overall_verdict(full) == "HARD-PASS"
    text = render(full, "HARD-PASS")
    assert "overall: HARD-PASS" in text
    assert "refused" not in text
    stub_gates = [
        Gate("tree-hygiene", "met", "clean"),
        Gate("ref-refuse", "met", "hold"),
        Gate("ref-hashes", "absent", "no store"),
        Gate("weight-pins", "absent", "no files"),
        Gate("nano-generate", "absent", "no weights"),
        Gate("ttfa-streaming", "absent", "no TTFA number"),
        Gate("persona-http", "absent", "down"),
        Gate("live-one-dot", "absent", "down"),
        Gate("late-pause", "met", "paused"),
    ]
    assert overall_verdict(stub_gates) == "STUB-PASS"
    stub_text = render(stub_gates, "STUB-PASS")
    assert "overall: STUB-PASS" in stub_text
    assert "HARD-PASS of the loop: refused" in stub_text
    assert "product_claim: false" in stub_text
    failed = list(stub_gates)
    failed[2] = Gate("ref-hashes", "failed", "mismatch")
    assert overall_verdict(failed) == "HARD-FAIL"
    assert exit_code("STUB-PASS", False) == 0
    assert exit_code("STUB-PASS", True) == 2
    assert exit_code("HARD-FAIL", False) == 1
    assert exit_code("HARD-PASS", True) == 0
    blob = report_json(stub_gates, "STUB-PASS", absent)
    assert "ttfa_ms" not in blob
    claimed = report_json(full, "HARD-PASS", model_stream)
    assert claimed["product_claim"] is True
    assert claimed["ttfa_ms"] == 90
    assert claimed["ttfa_kind"] == "streaming-model"


def worker_self_check() -> None:
    if not WORKER.is_file():
        raise RuntimeError("clone worker script missing")
    refused = probe_worker({"text": "hi", "reference": "clip.wav"}, stub=True, timeout_s=10)
    gate = classify_refuse(refused)
    if gate.status != "met":
        raise RuntimeError(gate.detail + "\n" + refused.stderr)
    env_weights = os.environ.get("GALION_CLONE_WEIGHTS", "")
    # The absent check is meaningful only when this process is not pointed at a real pack.
    root = weight_root()
    pack = all((root / rel).is_file() for rel in NANO_RELS)
    if not pack and not env_weights:
        absent = probe_worker({"text": "Hi.", "model": "nano", "persona": "samantha"}, stub=False, timeout_s=10)
        if "galion-clone-absent" not in absent.stderr or absent.pcm_bytes != 0:
            raise RuntimeError("expected galion-clone-absent with no PCM, got " + absent.stderr)


def main(argv: list[str]) -> int:
    os.chdir(REPO)
    if "--self-check" in argv:
        logic_self_check()
        worker_self_check()
        print("Galion Voice hard loop self-check")
        print("overall: STUB-PASS")
        print("product_claim: false")
        print("HARD-PASS of the loop: refused")
        print("self-check exercised the classifiers, the rights-hold refusal, and the absent-weight path.")
        return 0
    logic_self_check()
    gates, probe = collect()
    verdict = overall_verdict(gates)
    text = render(gates, verdict)
    sys.stdout.write(text)
    blob = report_json(gates, verdict, probe)
    sys.stdout.write("report: " + json.dumps(blob, separators=(",", ":")) + "\n")
    out_dir = REPO / "target" / "hard-loop"
    try:
        out_dir.mkdir(parents=True, exist_ok=True)
        (out_dir / "latest.txt").write_text(text + "report: " + json.dumps(blob, indent=2) + "\n", encoding="utf-8")
    except OSError:
        pass
    return exit_code(verdict, os.environ.get("GALION_HARD_REQUIRE") == "1")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except subprocess.TimeoutExpired as exc:
        sys.stderr.write(f"hard loop timed out: {exc}\n")
        raise SystemExit(1)
