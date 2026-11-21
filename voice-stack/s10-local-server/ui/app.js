// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

// One dot for the loopback server. Mood moves the mark. The page does not
// pick a sentence for how she feels. Audio plays at full gain.

const PROTOCOL = "galion-samantha-local/1";
const HEADER = 20;
const MIC_RATE = 16000;
const MIC_FRAME = 320;
const KIND_MIC = 1;
const KIND_TTS = 2;
const FLAG_START = 1;
const FLAG_END = 2;

const canvas = document.getElementById("s");
const caption = document.getElementById("caption");
const live = document.getElementById("live");
const listenBtn = document.getElementById("listen");
const form = document.getElementById("line");
const say = document.getElementById("say");
const ctx2d = canvas.getContext("2d");

const mood = { arousal: 0.3, valence: 0.25, dominance: 0.5 };
let state = "idle";
let ws = null;
let seq = 0;
let audioCtx = null;
let mic = null;
let playAt = 0;
const playing = [];
let pendingMic = new Int16Array(0);

function resize() {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.max(1, Math.floor(window.innerWidth * dpr));
  canvas.height = Math.max(1, Math.floor(window.innerHeight * dpr));
}
window.addEventListener("resize", resize);
resize();

function draw() {
  const w = canvas.width;
  const h = canvas.height;
  ctx2d.fillStyle = "#0B0F14";
  ctx2d.fillRect(0, 0, w, h);
  const radius = Math.min(w, h) * (0.07 + mood.arousal * 0.035);
  const warm = Math.round(236 + mood.valence * 16);
  ctx2d.beginPath();
  ctx2d.arc(w / 2, h / 2, radius, 0, Math.PI * 2);
  ctx2d.fillStyle = "rgb(" + warm + ",241,234)";
  ctx2d.fill();
  requestAnimationFrame(draw);
}
draw();

function setState(next) {
  state = next;
  live.textContent = next;
  listenBtn.setAttribute("aria-pressed", mic ? "true" : "false");
}

function connect() {
  const url = (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/v1/stream";
  ws = new WebSocket(url);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (ev) => {
    if (typeof ev.data === "string") onJson(ev.data);
    else onBinary(ev.data);
  };
  ws.onclose = () => {
    setState("offline");
    setTimeout(connect, 1000);
  };
}

function onJson(text) {
  let msg;
  try { msg = JSON.parse(text); } catch (_) { return; }
  if (msg.type === "ready" && msg.protocol !== PROTOCOL) {
    caption.textContent = "Protocol mismatch.";
    return;
  }
  if (msg.type === "mood") {
    mood.arousal = num(msg.arousal, mood.arousal);
    mood.valence = num(msg.valence, mood.valence);
    mood.dominance = num(msg.dominance, mood.dominance);
    if (msg.state) setState(msg.state);
  } else if (msg.type === "state" && msg.state) {
    setState(msg.state);
    if (msg.reason === "barge" || msg.reason === "end") stopPlayback();
  } else if (msg.type === "reply" && msg.text) {
    caption.textContent = msg.text;
    if (msg.mood) {
      mood.arousal = num(msg.mood.arousal, mood.arousal);
      mood.valence = num(msg.mood.valence, mood.valence);
      mood.dominance = num(msg.mood.dominance, mood.dominance);
    }
  } else if (msg.type === "error" && msg.error) {
    caption.textContent = msg.error;
  }
}

function num(v, fallback) {
  return typeof v === "number" && Number.isFinite(v) ? v : fallback;
}

function onBinary(buf) {
  const frame = decodeFrame(buf);
  if (!frame || frame.kind !== KIND_TTS) return;
  if (frame.flags & FLAG_START) stopPlayback();
  playPcm(frame.pcm, frame.rate);
  if (frame.flags & FLAG_END) setState("idle");
}

function decodeFrame(buf) {
  if (!(buf instanceof ArrayBuffer) || buf.byteLength < HEADER) return null;
  const view = new DataView(buf);
  if (view.getUint8(0) !== 0x47 || view.getUint8(1) !== 0x31 || view.getUint8(2) !== 0x50 || view.getUint8(3) !== 0x43) {
    return null;
  }
  const kind = view.getUint8(5);
  const flags = view.getUint16(6, true);
  const rate = view.getUint32(8, true);
  const n = (buf.byteLength - HEADER) / 2;
  const pcm = new Int16Array(n);
  for (let i = 0; i < n; i++) pcm[i] = view.getInt16(HEADER + i * 2, true);
  return { kind, flags, rate, pcm };
}

function encodeMic(pcm) {
  const buf = new ArrayBuffer(HEADER + pcm.length * 2);
  const view = new DataView(buf);
  view.setUint8(0, 0x47);
  view.setUint8(1, 0x31);
  view.setUint8(2, 0x50);
  view.setUint8(3, 0x43);
  view.setUint8(4, 1);
  view.setUint8(5, KIND_MIC);
  view.setUint16(6, 0, true);
  view.setUint32(8, MIC_RATE, true);
  view.setUint32(12, seq >>> 0, true);
  seq = (seq + 1) >>> 0;
  view.setUint16(16, 1, true);
  view.setUint16(18, 0, true);
  for (let i = 0; i < pcm.length; i++) view.setInt16(HEADER + i * 2, pcm[i], true);
  return buf;
}

function ensureAudio() {
  if (!audioCtx) audioCtx = new AudioContext();
  if (audioCtx.state === "suspended") audioCtx.resume();
  return audioCtx;
}

function playPcm(pcm, rate) {
  const ctx = audioCtx;
  if (!ctx || rate <= 0 || pcm.length === 0) return;
  const buf = ctx.createBuffer(1, pcm.length, rate);
  const ch = buf.getChannelData(0);
  for (let i = 0; i < pcm.length; i++) ch[i] = pcm[i] / 32768;
  const src = ctx.createBufferSource();
  src.buffer = buf;
  src.connect(ctx.destination);
  const now = ctx.currentTime;
  if (playAt < now) playAt = now;
  src.start(playAt);
  playAt += buf.duration;
  playing.push(src);
  src.onended = () => {
    const i = playing.indexOf(src);
    if (i >= 0) playing.splice(i, 1);
  };
}

function stopPlayback() {
  for (const src of playing) {
    try { src.stop(); } catch (_) { /* already stopped */ }
  }
  playing.length = 0;
  playAt = 0;
}

function downsample(input, inRate, outRate) {
  if (inRate === outRate) return input;
  const ratio = inRate / outRate;
  const outLen = Math.floor(input.length / ratio);
  const out = new Float32Array(outLen);
  for (let i = 0; i < outLen; i++) {
    const pos = i * ratio;
    const i0 = Math.floor(pos);
    const i1 = Math.min(i0 + 1, input.length - 1);
    const f = pos - i0;
    out[i] = input[i0] * (1 - f) + input[i1] * f;
  }
  return out;
}

function f32ToI16(f32) {
  const out = new Int16Array(f32.length);
  for (let i = 0; i < f32.length; i++) {
    let s = f32[i];
    if (s > 1) s = 1;
    if (s < -1) s = -1;
    out[i] = s < 0 ? Math.round(s * 0x8000) : Math.round(s * 0x7fff);
  }
  return out;
}

function pushMic(i16) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  const merged = new Int16Array(pendingMic.length + i16.length);
  merged.set(pendingMic, 0);
  merged.set(i16, pendingMic.length);
  let off = 0;
  while (merged.length - off >= MIC_FRAME) {
    ws.send(encodeMic(merged.subarray(off, off + MIC_FRAME)));
    off += MIC_FRAME;
  }
  pendingMic = merged.slice(off);
}

async function toggleMic() {
  ensureAudio();
  if (mic) {
    mic.stop();
    mic = null;
    pendingMic = new Int16Array(0);
    setState(state === "speaking" ? state : "idle");
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "barge" }));
    }
    return;
  }
  if (state === "speaking" && ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: "barge" }));
    stopPlayback();
  }
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true },
  });
  const ctx = ensureAudio();
  const source = ctx.createMediaStreamSource(stream);
  const proc = ctx.createScriptProcessor(2048, 1, 1);
  const mute = ctx.createGain();
  mute.gain.value = 0;
  proc.onaudioprocess = (ev) => {
    const raw = ev.inputBuffer.getChannelData(0);
    const down = downsample(raw, ctx.sampleRate, MIC_RATE);
    pushMic(f32ToI16(down));
  };
  source.connect(proc);
  proc.connect(mute);
  mute.connect(ctx.destination);
  mic = {
    stop() {
      proc.disconnect();
      source.disconnect();
      mute.disconnect();
      for (const track of stream.getTracks()) track.stop();
    },
  };
  setState("listening");
}

canvas.addEventListener("click", () => { toggleMic().catch((err) => { caption.textContent = String(err.message || err); }); });
listenBtn.addEventListener("click", () => { toggleMic().catch((err) => { caption.textContent = String(err.message || err); }); });

form.addEventListener("submit", (ev) => {
  ev.preventDefault();
  ensureAudio();
  const text = say.value.trim();
  if (!text || !ws || ws.readyState !== WebSocket.OPEN) return;
  ws.send(JSON.stringify({ type: "text", text }));
  say.value = "";
});

connect();
