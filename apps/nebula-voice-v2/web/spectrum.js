import { cie } from "./cie.js?v=abde48e";
import { DEMO, UNIVERSAL } from "./lines.js?v=abde48e";
import init, { preview_frame } from "./pkg/nebula_voice_v2.js?v=abde48e";

const strip = document.getElementById("spectrum");
function drawSpectrum() {
  const w = (strip.width = strip.clientWidth * Math.min(devicePixelRatio || 1, 2));
  const h = (strip.height = strip.clientHeight * Math.min(devicePixelRatio || 1, 2));
  const ctx = strip.getContext("2d");
  for (let x = 0; x < w; x++) {
    const nm = 380 + (x / (w - 1)) * (720 - 380);
    ctx.fillStyle = cie(nm);
    ctx.fillRect(x, 0, 1, h);
  }
  for (const row of DEMO) {
    const x = ((row.nm - 380) / (720 - 380)) * w;
    ctx.strokeStyle = "#fff";
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, h);
    ctx.stroke();
  }
}

const cards = document.getElementById("cards");
for (const row of DEMO) {
  const card = document.createElement("article");
  card.className = "card";
  const who = row.samantha_only ? `${row.name} (Samantha only)` : row.name;
  card.innerHTML = `
    <h2>${who}</h2>
    <div class="meta">${row.gas} · ${row.nm} nm · ${row.evidence}</div>
    <div class="swatches">
      <div class="swatch" style="background:${row.spectral}" title="spectral ${row.spectral}"></div>
      <div class="swatch" style="background:${row.display}" title="display ${row.display}"></div>
    </div>
    <div class="note">${row.physics}</div>`;
  cards.appendChild(card);
}

const body = document.querySelector("#universal tbody");
for (const row of UNIVERSAL) {
  const tr = document.createElement("tr");
  tr.innerHTML = `<td>${row.name}</td><td>${row.gas}</td><td>${row.nm}</td><td><span class="swatch" style="background:${row.spectral};display:inline-block"></span> ${row.spectral}</td>`;
  body.appendChild(tr);
}

drawSpectrum();
window.addEventListener("resize", drawSpectrum);

const sam = document.getElementById("preview-samantha");
const jar = document.getElementById("preview-jarvis");
await init();
function tick() {
  preview_frame("samantha", sam);
  preview_frame("jarvis", jar);
  requestAnimationFrame(tick);
}
requestAnimationFrame(tick);
window.__spectrumReady = true;
