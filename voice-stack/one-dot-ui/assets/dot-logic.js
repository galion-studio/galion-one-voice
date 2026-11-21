// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

// Pure helpers for the Samantha dot. No DOM.
// Numbers match voice-stack/one-dot-ui/src/look.rs and src/engine.rs.
// Radii: galion-render DotParams. Springs: v1-screen.html function spring.
// Luti: galion-render OrbMotion::radius_gain.
(function (root, factory) {
  const api = factory();
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.SamanthaDot = api;
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  "use strict";

  const SCHEMA = "galion-samantha-ws/1";
  const WS_PATH = "/v1/samantha";
  const DEFAULT_PORT = 7881;
  const TICK_MS = 80;
  const QUIET_MS = 640;
  const THINK_MS = 640;
  const SPEAK_MS = 2400;
  const MAX_UTTERANCE_MS = 15000;
  const LISTEN_CAP_MS = 1600;
  const VOICE_RMS = 0.02;
  const IDLE_RADIUS = 0.100;
  const LISTEN_RADIUS = 0.120;
  const SPEAK_RADIUS = 0.115;
  const THINK_RADIUS = 0.108;
  const OFFLINE_RADIUS = 0.060;
  const OFFLINE_BRIGHTNESS = 0.08;
  const LISTEN_PERIOD_MS = 3200;
  const VIOLET_DEEP = [76, 29, 149];
  const VIOLET = [124, 58, 237];
  const VIOLET_LIGHT = [196, 181, 253];
  const OBSIDIAN = [10, 10, 12];
  const AMBER = [245, 165, 36];
  const RESTING = { arousal: 0.3, valence: 0.25, dominance: 0.5 };
  const LUTI = {
    idle: { w: 1.8, z: 0.92 },
    listening: { w: 3.4, z: 0.88 },
    thinking: { w: 2.4, z: 0.90 },
    speaking: { w: 3.0, z: 0.92 },
    offline: { w: 2.0, z: 1.02 }
  };

  function clamp(x, a, b) {
    if (!isFinite(x)) return a;
    return Math.min(b, Math.max(a, x));
  }

  function sanePad(mood) {
    const m = mood || {};
    return {
      arousal: isFinite(+m.arousal) ? clamp(+m.arousal, 0, 1) : RESTING.arousal,
      valence: isFinite(+m.valence) ? clamp(+m.valence, -1, 1) : RESTING.valence,
      dominance: isFinite(+m.dominance) ? clamp(+m.dominance, 0, 1) : RESTING.dominance
    };
  }

  function lerpU8(a, b, t) {
    t = clamp(t, 0, 1);
    return Math.round(a + (b - a) * t);
  }

  function lerpRgb(a, b, t) {
    return [lerpU8(a[0], b[0], t), lerpU8(a[1], b[1], t), lerpU8(a[2], b[2], t)];
  }

  function look(pad) {
    const p = sanePad(pad);
    let breath = Math.round(2200 - p.arousal * 2000);
    breath = Math.max(800, Math.min(2400, breath));
    let rgb;
    if (p.valence <= 0.25) rgb = lerpRgb(VIOLET_DEEP, VIOLET, (p.valence + 1) / 1.25);
    else rgb = lerpRgb(VIOLET, VIOLET_LIGHT, (p.valence - 0.25) / 0.75);
    return {
      breathMs: breath,
      rgb: rgb,
      glow: 0.25 + 0.50 * p.dominance,
      radiusBias: (p.dominance - 0.5) * 0.012
    };
  }

  function sine(animMs, period, reduced) {
    if (reduced || !period) return 0;
    const p = (animMs % period) / period;
    return Math.sin(p * Math.PI * 2);
  }

  function geometry(state, level, animMs, pad, reduced) {
    const face = look(pad);
    const lv = clamp(level, 0, 1);
    let radius = IDLE_RADIUS;
    let brightness = 0.84;
    if (state === "listening") {
      const s = sine(animMs, LISTEN_PERIOD_MS, reduced);
      radius = LISTEN_RADIUS + 0.002 * s + 0.004 * lv + face.radiusBias;
      brightness = 0.96;
    } else if (state === "thinking") {
      const s = sine(animMs, face.breathMs, reduced);
      radius = THINK_RADIUS + 0.008 * s + face.radiusBias;
      brightness = 0.90 + 0.04 * s;
    } else if (state === "speaking") {
      const step = Math.floor(animMs / 80) % 4;
      const rhythm = [0, 0.5, 1, 0.5][step];
      const drive = Math.max(lv, 0.35 * rhythm);
      radius = SPEAK_RADIUS + 0.030 * drive + face.radiusBias;
      brightness = 0.97 + 0.03 * drive;
    } else {
      const s = sine(animMs, face.breathMs, reduced);
      radius = IDLE_RADIUS + 0.006 * s + face.radiusBias;
      brightness = 0.84 + 0.03 * s;
    }
    return { radius: clamp(radius, 0, 0.5), brightness: clamp(brightness, 0, 1) };
  }

  function backgroundAt(u, v, tMs) {
    u = clamp(u, 0, 1);
    v = clamp(v, 0, 1);
    const period = 24000;
    const phase = ((tMs % period) / period) * Math.PI * 2;
    const drift = 0.15 * Math.sin(phase);
    const ramp = clamp(0.5 * (u + v) + drift, 0, 1);
    const hue = lerpRgb(AMBER, VIOLET, ramp);
    const du = u - 0.5;
    const dv = v - 0.5;
    const r = Math.sqrt(du * du + dv * dv) * Math.SQRT2;
    const vignette = clamp(1 - r, 0, 1);
    const rgb = lerpRgb(OBSIDIAN, hue, 0.38 * vignette * vignette);
    return { r: rgb[0], g: rgb[1], b: rgb[2] };
  }

  function previewMood(tMs) {
    const t = +tMs || 0;
    return sanePad({
      arousal: RESTING.arousal + 0.05 * Math.sin(t / 1000),
      valence: RESTING.valence + 0.04 * Math.sin(t / 1300),
      dominance: RESTING.dominance + 0.03 * Math.sin(t / 1700)
    });
  }

  // Page spring. dtMs is display time. Finite dt is clamped to 1..50 ms.
  function spring(p, v, target, dtMs, w, z) {
    if (!isFinite(dtMs) || !isFinite(w) || !isFinite(z)) return [isFinite(p) ? p : 0, 0];
    if (!isFinite(target)) return [isFinite(p) ? p : 0, isFinite(v) ? v : 0];
    const h = Math.min(0.05, Math.max(0.001, dtMs / 1000));
    const p0 = isFinite(p) ? p : target;
    const v0 = isFinite(v) ? v : 0;
    const a = -w * w * (p0 - target) - 2 * z * w * v0;
    v = v0 + a * h;
    p = p0 + v * h;
    if (!isFinite(a) || !isFinite(v) || !isFinite(p)) return [target, 0];
    return [p, v];
  }

  function packPcm(samples, seq) {
    const n = samples.length;
    const buf = new ArrayBuffer(8 + n * 2);
    const view = new DataView(buf);
    view.setUint8(0, 0x47);
    view.setUint8(1, 0x53);
    view.setUint8(2, 1);
    view.setUint8(3, 0);
    view.setUint8(4, 1);
    view.setUint8(5, 0);
    view.setUint16(6, seq & 0xffff, true);
    for (let i = 0; i < n; i++) {
      let s = samples[i] | 0;
      if (s > 32767) s = 32767;
      if (s < -32768) s = -32768;
      view.setInt16(8 + i * 2, s, true);
    }
    return new Uint8Array(buf);
  }

  function parsePcm(bytes) {
    if (!bytes || bytes.length < 8 || bytes.length % 2 !== 0) return null;
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (view.getUint8(0) !== 0x47 || view.getUint8(1) !== 0x53 || view.getUint8(2) !== 1) return null;
    const dir = view.getUint8(3);
    const rateCode = view.getUint8(4);
    if (dir > 1 || rateCode > 1) return null;
    const seq = view.getUint16(6, true);
    const n = (bytes.length - 8) / 2;
    if (!n) return null;
    const samples = new Int16Array(n);
    for (let i = 0; i < n; i++) samples[i] = view.getInt16(8 + i * 2, true);
    return { dir: dir, sampleRate: rateCode === 0 ? 24000 : 16000, seq: seq, samples: samples };
  }

  function rmsInt16(samples) {
    if (!samples || !samples.length) return 0;
    let acc = 0;
    for (let i = 0; i < samples.length; i++) {
      const x = samples[i] / 32768;
      acc += x * x;
    }
    return Math.sqrt(acc / samples.length);
  }

  function helloJson() {
    return JSON.stringify({ schema: SCHEMA, type: "hello", session: null });
  }

  function stopJson() {
    return JSON.stringify({ schema: SCHEMA, type: "stop" });
  }

  function parseEvent(text) {
    let msg;
    try { msg = JSON.parse(text); }
    catch (e) { return null; }
    if (!msg || msg.schema !== SCHEMA || typeof msg.type !== "string") return null;
    if (msg.type === "welcome") return { type: "welcome" };
    if (msg.type === "pong") return { type: "pong" };
    if (msg.type === "error") return { type: "error", error: String(msg.error || "error") };
    if (msg.type === "dot") {
      const state = msg.state === "listening" || msg.state === "speaking" ? msg.state : "idle";
      return { type: "dot", state: state, level: clamp(+msg.level || 0, 0, 1) };
    }
    if (msg.type === "mood") {
      const pad = msg.pad && typeof msg.pad === "object" ? sanePad(msg.pad) : RESTING;
      return { type: "mood", t_ms: +msg.t_ms || 0, source: msg.source === "brain" ? "brain" : "resting", pad: pad };
    }
    if (msg.type === "turn") {
      const sources = Array.isArray(msg.sources) ? msg.sources.filter(function (s) { return typeof s === "string"; }).slice(0, 32) : [];
      const pad = msg.mood && typeof msg.mood === "object" ? sanePad(msg.mood) : RESTING;
      return { type: "turn", reply: String(msg.reply || "").slice(0, 4000), sources: sources, pad: pad };
    }
    if (msg.type === "vad") {
      return { type: "dot", state: msg.event === "start" ? "listening" : "idle", level: 0 };
    }
    return null;
  }

  function allowedWs(url) {
    try {
      const u = new URL(url);
      if (u.protocol !== "ws:" && u.protocol !== "wss:") return false;
      return u.hostname === "127.0.0.1" || u.hostname === "localhost";
    } catch (e) {
      return false;
    }
  }

  function speakEnv(elapsed) {
    const k = Math.max(0, Math.min(1, elapsed / SPEAK_MS));
    return Math.sin(k * Math.PI);
  }

  function createEngine(kind) {
    const mock = kind !== "passive";
    const eng = {
      tMs: 0, phase: "idle", left: 0, level: 0, quiet: 0, listen: 0, think: 0,
      voice: false, pcm: false, pending: null
    };
    function moodMsg() {
      return { type: "mood", t_ms: eng.tMs, source: "resting", pad: { arousal: RESTING.arousal, valence: RESTING.valence, dominance: RESTING.dominance } };
    }
    function outbound(includeDot) {
      const out = [moodMsg()];
      if (includeDot) out.push({ type: "dot", state: eng.phase === "thinking" ? "idle" : eng.phase, level: eng.level });
      if (eng.phase === "thinking") out[out.length - 1] = { type: "dot", state: "thinking", level: 0 };
      if (eng.pending) {
        out.push(eng.pending);
        eng.pending = null;
      }
      return out;
    }
    function beginThink() {
      if (eng.phase === "thinking" || eng.phase === "speaking") return;
      eng.phase = "thinking";
      eng.think = 0;
      eng.level = 0;
    }
    function beginSpeak() {
      if (!mock) { eng.level = 0; return; }
      eng.pending = { type: "turn", reply: "Hi. I'm here.", sources: ["fake:reply-engine"], pad: { arousal: RESTING.arousal, valence: RESTING.valence, dominance: RESTING.dominance } };
      eng.level = speakEnv(0);
      eng.left = SPEAK_MS;
      eng.phase = "speaking";
    }
    function advance() {
      if (eng.phase === "listening") {
        eng.listen += TICK_MS;
        if (eng.voice) {
          if (eng.level >= VOICE_RMS) eng.quiet = 0;
          else {
            eng.quiet += TICK_MS;
            if (eng.quiet >= QUIET_MS || eng.listen >= MAX_UTTERANCE_MS) beginThink();
          }
        } else if (eng.level === 0 && eng.listen >= LISTEN_CAP_MS) {
          beginThink();
        }
      } else if (eng.phase === "thinking") {
        eng.think += TICK_MS;
        if (eng.think >= THINK_MS) beginSpeak();
      } else if (eng.phase === "speaking") {
        if (eng.left <= TICK_MS) {
          eng.phase = "idle";
          eng.level = 0;
        } else {
          eng.left -= TICK_MS;
          eng.level = speakEnv(SPEAK_MS - eng.left);
        }
      } else {
        eng.level = 0;
      }
    }
    return {
      hello: function () { return [{ type: "welcome" }]; },
      start: function () {
        eng.phase = "listening";
        eng.quiet = 0;
        eng.listen = 0;
        eng.think = 0;
        eng.voice = false;
        eng.level = 0;
        eng.pending = null;
        return [{ type: "dot", state: "listening", level: 0 }];
      },
      stop: function () {
        eng.phase = "idle";
        eng.level = 0;
        eng.voice = false;
        eng.pending = null;
        return [{ type: "dot", state: "idle", level: 0 }];
      },
      noteLevel: function (rms) {
        if (eng.phase === "idle") eng.phase = "listening";
        eng.level = clamp(+rms || 0, 0, 1);
        eng.pcm = true;
        if (eng.level >= VOICE_RMS) eng.voice = true;
      },
      onTick: function () {
        eng.tMs += TICK_MS;
        if (!eng.pcm && eng.phase === "listening") eng.level = 0;
        eng.pcm = false;
        advance();
        return outbound(true);
      }
    };
  }

  return {
    SCHEMA: SCHEMA,
    WS_PATH: WS_PATH,
    DEFAULT_PORT: DEFAULT_PORT,
    TICK_MS: TICK_MS,
    QUIET_MS: QUIET_MS,
    THINK_MS: THINK_MS,
    SPEAK_MS: SPEAK_MS,
    MAX_UTTERANCE_MS: MAX_UTTERANCE_MS,
    LISTEN_CAP_MS: LISTEN_CAP_MS,
    VOICE_RMS: VOICE_RMS,
    IDLE_RADIUS: IDLE_RADIUS,
    LISTEN_RADIUS: LISTEN_RADIUS,
    SPEAK_RADIUS: SPEAK_RADIUS,
    THINK_RADIUS: THINK_RADIUS,
    OFFLINE_RADIUS: OFFLINE_RADIUS,
    OFFLINE_BRIGHTNESS: OFFLINE_BRIGHTNESS,
    RESTING: RESTING,
    LUTI: LUTI,
    VIOLET: VIOLET,
    sanePad: sanePad,
    look: look,
    geometry: geometry,
    backgroundAt: backgroundAt,
    previewMood: previewMood,
    spring: spring,
    packPcm: packPcm,
    parsePcm: parsePcm,
    rmsInt16: rmsInt16,
    helloJson: helloJson,
    stopJson: stopJson,
    parseEvent: parseEvent,
    allowedWs: allowedWs,
    createEngine: createEngine
  };
});
