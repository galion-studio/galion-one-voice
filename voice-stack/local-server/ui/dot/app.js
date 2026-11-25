// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

// One dot. No caption, no second control. Tap the mark to listen.
// The socket is galion-samantha-ws/1.

const SCHEMA = "galion-samantha-ws/1";
const HEADER = 8;
const MIC_RATE = 24000;
const MIC_FRAME = 480;

const canvas = document.getElementById("s");
const ctx2d = canvas.getContext("2d");
let level = 0;
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
  ctx2d.fillStyle = "#0A0A0C";
  ctx2d.fillRect(0, 0, w, h);
  const radius = Math.min(w, h) * (0.07 + Math.min(1, level) * 0.03);
  ctx2d.beginPath();
  ctx2d.arc(w / 2, h / 2, radius, 0, Math.PI * 2);
  ctx2d.fillStyle = "#F5F5F7";
  ctx2d.fill();
  requestAnimationFrame(draw);
}
draw();

function connect() {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
  const url = (location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/v1/samantha";
  ws = new WebSocket(url);
  ws.binaryType = "arraybuffer";
  ws.onopen = () => {
    ws.send(JSON.stringify({ schema: SCHEMA, type: "hello", session: null }));
  };
  ws.onmessage = (ev) => {
    if (typeof ev.data === "string") onJson(ev.data);
    else onBinary(ev.data);
  };
  ws.onclose = () => {
    ws = null;
    setTimeout(connect, 1000);
  };
}

function onJson(text) {
  let msg;
  try { msg = JSON.parse(text); } catch (_) { return; }
  if (msg.type === "vad" && msg.event === "start") level = Math.max(level, 0.55);
  if (msg.type === "vad" && msg.event === "end") level = Math.max(0.12, level * 0.6);
  if (msg.type === "dot" && msg.state === "listening") level = Math.max(level, 0.5);
  if (msg.type === "dot" && msg.state === "speaking") level = Math.max(level, 0.7);
  if (msg.type === "dot" && typeof msg.level === "number") level = Math.max(level * 0.5, msg.level);
  if (msg.type === "dot" && msg.state === "idle") stopPlayback();
  if (msg.type === "mood" && msg.pad && typeof msg.pad.arousal === "number") {
    level = Math.max(level * 0.9, msg.pad.arousal * 0.15);
  }
  if (msg.type === "inner") {
    const node = document.getElementById("inner");
    if (!node) return;
    const slots = Array.isArray(msg.slots) ? msg.slots.join(" ") : "";
    const surprise = typeof msg.surprise === "number" ? String(msg.surprise) : "";
    const parts = [msg.verb, msg.lean, surprise, slots].filter((part) => part);
    node.textContent = parts.join(" ");
  }
}

function onBinary(buf) {
  const frame = decodeFrame(buf);
  if (!frame || frame.dir !== 1) return;
  playPcm(frame.pcm, frame.rate);
}

function decodeFrame(buf) {
  if (!(buf instanceof ArrayBuffer) || buf.byteLength < HEADER) return null;
  const view = new DataView(buf);
  if (view.getUint8(0) !== 0x47 || view.getUint8(1) !== 0x53 || view.getUint8(2) !== 1) return null;
  const dir = view.getUint8(3);
  const rate = view.getUint8(4) === 0 ? 24000 : 16000;
  const n = (buf.byteLength - HEADER) / 2;
  const pcm = new Int16Array(n);
  for (let i = 0; i < n; i++) pcm[i] = view.getInt16(HEADER + i * 2, true);
  return { dir, rate, pcm };
}

function encodeMic(pcm) {
  const buf = new ArrayBuffer(HEADER + pcm.length * 2);
  const view = new DataView(buf);
  view.setUint8(0, 0x47);
  view.setUint8(1, 0x53);
  view.setUint8(2, 1);
  view.setUint8(3, 0);
  view.setUint8(4, 0);
  view.setUint8(5, 0);
  view.setUint16(6, seq & 0xffff, true);
  seq = (seq + 1) & 0xffff;
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
    try { src.stop(); } catch (_) { /* already ended */ }
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
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ schema: SCHEMA, type: "stop" }));
    }
    stopPlayback();
    return;
  }
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true },
  });
  const ctx = ensureAudio();
  const source = ctx.createMediaStreamSource(stream);
  const mute = ctx.createGain();
  mute.gain.value = 0;
  await ctx.audioWorklet.addModule("/dot/capture-worklet.js");
  const node = new AudioWorkletNode(ctx, "galion-mic");
  node.port.onmessage = (ev) => {
    const raw = ev.data;
    if (!(raw instanceof Float32Array)) return;
    pushMic(f32ToI16(downsample(raw, ctx.sampleRate, MIC_RATE)));
  };
  source.connect(node);
  node.connect(mute);
  mute.connect(ctx.destination);
  mic = {
    stop() {
      node.disconnect();
      source.disconnect();
      mute.disconnect();
      for (const track of stream.getTracks()) track.stop();
    },
  };
}

canvas.addEventListener("click", () => {
  toggleMic().catch(() => {});
});

connect();
