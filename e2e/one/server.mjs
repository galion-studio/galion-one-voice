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
  delayMs: 0,
  fail: false,
  status: "Found",
  reply: "The wash is amber to violet.",
  citations: [{ n: 1, text: "wash-note.txt" }],
  found: ["The wash is amber to violet."],
  sources: [],
  nextId: 1,
  notes: [],
};

function readBody(req) {
  return new Promise((resolve) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks).toString()));
  });
}

function sendJson(res, code, body) {
  const raw = typeof body === "string" ? body : JSON.stringify(body);
  res.writeHead(code, { "content-type": "application/json", "cache-control": "no-store" });
  res.end(raw);
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
    sendJson(res, 200, { schema: "galion-research/1", app: "hermes", status: "Idle" });
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/history") {
    sendJson(res, 200, { schema: "galion-research/1", app: "hermes", items: research.notes.slice().reverse() });
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/sources") {
    sendJson(res, 200, { sources: research.sources });
    return;
  }
  if (req.method === "POST" && url.pathname === "/__research") {
    readBody(req).then((raw) => {
      const patch = JSON.parse(raw || "{}");
      if (patch.reset) {
        research.sources = [];
        research.nextId = 1;
        research.delayMs = 0;
        research.fail = false;
        research.status = "Found";
        research.reply = "The wash is amber to violet.";
        research.citations = [{ n: 1, text: "wash-note.txt" }];
        research.found = ["The wash is amber to violet."];
        research.notes = [];
      }
      if ("delayMs" in patch) research.delayMs = Number(patch.delayMs) || 0;
      if ("fail" in patch) research.fail = Boolean(patch.fail);
      if ("status" in patch) research.status = patch.status;
      if ("reply" in patch) research.reply = patch.reply;
      if ("citations" in patch) research.citations = patch.citations;
      if ("found" in patch) research.found = patch.found;
      res.writeHead(204);
      res.end();
    });
    return;
  }
  if (req.method === "POST" && url.pathname === "/research/sources") {
    readBody(req).then((raw) => {
      const body = JSON.parse(raw || "{}");
      const name = typeof body.name === "string" ? body.name.split("/").pop() : "";
      if (!name) {
        sendJson(res, 400, { error: "name" });
        return;
      }
      research.sources.push({
        id: research.nextId,
        name,
        kind: body.kind === "image" ? "image" : "file",
        chars: typeof body.text === "string" ? body.text.length : 0,
      });
      research.nextId += 1;
      sendJson(res, 200, { sources: research.sources });
    });
    return;
  }
  if (req.method === "POST" && url.pathname === "/research/ask") {
    readBody(req).then((raw) => {
      const body = JSON.parse(raw || "{}");
      const question = typeof body.question === "string" ? body.question : "";
      const delay = research.delayMs;
      const fail = research.fail;
      const payload = {
        schema: "galion-research/1",
        app: "hermes",
        question,
        status: research.status,
        reply: research.reply,
        citations: research.citations,
        found: research.found,
        inferred: [],
        unknown: [],
      };
      setTimeout(() => {
        if (fail) {
          sendJson(res, 500, { ok: false });
          return;
        }
        research.notes.push({ status: payload.status, question });
        sendJson(res, 200, payload);
      }, delay);
    });
    return;
  }
  res.writeHead(204);
  res.end();
});

server.listen(port, "127.0.0.1");
