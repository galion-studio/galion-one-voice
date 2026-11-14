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
};
let tMs = 1000;
let holdDot = false;

const research = {
  failAsk: false,
  failSource: false,
  status: "Found",
  reply: "The wash is amber to violet.",
  sources: [],
  nextId: 1,
};

function resetResearch() {
  research.failAsk = false;
  research.failSource = false;
  research.status = "Found";
  research.reply = "The wash is amber to violet.";
  research.sources = [];
  research.nextId = 1;
}

function sendJson(res, code, obj) {
  const body = JSON.stringify(obj);
  res.writeHead(code, { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" });
  res.end(body);
}

function readBody(req) {
  return new Promise((resolve) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks).toString()));
  });
}

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
  if (req.method === "GET" && (url.pathname === "/research" || url.pathname === "/research/")) {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
    res.end(researchHtml);
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/hermes") {
    sendJson(res, 200, { app: "hermes", status: "Idle", key: "none" });
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/history") {
    sendJson(res, 200, { items: [] });
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/sources") {
    sendJson(res, 200, { sources: research.sources });
    return;
  }
  if (req.method === "POST" && url.pathname === "/__research") {
    readBody(req).then((raw) => {
      const patch = JSON.parse(raw || "{}");
      if (patch.reset) resetResearch();
      if ("failAsk" in patch) research.failAsk = Boolean(patch.failAsk);
      if ("failSource" in patch) research.failSource = Boolean(patch.failSource);
      if (typeof patch.status === "string") research.status = patch.status;
      if (typeof patch.reply === "string") research.reply = patch.reply;
      sendJson(res, 200, { ok: true });
    });
    return;
  }
  if (req.method === "POST" && url.pathname === "/research/sources") {
    readBody(req).then((raw) => {
      if (research.failSource) {
        sendJson(res, 500, { error: "not attached" });
        return;
      }
      let body = {};
      try {
        body = JSON.parse(raw || "{}");
      } catch {
        body = {};
      }
      const name = typeof body.name === "string" && body.name ? body.name : "file";
      const kind = body.kind === "image" ? "image" : "file";
      const text = typeof body.text === "string" ? body.text : "";
      research.sources.push({
        id: "s" + research.nextId,
        name,
        kind,
        chars: kind === "image" ? 0 : text.length,
      });
      research.nextId += 1;
      sendJson(res, 200, { sources: research.sources });
    });
    return;
  }
  if (req.method === "POST" && url.pathname === "/research/ask") {
    readBody(req).then((raw) => {
      if (research.failAsk) {
        sendJson(res, 500, { error: "unreachable" });
        return;
      }
      let question = "";
      try {
        const body = JSON.parse(raw || "{}");
        question = typeof body.question === "string" ? body.question : "";
      } catch {
        question = "";
      }
      sendJson(res, 200, {
        question,
        reply: research.reply,
        status: research.status,
        citations: [],
        found: [research.reply],
        inferred: [],
        unknown: [],
      });
    });
    return;
  }
  res.writeHead(204);
  res.end();
});

server.listen(port, "127.0.0.1");
