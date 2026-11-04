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
import re
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
# One forbidden list. The source of truth is FORBIDDEN in
# crates/galion-cascade/src/hermes.rs (the guard the model reply goes through).
# FORBIDDEN_MIRROR is the copy for a box without the crate source; the self-check
# asserts it equals the parsed list, and the Rust test
# forbidden_list_is_mirrored_by_the_hard_harness asserts the other direction.
HERMES_RS = REPO / "crates" / "galion-cascade" / "src" / "hermes.rs"
FORBIDDEN_MIRROR = (
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


def parse_rust_forbidden(text: str) -> tuple[str, ...]:
    start = text.find("pub const FORBIDDEN: &[&str] = &[")
    if start < 0:
        return ()
    block = text[start:]
    block = block[block.find("[", block.find("=")) + 1 : block.find("];")]
    return tuple(re.findall(r'"((?:[^"\\]|\\.)*)"', block))


def load_forbidden() -> tuple[str, ...]:
    try:
        parsed = parse_rust_forbidden(HERMES_RS.read_text(encoding="utf-8"))
    except OSError:
        parsed = ()
    return parsed or FORBIDDEN_MIRROR


FORBIDDEN = load_forbidden()
# Spoken by hermes.rs when the model can't be reached (not a model reply).
UNREACHABLE_LINE = "model can't be reached"
# Words that name the grief fact the probe states ("My mother died this morning.").
MOTHER_WORDS = ("mother", "mom", "mum", "mama")


def target_dir() -> Path:
    """The cargo target dir: CARGO_TARGET_DIR (relative to REPO when relative), else REPO/target."""
    raw = os.environ.get("CARGO_TARGET_DIR", "").strip()
    if not raw:
        return REPO / "target"
    path = Path(raw)
    return path if path.is_absolute() else REPO / path


def llm_url() -> str:
    """GALION_LLM_URL is what hermes.rs reads; GALION_HERMES_URL is the old harness alias."""
    return (os.environ.get("GALION_LLM_URL", "") or os.environ.get("GALION_HERMES_URL", "")).strip()


def is_loopback_url(url: str) -> bool:
    parsed = urlparse(url)
    return parsed.scheme == "http" and (parsed.hostname or "") in ("127.0.0.1", "::1", "localhost")


def vosk_model_dir() -> Path:
    raw = os.environ.get("GALION_VOSK_MODEL", "").strip()
    if raw:
        return Path(raw)
    return REPO / "weights" / "asr" / "vosk-model-small-en-us-0.15"


def names_fact(reply: str, words: tuple[str, ...]) -> bool:
    low = reply.lower()
    return any(re.search(r"\b" + re.escape(w) + r"\b", low) for w in words)


def reply_of(payload: str) -> str:
    try:
        blob = json.loads(payload)
    except ValueError:
        return ""
    value = blob.get("reply") if isinstance(blob, dict) else None
    return value if isinstance(value, str) else ""


def llm_source(content: list[str], url: str, gq: str) -> str:
    """Who wrote the content-turn replies. Local policy lines (switch, quiet) are not content."""
    texts = [c.strip() for c in content if c.strip()]
    if not texts:
        return ""
    if any(t.lower() == UNREACHABLE_LINE for t in texts):
        return "unreachable"
    if all(t.lower() in FLOOR_LINES for t in texts):
        return "floor"
    if is_loopback_url(url):
        return "hermes"
    if gq.endswith(".gq") and Path(gq).is_file():
        return "student-gq"
    return "floor"
KILL_MS = 1500
HEAR_MS = 160
STATUSES = ("met", "absent", "failed", "stub")
REAL_ENGINES = frozenset({"chatterbox-nano", "nano"})
REQUIRED = (
    "tree-hygiene",
    "ref-refuse",
    "ref-hashes",
    "weight-pins",
    "hf-fetch",
    "piper-pcm",
    "ttfa-streaming",
    "real-sampling",
    "real-llm",
    "real-thinking",
    "real-persona",
    "full-chain",
    "persona-http",
    "live-one-dot",
    "late-pause",
)
FLOOR_LINES = (
    "your mother died this morning.",
    "the job. what did the room",
    "who were you picturing",
    "here. what do you need?",
    "the job is yours.",
    "i can't check that from here.",
)


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


def stub_armed(flag_pcm: str = "", flag_frames: str = "") -> bool:
    pcm = flag_pcm if flag_pcm != "" else os.environ.get("GALION_CLONE_STUB_PCM", "")
    frames = flag_frames if flag_frames != "" else os.environ.get("GALION_CLONE_STUB_FRAMES", "")
    return pcm == "1" or frames == "1"


def real_engine(engine: str | None) -> bool:
    return (engine or "") in REAL_ENGINES


def voice_product_claim(probe: WorkerProbe | None, armed: bool) -> bool:
    return bool(
        probe
        and not armed
        and real_engine(probe.engine)
        and probe.pcm_bytes >= 2
        and not probe.hung
    )


def classify_generate(
    probe: WorkerProbe,
    weights_met: bool,
    weights_failed: bool,
    sent_ref: bool,
    armed: bool,
) -> Gate:
    if armed or probe.engine == "protocol-stub":
        return Gate(
            "nano-generate",
            "stub",
            "GALION_CLONE_STUB_PCM or GALION_CLONE_STUB_FRAMES, or engine protocol-stub. "
            "Protocol PCM is not Nano. product_claim false. Not HARD-PASS.",
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
    if real_engine(probe.engine) and probe.pcm_bytes >= 2:
        how = "conditioned on the allowlisted reference" if sent_ref else "text only; no reference clip was sent"
        return Gate(
            "nano-generate",
            "met",
            f"engine {probe.engine}, {probe.pcm_bytes} PCM bytes at 24 kHz s16le, {how}. product_claim true for this PCM.",
        )
    if probe.engine == "model" and probe.pcm_bytes >= 2:
        return Gate(
            "nano-generate",
            "failed",
            "engine model is not chatterbox-nano or nano. product_claim false. Not HARD-PASS.",
        )
    if weights_failed or weights_met:
        return Gate(
            "nano-generate",
            "failed",
            f"weights are on disk but the worker did not generate Nano PCM (engine {probe.engine or 'none'}). Not HARD-PASS.",
        )
    return Gate(
        "nano-generate",
        "absent",
        "generate is not wired and weights are absent. No PCM. Not HARD-PASS.",
    )


def classify_ttfa(probe: WorkerProbe, weights_met: bool, weights_failed: bool, armed: bool) -> Gate:
    if armed or probe.engine == "protocol-stub":
        return Gate(
            "ttfa-streaming",
            "stub",
            "protocol stub clock discarded. product_claim false. No real first-audio number. Not HARD-PASS.",
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
    if not real_engine(probe.engine) or probe.pcm_bytes < 2 or probe.ttfa_ms is None:
        if weights_met or weights_failed or probe.pcm_bytes >= 2:
            return Gate(
                "ttfa-streaming",
                "failed",
                f"engine {probe.engine or 'none'} is not chatterbox-nano or nano. No product TTFA. Not HARD-PASS.",
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
        f"engine {probe.engine}, streaming, ttfa_ms {probe.ttfa_ms}, whole_ms {whole}.{extra}",
    )


def classify_sampling(frame_ok: bool, late_ok: bool, alloc_ok: bool, ran: bool) -> Gate:
    if not ran:
        return Gate(
            "real-sampling",
            "absent",
            "Heartbeat::step was not executed. 24 kHz, 80 ms EmotionClock, allocs, and late=pause are unmeasured. Not HARD-PASS.",
        )
    if not alloc_ok:
        return Gate(
            "real-sampling",
            "failed",
            "Heartbeat::step allocated. allocs are not 0. Not HARD-PASS.",
        )
    if not frame_ok or not late_ok:
        return Gate(
            "real-sampling",
            "failed",
            "80 ms / 24 kHz frame or late=pause did not hold. Not HARD-PASS.",
        )
    return Gate(
        "real-sampling",
        "met",
        "24 kHz, 1920 samples, 80 ms EmotionClock frames, Heartbeat::step allocs=0, late=pause under the counting-allocator load.",
    )


def classify_llm(text: str, source: str) -> Gate:
    low = text.lower()
    if any(phrase in low for phrase in FORBIDDEN):
        return Gate("real-llm", "failed", "canned empathy in the reply. Not HARD-PASS.")
    if source == "hermes":
        return Gate("real-llm", "met", "Hermes produced the Samantha/Jarvis turn. No canned empathy line.")
    if source == "student-gq":
        return Gate("real-llm", "met", "Open Mind student .gq produced the Samantha/Jarvis turn. No canned empathy line.")
    if source == "unreachable":
        return Gate("real-llm", "absent", "The model can't be reached from the persona floor. Not HARD-PASS.")
    if text.strip():
        return Gate(
            "real-llm",
            "stub",
            "Instruction floor answered from the persona table. That is not Hermes and not a student .gq. Not HARD-PASS.",
        )
    return Gate(
        "real-llm",
        "absent",
        "No Hermes reply and no student .gq reply. Not HARD-PASS.",
    )


def classify_thinking(text: str | None) -> Gate:
    if text is None:
        return Gate(
            "real-thinking",
            "absent",
            "No nerve or thought trace on disk. The Admin UI check stays unsigned. Not HARD-PASS.",
        )
    low = text.lower()
    if "interim" in low and ("placeholder" in low or "fake" in low):
        return Gate("real-thinking", "failed", "The thought trace contains a fake interim. Not HARD-PASS.")
    if '"kind"' in text or "voice." in text or "thought" in low:
        return Gate(
            "real-thinking",
            "met",
            "A nerve or thought trace is on disk for Maciej to read. This harness does not sign ADMIN-UI-CHECK.md.",
        )
    return Gate(
        "real-thinking",
        "absent",
        "The trace file has no thought or nerve event. Not HARD-PASS.",
    )


def classify_real_persona(refs_met: bool, emotion_live: bool, hub_seen: bool) -> Gate:
    missing = []
    if not refs_met:
        missing.append("GALION_CLONE_LOCAL_REFS and GALION_CLIP_STORE hashes")
    if not emotion_live:
        missing.append("galion-emotion on the live feed")
    if not hub_seen:
        missing.append("Jarvis hub texture at playback")
    if missing:
        return Gate(
            "real-persona",
            "absent",
            "Still required: " + "; ".join(missing) + ". Not HARD-PASS.",
        )
    return Gate(
        "real-persona",
        "met",
        "Reference hashes match, galion-emotion is on the live feed, and the Jarvis hub texture is in playback.",
    )


def classify_chain(links: dict[str, bool]) -> Gate:
    missing = [name for name, ok in links.items() if not ok]
    if missing:
        return Gate(
            "full-chain",
            "absent",
            "Unmeasured: " + ", ".join(missing) + ". Not HARD-PASS.",
        )
    return Gate(
        "full-chain",
        "met",
        "Vosk, instruction floor, Piper PCM, 80 ms EmotionClock, and playback were measured, with a real ttfa_ms.",
    )


def classify_hf(script_text: str) -> Gate:
    if "GALION_CLONE_WEIGHTS_SOURCE" in script_text and "release|hf" in script_text:
        return Gate(
            "hf-fetch",
            "met",
            "GALION_CLONE_WEIGHTS_SOURCE=hf remains a fetch path. This harness does not download.",
        )
    return Gate(
        "hf-fetch",
        "failed",
        "The Hugging Face fetch path is missing from fetch-clone-weights.sh. Not HARD-PASS.",
    )


def overall_verdict(gates: list[Gate]) -> str:
    by = {gate.name: gate for gate in gates}
    if any(name not in by for name in REQUIRED):
        return "HARD-FAIL"
    if all(by[name].status == "met" for name in REQUIRED):
        return "HARD-PASS"
    return "HARD-FAIL"


def missing_gates(gates: list[Gate]) -> list[str]:
    by = {gate.name: gate for gate in gates}
    missing = [name for name in REQUIRED if name not in by or by[name].status != "met"]
    return missing


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


def piper_ttfa_token(pcm_bytes: int, ttfa_ms: int | None, claim: bool) -> str:
    if claim and pcm_bytes >= 2 and ttfa_ms is not None:
        return str(ttfa_ms)
    return "absent"


def report_line(
    verdict: str,
    ttfa: str,
    allocs: str,
    late: str,
    persona: str,
    dot: str,
    claim: bool,
    missing: list[str],
) -> str:
    claim_s = "true" if claim else "false"
    gaps = ",".join(missing) if missing else "none"
    return (
        f"report-line: {verdict} ttfa_ms={ttfa} allocs={allocs} "
        f"late_frames={late} persona_http={persona} dot={dot} "
        f"product_claim={claim_s} missing={gaps}"
    )


def render(gates: list[Gate], verdict: str, line: str) -> str:
    lines = [
        "Galion Voice hard loop",
        "CPU only on this box. Piper is the real mouth. Flite, clone protocol PCM, and cloud TTS are not.",
        "HARD-PASS requires every real gate. A missing component is HARD-FAIL, named on the report line.",
        "GALION_CLONE_STUB_PCM and GALION_CLONE_STUB_FRAMES never yield HARD-PASS.",
        "A gate status of met is that gate only.",
    ]
    for gate in gates:
        lines.append(f"gate {gate.name}: {gate.status} — {gate.detail}")
    lines.append(f"overall: {verdict}")
    if verdict != "HARD-PASS":
        lines.append("HARD-PASS of the loop: refused")
    lines.append(line)
    text = "\n".join(lines) + "\n"
    assert_no_bare_pass(text)
    return text


def report_json(gates: list[Gate], verdict: str, line: str, claim: bool, ttfa_ms: int | None) -> dict:
    body: dict = {
        "product": "Galion Voice",
        "overall": verdict,
        "hard_pass": verdict == "HARD-PASS",
        "product_claim": claim,
        "engine": "piper" if claim else None,
        "report_line": line,
        "missing": missing_gates(gates),
        "gates": [{"name": g.name, "status": g.status, "detail": g.detail} for g in gates],
    }
    if claim and ttfa_ms is not None:
        body["ttfa_ms"] = ttfa_ms
        body["ttfa_kind"] = "piper-cpu"
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


def cargo_test(bin_name: str, test_filter: str) -> tuple[int, str]:
    def once(offline: bool) -> subprocess.CompletedProcess[str]:
        cmd = ["cargo", "test", "-p", "galion-heartbeat"]
        if offline:
            cmd.append("--offline")
        cmd += ["--test", bin_name, "--", test_filter, "--test-threads=1"]
        return subprocess.run(cmd, cwd=str(REPO), capture_output=True, text=True, timeout=900)

    ran = once(True)
    text = ran.stdout + "\n" + ran.stderr
    if ran.returncode != 0 and "offline" in text.lower():
        ran = once(False)
        text = ran.stdout + "\n" + ran.stderr
    return ran.returncode, text


def run_clock() -> tuple[Gate, Gate, str, str]:
    if os.environ.get("GALION_HARD_SKIP_CARGO") == "1":
        late = Gate(
            "late-pause",
            "absent",
            "GALION_HARD_SKIP_CARGO=1, so Heartbeat::step was not executed. Not HARD-PASS.",
        )
        return classify_sampling(False, False, False, False), late, "absent", "absent"
    code, text = cargo_test("heartbeat", "")
    late_names = (
        "late_frame_is_a_pause_no_extra_trigger_no_burst",
        "short_pcm_frame_is_treated_as_late_pause",
    )
    late_ok = code == 0 and "test result: ok" in text and all(name in text for name in late_names)
    frame_ok = code == 0 and "frame_constants_match_locks" in text and "test result: ok" in text
    alloc_code, alloc_text = cargo_test("zero_alloc", "step_allocates_nothing_with_rika_on")
    alloc_ok = (
        alloc_code == 0
        and "test result: ok" in alloc_text
        and "step_allocates_nothing_with_rika_on" in alloc_text
    )
    if late_ok:
        late = Gate(
            "late-pause",
            "met",
            "Heartbeat::step late frame is paused, me onset stays closed, and a short PCM frame pauses. "
            "No catch-up burst.",
        )
    else:
        tail = " ".join(text.strip().split())[-400:]
        late = Gate("late-pause", "failed", f"late=pause test did not pass. {tail} Not HARD-PASS.")
    allocs = "0" if alloc_ok else "fail"
    late_token = "proved" if late_ok else "absent"
    return classify_sampling(frame_ok, late_ok, alloc_ok, True), late, allocs, late_token


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


def persona_http(base: str) -> tuple[Gate, list[str], str, list[str]]:
    samples: list[str] = []
    try:
        status, _, health = http_exchange(base, "GET", "/health")
    except OSError as exc:
        return Gate("persona-http", "absent", f"no live server at {base} ({exc}). Not HARD-PASS."), samples, "", []
    if status != 200 or health.decode().strip() != "ok":
        return Gate("persona-http", "failed", f"/health was {status} {health!r}. Not HARD-PASS."), samples, "", []
    try:
        status, hdrs, _ = http_exchange(base, "GET", "/v2")
        state_code, _, state = http_exchange(base, "GET", "/v2/state")
    except OSError as exc:
        return Gate("persona-http", "failed", f"persona routes dropped ({exc}). Not HARD-PASS."), samples, "", []
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

    # Content turns go to the model (#23). The gate checks that the reply names
    # the fact, not that it equals a floor string.
    grief = say("My mother died this morning.")
    if not _json_has(grief, "mood", "grief") or not names_fact(reply_of(grief), MOTHER_WORDS):
        problems.append(f"Samantha grief did not name the fact (reply {reply_of(grief)!r})")
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
    if not _json_has(jarvis, "persona", "jarvis") or not reply_of(jarvis).strip():
        problems.append("Jarvis pin did not stick on the switch line")
    jarvis_grief = say("My mother died this morning.")
    if not _json_has(jarvis_grief, "persona", "jarvis") or not _json_has(jarvis_grief, "mood", "grief"):
        problems.append("pinned Jarvis left the grief fact")
    elif not names_fact(reply_of(jarvis_grief), MOTHER_WORDS):
        problems.append(f"pinned Jarvis did not name the fact (reply {reply_of(jarvis_grief)!r})")
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
    said = "\n".join((grief, joy, vuln, quiet, jarvis, jarvis_grief))
    content = [reply_of(x) for x in (grief, joy, vuln, jarvis_grief)]
    if problems:
        return Gate("persona-http", "failed", "; ".join(problems) + ". Not HARD-PASS."), samples, said, content
    return (
        Gate(
            "persona-http",
            "met",
            f"Live {base} answered Samantha and Jarvis without a canned empathy line. "
            "film_voice is false. Grief replies name the fact. This HTTP floor is not a Nano generate.",
        ),
        samples,
        said,
        content,
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


def live_one_dot(base: str, launched: bool) -> tuple[Gate, dict]:
    samples: list[str] = []
    notes = {"emotion": False, "clock": False, "speaking": False}
    deadline = time.monotonic() + 8.0
    try:
        while time.monotonic() < deadline:
            _, _, payload = http_exchange(base, "GET", "/dot")
            samples.append(payload.decode("utf-8", "replace"))
            time.sleep(0.08)
    except OSError as exc:
        return Gate("live-one-dot", "absent", f"/dot was not reachable ({exc}). Not HARD-PASS."), notes
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
    if len(times) >= 2 and (max(times) - min(times)) % 80 == 0 and max(times) > min(times):
        notes["clock"] = True
    notes["emotion"] = any('"emotion"' in sample for sample in samples)
    if not live:
        return Gate(
            "live-one-dot",
            "failed",
            "the /dot feed did not advance t_ms or engine frames. Not a live one-dot. Not HARD-PASS.",
        ), notes
    if not any('"persona":' in s for s in samples):
        return Gate("live-one-dot", "failed", "/dot has no persona field. Not HARD-PASS."), notes
    spoke = [state for mode, state in pairs if mode == "speaking"]
    other = [mode for mode in modes if mode != "speaking"]
    speak_wins = bool(spoke) and bool(other) and all(state == "speaking" for state in spoke)
    notes["speaking"] = bool(spoke) and all(state == "speaking" for state in spoke)
    if speak_wins:
        return Gate(
            "live-one-dot",
            "met",
            "Live /dot advanced with the heartbeat. Speaking was observed and the state was speaking "
            "(speak wins). A non-speaking sample was also observed.",
        ), notes
    if launched:
        return Gate(
            "live-one-dot",
            "failed",
            "synthetic demo did not show speaking winning over another mode. Not HARD-PASS.",
        ), notes
    return Gate(
        "live-one-dot",
        "absent",
        "The attached /dot feed is moving, and speaking-wins was not observed. "
        "Launch with GALION_HARD_HTTP=1 so the harness runs the synthetic room. Not HARD-PASS.",
    ), notes


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
    binary = target_dir() / "debug" / "examples" / "dot_console_demo"
    if built.returncode != 0 or not binary.is_file():
        sys.stderr.write(built.stderr[-800:] + "\n")
        return None
    log_dir = target_dir() / "hard-loop"
    log_dir.mkdir(parents=True, exist_ok=True)
    log = (log_dir / "demo.log").open("w", encoding="utf-8")
    env = demo_env(os.environ)
    return subprocess.Popen(
        [
            str(binary),
            "--headless",
            "--realtime",
            "--synthetic",
            "--no-png",
            "--feed",
            "real",
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


def demo_env(base_env) -> dict[str, str]:
    """The self-launched demo inherits GALION_LLM / GALION_LLM_URL / GALION_ASR.

    GALION_HERMES_URL is mapped onto GALION_LLM_URL when only the alias is set.
    GALION_ASR defaults to vosk when the small-en model is on disk. Nothing is
    forced to canned or stub."""
    env = dict(base_env)
    env.pop("GALION_CLONE_FETCH", None)
    if not env.get("GALION_LLM_URL", "").strip() and env.get("GALION_HERMES_URL", "").strip():
        env["GALION_LLM_URL"] = env["GALION_HERMES_URL"].strip()
    if not env.get("GALION_ASR", "").strip():
        model = Path(env.get("GALION_VOSK_MODEL", "").strip() or vosk_model_dir())
        if (model / "am" / "final.mdl").is_file():
            env["GALION_ASR"] = "vosk"
            env.setdefault("GALION_VOSK_MODEL", str(model))
    return env


def post_turn(base: str, wav: Path, log_path: Path, seconds: float = 30.0) -> bool:
    """POST one real speech WAV to /turn and wait for the demo's reply log line.

    True only when the line says asr=vosk with no error."""
    try:
        body = wav.read_bytes()
        code, _, _ = http_exchange(
            base,
            "POST",
            "/turn",
            body=body,
            headers={"Content-Type": "audio/wav", "Content-Length": str(len(body))},
        )
    except OSError:
        return False
    if code != 202:
        return False
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        text = log_path.read_text(encoding="utf-8", errors="replace") if log_path.is_file() else ""
        for line in text.splitlines():
            if line.startswith("phone reply ") and " asr=vosk " in line:
                return line.rstrip().endswith("err=-")
        time.sleep(0.2)
    return False


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


def locate_piper() -> tuple[Path, Path, Path] | None:
    runtime = Path(os.environ.get("GALION_A21_RUNTIME", str(REPO / "weights/runtime/piper/piper")))
    model_env = os.environ.get("GALION_TTS_WEIGHTS", "").strip()
    if model_env:
        model = Path(model_env)
        if model.is_dir():
            model = model / "en_US-ljspeech-medium.onnx"
    else:
        model = REPO / "weights/tts/en_US-ljspeech-medium.onnx"
    config = Path(str(model) + ".json")
    if "flite" in runtime.name.lower():
        return None
    if runtime.is_file() and os.access(runtime, os.X_OK) and model.is_file() and config.is_file():
        return runtime, model, config
    return None


def voice_sample_rate(config: Path) -> int | None:
    match = re.search(r'"sample_rate"\s*:\s*(\d+)', config.read_text(encoding="utf-8", errors="replace"))
    return int(match.group(1)) if match else None


def probe_piper(timeout_s: float = 60.0) -> dict:
    found = locate_piper()
    empty = {"found": False, "rate": None, "pcm_bytes": 0, "ttfa_ms": None, "hung": False, "flite": False}
    runtime_name = os.environ.get("GALION_A21_RUNTIME", "")
    if "flite" in Path(runtime_name).name.lower():
        empty["flite"] = True
        return empty
    if found is None:
        return empty
    runtime, model, config = found
    rate = voice_sample_rate(config)
    proc = subprocess.Popen(
        [
            str(runtime),
            "--model",
            str(model),
            "--config",
            str(config),
            "--output_raw",
            "--json-input",
            "--noise_scale",
            "0",
            "--noise_w",
            "0",
            "--length_scale",
            "1.0",
            "--sentence_silence",
            "0",
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=str(REPO),
    )
    assert proc.stdin and proc.stdout and proc.stderr
    start = time.monotonic()
    try:
        proc.stdin.write(b'{"text":"Hi."}\n')
        proc.stdin.close()
    except BrokenPipeError:
        pass
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout.fileno(), selectors.EVENT_READ, "out")
    selector.register(proc.stderr.fileno(), selectors.EVENT_READ, "err")
    pcm = bytearray()
    err = bytearray()
    first: float | None = None
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
                if first is None and chunk:
                    first = now
                pcm.extend(chunk)
            else:
                err.extend(chunk)
        if b"Real-time factor:" in err and proc.poll() is not None:
            break
    if hung or proc.poll() is None:
        proc.kill()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2)
    ttfa = int((first - start) * 1000) if first is not None else None
    return {
        "found": True,
        "rate": rate,
        "pcm_bytes": len(pcm),
        "ttfa_ms": ttfa,
        "hung": hung,
        "flite": False,
    }


def classify_piper(row: dict) -> Gate:
    if row.get("flite"):
        return Gate(
            "piper-pcm",
            "failed",
            "missing: Piper. The named runtime is flite, which is not the real voice. Not HARD-PASS.",
        )
    if not row.get("found"):
        return Gate(
            "piper-pcm",
            "failed",
            "missing: Piper CPU runtime or en_US-ljspeech-medium pack on this box. "
            "Not downloaded. Not a clone stub. Not cloud TTS. Not HARD-PASS.",
        )
    if row.get("hung") or not row.get("rate") or int(row.get("pcm_bytes") or 0) < 2:
        return Gate(
            "piper-pcm",
            "failed",
            "missing: Piper PCM at the voice sample rate. Not HARD-PASS.",
        )
    return Gate(
        "piper-pcm",
        "met",
        f"engine piper, {row['pcm_bytes']} bytes s16le at {row['rate']} Hz from the voice json. "
        "product_claim true for this PCM. CPU only.",
    )


def classify_piper_ttfa(row: dict) -> Gate:
    if not row.get("found") or int(row.get("pcm_bytes") or 0) < 2 or row.get("ttfa_ms") is None:
        return Gate(
            "ttfa-streaming",
            "failed",
            "missing: real Piper ttfa_ms. No number is claimed. Not HARD-PASS.",
        )
    return Gate(
        "ttfa-streaming",
        "met",
        f"Piper CPU first PCM at {row['ttfa_ms']} ms. engine piper. sample rate {row.get('rate')} Hz.",
    )


def collect() -> tuple[list[Gate], dict[str, str]]:
    before = locks_digest()
    pins = parse_pins(PINS.read_text(encoding="utf-8"))
    weights = evaluate_weights(weight_root(), pins)
    refs = evaluate_refs(
        REPO,
        os.environ.get("GALION_CLONE_LOCAL_REFS", ""),
        os.environ.get("GALION_CLIP_STORE", ""),
    )
    refuse = probe_worker({"text": "hi", "reference": "clip.wav"}, stub=True, timeout_s=10)
    if stub_armed():
        piper = {"ttfa_ms": None}
        voice = Gate(
            "piper-pcm",
            "failed",
            "missing: a clean Piper run. GALION_CLONE_STUB_PCM or GALION_CLONE_STUB_FRAMES is set. "
            "A stub flag cannot yield HARD-PASS.",
        )
        ttfa = Gate(
            "ttfa-streaming",
            "failed",
            "missing: real Piper ttfa_ms. A clone stub flag is set. No number is claimed.",
        )
    else:
        piper = probe_piper()
        voice = classify_piper(piper)
        ttfa = classify_piper_ttfa(piper)
    hygiene = classify_hygiene(tracked_files(), locks_digest() == before)
    gates = [
        hygiene,
        classify_refuse(refuse),
        refs,
        weights,
        classify_hf((REPO / "scripts/clone/fetch-clone-weights.sh").read_text(encoding="utf-8")),
        voice,
        ttfa,
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
    said = ""
    content: list[str] = []
    turn_ok = False
    notes = {"emotion": False, "clock": False, "speaking": False}
    hub = False
    persona_gate = Gate("persona-http", "absent", f"no live server at {base}. Not HARD-PASS.")
    dot_gate = Gate("live-one-dot", "absent", f"no live server at {base}. Not HARD-PASS.")
    if http_recorded:
        persona_gate = next(g for g in gates if g.name == "persona-http")
        dot_gate = next(g for g in gates if g.name == "live-one-dot")
        gates = [g for g in gates if g.name not in ("persona-http", "live-one-dot")]
    elif reachable:
        try:
            dot_gate, notes = live_one_dot(base, launched)
            persona_gate, _samples, said, content = persona_http(base)
            try:
                # Reply text of the probe turns (typed probe lines, not caller audio) for the report.
                said_dir = target_dir() / "hard-loop"
                said_dir.mkdir(parents=True, exist_ok=True)
                (said_dir / "said.ndjson").write_text(said + "\n", encoding="utf-8")
            except OSError:
                pass
            turn_wav = os.environ.get("GALION_HARD_TURN_WAV", "").strip()
            if turn_wav and Path(turn_wav).is_file() and launched:
                turn_ok = post_turn(base, Path(turn_wav), target_dir() / "hard-loop" / "demo.log")
            try:
                status, _, page = http_exchange(base, "GET", "/")
                if status == 200:
                    low = page.decode("utf-8", "replace").lower()
                    hub = "hub" in low and "texture" in low
            except OSError:
                hub = False
        finally:
            if proc is not None:
                proc.terminate()
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    proc.kill()
    sampling, late, allocs, late_frames = run_clock()
    log_path = target_dir() / "hard-loop" / "demo.log"
    log_text = ""
    if log_path.is_file():
        log_text = log_path.read_text(encoding="utf-8", errors="replace")
        found = None
        for token in log_text.split():
            if token.startswith("paused="):
                found = token.split("=", 1)[1]
        if found and found.isdigit():
            late_frames = found
    # galion-emotion on the live feed is read from the /dot samples, attached or launched.
    emotion_live = bool(notes["emotion"])
    model = vosk_model_dir() / "am" / "final.mdl"
    vosk_ok = model.is_file() and turn_ok and "interim" not in log_text.lower()
    source = llm_source(content, llm_url(), os.environ.get("GALION_STUDENT_GQ", "").strip())
    llm = classify_llm("\n".join(content), source)
    trace = None
    for key in ("GALION_NERVE_LOG", "GALION_THOUGHT_LOG"):
        raw = os.environ.get(key, "").strip()
        if raw and Path(raw).is_file():
            trace = Path(raw).read_text(encoding="utf-8", errors="replace")[:20000]
            break
    thinking = classify_thinking(trace)
    real_persona = classify_real_persona(refs.status == "met", emotion_live, hub)
    chain = classify_chain(
        {
            "vosk": vosk_ok,
            "instruction-floor": persona_gate.status == "met",
            "piper": voice.status == "met",
            "streaming-pcm": ttfa.status == "met",
            "emotion-clock": bool(notes["clock"]) and sampling.status == "met",
            "playback": bool(notes["speaking"]) and voice.status == "met",
            "ttfa_ms": ttfa.status == "met",
        }
    )
    gates.extend((persona_gate, dot_gate, sampling, late, llm, thinking, real_persona, chain))
    claim = voice.status == "met" and ttfa.status == "met" and not stub_armed()
    facts = {
        "allocs": allocs,
        "late_frames": late_frames,
        "persona_http": persona_gate.status,
        "dot": dot_gate.status,
        "ttfa": str(piper["ttfa_ms"]) if claim and piper.get("ttfa_ms") is not None else "absent",
        "claim": "true" if claim else "false",
    }
    if locks_digest() != before:
        gates[0] = Gate(
            "tree-hygiene",
            "failed",
            "LOCKS.toml changed during the run. This harness does not edit it. Not HARD-PASS.",
        )
    return gates, facts


def logic_self_check() -> None:
    stub = WorkerProbe(0, "protocol-stub", 402, 4, 5, False, "galion-clone-engine: protocol-stub\n", False)
    absent = WorkerProbe(2, None, 0, None, None, False, "galion-clone-absent\n", False)
    model_stream = WorkerProbe(0, "model", 48000, 90, 400, True, "galion-clone-engine: model\n", False)
    nano_pcm = WorkerProbe(0, "chatterbox-nano", 48000, 90, 400, True, "galion-clone-engine: chatterbox-nano\n", False)
    nano_block = WorkerProbe(0, "chatterbox-nano", 48000, 1800, 1800, False, "galion-clone-engine: chatterbox-nano\n", False)
    nano_fast = WorkerProbe(0, "nano", 800, 200, 210, False, "galion-clone-engine: nano\n", False)
    unwired = WorkerProbe(0, "unwired", 0, None, None, False, "galion-clone-unwired\n", False)
    refused = WorkerProbe(3, None, 0, None, None, False, "galion-clone-refuse: reference clips are on the rights hold\n", False)

    assert classify_refuse(refused).status == "met"
    assert classify_generate(stub, False, False, False, True).status == "stub"
    assert classify_generate(absent, False, False, False, False).status == "absent"
    assert classify_generate(model_stream, True, False, True, False).status == "failed"
    assert classify_generate(nano_pcm, True, False, True, False).status == "met"
    assert classify_generate(nano_pcm, True, False, True, True).status == "stub"
    assert classify_generate(unwired, True, False, False, False).status == "failed"
    assert classify_generate(refused, True, False, True, False).status == "failed"
    assert classify_ttfa(stub, False, False, False).status == "stub"
    assert "ttfa_ms" not in classify_ttfa(stub, False, False, True).detail
    assert classify_ttfa(absent, False, False, False).status == "absent"
    assert "ttfa_ms" not in classify_ttfa(absent, False, False, False).detail
    assert classify_ttfa(model_stream, True, False, False).status == "failed"
    assert classify_ttfa(nano_pcm, True, False, False).status == "met"
    assert classify_ttfa(nano_pcm, True, False, False).detail.find("90") >= 0
    assert classify_ttfa(nano_block, True, False, False).status == "failed"
    assert "whole-utterance" in classify_ttfa(nano_block, True, False, False).detail
    assert classify_ttfa(nano_fast, True, False, False).status == "failed"
    assert not voice_product_claim(stub, False)
    assert not voice_product_claim(nano_pcm, True)
    assert voice_product_claim(nano_pcm, False)
    assert piper_ttfa_token(402, 4, False) == "absent"
    assert piper_ttfa_token(4000, 90, True) == "90"
    assert piper_ttfa_token(4000, 90, False) == "absent"
    assert classify_llm("Your mother died this morning.", "floor").status == "stub"
    assert classify_llm("I hear you now.", "floor").status == "failed"
    assert classify_llm("The room stayed quiet.", "hermes").status == "met"
    assert classify_llm("The room stayed quiet.", "student-gq").status == "met"
    assert classify_llm(UNREACHABLE_LINE, "unreachable").status == "absent"
    assert classify_llm("I'm sorry to hear about your mother.", "hermes").status == "failed"
    assert FORBIDDEN == FORBIDDEN_MIRROR, "hard harness FORBIDDEN_MIRROR drifted from hermes.rs FORBIDDEN"
    assert parse_rust_forbidden('pub const FORBIDDEN: &[&str] = &[\n    "a b",\n    "c\'d",\n];') == ("a b", "c'd")
    assert names_fact("She was your mother.", MOTHER_WORDS)
    assert not names_fact("I'll stay on what you said.", MOTHER_WORDS)
    assert not names_fact("Smother it.", MOTHER_WORDS)
    assert llm_source(["Your mother died this morning.", "", "", ""], "", "") == "floor"
    assert llm_source([UNREACHABLE_LINE], "http://127.0.0.1:18080/v1/chat/completions", "") == "unreachable"
    assert llm_source(["Your mother died this morning, and you are telling me now."], "http://127.0.0.1:1/v1/chat/completions", "") == "hermes"
    assert llm_source(["Your mother died this morning."], "http://[::1]:1/v1", "") == "floor"
    assert llm_source(["The job is yours now."], "http://[::1]:1/v1", "") == "hermes"
    assert llm_source(["x"], "https://api.example.invalid/v1", "") == "floor"
    alias = demo_env({"GALION_HERMES_URL": "http://127.0.0.1:1/v1/chat/completions", "GALION_LLM": "hermes"})
    assert alias["GALION_LLM_URL"] == "http://127.0.0.1:1/v1/chat/completions"
    assert alias["GALION_LLM"] == "hermes"
    assert demo_env({"GALION_ASR": "stub"})["GALION_ASR"] == "stub"
    assert "GALION_LLM" not in demo_env({}) or demo_env({}).get("GALION_LLM") != "canned"
    old_target = os.environ.get("CARGO_TARGET_DIR")
    try:
        os.environ["CARGO_TARGET_DIR"] = "/tmp/galion-shared-target"
        assert target_dir() == Path("/tmp/galion-shared-target")
        os.environ.pop("CARGO_TARGET_DIR")
        assert target_dir() == REPO / "target"
    finally:
        if old_target is not None:
            os.environ["CARGO_TARGET_DIR"] = old_target
    assert classify_thinking(None).status == "absent"
    assert classify_thinking('{"kind":"voice.frame_miss"}').status == "met"
    assert classify_thinking("interim placeholder transcript").status == "failed"
    assert classify_real_persona(False, True, True).status == "absent"
    assert classify_real_persona(True, True, True).status == "met"
    assert classify_chain({"vosk": False, "nano": True}).status == "absent"
    assert classify_chain({"vosk": True, "nano": True}).status == "met"
    assert classify_sampling(True, True, True, True).status == "met"
    assert classify_sampling(True, True, False, True).status == "failed"
    assert classify_hf("GALION_CLONE_WEIGHTS_SOURCE\ncase release|hf").status == "met"

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

    full = [Gate(name, "met", "ok-for-test") for name in REQUIRED]
    hard_line = report_line("HARD-PASS", "90", "0", "proved", "met", "met", True, [])
    assert overall_verdict(full) == "HARD-PASS"
    assert overall_verdict(full[:-1]) == "HARD-FAIL"
    text = render(full, "HARD-PASS", hard_line)
    assert "overall: HARD-PASS" in text
    assert "refused" not in text
    assert "report-line: HARD-PASS ttfa_ms=90 allocs=0 late_frames=proved persona_http=met dot=met product_claim=true missing=none" in text
    incomplete = [
        Gate(name, "met" if name in ("tree-hygiene", "ref-refuse", "hf-fetch", "late-pause") else "absent", "x")
        for name in REQUIRED
    ]
    gaps = missing_gates(incomplete)
    fail_line = report_line("HARD-FAIL", "absent", "0", "proved", "absent", "absent", False, gaps)
    assert overall_verdict(incomplete) == "HARD-FAIL"
    assert "piper-pcm" in gaps
    fail_text = render(incomplete, "HARD-FAIL", fail_line)
    assert "overall: HARD-FAIL" in fail_text
    assert "HARD-PASS of the loop: refused" in fail_text
    assert "product_claim=false" in fail_text
    assert "ttfa_ms=absent" in fail_text
    assert "STUB-PASS" not in fail_text
    assert classify_piper({"found": False, "flite": False, "pcm_bytes": 0, "rate": None, "hung": False}).status == "failed"
    assert classify_piper({"found": True, "flite": False, "pcm_bytes": 4000, "rate": 22050, "hung": False}).status == "met"
    assert classify_piper_ttfa({"found": False, "pcm_bytes": 0, "ttfa_ms": None}).status == "failed"
    assert classify_piper_ttfa({"found": True, "pcm_bytes": 4000, "ttfa_ms": 90, "rate": 22050}).status == "met"
    blob = report_json(incomplete, "HARD-FAIL", fail_line, False, None)
    assert "ttfa_ms" not in blob
    claimed = report_json(full, "HARD-PASS", hard_line, True, 90)
    assert claimed["product_claim"] is True
    assert claimed["engine"] == "piper"
    assert claimed["ttfa_ms"] == 90
    assert claimed["ttfa_kind"] == "piper-cpu"
    assert exit_code("HARD-FAIL", False) == 1
    assert exit_code("HARD-PASS", True) == 0


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
        print("classifiers held. This is not a product verdict.")
        print("HARD-PASS of the loop: refused")
        return 0
    logic_self_check()
    gates, facts = collect()
    verdict = overall_verdict(gates)
    claim = facts["claim"] == "true"
    line = report_line(
        verdict,
        facts["ttfa"] if claim else "absent",
        facts["allocs"],
        facts["late_frames"],
        facts["persona_http"],
        facts["dot"],
        claim,
        missing_gates(gates),
    )
    text = render(gates, verdict, line)
    sys.stdout.write(text)
    ttfa_ms = int(facts["ttfa"]) if claim and facts["ttfa"].isdigit() else None
    blob = report_json(gates, verdict, line, claim, ttfa_ms)
    sys.stdout.write("report: " + json.dumps(blob, separators=(",", ":")) + "\n")
    out_dir = target_dir() / "hard-loop"
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
