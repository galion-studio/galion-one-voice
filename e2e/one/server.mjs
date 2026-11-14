// Serves the real /one page and a controllable /dot for desktop Playwright.
// Tones stay in the page. This file does not synthesize audio.
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const html = readFileSync(join(root, "crates/galion-console/assets/v1-screen.html"));
const research = readFileSync(join(root, "crates/galion-console/assets/research.html"));
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
  if (req.method === "GET" && (url.pathname === "/research" || url.pathname === "/research/")) {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
    res.end(research);
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/hermes") {
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end('{"app":"hermes","status":"Idle","key":"none"}');
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/history") {
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end('{"items":[]}');
    return;
  }
  if (req.method === "GET" && url.pathname === "/research/sources") {
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end('{"sources":[]}');
    return;
  }
  if (req.method === "POST" && (url.pathname === "/research/ask" || url.pathname === "/research/sources")) {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => {
      let body = {};
      try {
        body = JSON.parse(Buffer.concat(chunks).toString() || "{}");
      } catch {
        body = {};
      }
      if (url.pathname === "/research/sources") {
        const name = typeof body.name === "string" && body.name ? body.name : "note.txt";
        const kind = body.kind === "image" ? "image" : "file";
        res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
        res.end(JSON.stringify({ sources: [{ id: "s1", name, kind, chars: 18 }] }));
        return;
      }
      const question = typeof body.question === "string" ? body.question : "";
      let status = "Unknown";
      if (/infer/i.test(question)) status = "Inferred";
      else if (/found/i.test(question)) status = "Found";
      const line =
        status === "Found"
          ? "The note is in hand."
          : status === "Inferred"
            ? "A cooler mix would read as night."
            : "The hex is not in the note.";
      res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
      res.end(
        JSON.stringify({
          question,
          reply: line,
          citations: [],
          found: status === "Found" ? [line] : [],
          inferred: status === "Inferred" ? [line] : [],
          unknown: status === "Unknown" ? [line] : [],
          status,
        }),
      );
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
