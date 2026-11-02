#!/usr/bin/env python3
# Galion Voice — canned demo. Perpetual Open MIT License.
# Pure Python stdlib. No model, no downloads.
"""
Offline wav->wav canned demo for Galion Voice.

Modes:
  gen       write a synthetic placeholder input WAV (3 utterances, a breath-in-silence, click track)
  run       wav->wav: read input, run the pipeline, write output WAV + console capture (+ KD targets)
  all       gen + run (default)
  selftest  policy self-checks (counterbalance table, consent/purpose gating); exit 1 on failure

Pipeline (dual-band, full-duplex shape):
  band A (user in)  -> 20 ms frames: RMS + zero-crossing pitch proxy -> caller VAD frame (stub, conf 0.3)
  consent           -> SENSE+ADAPT and an allowed purpose, else NO reading: Galion stays neutral-warm
  policy (A14/F)    -> COUNTERBALANCE, never mirror: port of galion-emotion StubExpressor::respond
                       (angry/anxious -> calmer, sad/tired -> gentle, joy/excited -> shared slightly below)
  prosody           -> port of StubExpressor::plan: pitch shift + energy from Galion's TARGET, not the caller
  Rika opener       -> optional (--rika-opener): port of galion-emotion rika.rs state machine on 80 ms
                       heartbeat frames; wordless soft hum ("mm") only. OFF by default; hard OFF in
                       workplace/education (AI Act Art. 5(1)(f)); no consent -> never reads the user band.
  band B (agent out)-> RIGHT channel; user input LEFT channel (stereo = both bands)
  KD exporter       -> every demo spec the canned path fails is written as one JSONL distillation target.
"""
import math, os, struct, sys, wave, random, argparse, time, json

SR = 16000
FRAME = 320            # 20 ms analysis frame
HB = 4                 # heartbeat = 4 analysis frames = 80 ms (LOCK)
SENSE, ADAPT = 0x01, 0x02
PURPOSES = ("personal", "accessibility", "workplace", "education")
PROHIBITED = ("workplace", "education")
NEUTRAL_WARM = (0.3, 0.35, 0.5)
LOW_AROUSAL_HEADROOM = 0.15
STUB_CONF = 0.3        # EnergyStubDetector confidence
SPEECH_REPLY = 0.25    # canned reply peak level
HUM_LEVEL = 0.05       # soft hum peak: 20*log10(0.25/0.05) = 14 dB below reply (spec >= 12 dB)
SPEC_ONSET_MS = 300
SPEC_E2A_FRAMES = 2

# ------------------------------------------------------------------ wav io
def write_wav(path, chans, sr=SR):
    n = len(chans[0])
    with wave.open(path, "wb") as w:
        w.setnchannels(len(chans)); w.setsampwidth(2); w.setframerate(sr)
        buf = bytearray()
        for i in range(n):
            for c in chans:
                v = max(-1.0, min(1.0, c[i]))
                buf += struct.pack("<h", int(v * 32767))
        w.writeframes(bytes(buf))

def read_wav_mono(path):
    with wave.open(path, "rb") as w:
        ch, sw, sr, n = w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()
        raw = w.readframes(n)
    assert sw == 2, "16-bit PCM only"
    vals = struct.unpack("<%dh" % (n * ch), raw)
    mono = [sum(vals[i*ch:(i+1)*ch]) / ch / 32768.0 for i in range(n)]
    return mono, sr

# ------------------------------------------------------------------ input
UTTS = [(0.3, 1.5, 180, 0.35), (4.6, 5.8, 240, 0.6), (7.3, 8.6, 130, 0.8)]  # start,end,f0,level: neutral, excited, angry
BREATH = (3.3, 3.6, 0.025)   # Rika segment: unvoiced breath inside the silence, then a hesitation

def gen_input(seconds=10.0, seed=13):
    """Placeholder 'user': 3 tone-burst utterances with vibrato, a breath-in-silence before utterance 2
    (the Rika moment: pause, inhale, hesitate, speak) and a 4 Hz click track (timing reference)."""
    rnd = random.Random(seed)
    n = int(seconds * SR); x = [0.0] * n
    for s, e, f0, lvl in UTTS:
        a, b = int(s*SR), int(e*SR); ph = 0.0
        for i in range(a, b):
            t = (i - a) / SR; L = (b - a) / SR
            env = math.sin(math.pi * t / L) ** 0.5
            f = f0 * (1 + 0.04 * math.sin(2*math.pi*5*t))
            ph += 2*math.pi*f/SR
            x[i] += lvl * env * (0.7*math.sin(ph) + 0.2*math.sin(2*ph) + 0.1*math.sin(3*ph)) + 0.01*rnd.uniform(-1,1)
    s, e, lvl = BREATH
    a, b = int(s*SR), int(e*SR)
    for i in range(a, b):  # breath = band-limited-ish white noise with a soft envelope (high ZCR, unvoiced)
        env = math.sin(math.pi * (i - a) / (b - a)) ** 0.3
        x[i] += lvl * env * rnd.uniform(-1, 1) * math.sqrt(3)
    for k in range(int(seconds * 4)):  # click track; period 3.125 heartbeats, so never 2 frames in a row
        c = int(k * SR / 4)
        for j in range(40):
            if c + j < n: x[c+j] += 0.15 * math.exp(-j/6)
    return x

# ------------------------------------------------------------------ analysis + emotion (stub)
def analyze(x, sr):
    frames = []
    for f in range(len(x) // FRAME):
        seg = x[f*FRAME:(f+1)*FRAME]
        rms = math.sqrt(sum(v*v for v in seg) / FRAME)
        zc = sum(1 for i in range(1, FRAME) if (seg[i-1] < 0) != (seg[i] < 0))
        frames.append((rms, zc * sr / (2 * FRAME)))
    return frames

def emotion(rms, pitch):
    """Heuristic console label (legacy proxy) — kept for the capture."""
    arousal = min(1.0, rms / 0.4)
    valence = max(-1.0, min(1.0, (pitch - 190) / 80))
    label = "calm" if arousal < 0.3 else ("excited" if valence > 0.2 else ("tense" if valence < -0.2 else "engaged"))
    return arousal, valence, label

def clamp(x, lo, hi): return max(lo, min(hi, x))

def vad(v, a, d=0.5, c=STUB_CONF):
    return {"v": clamp(v, -1, 1), "a": clamp(a, 0, 1), "d": clamp(d, 0, 1), "c": clamp(c, 0, 1)}

def caller_frame(rms, pitch):
    a, v, _ = emotion(rms, pitch)
    return vad(v, a)  # no dominance proxy on the stub path: 0.5

def label(f):
    """Port of galion-emotion frame.rs EmotionFrame::label."""
    if f["c"] < 0.2: return "Neutral"
    v, a = f["v"], f["a"]
    if v >= 0.25 and a >= 0.6: return "Excited"
    if v >= 0.25: return "Joy"
    if v <= -0.25 and a >= 0.6 and f["d"] >= 0.5: return "Angry"
    if v <= -0.25 and a >= 0.6: return "Anxious"
    if v <= -0.25 and a < 0.35: return "Tired"
    if v <= -0.25: return "Sad"
    if a < 0.25: return "Calm"
    return "Neutral"

def respond(c):
    """Port of StubExpressor::respond (decision F: counterbalance; joy is the only mirrored state)."""
    out = vad(*NEUTRAL_WARM, c=1.0)
    if c is None: return out
    lab = label(c)
    if lab == "Angry": out.update(a=0.2, d=0.35, v=0.2)
    elif lab == "Anxious": out.update(a=0.15, d=0.55, v=0.3)
    elif lab in ("Sad", "Tired"): out.update(a=0.2, v=0.25, d=0.45)
    elif lab in ("Joy", "Excited"): out.update(v=max(c["v"] * 0.9, 0.3), a=max(c["a"] * 0.85, 0.35))
    return vad(out["v"], out["a"], out["d"], out["c"])

def mirror(c):
    """The OLD canned behaviour (pitch tracked the caller). Kept only so selftest proves it is rejected."""
    return dict(c)

def check_counterbalance(c, t):
    """Port of film.rs check_counterbalance. Returns None if OK, else a violation string."""
    if t["v"] < 0.0: return f"NegativeValence(target={t['v']:.2f})"
    lab = label(c)
    if lab in ("Angry", "Anxious"):
        if t["a"] >= c["a"]: return f"ArousalMirrored(caller={c['a']:.2f},target={t['a']:.2f})"
        if lab == "Angry" and t["d"] >= c["d"]: return f"DominanceEscalated(caller={c['d']:.2f},target={t['d']:.2f})"
    if lab in ("Sad", "Tired"):
        if t["a"] > max(c["a"] + LOW_AROUSAL_HEADROOM, NEUTRAL_WARM[1]):
            return f"OverEnergised(caller={c['a']:.2f},target={t['a']:.2f})"
    return None

def plan(t):
    """Port of StubExpressor::plan (pitch shift + energy only; the canned band has no rate/warmth)."""
    st = clamp(2.0 * t["v"] + 1.5 * (t["a"] - 0.35), -4.0, 4.0)
    energy = clamp(0.75 + 0.4 * t["a"], 0.6, 1.2)
    pre_pause_ms = 250 if t["a"] < 0.25 else 0
    return st, energy, pre_pause_ms

def mean_frame(fs):
    n = len(fs)
    return vad(sum(f["v"] for f in fs)/n, sum(f["a"] for f in fs)/n, sum(f["d"] for f in fs)/n, sum(f["c"] for f in fs)/n)

# ------------------------------------------------------------------ consent
def consent_allows(bits, purpose):
    """Default deny. Workplace/education are prohibited regardless of bits (not overridable)."""
    if purpose in PROHIBITED: return False, "DenyProhibitedPurpose"
    if not bits & SENSE: return False, "NoSense"
    if not bits & ADAPT: return False, "NoAdapt"
    return True, "Allow"

def parse_bits(s):
    bits = 0
    for tok in (s or "").replace("+", ",").split(","):
        tok = tok.strip().lower()
        if tok == "sense": bits |= SENSE
        elif tok == "adapt": bits |= ADAPT
        elif tok in ("", "none"): pass
        else: raise SystemExit(f"unknown consent bit {tok!r} (use sense,adapt|none)")
    return bits

# ------------------------------------------------------------------ Rika (port of rika.rs, stub path)
class Rika:
    """Faithful-enough Python port of galion-emotion rika.rs Rika::step on the RMS/ZCR stub path
    (no affect/tension triggers). One step per 80 ms heartbeat. Never lexical."""
    Q, HYST, T, D, R = 6, 2, 25, 4, 100
    SHIFT_DB, BREATH_ZCR, VOICE_RMS, VOICE_ZCR, PRESENCE = 6.0, 0.3, 0.02, 0.25, 0.0015
    SOFT_ACK_AFTER = 12

    def __init__(self, enabled, soft_ack):
        self.enabled, self.soft_ack = enabled, soft_ack
        self.state, self.frame = "OFF", 0
        self.quiet_run = self.hits = self.armed_frames = self.release_left = self.refr = 0
        self.floor = None; self.spoken = False; self.ack_used = False
        self.rep = {"evidence": None, "armed": None, "effect": None, "onset": None, "soft_ack": None,
                    "outcome": None, "off_frames": 0, "lexical": 0}

    def _idle(self): self.state, self.quiet_run, self.hits = "IDLE", 0, 0

    def step(self, allowed, me_speaking, features):
        idx = self.frame; self.frame += 1
        if not (self.enabled and allowed):
            self.state = "OFF"; self.floor = None; self.spoken = False; self.rep["off_frames"] += 1
            return {"state": "OFF", "cue": "None", "lean_in": 0.0}  # features() is never called: no PCM read
        if self.state == "OFF": self.state = "IDLE"
        rms, zcr = features()
        voiced = rms >= self.VOICE_RMS and zcr <= self.VOICE_ZCR
        if voiced: self.spoken = True
        def shift():
            if voiced or self.floor is None: return False
            db = 20 * math.log10(max(rms, 1e-7) / max(self.floor, 1e-6))
            return db >= self.SHIFT_DB and zcr >= self.BREATH_ZCR
        def track(): self.floor = rms if self.floor is None else self.floor + 0.1 * (rms - self.floor)
        s = self.state
        if s == "IDLE":
            if voiced: self.quiet_run = 0
            else:
                self.quiet_run += 1; track()
                if self.quiet_run >= self.Q: self.state = "QUIET"
        elif s == "QUIET":
            if voiced: self._idle()
            elif shift(): self.state, self.hits = "CANDIDATE", 1; self.cand = idx
            else: track()
        elif s == "CANDIDATE":
            if voiced: self._idle()
            elif shift():
                self.hits += 1
                if self.hits >= self.HYST:
                    self.state, self.armed_frames = "ARMED", 0
                    if self.rep["armed"] is None:
                        self.rep.update(armed=idx, effect=idx + 1, evidence=self.cand)
            else: self.state, self.hits = "QUIET", 0
        elif s == "ARMED":
            self.armed_frames += 1
            if voiced:
                self.state = "CAUGHT"
                if self.rep["onset"] is None: self.rep.update(onset=idx, outcome="caught")
            elif self.armed_frames >= self.T:
                self.state, self.release_left = "RELEASE", self.D
                if self.rep["outcome"] is None: self.rep["outcome"] = "released"
        elif s == "CAUGHT": self.state, self.refr = "REFRACTORY", self.R
        elif s == "RELEASE":
            self.release_left -= 1
            if self.release_left <= 0: self.state, self.refr = "REFRACTORY", self.R
        elif s == "REFRACTORY":
            if not voiced: track()
            self.refr -= 1
            if self.refr <= 0: self._idle(); self.ack_used = False
        out = {"state": self.state, "cue": "None", "lean_in": 0.0}
        if self.state in ("ARMED", "CAUGHT"):
            out.update(cue="LeanIn", lean_in=1.0)
            if self.state == "CAUGHT": out["yield_now"] = True
            elif self.soft_ack and self.spoken and not self.ack_used and not me_speaking \
                    and self.armed_frames >= self.SOFT_ACK_AFTER:
                out["cue"] = "SoftBreath"; self.ack_used = True
                if self.rep["soft_ack"] is None: self.rep["soft_ack"] = idx
        elif self.state == "RELEASE":
            out["lean_in"] = self.release_left / self.D; out["cue"] = "LeanIn" if out["lean_in"] > 0 else "None"
        return out

def hum(n, f0, sr=SR, level=HUM_LEVEL):
    """Wordless closed-mouth 'mm': low-harmonic tone, Hann envelope, n samples. Never a word."""
    y = []; ph = 0.0
    for i in range(n):
        env = 0.5 - 0.5 * math.cos(2 * math.pi * i / max(1, n - 1))
        ph += 2 * math.pi * f0 / sr
        y.append(level * env * (0.85 * math.sin(ph) + 0.15 * math.sin(2 * ph)))
    return y

# ------------------------------------------------------------------ canned responder
def canned_reply(frames, sr, x=None, bits=0, purpose="personal", rika_opener=False, lag_frames=12):
    """Agent band: soft backchannel while the user talks, a finite reply after each turn.
    Pitch/energy follow Galion's COUNTERBALANCED target (decision F), never the caller's raw emotion.
    Without consent (or in workplace/education) nothing is read: neutral-warm throughout."""
    allowed, why = consent_allows(bits, purpose)
    n = len(frames) * FRAME; y = [0.0] * n; ph = 0.0; amp = 0.0
    events, turns = [], []
    rika = Rika(enabled=rika_opener, soft_ack=rika_opener)
    rika_log = []
    hold = False
    in_turn = False; turn_start = 0; quiet = 0; turn_fs = []; reply = None  # reply=(start,end,target,st,en)
    nw_st, nw_en, _ = plan(respond(None))
    for f in range(len(frames)):
        user_now = frames[f][0] > 0.05
        if user_now:
            if not in_turn: in_turn, turn_start, turn_fs = True, f, []
            quiet = 0; last_voiced = f
            turn_fs.append(caller_frame(*frames[f]))
        elif in_turn:
            quiet += 1
            if quiet >= 5:  # 100 ms hangover = end of turn
                in_turn = False
                caller = mean_frame(turn_fs) if allowed else None
                tgt = respond(caller)
                st, en, pre = plan(tgt)
                start = last_voiced + 1 + lag_frames + pre // 20
                dur = min(last_voiced + 1 - turn_start, 50)
                reply = (start, start + dur, tgt, st, en)
                turns.append({"turn": len(turns) + 1, "user_start": turn_start, "user_end": last_voiced,
                              "caller": caller, "caller_label": label(caller) if caller else None,
                              "target": tgt, "pitch_st": st, "energy": en, "pre_pause_ms": pre,
                              "reply_start": start, "reply_end": start + dur,
                              "violation": check_counterbalance(caller, tgt) if caller else None,
                              "mirror_violation": check_counterbalance(caller, mirror(caller)) if caller else None})
        # backchannel target: lagged caller frame (canned 240 ms lag), counterbalanced
        if user_now:
            lag = frames[max(0, f - lag_frames)]
            c = caller_frame(*lag) if allowed else None
            st, en, _ = plan(respond(c))
            target = 0.06 * en
        elif reply and reply[0] <= f < reply[1]:
            st, en = reply[3], reply[4]; target = SPEECH_REPLY * en
        else:
            st, en = nw_st, nw_en; target = 0.0
        me_speaking = target > 0.0 or amp > 0.01
        # Rika: once per heartbeat (every 4th analysis frame, on the heartbeat's 4 frames of user PCM)
        if f % HB == HB - 1:
            hb = f // HB
            def feats(hb=hb):
                seg = x[hb*HB*FRAME:(hb+1)*HB*FRAME]
                rms = math.sqrt(sum(v*v for v in seg) / len(seg))
                zcr = sum(1 for i in range(1, len(seg)) if (seg[i-1] >= 0) != (seg[i] >= 0)) / len(seg)
                return rms, zcr
            ro = rika.step(allowed, me_speaking, feats)
            rika_log.append((hb, ro))
            if ro["cue"] == "SoftBreath":
                # one heartbeat of hum, starting next frame (effect ≤ 2 frames), on the agent band
                h = hum(HB * FRAME, 220 * 2 ** (nw_st / 12))
                s0 = (f + 1) * FRAME
                for i, v in enumerate(h):
                    if s0 + i < n: y[s0 + i] += v
            hold = ro["state"] == "ARMED"  # held breath: no new me-band onset while armed
        if hold and not user_now and amp < 0.01: target = 0.0
        f0 = 220 * 2 ** (st / 12)
        for i in range(FRAME):
            amp += (target - amp) * 0.002
            ph += 2*math.pi*f0/sr
            y[f*FRAME+i] += amp * (0.8*math.sin(ph) + 0.2*math.sin(1.5*ph))
        events.append((f, frames[f][0], amp, emotion(*frames[f])[2], user_now))
    # measure reply onset: first frame the agent envelope reaches half its reply level
    for t in turns:
        half = 0.5 * SPEECH_REPLY * t["energy"]; onset = None
        for f in range(t["reply_start"], min(len(events), t["reply_end"] + 25)):
            if events[f][2] >= half: onset = f; break
        t["onset_frame"] = onset
        t["onset_ms"] = None if onset is None else (onset - t["user_end"]) * 1000 * FRAME // sr
    return y, events, turns, rika, rika_log, (allowed, why)

# ------------------------------------------------------------------ specs -> KD targets
def r2(x): return None if x is None else round(x, 3)
def fr(f): return None if f is None else {k: r2(v) for k, v in f.items()}

def evaluate_specs(turns, rika, gate, rika_opener):
    """Each spec the canned path cannot meet becomes a KD target (decision D). Returns (results, kd_rows)."""
    allowed, why = gate
    res, kd = [], []
    # F-counterbalance (must pass now; a failure here is a bug, not a KD target)
    if allowed:
        bad = [t for t in turns if t["violation"]]
        res.append(("F-counterbalance", "FAIL" if bad else "PASS",
                    f"{len(turns)-len(bad)}/{len(turns)} turns counterbalanced; old mirror rejected on "
                    f"{sum(1 for t in turns if t['mirror_violation'])}/{len(turns)}"))
    else:
        res.append(("F-counterbalance", "PASS", f"no reading ({why}): neutral-warm only"))
    # D-onset: p50 of reply onset after user end <= 300 ms (policy pre-pause counts; it is deliberate)
    ons = sorted(10**6 if t["onset_ms"] is None else t["onset_ms"] for t in turns)
    p50 = ons[len(ons) // 2] if ons else None
    ok = p50 is not None and p50 <= SPEC_ONSET_MS
    if not ok:
        for t in turns:
            kd.append({"spec": "D-onset", "frame": t["user_end"], "band": "me", "expected": {"onset_ms_p50_max": SPEC_ONSET_MS},
                       "canned": {"onset_ms": t["onset_ms"]}, "teacher": "moshiko", "teacher_output": None})
    res.append(("D-onset", "PASS" if ok else "FAIL",
                f"p50 {p50} ms <= {SPEC_ONSET_MS} ms (per turn: " + ",".join(str(t["onset_ms"]) for t in turns) + " ms; pre-pause included)"))
    # E-in -> A-out <= 2 heartbeat frames: canned backchannel uses a fixed 12x20 ms lag = 3 heartbeats
    lag_hb = math.ceil(12 / HB)
    if allowed:
        for t in turns:
            kd.append({"spec": "E-in-A-out", "frame": t["user_start"], "band": "feeling",
                       "expected": {"frames_max": SPEC_E2A_FRAMES}, "canned": {"frames": lag_hb},
                       "teacher": "moshiko", "teacher_output": None})
        res.append(("E-in-A-out", "FAIL", f"canned lag {lag_hb} heartbeats > {SPEC_E2A_FRAMES}"))
    else:
        res.append(("E-in-A-out", "SKIP", f"no reading ({why})"))
    # D-language: tones are not speech
    for t in turns:
        kd.append({"spec": "D-language", "frame": t["reply_start"], "band": "me", "expected": {"speech": True},
                   "canned": {"speech": False, "kind": "tone"}, "teacher": "moshiko", "teacher_output": None})
    res.append(("D-language", "FAIL", "agent band is tones, not speech"))
    # emotion: heuristic RMS/ZC proxy vs SER oracle
    if allowed:
        for t in turns:
            kd.append({"spec": "emotion-ser", "frame": t["user_start"], "band": "feeling",
                       "expected": {"source": "ser-oracle-label"},
                       "canned": {"label": t["caller_label"], "vad": fr(t["caller"])},
                       "teacher": "sensevoice-small", "teacher_output": None})
        res.append(("emotion-ser", "FAIL", "heuristic RMS/ZC proxy (conf 0.3), not an SER label"))
    else:
        res.append(("emotion-ser", "SKIP", f"no reading ({why})"))
    # E-Rika opener (canned port; Rust R1–R9 stay authoritative in G6)
    r = rika.rep
    if not rika_opener:
        res.append(("E-Rika-opener", "SKIP", "opener off (default; Q14/Q11 opt-in)"))
    elif not allowed:
        res.append(("E-Rika-opener", "SKIP", f"opener requested but Rika OFF ({why}); user band never read"))
    else:
        checks = {
            "armed": r["armed"] is not None,
            "effect<=2": r["armed"] is not None and r["effect"] - r["evidence"] <= 2,
            "caught": r["outcome"] == "caught",
            "yield<=1": r["onset"] is not None,
            "non-lexical": r["lexical"] == 0,
            "hum>=12dB-below": 20 * math.log10(SPEECH_REPLY / HUM_LEVEL) >= 12,
        }
        bad = [k for k, v in checks.items() if not v]
        if bad:
            kd.append({"spec": "E-Rika-opener", "frame": r["evidence"], "band": "feeling",
                       "expected": {"lean_in": "non-verbal", "checks": sorted(checks)}, "canned": {"failed": bad},
                       "teacher": "r-fixture", "teacher_output": None})
        res.append(("E-Rika-opener", "FAIL" if bad else "PASS",
                    f"evidence hb {r['evidence']} -> armed hb {r['armed']} -> soft hum hb {r['soft_ack']} -> "
                    f"user onset hb {r['onset']} ({r['outcome']})" + (f"; failed {bad}" if bad else "")))
    for row in kd: row["frame_ms"] = 1000 * FRAME * (HB if row["spec"] == "E-Rika-opener" else 1) // SR
    return res, kd

# ------------------------------------------------------------------ console
def bar(v, w=20):
    k = int(max(0, min(1, v)) * w); return "#" * k + "." * (w - k)

def try_live_model():
    p = os.environ.get("GALION_MODEL")
    if not p: return None, "GALION_MODEL unset"
    if not os.path.exists(p): return None, f"GALION_MODEL={p} not found"
    # HOOK: load galion-tensor student (safetensors only) via FFI/subprocess here.
    return None, f"GALION_MODEL={p} present but live backend not wired yet (skeleton)"

def fmt_vad(f): return f"V{f['v']:+.2f} A{f['a']:.2f} D{f['d']:.2f}"

def run(inp, outp, cap, canned=False, bits=0, purpose="personal", rika_opener=False, kd_out=None):
    x, sr = read_wav_mono(inp)
    t0 = time.time()
    model, why = try_live_model()
    backend = "live" if model else "canned"
    frames = analyze(x, sr)
    y, ev, turns, rika, rlog, gate = canned_reply(frames, sr, x, bits, purpose, rika_opener)
    x = x[:len(y)]
    write_wav(outp, [x, y], sr)
    dt = time.time() - t0
    allowed, gwhy = gate
    cb = "+".join(n for b, n in ((SENSE, "SENSE"), (ADAPT, "ADAPT")) if bits & b) or "none"
    rstate = {hb: ro for hb, ro in rlog}
    lines = [f"GALION VOICE canned demo | backend={backend} ({why})",
             f"in={inp} out={outp} sr={sr} frames={len(frames)} frame_ms={1000*FRAME//sr} heartbeat_ms={1000*FRAME*HB//sr}",
             f"consent={cb} purpose={purpose} gate={gwhy} | affect: {'counterbalance (F)' if allowed else 'no reading -> neutral-warm'}"
             f" | rika opener: {'requested' if rika_opener else 'off (default)'}{'' if (allowed or not rika_opener) else ' -> OFF ('+gwhy+')'}",
             "t(s)   USER band            AGENT band           user-proxy rika"]
    for f, urms, amp, lab, un in ev[::10]:
        ro = rstate.get(f // HB)
        rs = ro["state"] if ro else "-"
        lines.append(f"{f*FRAME/sr:5.2f}  {bar(urms/0.5)} {bar(amp/0.3)} {lab:<8}{rs}{' (user speaking)' if un else ''}")
    for hb, ro in rlog:
        if ro["cue"] == "SoftBreath":
            lines.append(f"{(hb+1)*HB*FRAME/sr:5.2f}  rika: wordless soft hum ('mm', {HB*FRAME*1000//sr} ms, "
                         f"{20*math.log10(SPEECH_REPLY/HUM_LEVEL):.0f} dB below reply) — no words")
    lines.append("turns (caller -> Galion target; decision F: counterbalance, mirror only joy):")
    for t in turns:
        if t["caller"]:
            lines.append(f"  turn {t['turn']} {t['user_start']*FRAME/sr:.2f}-{(t['user_end']+1)*FRAME/sr:.2f}s caller={t['caller_label']}"
                         f" ({fmt_vad(t['caller'])}) -> galion {fmt_vad(t['target'])} pitch {t['pitch_st']:+.2f} st"
                         f" energy {t['energy']:.2f} pre-pause {t['pre_pause_ms']} ms"
                         f" [{'counterbalance OK' if not t['violation'] else 'VIOLATION '+t['violation']}"
                         f"{'; mirror would violate' if t['mirror_violation'] else ''}] onset {t['onset_ms']} ms")
        else:
            lines.append(f"  turn {t['turn']} {t['user_start']*FRAME/sr:.2f}-{(t['user_end']+1)*FRAME/sr:.2f}s caller=no reading"
                         f" -> galion neutral-warm ({fmt_vad(t['target'])}) onset {t['onset_ms']} ms")
    res, kd = evaluate_specs(turns, rika, gate, rika_opener)
    lines.append("specs (failing -> KD targets, decision D):")
    for sid, st, note in res: lines.append(f"  {st:<4} {sid}: {note}")
    lines.append(f"kd targets: {len(kd)}" + (f" -> {kd_out}" if kd_out else " (use --kd-out to export)"))
    if canned:
        lines.append(f"processed {len(x)/sr:.2f}s audio | RTF=canned (not a model benchmark; wall time omitted for hash stability)")
    else:
        lines.append(f"processed {len(x)/sr:.2f}s audio in {dt:.2f}s wall (RTF={dt/(len(x)/sr):.3f}, canned path) on CPU, pure stdlib")
    txt = "\n".join(lines) + "\n"
    with open(cap, "w") as fh: fh.write(txt)
    if kd_out:
        with open(kd_out, "w") as fh:
            for row in kd: fh.write(json.dumps(row, sort_keys=True, ensure_ascii=True) + "\n")
    sys.stdout.write(txt)
    return res, kd

# ------------------------------------------------------------------ selftest
def selftest():
    fails = []
    grid = [vad(v / 3, a / 5, d / 2, c) for v in range(-3, 4) for a in range(0, 6) for d in range(0, 3) for c in (0.1, 0.3, 0.9)]
    for c in grid:
        if check_counterbalance(c, respond(c)): fails.append(f"respond violates for {c}")
    for c in (vad(-0.8, 0.9, 0.8, 0.9), vad(-0.7, 0.9, 0.2, 0.9), vad(-0.5, 0.5, 0.5, 0.9)):
        if not check_counterbalance(c, mirror(c)): fails.append(f"mirror not rejected for {label(c)}")
    if check_counterbalance(vad(0.8, 0.8, 0.5, 0.9), mirror(vad(0.8, 0.8, 0.5, 0.9))): fails.append("joy mirror rejected")
    for p in PROHIBITED:
        if consent_allows(SENSE | ADAPT, p)[0]: fails.append(f"{p} allowed")
    if consent_allows(0, "personal")[0] or consent_allows(SENSE, "personal")[0]: fails.append("default not deny")
    called = []
    r = Rika(enabled=True, soft_ack=True)
    r.step(False, False, lambda: called.append(1) or (0.0, 0.0))
    if called: fails.append("rika read PCM without consent")
    if Rika(False, False).step(True, False, lambda: (0.0, 0.0))["state"] != "OFF": fails.append("rika on by default")
    for f in fails: print("SELFTEST FAIL", f)
    print(f"SELFTEST {'PASS' if not fails else 'FAIL'} ({len(grid)} VAD grid points + mirror/joy/purpose/default/rika checks)")
    return not fails

if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", nargs="?", default="all", choices=["gen", "run", "all", "selftest"])
    ap.add_argument("--in", dest="inp", default="out/demo_input.wav")
    ap.add_argument("--out", dest="outp", default="out/demo_output_stereo.wav")
    ap.add_argument("--capture", default="out/console_capture.txt")
    ap.add_argument("--kd-out", default=None, help="write failing specs as KD targets (JSONL)")
    ap.add_argument("--consent", default="none", help="consent bits: none (default) | sense,adapt")
    ap.add_argument("--purpose", default="personal", choices=PURPOSES)
    ap.add_argument("--rika-opener", action="store_true", help="opt-in Rika opener (needs --consent sense,adapt; never in workplace/education)")
    ap.add_argument("--canned", action="store_true", help="deterministic canned mode: stable hash (A14 golden)")
    a = ap.parse_args()
    if a.mode == "selftest": sys.exit(0 if selftest() else 1)
    if a.canned or os.environ.get("GALION_DEMO_CANNED") == "1":
        a.canned = True
        os.environ.pop("GALION_MODEL", None)  # golden must never depend on a local model file
    os.makedirs(os.path.dirname(a.outp) or ".", exist_ok=True)
    if a.mode in ("gen", "all"):
        write_wav(a.inp, [gen_input()]); print(f"wrote {a.inp}")
    if a.mode in ("run", "all"):
        run(a.inp, a.outp, a.capture, canned=a.canned, bits=parse_bits(a.consent), purpose=a.purpose,
            rika_opener=a.rika_opener, kd_out=a.kd_out)
