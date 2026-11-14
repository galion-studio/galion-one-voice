// Serves the real /one page and a controllable /dot for desktop Playwright.
// Tones stay in the page. This file does not synthesize audio.
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const html = readFileSync(join(root, "crates/galion-console/assets/v1-screen.html"));
const researchHtml = readFileSync(join(root, "crates/galion-console/assets/research.html"));
const vocab = readFileSync(join(root, "crates/galion-console/assets/nebula/vocabulary.mjs"));

const neutral = {
  schema: "galion-nebula/1",
  voice: "galion",
  timestamp: 0,
  vad: 0,
  energy: 0,
  valence: 0,
  arousal: 0.1,
  dominance: 0.2,
  weights: [{ emotion: "neutral", weight: 1, element: "HeII-541.2" }],
};

const state = {
  mode: "idle",
  emotion: { sense: true, label: "neutral", valence: 0, arousal: 0.1, lean: 0, confidence: 1 },
  nebula: neutral,
  persona: "galion",
  approval: false,
  researchFail: false,
  sources: [],
};
let tMs = 1000;
let holdDot = false;

function dotBody() {
  tMs += 80;
  const body = {
    t_ms: tMs,
    radius: state.mode === "listening" ? 0.135 : state.mode === "speaking" ? 0.105 : 0.1,
    brightness: state.mode === "listening" ? 0.72 : 0.48,
    mode: state.mode,
    state: state.mode,
    lean: 0,
    level: state.mode === "listening" ? 0.2 : 0,
    e: 0,
    engine: { hz: 240, cost: 48 },
    emotion: state.emotion,
    nebula: state.nebula,
    persona: state.persona,
  };
  if (state.approval) body.approval = true;
  return JSON.stringify(body);
}

const port = Number(process.env.PORT || 4173);
const server = createServer((req, res) => {
  const url = new URL(req.url || "/", "http://127.0.0.1");
  if (req.method === "POST" && (url.pathname === "/__hold" || url.pathname === "/__release")) {
    holdDot = url.pathname === "/__hold";
    res.writeHead(204);
    res.end();
    return;
  }
  if (req.method === "POST" && url.pathname === "/__state") {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => {
      const patch = JSON.parse(Buffer.concat(chunks).toString() || "{}");
      if (patch.mode) state.mode = patch.mode;
      if (patch.emotion) state.emotion = patch.emotion;
      if (patch.nebula) state.nebula = patch.nebula;
      if ("approval" in patch) state.approval = Boolean(patch.approval);
      if ("researchFail" in patch) state.researchFail = Boolean(patch.researchFail);
      if (patch.resetSources) state.sources = [];
      if (patch.persona) state.persona = patch.persona;
      res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
      res.end('{"ok":true}');
    });
    return;
  }
  if (req.method === "GET" && (url.pathname === "/one" || url.pathname === "/one/")) {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
    res.end(html);
    return;
  }
  if (req.method === "GET" && (url.pathname === "/research" || url.pathname === "/research/")) {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
    res.end(researchHtml);
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/hermes") {
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end('{"schema":"galion-research/1","app":"hermes","status":"Idle"}');
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/history") {
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end('{"schema":"galion-research/1","app":"hermes","items":[]}');
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/sources") {
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end(JSON.stringify({ schema: "galion-research/1", sources: state.sources }));
    return;
  }
  if (req.method === "POST" && (url.pathname === "/research/ask" || url.pathname === "/research/sources")) {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => {
      if (state.researchFail) {
        res.writeHead(500, { "content-type": "application/json", "cache-control": "no-store" });
        res.end('{"ok":false}');
        return;
      }
      let posted = {};
      try {
        posted = JSON.parse(Buffer.concat(chunks).toString() || "{}");
      } catch (err) {
        posted = {};
      }
      if (url.pathname === "/research/sources") {
        const text = typeof posted.text === "string" ? posted.text : "";
        state.sources.push({
          id: "s" + (state.sources.length + 1),
          name: posted.name || "file",
          kind: posted.kind === "image" ? "image" : "file",
          chars: text.length,
        });
        res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
        res.end(JSON.stringify({ schema: "galion-research/1", sources: state.sources }));
        return;
      }
      const question = typeof posted.question === "string" ? posted.question : "";
      res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
      res.end(JSON.stringify({
        schema: "galion-research/1",
        app: "hermes",
        question,
        reply: "The wash is amber.",
        status: "Found",
        citations: [],
        found: ["The wash is amber."],
        inferred: [],
        unknown: [],
      }));
    });
    return;
  }
  if (req.method === "GET" && url.pathname === "/nebula/vocabulary.mjs") {
    res.writeHead(200, { "content-type": "text/javascript; charset=utf-8", "cache-control": "no-store" });
    res.end(vocab);
    return;
  }
  if (req.method === "GET" && url.pathname === "/dot") {
    if (holdDot) {
      res.writeHead(204);
      res.end();
      return;
    }
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end(dotBody());
    return;
  }
  if (req.method === "POST" && url.pathname === "/session") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end('{"token":"desktop-e2e"}');
    return;
  }
  res.writeHead(204);
  res.end();
});

server.listen(port, "127.0.0.1");
