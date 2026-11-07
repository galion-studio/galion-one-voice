import { cie as cieHex } from "./cie.js";

// Gesture glue for the fullscreen nebula. No visible controls.
const params = new URLSearchParams(location.search);
const voice = document.body.dataset.voice || params.get("voice") || "samantha";
const quality = params.get("quality") || "normal";

const canvas = document.getElementById("nebula");
const overlay = document.getElementById("overlay");
const strip = document.getElementById("strip");
const labels = document.getElementById("labels");

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

function fit() {
  const mobile = Math.min(window.innerWidth, window.innerHeight) < 800;
  const cap = mobile ? 1.5 : 2;
  const dpr = Math.min(window.devicePixelRatio || 1, cap);
  canvas.style.width = "100vw";
  canvas.style.height = "100dvh";
  return dpr;
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
  }
}

// CIE 1931 approximate locus used for the overlay strip. The presentation
// page uses the same function. It is not a CSS hue gradient.
function cie(nm) { return cieHex(nm); }

export async function start(mod) {
  fit();
  const hz = await refreshHz();
  const renderer = (navigator.gpu && "discrete") || "";
  const discrete = /nvidia|radeon rx|geforce|rtx|arc\(tm\)/i.test(renderer);
  const app = new mod.Nebula(voice, quality, innerWidth, innerHeight, hz, discrete);
  window.__nebula = {
    ready: true,
    state: () => JSON.parse(app.debug()),
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
  window.addEventListener("resize", () => { fit(); if (!overlay.hidden) drawStrip(app); });
  let last = performance.now();
  function tick(now) {
    const dt = now - last;
    last = now;
    const target = JSON.parse(app.debug()).fps;
    if (quality === "pi" && dt < 1000 / 30) {
      requestAnimationFrame(tick);
      return;
    }
    if (quality === "high" && dt < 1000 / 120) {
      requestAnimationFrame(tick);
      return;
    }
    app.frame(dt, canvas);
    if (!overlay.hidden) drawStrip(app);
    const _insets = safeInsets();
    requestAnimationFrame(tick);
    void target;
  }
  requestAnimationFrame(tick);
}
