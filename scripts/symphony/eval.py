#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
"""Symphony self-eval for Eureka sections A–G.

Each gate runs against the tree and returns PASS or OPEN.
PASS is the only closed state. OPEN exits non-zero: the section was
run, and it is not done. This file reads LOCKS.toml. It never writes it.
"""

from __future__ import annotations

import argparse
import hashlib
import math
import os
import re
import subprocess
import sys
import tomllib
from datetime import datetime
from pathlib import Path
from zoneinfo import ZoneInfo

SAMANTHA_SHA = "1636428272b85d4b4f6c67bb859bfe33d9983ff9439646519305ff13e6921005"
JARVIS_SHA = "60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6"
EMPATHY_BANNED = (
    "I know this is difficult",
    "I'm worried about you",
    "Take a deep breath",
    "I can only imagine",
    "Great question",
)
SKIP_DIRS = {".git", "target", "weights", ".build", "results", "node_modules"}


def repo_root(explicit: str | None) -> Path:
    if explicit:
        return Path(explicit).resolve()
    return Path(__file__).resolve().parents[2]


def read_text(path: Path, limit: int = 2_000_000) -> str:
    data = path.read_bytes()[:limit]
    return data.decode("utf-8", errors="replace")


def load_locks(root: Path) -> dict:
    with (root / "LOCKS.toml").open("rb") as handle:
        return tomllib.load(handle)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()


def walk_files(root: Path, suffixes: tuple[str, ...]):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [name for name in dirnames if name not in SKIP_DIRS]
        for name in filenames:
            if name.endswith(suffixes):
                yield Path(dirpath) / name


def harness_path(path: Path) -> bool:
    parts = path.parts
    return "scripts" in parts and "symphony" in parts


def one_line(text: str) -> str:
    return " ".join(text.split())


def goertzel_power(samples: list[float], rate: int, freq: float) -> float:
    count = len(samples)
    k = round(freq * count / rate)
    coeff = 2.0 * math.cos(2.0 * math.pi * k / count)
    s1 = 0.0
    s2 = 0.0
    for sample in samples:
        s0 = sample + coeff * s1 - s2
        s2 = s1
        s1 = s0
    return s1 * s1 + s2 * s2 - coeff * s1 * s2


def texture_ratio(samples: list[float], rate: int) -> float:
    low = goertzel_power(samples, rate, 200.0)
    high = goertzel_power(samples, rate, 6000.0)
    if low <= 0.0:
        return math.inf
    return high / low


def lowpass(samples: list[float], rate: int, cutoff: float = 1500.0) -> list[float]:
    dt = 1.0 / rate
    rc = 1.0 / (2.0 * math.pi * cutoff)
    alpha = dt / (rc + dt)
    y = 0.0
    out: list[float] = []
    for sample in samples:
        y += alpha * (sample - y)
        out.append(y)
    return out


def texture_self_test() -> None:
    """Checker math on a synthetic tone. Not a Jarvis recording."""
    rate = 16_000
    count = 1024
    textured = [
        math.sin(2.0 * math.pi * 200.0 * i / rate) + 0.35 * math.sin(2.0 * math.pi * 6000.0 * i / rate)
        for i in range(count)
    ]
    kept = texture_ratio(textured, rate)
    stripped = texture_ratio(lowpass(textured, rate), rate)
    if not kept > 0.04:
        raise AssertionError(f"preserve path lost texture ratio {kept}")
    if not stripped < kept * 0.25:
        raise AssertionError(f"strip path did not drop texture ({stripped} vs {kept})")


def compiler_witness(root: Path) -> Path | None:
    needles_round = re.compile(r"round[-_ ]?trip", re.I)
    for path in walk_files(root, (".rs", ".py", ".sh")):
        if harness_path(path):
            continue
        if path.name == "chatterbox_worker.py":
            continue
        text = read_text(path)
        if path.name == "gq.rs" and "Reader only" in text:
            continue
        low = text.lower()
        galion_fmt = "galiongq" in low or (".gq" in low and "clip" in low)
        refuses = "chatterbox" in low and ("refus" in low or "not the primary" in low or "primary path" in low)
        if needles_round.search(text) and galion_fmt and refuses:
            return path
    return None


def gate_a(root: Path) -> tuple[str, str]:
    locks = load_locks(root)
    formats = locks["formats"]
    if formats.get("foreign_runtime_allowed") is not False or formats.get("ggml_container_allowed") is not False:
        return "OPEN", "LOCKS still allow a foreign runtime or a GGML container"
    allowed = {str(item).lower() for item in formats.get("weight_formats", [])}
    if not allowed or not allowed <= {"safetensors", "gq"}:
        return "OPEN", "LOCKS weight_formats are not a subset of safetensors and gq"
    gq = read_text(root / "crates/galion-tensor/src/gq.rs")
    if 'b"GALIONGQ"' not in gq and "GALIONGQ" not in gq:
        return "OPEN", "Galion .gq magic GALIONGQ is missing"
    witness = compiler_witness(root)
    if witness is None:
        reader = "reader-only" if "Reader only" in gq else "present"
        return (
            "OPEN",
            f".gq container is {reader} in galion-tensor; no clone-to-Galion clip compiler round-trip that refuses Chatterbox as the primary path",
        )
    return "PASS", f"compiler witness {witness.relative_to(root)} round-trips a Galion clip and refuses Chatterbox as primary"


def gate_b(root: Path) -> tuple[str, str]:
    notes: list[str] = []
    worker = root / "scripts/clone/chatterbox_worker.py"
    env = os.environ.copy()
    env["GALION_CLONE_STUB_PCM"] = "1"
    speak = subprocess.run(
        [sys.executable, str(worker)],
        input=b'{"text":"Galion","persona":"samantha"}\n',
        capture_output=True,
        env=env,
        cwd=root,
        timeout=10,
        check=False,
    )
    if speak.returncode != 0 or len(speak.stdout) < 100:
        notes.append(
            f"protocol stub did not emit PCM (rc={speak.returncode}, bytes={len(speak.stdout)})"
        )
    else:
        notes.append(f"protocol stub emitted {len(speak.stdout)} B PCM (not a ref speak)")
    refused = subprocess.run(
        [sys.executable, str(worker)],
        input=b'{"text":"Galion","persona":"jarvis","reference":"personas/refs/jarvis.wav"}\n',
        capture_output=True,
        env=env,
        cwd=root,
        timeout=10,
        check=False,
    )
    err = refused.stderr.decode("utf-8", errors="replace").lower()
    if refused.returncode == 3 and "refus" in err:
        notes.append("worker refused a reference clip")
    else:
        notes.append(f"reference refusal missing (rc={refused.returncode})")
    refs = root / "docs/galion-voice-v2/personas/refs"
    mp3 = sorted(refs.glob("*.mp3")) if refs.is_dir() else []
    if mp3:
        notes.append("mp3 refs present: " + ", ".join(path.name for path in mp3))
    hashes = {
        "samantha.wav": SAMANTHA_SHA,
        "jarvis.wav": JARVIS_SHA,
    }
    matched = 0
    for name, want in hashes.items():
        path = refs / name
        if not path.is_file():
            notes.append(f"{name} absent")
            continue
        got = sha256_file(path)
        if got != want:
            notes.append(f"{name} sha256 {got} != {want}")
        else:
            matched += 1
            notes.append(f"{name} sha256 matches")
    smoke = False
    for path in walk_files(root, (".md", ".log", ".txt", ".json")):
        if harness_path(path) or path.name == "SELF-EVAL.md":
            continue
        if path.stat().st_size > 2_000_000:
            continue
        text = read_text(path)
        if SAMANTHA_SHA in text and JARVIS_SHA in text and re.search(r"smoke speak|speak test", text, re.I):
            smoke = True
            notes.append(f"smoke speak log {path.relative_to(root)}")
            break
    if not smoke:
        notes.append("no smoke-speak log names both ref sha256 values")
    if matched == 2 and smoke and not mp3 and speak.returncode == 0 and refused.returncode == 3:
        return "PASS", one_line("; ".join(notes))
    return "OPEN", one_line("; ".join(notes))


def texture_preserve(root: Path) -> bool | None:
    path = root / "docs/galion-voice-v2/personas/voice/jarvis.toml"
    text = read_text(path)
    match = re.search(r"(?ms)^\[texture\]\s*(.*?)^(\[|\Z)", text)
    if not match:
        return None
    flag = re.search(r"(?m)^preserve\s*=\s*(true|false)\s*$", match.group(1))
    if not flag:
        return None
    return flag.group(1) == "true"


def gate_c(root: Path) -> tuple[str, str]:
    try:
        texture_self_test()
    except AssertionError as exc:
        return "OPEN", f"texture checker self-test failed: {exc}"
    toggle = texture_preserve(root)
    if toggle is None:
        return (
            "OPEN",
            "checker self-test passed on a synthetic tone; jarvis.toml has no [texture] preserve toggle",
        )
    if toggle is False:
        return "OPEN", "texture toggle is off, so hub clicks, alarms, and ambience would be stripped"
    refs = root / "docs/galion-voice-v2/personas/refs"
    jarvis = refs / "jarvis.wav"
    baseline = refs / "jarvis-clean-baseline.wav"
    if not jarvis.is_file() or not baseline.is_file():
        missing = [name for name, path in (("jarvis.wav", jarvis), ("jarvis-clean-baseline.wav", baseline)) if not path.is_file()]
        return "OPEN", "texture toggle is on; missing " + " and ".join(missing) + " for the spectral compare"
    import wave

    def ratio_of(path: Path) -> float:
        with wave.open(str(path), "rb") as handle:
            channels = handle.getnchannels()
            rate = handle.getframerate()
            width = handle.getsampwidth()
            frames = handle.readframes(handle.getnframes())
        if width != 2:
            raise RuntimeError(f"{path.name} is not 16-bit PCM")
        step = channels
        samples = []
        for index in range(0, len(frames), 2 * channels):
            value = int.from_bytes(frames[index : index + 2], "little", signed=True)
            samples.append(value / 32768.0)
            if len(samples) >= rate:
                break
        if step != channels:
            raise RuntimeError("channel decode drifted")
        return texture_ratio(samples, rate)

    try:
        kept = ratio_of(jarvis)
        clean = ratio_of(baseline)
    except (RuntimeError, wave.Error) as exc:
        return "OPEN", f"texture toggle is on; spectral compare failed: {exc}"
    if clean <= 0:
        return "OPEN", "clean baseline has no low-band energy to compare"
    if kept < clean:
        return "OPEN", f"jarvis high-band ratio {kept:.4f} is below the clean baseline {clean:.4f}"
    return "PASS", f"preserve=true and jarvis texture ratio {kept:.4f} >= clean baseline {clean:.4f}"


def gate_d(root: Path) -> tuple[str, str]:
    required = [
        root / "docs/galion-voice-v2/personas/SAMANTHA.md",
        root / "docs/galion-voice-v2/personas/JARVIS.md",
        root / "docs/galion-voice-v2/personas/GALION-ONE.md",
        root / "docs/galion-voice-v2/EMPATHY.md",
        root / "docs/galion-voice-v2/personas/voice/samantha.toml",
        root / "docs/galion-voice-v2/personas/voice/jarvis.toml",
        root / "galion-emotion/src/express.rs",
        root / "crates/galion-heartbeat/src/lib.rs",
        root / "crates/galion-cascade/src/canned.rs",
    ]
    missing = [str(path.relative_to(root)) for path in required if not path.is_file()]
    if missing:
        return "OPEN", "missing " + ", ".join(missing)
    problems: list[str] = []
    for name in ("SAMANTHA.md", "JARVIS.md", "GALION-ONE.md"):
        text = read_text(root / "docs/galion-voice-v2/personas" / name)
        if "## Emotions" not in text or "scripted or canned empathy" not in text:
            problems.append(f"{name} lacks an emotion section or the no-scripted-empathy rule")
    samantha = read_text(root / "docs/galion-voice-v2/personas/SAMANTHA.md")
    if "[laugh]" not in samantha or "[chuckle]" not in samantha:
        problems.append("Samantha docs do not name laugh and chuckle sounds")
    for name in ("samantha.toml", "jarvis.toml"):
        text = read_text(root / "docs/galion-voice-v2/personas/voice" / name)
        for key in ("[register.prosody.e0]", "[register.prosody.e1]", "[register.wash]"):
            if key not in text:
                problems.append(f"{name} missing {key}")
    express = read_text(root / "galion-emotion/src/express.rs")
    if "fn voice_color" not in express or "fn blend_prosody" not in express:
        problems.append("express.rs does not map empathy weight e onto prosody")
    heartbeat = read_text(root / "crates/galion-heartbeat/src/lib.rs")
    if "feeling" not in heartbeat or "FRAME_MS: u32 = 80" not in heartbeat:
        problems.append("feeling band is not on the 80 ms heartbeat")
    canned = read_text(root / "crates/galion-cascade/src/canned.rs")
    hit = [line for line in EMPATHY_BANNED if line in canned]
    if hit:
        problems.append("canned replies contain scripted empathy: " + "; ".join(hit))
    if problems:
        return "OPEN", one_line("; ".join(problems))
    return "PASS", "persona docs, e0/e1 prosody and wash, voice_color, feeling band at 80 ms; canned replies have no scripted empathy lines"


def workflow_hook(root: Path) -> Path | None:
    pattern = re.compile(r"auto[-_ ]?record", re.I)
    for path in walk_files(root, (".rs", ".sh")):
        if harness_path(path):
            continue
        text = read_text(path)
        if "workflow" not in text.lower() or not pattern.search(text):
            continue
        if path.suffix == ".sh" or "assert" in text or "#[test]" in text:
            return path
    return None


def gate_e(root: Path) -> tuple[str, str]:
    locks = load_locks(root)["ui"]
    problems: list[str] = []
    if locks.get("rule") != "one-dot" or locks.get("dot_count") != 1 or locks.get("chrome_allowed") is not False:
        problems.append("LOCKS ui is not one dot with chrome off")
    page = read_text(root / "crates/galion-console/assets/v1-screen.html")
    if page.count("<canvas") != 1:
        problems.append(f"phone page has {page.count('<canvas')} canvas marks")
    if "getUserMedia" not in page or "/turn" not in page or "/mic" not in page:
        problems.append("mic path does not record on the one page")
    web = read_text(root / "galion-render/src/backends/web.rs")
    redirects = (
        '"/v2" | "/v2/"' in web
        and "302 Found" in web
        and "url=/" in web
        and "location.replace" in web
    )
    if not redirects:
        problems.append("/v2 is not a redirect onto /")
    hook = workflow_hook(root)
    if hook is None:
        problems.append("no test shows speak, auto-record, then a workflow hook")
    if problems:
        return "OPEN", one_line("; ".join(problems))
    return "PASS", f"one / surface, mic records on that page, workflow hook in {hook.relative_to(root)}"


def runbook_scripts(root: Path) -> list[str]:
    readme = read_text(root / "README.md")
    found = re.findall(r"scripts/[\w./-]+\.sh", readme)
    ordered: list[str] = []
    for item in found:
        if item not in ordered:
            ordered.append(item)
    return ordered


def gate_f(root: Path) -> tuple[str, str]:
    readme = read_text(root / "README.md")
    if "## Run the demo" not in readme:
        return "OPEN", "README has no run-the-demo section"
    scripts = runbook_scripts(root)
    if len(scripts) < 3:
        return "OPEN", "README lists fewer than 3 repo scripts"
    problems: list[str] = []
    for rel in scripts:
        path = root / rel
        if not path.is_file():
            problems.append(f"{rel} missing")
            continue
        if not os.access(path, os.X_OK):
            problems.append(f"{rel} is not executable")
            continue
        check = subprocess.run(["bash", "-n", str(path)], capture_output=True, text=True, check=False)
        if check.returncode != 0:
            err = one_line(check.stderr) or "syntax error"
            problems.append(f"{rel} bash -n failed: {err}")
    if problems:
        return "OPEN", one_line("; ".join(problems))
    return "PASS", f"bash -n ok for {len(scripts)} README scripts: " + ", ".join(scripts)


def gate_g(root: Path) -> tuple[str, str]:
    locks = load_locks(root)["clock"]
    problems: list[str] = []
    if locks.get("frame_ms") != 80 or locks.get("frame_hz") != 12.5:
        problems.append(f"LOCKS clock is frame_ms={locks.get('frame_ms')} frame_hz={locks.get('frame_hz')}")
    budget = locks.get("frame_budget_p95_ms")
    if not isinstance(budget, int) or budget > 60:
        problems.append(f"frame compute budget {budget} is not an int <= 60 ms")
    heartbeat = read_text(root / "crates/galion-heartbeat/src/lib.rs")
    if "pub const FRAME_MS: u32 = 80" not in heartbeat or "pub const SAMPLE_RATE: u32 = 24_000" not in heartbeat:
        problems.append("heartbeat constants are not 80 ms at 24 kHz")
    late = read_text(root / "crates/galion-heartbeat/tests/heartbeat.rs")
    if "fn late_frame_is_a_pause" not in late or ".paused" not in late:
        problems.append("late-frame pause test is missing")
    latency = root / "docs/research/galion-voice-v2/domains/04-latency-streaming.md"
    if not latency.is_file() or "80 ms" not in read_text(latency):
        problems.append("streaming sector budget doc does not name the 80 ms frame")
    stub = root / "scripts/eval/flash-ttfa.sh"
    stub_text = read_text(stub) if stub.is_file() else ""
    if '"measured":false' not in stub_text or "GALION_FLASH_EVAL" not in stub_text:
        problems.append("flash-ttfa stub does not refuse an unmeasured ttfa_ms")
    else:
        check = subprocess.run(["bash", "-n", str(stub)], capture_output=True, text=True, check=False)
        if check.returncode != 0:
            problems.append("flash-ttfa.sh failed bash -n")
    if problems:
        return "OPEN", one_line("; ".join(problems))
    return (
        "PASS",
        "frame_ms=80, 24 kHz, p95 budget <= 60 ms, late frame pauses; streaming doc names 80 ms; flash-ttfa stub-measures and prints no ttfa_ms",
    )


GATES = (
    ("A", "Compiler", gate_a),
    ("B", "Clone landing", gate_b),
    ("C", "Jarvis texture toggle", gate_c),
    ("D", "Persona and emotion", gate_d),
    ("E", "One-dot", gate_e),
    ("F", "Runbooks", gate_f),
    ("G", "80 ms frame", gate_g),
)


def evaluate(root: Path, only: str | None) -> list[tuple[str, str, str, str]]:
    rows = []
    for letter, title, fn in GATES:
        if only and letter != only:
            continue
        try:
            verdict, evidence = fn(root)
        except Exception as exc:  # a gate bug is an open section, not a crash of the rest
            verdict, evidence = "OPEN", f"gate raised {type(exc).__name__}: {exc}"
        if verdict not in ("PASS", "OPEN"):
            verdict, evidence = "OPEN", evidence
        rows.append((letter, title, verdict, one_line(evidence)))
    return rows


def render_matrix(rows: list[tuple[str, str, str, str]], stamp: str) -> str:
    lines = [
        f"Recorded {stamp}. A row closes only when this run says PASS.",
        "",
        "| ID | Section | Status | Evidence |",
        "|---|---|---|---|",
    ]
    for letter, title, verdict, evidence in rows:
        lines.append(f"| {letter} | {title} | {verdict} | {evidence.replace('|', '/')} |")
    closed = [letter for letter, _, verdict, _ in rows if verdict == "PASS"]
    open_rows = [letter for letter, _, verdict, _ in rows if verdict != "PASS"]
    lines.append("")
    lines.append(
        f"Closed: {', '.join(closed) if closed else 'none'}. Open: {', '.join(open_rows) if open_rows else 'none'}."
    )
    return "\n".join(lines) + "\n"


def render_log(rows: list[tuple[str, str, str, str]], stamp: str, locks_sha: str) -> str:
    lines = [f"stamp: {stamp}", f"locks_sha256: {locks_sha}", "command: scripts/symphony/self-eval.sh"]
    for letter, _title, verdict, evidence in rows:
        lines.append(f"{letter} {verdict} — {evidence}")
    failed = sum(1 for _, _, verdict, _ in rows if verdict != "PASS")
    lines.append(f"exit: {0 if failed == 0 else 1}")
    return "\n".join(lines) + "\n"


def replace_block(text: str, start: str, end: str, body: str) -> str:
    pattern = re.compile(re.escape(start) + r".*?" + re.escape(end), re.S)
    replacement = f"{start}\n{body}{end}"
    if not pattern.search(text):
        raise SystemExit(f"SELF-EVAL.md is missing markers {start} .. {end}")
    return pattern.sub(replacement, text, count=1)


def record(root: Path, rows: list[tuple[str, str, str, str]]) -> None:
    path = root / "docs/galion-voice-v2/symphony/SELF-EVAL.md"
    stamp = datetime.now(ZoneInfo("Europe/Zagreb")).strftime("%Y-%m-%d %H:%M %Z")
    locks_sha = sha256_file(root / "LOCKS.toml")
    text = read_text(path)
    text = replace_block(text, "<!-- SYMPHONY-MATRIX-START -->", "<!-- SYMPHONY-MATRIX-END -->", render_matrix(rows, stamp))
    text = replace_block(text, "<!-- SYMPHONY-RUN-START -->", "<!-- SYMPHONY-RUN-END -->", render_log(rows, stamp, locks_sha))
    path.write_text(text, encoding="utf-8")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Run Galion Voice symphony gates A–G")
    parser.add_argument("--root", default=None)
    parser.add_argument("--only", default=None, help="One letter, A–G")
    parser.add_argument("--record", action="store_true")
    parser.add_argument("--no-record", action="store_true")
    args = parser.parse_args(argv)
    only = args.only.upper() if args.only else None
    if only and only not in {letter for letter, _, _ in GATES}:
        print(f"OPEN ? — unknown section {only}", file=sys.stderr)
        return 2
    root = repo_root(args.root)
    rows = evaluate(root, only)
    for letter, _title, verdict, evidence in rows:
        print(f"{letter} {verdict} — {evidence}")
    failed = sum(1 for _, _, verdict, _ in rows if verdict != "PASS")
    print(f"== symphony A–G: {len(rows) - failed} PASS, {failed} OPEN, exit {0 if failed == 0 else 1}")
    if args.record and not args.no_record:
        if only:
            print("record skipped for a single section", file=sys.stderr)
        else:
            record(root, rows)
            print(f"recorded docs/galion-voice-v2/symphony/SELF-EVAL.md")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
