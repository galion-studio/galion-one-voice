#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# t301 · live Piper CPU stream TTFA (see scripts/eval/piper-stream-ttfa.sh).
#
# Every number below is a wall clock around a real Piper child process. There
# is no stub clock and no fixed sleep standing in for audio. When the A21 pack
# or the runtime is missing, the JSON says measured:false, carries no ms field,
# and the exit code is 2.
#
# Parts:
#   speechstack   cold/warm first-clause PCM from scripts/bench-piper-warmth.sh
#                 (the in-tree PiperWorker, GALION_SPEECH=a21)
#   raw_spawn     fresh `piper --output_raw --json-input`, spawn -> first PCM
#                 byte (cold), then a second line on the same child (warm)
#   drain         rough demo: the raw_spawn PCM, resampled 22050 -> 24000,
#                 cut into 1920-sample (80 ms) frames, drained on an 80 ms tick
#                 that starts at the first PCM byte. A tick with no full frame
#                 that has arrived is `late` -> pause (silence, no catch-up
#                 burst). Arrival times are the real byte arrival times.
#
# Usage: python3 scripts/eval/piper_stream_ttfa.py [--no-speechstack] [--wav OUT.wav]
# The WAV (if asked) is for listening only; never commit it.
import argparse, hashlib, json, os, re, struct, subprocess, sys, threading, time, wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VOICE = "en_US-ljspeech-medium"
COLD_TEXT = "Streaming check. Galion Voice on Piper CPU."
WARM_TEXT = "Hi! This clause is the warm path check."
OUT_RATE, FRAME = 24000, 1920  # 80 ms at 24 kHz


def pinned():
    src = (ROOT / "crates/galion-ml-voice/src/speech/weights.rs").read_text()
    return dict(re.findall(r'\("([^"]+)",\s*"([0-9a-f]{64})"\)', src))


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def absent(reason, **kw):
    print(json.dumps({"engine": "piper", "voice": VOICE, "measured": False, "stub": False,
                      "product_claim": False, "pack": "absent", "reason": reason, **kw}))
    sys.exit(2)


def raw_spawn(runtime, model, config):
    """Cold: spawn -> first stdout byte. Warm: second JSON line on the same child."""
    arrivals = []  # (t_since_spawn_s, nbytes) for the cold utterance
    t0 = time.perf_counter()
    p = subprocess.Popen([str(runtime), "--model", str(model), "--config", str(config),
                          "--output_raw", "--json-input", "--sentence_silence", "0"],
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    done = threading.Event()
    lines = []

    def err_reader():
        for ln in p.stderr:
            ln = ln.decode(errors="replace")
            lines.append(ln)
            if "Real-time factor" in ln:
                done.set()
    threading.Thread(target=err_reader, daemon=True).start()

    def utter(text, log):
        done.clear()
        start = time.perf_counter()
        p.stdin.write((json.dumps({"text": text}) + "\n").encode())
        p.stdin.flush()
        first, total = None, 0
        fd = p.stdout.fileno()
        os.set_blocking(fd, False)
        while True:
            try:
                b = os.read(fd, 65536)
            except BlockingIOError:
                b = None
            if b:
                now = time.perf_counter()
                if first is None:
                    first = now
                total += len(b)
                if log is not None:
                    log.append((now - t0, b))
                continue
            if b == b"":
                break
            if done.is_set():
                # Piper prints RTF after the last write; one final drain pass.
                time.sleep(0.005)
                try:
                    while (b := os.read(fd, 65536)):
                        total += len(b)
                        if log is not None:
                            log.append((time.perf_counter() - t0, b))
                except BlockingIOError:
                    pass
                break
            time.sleep(0.0005)
        return start, first, total

    _, cold_first, cold_bytes = utter(COLD_TEXT, arrivals)
    w_start, w_first, w_bytes = utter(WARM_TEXT, None)
    p.stdin.close()
    p.wait(timeout=10)
    if cold_first is None or w_first is None:
        raise RuntimeError("piper produced no PCM: " + "".join(lines[-5:]))
    return {
        "cold_spawn_first_pcm_byte_ms": round((cold_first - t0) * 1000, 1),
        "warm_line_first_pcm_byte_ms": round((w_first - w_start) * 1000, 1),
        "cold_pcm_bytes": cold_bytes,
        "warm_pcm_bytes": w_bytes,
        "clock": "spawn (cold) / stdin write (warm) until first stdout byte",
    }, arrivals


def drain(arrivals, in_rate):
    """80 ms tick over real arrival times; late tick = pause, never a burst."""
    pcm = b"".join(b for _, b in arrivals)
    n = len(pcm) // 2
    src = struct.unpack("<%dh" % n, pcm[: n * 2])
    # sample index -> arrival time (s since spawn)
    avail = []  # (t, cumulative input samples)
    cum = 0
    for t, b in arrivals:
        cum += len(b) // 2
        avail.append((t, cum))
    ratio = in_rate / OUT_RATE
    out_n = int(n / ratio)
    out = [0] * out_n
    for i in range(out_n):  # linear resample, demo quality
        x = i * ratio
        j = int(x)
        f = x - j
        a = src[j]
        b = src[j + 1] if j + 1 < n else a
        out[i] = int(a + (b - a) * f)
    frames = out_n // FRAME + (1 if out_n % FRAME else 0)

    def ready_at(k):  # time when input needed for output frame k has arrived
        need = min(n, int(((k + 1) * FRAME) * ratio) + 1)
        for t, c in avail:
            if c >= need:
                return t
        return avail[-1][0]
    t_start = avail[0][0]
    tick, played, late, played_ticks = 0.080, 0, 0, []
    t = t_start
    while played < frames:
        if ready_at(played) <= t:
            played += 1
            played_ticks.append("p")
        else:
            late += 1
            played_ticks.append("_")
        t += tick
        if len(played_ticks) > 10000:
            break
    tail = out_n % FRAME
    if tail:
        out.extend([0] * (FRAME - tail))
    return {
        "in_rate": in_rate, "out_rate": OUT_RATE, "frame_samples": FRAME, "tick_ms": 80,
        "frames": frames, "late_ticks": late, "late_policy": "pause",
        "audio_ms": round(out_n / OUT_RATE * 1000, 1),
        "ticks": "".join(played_ticks),
    }, out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--no-speechstack", action="store_true")
    ap.add_argument("--wav", help="write drained 24 kHz PCM here (do not commit)")
    a = ap.parse_args()

    model = ROOT / "weights/tts" / f"{VOICE}.onnx"
    config = ROOT / "weights/tts" / f"{VOICE}.onnx.json"
    runtime = Path(os.environ.get("GALION_A21_RUNTIME") or ROOT / "weights/runtime/piper/piper")
    if not model.is_file() or not config.is_file():
        absent("A21 pack missing (run scripts/a21/fetch-a21.sh)")
    if not os.access(runtime, os.X_OK):
        absent(f"Piper runtime missing at {runtime}")
    pins = pinned()
    for f in (model, config):
        if pins.get(f.name) != sha256(f):
            absent(f"sha256 mismatch vs PINNED for {f.name}")
    in_rate = json.loads(config.read_text())["audio"]["sample_rate"]

    res = {"engine": "piper", "voice": VOICE, "pack": "present", "sha256": "pinned",
           "runtime": str(runtime.relative_to(ROOT)) if runtime.is_relative_to(ROOT) else str(runtime),
           "cpu_only": True, "measured": True, "stub": False, "product_claim": True}

    if not a.no_speechstack:
        env = dict(os.environ, GALION_SPEECH="a21", GALION_A21_RUNTIME=str(runtime))
        r = subprocess.run(["bash", str(ROOT / "scripts/bench-piper-warmth.sh")], cwd=ROOT,
                           env=env, capture_output=True, text=True)
        line = next((l for l in r.stdout.splitlines() if l.startswith("{")), None)
        ss = json.loads(line) if line else {"measured": False, "error": r.stderr[-400:]}
        res["speechstack"] = ss
        if ss.get("measured") is True:
            res["cold_first_pcm_ms"] = ss.get("cold_first_pcm_ms")
            res["warm_first_pcm_ms"] = ss.get("warm_first_pcm_ms")

    raw, arrivals = raw_spawn(runtime, model, config)
    res["raw_spawn"] = raw
    res["cold_spawn_first_pcm_byte_ms"] = raw["cold_spawn_first_pcm_byte_ms"]
    if "warm_first_pcm_ms" not in res:
        # SpeechStack skipped: the raw warm line is the warm number.
        res["warm_first_pcm_ms"] = raw["warm_line_first_pcm_byte_ms"]
        res["warm_source"] = "raw_spawn"
    else:
        res["warm_source"] = "speechstack"
    d, out = drain(arrivals, in_rate)
    res["drain"] = d
    res["phrase_cold"], res["phrase_warm"] = COLD_TEXT, WARM_TEXT
    if a.wav:
        with wave.open(a.wav, "wb") as w:
            w.setnchannels(1); w.setsampwidth(2); w.setframerate(OUT_RATE)
            w.writeframes(struct.pack("<%dh" % len(out), *out))
        res["wav"] = a.wav
    if res.get("warm_first_pcm_ms") is None:
        res["measured"] = False
        res["product_claim"] = False
    print(json.dumps(res))
    sys.exit(0 if res["measured"] else 1)


if __name__ == "__main__":
    main()
