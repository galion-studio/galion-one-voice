// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

"use strict";

const D = require("./dot-logic.js");

function assert(cond, msg) {
  if (!cond) {
    console.error("FAIL " + msg);
    process.exit(1);
  }
}

function close(a, b, name) {
  assert(Math.abs(a - b) < 1e-5, name + " " + a + " vs " + b);
}

const rest = D.look(D.RESTING);
assert(rest.breathMs === 1600, "breath");
assert(rest.rgb[0] === 124 && rest.rgb[1] === 58 && rest.rgb[2] === 237, "violet");
close(rest.glow, 0.5, "glow");

const deep = D.look({ arousal: 0.3, valence: -1, dominance: 0.5 });
assert(deep.rgb[0] === 76 && deep.rgb[1] === 29 && deep.rgb[2] === 149, "deep");
assert(D.look({ arousal: 0, valence: 0.25, dominance: 0.5 }).breathMs === 2200, "calm breath");
assert(D.look({ arousal: 1, valence: 0.25, dominance: 0.5 }).breathMs === 800, "fast breath");

const idle0 = D.geometry("idle", 0, 0, D.RESTING, false);
close(idle0.radius, 0.1, "idle r");
close(idle0.brightness, 0.84, "idle b");
const idle400 = D.geometry("idle", 0, 400, D.RESTING, false);
close(idle400.radius, 0.106, "idle 400");
const held = D.geometry("idle", 0, 400, D.RESTING, true);
close(held.radius, 0.1, "reduced");
const listen = D.geometry("listening", 1, 0, D.RESTING, false);
close(listen.radius, 0.124, "listen");
const speak = D.geometry("speaking", 0, 160, D.RESTING, false);
close(speak.radius, 0.1255, "speak");
const env = D.geometry("speaking", 0.5, 0, D.RESTING, false);
close(env.radius, 0.13, "env");

const centre = D.backgroundAt(0.5, 0.5, 0);
assert(centre.r === 77 && centre.g === 49 && centre.b === 60, "wash centre " + JSON.stringify(centre));
const corner = D.backgroundAt(0, 0, 0);
assert(corner.r === 10 && corner.g === 10 && corner.b === 12, "wash corner");

const mood0 = D.previewMood(0);
close(mood0.arousal, 0.3, "mood a");
close(mood0.valence, 0.25, "mood v");
close(mood0.dominance, 0.5, "mood d");

const stepped = D.spring(0.1, 0, 0.2, 1000 / 60, 1.8, 0.92);
assert(stepped[0] > 0.1 && stepped[0] < 0.2, "spring");

const packed = D.packPcm(Int16Array.of(-2), 1);
const hex = Array.from(packed).map(function (b) { return b.toString(16).padStart(2, "0"); }).join("");
assert(hex === "4753010001000100feff", hex);
const parsed = D.parsePcm(packed);
assert(parsed.sampleRate === 16000 && parsed.seq === 1 && parsed.samples[0] === -2, "parse pcm");

const ev = D.parseEvent('{"schema":"galion-samantha-ws/1","type":"turn","reply":"Hi. I\'m here.","intent":null,"arm":null,"sources":["fake:reply-engine"],"mood":{"arousal":0.3,"valence":0.25,"dominance":0.5}}');
assert(ev && ev.type === "turn" && ev.reply === "Hi. I'm here.", "event");
assert(D.parseEvent('{"schema":"nope","type":"pong"}') === null, "bad schema");

const helloMsg = JSON.parse(D.helloJson());
assert(helloMsg.schema === D.SCHEMA && helloMsg.type === "hello" && helloMsg.session === null, "hello");
assert(JSON.parse(D.stopJson()).type === "stop", "stop");
assert(D.allowedWs("ws://127.0.0.1:7881/v1/samantha") === true, "ws ok");
assert(D.allowedWs("ws://localhost:7881/v1/samantha") === true, "ws local");
assert(D.allowedWs("wss://example.com/v1/samantha") === false, "ws remote");

function dotOf(list) {
  for (let i = 0; i < list.length; i++) if (list[i].type === "dot") return list[i];
  return null;
}
function turnOf(list) {
  for (let i = 0; i < list.length; i++) if (list[i].type === "turn") return list[i];
  return null;
}

const eng = D.createEngine("mock");
const hello = eng.hello();
assert(hello[0].type === "welcome", "hello");
const started = eng.start();
assert(dotOf(started).state === "listening", "talk");
for (let i = 1; i <= 19; i++) {
  const tick = eng.onTick();
  assert(dotOf(tick).state === "listening", "tick " + i + " " + dotOf(tick).state);
}
assert(dotOf(eng.onTick()).state === "thinking", "tick 20");
for (let i = 21; i <= 27; i++) {
  const tick = eng.onTick();
  assert(dotOf(tick).state === "thinking", "tick " + i);
}
const spoke = eng.onTick();
assert(dotOf(spoke).state === "speaking", "tick 28");
assert(turnOf(spoke) && turnOf(spoke).reply === "Hi. I'm here.", "greeting");
assert(turnOf(spoke).sources[0] === "fake:reply-engine", "source");

const quiet = D.createEngine("passive");
quiet.start();
for (let i = 0; i < 40; i++) {
  const tick = quiet.onTick();
  assert(!turnOf(tick), "passive caption");
  const dot = dotOf(tick);
  if (dot) assert(dot.state !== "speaking", "passive spoke");
}

console.log("selfcheck ok");
