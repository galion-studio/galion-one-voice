import { cie as cieHex } from "./cie.js?v=abde48e";
import { createGpuRenderer, probeGpu } from "./gpu.js?v=abde48e";

// Gesture glue for the fullscreen nebula. No visible controls.
const params = new URLSearchParams(location.search);
const voice = document.body.dataset.voice || params.get("voice") || "samantha";
const qualityOverride = params.get("quality");
const rendererOverride = (params.get("renderer") || "").toLowerCase();
const showHud = params.get("fps") === "1" || params.get("fps") === "true";

const overlay = document.getElementById("overlay");
const strip = document.getElementById("strip");
const labels = document.getElementById("labels");
let canvas = document.getElementById("nebula");
const hud = document.getElementById("hud");

function refreshHz() {
  return new Promise((resolve) => {
    let last = performance.now();
    const samples = [];
    function tick(now) {
      const dt = now - last;
      last = now;
      if (dt > 2 && dt < 80) samples.push(1000 / dt);
      if (samples.length < 20) requestAnimationFrame(tick);
      else {
        samples.sort((a, b) => a - b);
        resolve(samples[Math.floor(samples.length / 2)]);
      }
    }
    requestAnimationFrame(tick);
  });
}

function safeInsets() {
  const probe = document.getElementById("safe");
  const r = probe.getBoundingClientRect();
  return {
    left: r.left,
    top: r.top,
    right: Math.max(0, window.innerWidth - r.right),
    bottom: Math.max(0, window.innerHeight - r.bottom),
  };
}

function chooseDpr(gpu) {
  const mobile = Math.min(window.innerWidth, window.innerHeight) < 800;
  const cap = gpu ? 3 : (mobile ? 1.5 : 2);
  let dpr = Math.min(window.devicePixelRatio || 1, cap);
  const cssW = canvas.clientWidth || window.innerWidth;
  const cssH = canvas.clientHeight || window.innerHeight;
  // CPU splat stays at the iPhone 14 / 1.5 budget. GPU can fill a 3× phone.
  const maxPixels = gpu ? 390 * 844 * 3 * 3 : 390 * 844 * 1.5 * 1.5;
  const area = Math.max(1, cssW * cssH);
  if (area * dpr * dpr > maxPixels) {
    dpr = Math.max(1, Math.sqrt(maxPixels / area));
  }
  return dpr;
}

function fit(gpu) {
  const dpr = chooseDpr(gpu);
  canvas.style.width = "100%";
  canvas.style.height = "100%";
  const cssW = canvas.clientWidth || window.innerWidth;
  const cssH = canvas.clientHeight || window.innerHeight;
  const w = Math.max(2, Math.round(cssW * dpr));
  const h = Math.max(2, Math.round(cssH * dpr));
  if (canvas.width !== w) canvas.width = w;
  if (canvas.height !== h) canvas.height = h;
  canvas.dataset.dpr = String(dpr);
  return { dpr, cssW, cssH, width: w, height: h };
}

let pointers = new Map();
let pinch = 0;
let moved = 0;
let lastTap = 0;
let clickTimer = 0;

function toggleOverlay(app) {
  const on = app.toggle_overlay();
  overlay.hidden = !on;
  if (on) drawStrip(app);
}

function drawStrip(app) {
  const rows = JSON.parse(app.dominant_json());
  const ctx = strip.getContext("2d");
  const w = (strip.width = strip.clientWidth * Math.min(devicePixelRatio || 1, 2));
  const h = (strip.height = 36 * Math.min(devicePixelRatio || 1, 2));
  ctx.clearRect(0, 0, w, h);
  for (let x = 0; x < w; x++) {
    const nm = 380 + (x / w) * (720 - 380);
    ctx.fillStyle = cie(nm);
    ctx.fillRect(x, 0, 1, h);
  }
  labels.textContent = "";
  for (const row of rows) {
    const x = ((row.nm - 380) / (720 - 380)) * strip.clientWidth;
    ctx.fillStyle = "#fff";
    ctx.fillRect((x / strip.clientWidth) * w - 1, 0, 2, h);
    const mark = document.createElement("span");
    mark.style.left = `${x}px`;
    mark.textContent = `${row.emotion}  ${row.gas}  ${row.nm} nm`;
    labels.appendChild(mark);
    const half = mark.offsetWidth / 2;
    const inset = half + 4;
    const max = Math.max(inset, strip.clientWidth - inset);
    mark.style.left = `${Math.min(Math.max(x, inset), max)}px`;
  }
}

function cie(nm) { return cieHex(nm); }

function spriteView(mod, app, count) {
  const mem = mod.wasm_memory();
  return new Float32Array(mem.buffer, app.sprites_ptr(), count * 8);
}

// The bridge writes one JSON object per line on loopback TCP.
// The page speaks that same object as a WebSocket text frame.
// Backoff matches `nebula listen`: 80, 160, 320, 640, then 1280 ms.
// A drop clears the emotion. It does not keep the last simplex.
function attachFeed(app) {
  const backoff = [80, 160, 320, 640, 1280];
  let attempt = 0;
  const raw = params.get("feed") || "127.0.0.1:8766";
  const url = raw.startsWith("ws://") || raw.startsWith("wss://") ? raw : "ws://" + raw;
  function fail() {
    app.clear_emotion();
    canvas.dataset.schema = "galion-voice-v2-emotion-feed/1";
    canvas.dataset.estimate = "none";
    const wait = backoff[Math.min(attempt, backoff.length - 1)];
    attempt += 1;
    setTimeout(connect, wait);
  }
  function connect() {
    let ws;
    try { ws = new WebSocket(url); }
    catch (e) { fail(); return; }
    ws.onopen = () => { attempt = 0; };
    ws.onmessage = (ev) => {
      const text = String(ev.data || "").replace(/\n$/, "").trim();
      if (!text) return;
      let msg = null;
      try { msg = JSON.parse(text); }
      catch (e) { app.clear_emotion(); return; }
      if (!msg || typeof msg.schema !== "string" || !msg.schema) {
        app.clear_emotion();
        return;
      }
      canvas.dataset.schema = msg.schema;
      canvas.dataset.estimate = typeof msg.estimate === "string" ? msg.estimate : "";
      app.apply_line(text);
    };
    ws.onerror = () => { try { ws.close(); } catch (e) {} };
    ws.onclose = () => fail();
  }
  connect();
}

export async function start(mod) {
  const hz = await refreshHz();
  const probe = rendererOverride === "cpu"
    ? { name: "", strong: false, discrete: false }
    : await probeGpu();
  const quality = qualityOverride || (probe.strong ? "high" : "normal");
  let prefer = rendererOverride;
  if (!prefer && /swiftshader|llvmpipe|softpipe|microsoft basic render/.test((probe.name || "").toLowerCase())) {
    prefer = "webgl2";
  }
  let gpu = rendererOverride === "cpu"
    ? null
    : await createGpuRenderer(canvas, prefer);
  if (gpu && gpu.canvas !== canvas) {
    canvas = gpu.canvas;
  }
  canvas = document.getElementById("nebula");
  const usingGpu = !!gpu;
  fit(usingGpu);
  const app = new mod.Nebula(
    voice,
    quality,
    innerWidth,
    innerHeight,
    hz,
    !!probe.discrete,
  );
  if (params.get("demo") === "1") app.set_demo(true);
  else attachFeed(app);
  const floor = JSON.parse(app.floor_rgb());
  const clear = [floor.r, floor.g, floor.b];
  let rendererName = usingGpu ? gpu.api : "cpu";
  let measuredFps = 0;
  let qualityNow = quality;
  const warmup = [];
  let warmed = !usingGpu || quality !== "high" || !!qualityOverride;
  const hudOn = showHud;
  if (hud && hudOn) hud.hidden = false;

  const snapshot = () => {
    const state = JSON.parse(app.debug());
    return {
      ...state,
      quality: qualityNow,
      renderer: rendererName,
      gpu: probe.name || "",
      canvas: [canvas.width, canvas.height],
      dpr: Number(canvas.dataset.dpr || 1),
      fpsMeasured: measuredFps,
    };
  };
  window.__nebula = {
    ready: true,
    state: snapshot,
  };

  canvas.addEventListener("pointerdown", (e) => {
    canvas.setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 3) toggleOverlay(app);
    moved = 0;
  });
  canvas.addEventListener("pointermove", (e) => {
    const prev = pointers.get(e.pointerId);
    if (!prev) return;
    const dx = e.clientX - prev.x;
    const dy = e.clientY - prev.y;
    prev.x = e.clientX;
    prev.y = e.clientY;
    moved += Math.hypot(dx, dy);
    if (pointers.size === 1) app.orbit(dx, dy);
    if (pointers.size === 2) {
      const pts = [...pointers.values()];
      const dist = Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y);
      if (pinch > 0) app.zoom(pinch / dist);
      pinch = dist;
    }
  });
  function up(e) {
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinch = 0;
    if (e.type === "pointerup" && moved < 8 && pointers.size === 0) {
      const now = performance.now();
      if (now - lastTap < 280) {
        clearTimeout(clickTimer);
        lastTap = 0;
        if (document.fullscreenElement) document.exitFullscreen();
        else canvas.requestFullscreen?.();
      } else {
        lastTap = now;
        clickTimer = setTimeout(() => app.cycle(), 280);
      }
    }
  }
  canvas.addEventListener("pointerup", up);
  canvas.addEventListener("pointercancel", up);
  canvas.addEventListener("wheel", (e) => {
    e.preventDefault();
    app.zoom(Math.exp(e.deltaY * 0.001));
  }, { passive: false });
  canvas.addEventListener("dblclick", (e) => e.preventDefault());
  window.addEventListener("keydown", (e) => {
    if (e.key === "s" || e.key === "S") toggleOverlay(app);
    if (e.key === "f" || e.key === "F") {
      if (document.fullscreenElement) document.exitFullscreen();
      else canvas.requestFullscreen?.();
    }
  });
  window.addEventListener("resize", () => {
    fit(usingGpu);
    if (!overlay.hidden) drawStrip(app);
  });

  let last = performance.now();
  let fpsEma = 0;
  function tick(now) {
    const dt = now - last;
    last = now;
    if (dt > 0 && dt < 250) {
      const inst = 1000 / dt;
      fpsEma = fpsEma ? fpsEma * 0.9 + inst * 0.1 : inst;
      measuredFps = fpsEma;
    }
    const state = JSON.parse(app.debug());
    const minDt = 1000 / Math.max(1, state.fps);
    if (state.fps <= 30 && dt < minDt * 0.92) {
      requestAnimationFrame(tick);
      return;
    }
    if (usingGpu && gpu) {
      const sized = fit(true);
      app.step(dt);
      const count = app.pack_sprites(sized.cssW, sized.cssH, sized.dpr);
      if (count > 0) {
        gpu.draw(spriteView(mod, app, count), count, sized.width, sized.height, clear);
      }
    } else {
      fit(false);
      app.frame(dt, canvas);
    }
    if (!overlay.hidden) drawStrip(app);
    if (!warmed) {
      if (dt > 2 && dt < 80) warmup.push(dt);
      const elapsed = warmup.reduce((a, b) => a + b, 0);
      if (elapsed >= 1500 && warmup.length >= 20) {
        warmed = true;
        const sorted = warmup.slice().sort((a, b) => a - b);
        const median = sorted[Math.floor(sorted.length / 2)];
        if (median > minDt * 1.35) {
          app.apply_quality("normal");
          qualityNow = "normal";
        }
      }
    }
    if (hud && hudOn) {
      const snap = snapshot();
      hud.textContent = [
        `${snap.fpsMeasured.toFixed(1)} fps`,
        `tier ${String(snap.mode).toLowerCase()}  target ${snap.fps}`,
        `${snap.renderer}  ${snap.particles} p`,
        `${snap.canvas[0]}×${snap.canvas[1]}  dpr ${snap.dpr.toFixed(2)}`,
        snap.gpu || "",
      ].filter(Boolean).join("\n");
    }
    const _insets = safeInsets();
    requestAnimationFrame(tick);
    void _insets;
  }
  requestAnimationFrame(tick);
}
