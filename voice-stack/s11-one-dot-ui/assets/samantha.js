// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

// The page. Motion is the Luti spring from v1-screen.html.
// The microphone is opened only from onTalk.
(function () {
  "use strict";

  const D = window.SamanthaDot;
  const params = new URLSearchParams(location.search);
  const fileMode = location.protocol === "file:";
  const mock = fileMode || params.get("mock") === "1";
  const talk = document.getElementById("talk");
  const stopBtn = document.getElementById("stop");
  const captionsBtn = document.getElementById("captions");
  const panel = document.getElementById("caption-panel");
  const captionEl = document.getElementById("caption");
  const sourcesEl = document.getElementById("sources");
  const live = document.getElementById("live");
  const notice = document.getElementById("notice");
  const canvas = document.getElementById("field");
  const ctx = canvas.getContext("2d", { alpha: false });
  const motionMq = window.matchMedia("(prefers-reduced-motion: reduce)");

  const bg = document.createElement("canvas");
  bg.width = 96;
  bg.height = 54;
  const bx = bg.getContext("2d", { alpha: false });
  const img = bx.createImageData(96, 54);

  let engine = mock ? D.createEngine("mock") : null;
  let ws = null;
  let link = mock ? "mock" : "starting";
  let serverState = "idle";
  let mood = D.RESTING;
  let level = 0;
  let viewLevel = 0;
  let playLevel = 0;
  let session = false;
  let seenActive = false;
  let optimistic = false;
  let arming = false;
  let talkGen = 0;
  let captionsOn = false;
  let lastCaption = null;
  let gotPcm = false;
  let mic = null;
  let audioCtx = null;
  let playAt = 0;
  let playEnd = 0;
  let playNodes = [];
  let seq = 0;
  let gumCount = 0;
  let attempt = 0;
  const backoff = [80, 160, 320, 640, 1280];

  let rP = D.IDLE_RADIUS;
  let rV = 0;
  let bP = 0.84;
  let bV = 0;
  let wS = D.LUTI.idle.w;
  let zS = D.LUTI.idle.z;
  let trLp = D.IDLE_RADIUS;
  let tbLp = 0.84;
  let lastDraw = performance.now();
  const t0 = performance.now();
  const frameMs = [];
  let frames = 0;

  function reduced() {
    return document.body.dataset.motion === "reduced";
  }

  function applyMotion() {
    document.body.dataset.motion = motionMq.matches ? "reduced" : "full";
  }
  applyMotion();
  if (motionMq.addEventListener) motionMq.addEventListener("change", applyMotion);
  document.body.dataset.link = link;
  document.body.dataset.mic = "off";

  function setNotice(text) {
    notice.textContent = text || "";
  }

  function setState(state) {
    serverState = state || "idle";
    document.body.dataset.state = serverState;
    talk.setAttribute("aria-pressed", session ? "true" : "false");
    const labels = {
      idle: "Talk to Samantha",
      listening: "Samantha is listening",
      thinking: "Samantha is thinking",
      speaking: "Samantha is speaking"
    };
    talk.setAttribute("aria-label", labels[serverState] || labels.idle);
    const names = { idle: "Idle", listening: "Listening", thinking: "Thinking", speaking: "Speaking" };
    const said = names[serverState] || serverState;
    if (live.textContent !== said) live.textContent = said;
  }

  function renderCaption() {
    if (!captionsOn || !lastCaption) {
      panel.hidden = true;
      captionEl.textContent = "";
      sourcesEl.replaceChildren();
      return;
    }
    panel.hidden = false;
    captionEl.textContent = lastCaption.text || "";
    sourcesEl.replaceChildren();
    const list = lastCaption.sources || [];
    for (let i = 0; i < list.length; i++) {
      const li = document.createElement("li");
      li.textContent = list[i];
      sourcesEl.appendChild(li);
    }
  }

  function onEvent(ev) {
    if (!ev) return;
    if (ev.state) {
      if (ev.state === "idle" && session && seenActive) {
        session = false;
        optimistic = false;
        releaseMic();
      }
      if (ev.state !== "idle") seenActive = true;
      if (ev.state === "speaking" && !gotPcm) setNotice("No voice on this preview.");
      setState(ev.state);
    }
    if (ev.mood) mood = ev.mood;
    if (typeof ev.level === "number") level = ev.level;
    if (ev.caption === null) {
      lastCaption = null;
      renderCaption();
    } else if (ev.caption) {
      lastCaption = ev.caption;
      renderCaption();
    }
  }

  function visualNow() {
    if (link === "down") return "offline";
    if (playing()) return "speaking";
    if (optimistic && serverState !== "speaking") return "listening";
    return serverState || "idle";
  }

  function shownLevel() {
    const visual = visualNow();
    if (visual === "listening") return Math.max(level, viewLevel);
    if (visual === "speaking") return Math.max(level, playLevel);
    return level;
  }

  function playing() {
    return !!(audioCtx && audioCtx.currentTime < playEnd);
  }

  function sendControl(action) {
    if (engine) onEvent(engine.onControl(action));
    else if (ws && ws.readyState === 1) ws.send(D.controlJson(action));
  }

  function unlockAudio() {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    if (!audioCtx) audioCtx = new AC({ latencyHint: "interactive" });
    if (audioCtx.state === "suspended") audioCtx.resume().catch(function () {});
  }

  function openMic() {
    gumCount += 1;
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
      return Promise.reject(new Error("no mic"));
    }
    return navigator.mediaDevices.getUserMedia({
      audio: { echoCancellation: true, noiseSuppression: true, channelCount: 1 },
      video: false
    }).then(function (stream) {
      unlockAudio();
      const src = audioCtx.createMediaStreamSource(stream);
      const proc = audioCtx.createScriptProcessor(2048, 1, 1);
      const mute = audioCtx.createGain();
      mute.gain.value = 0;
      const hold = [];
      let frac = 0;
      const pending = [];
      proc.onaudioprocess = function (ev) {
        const input = ev.inputBuffer.getChannelData(0);
        let sum = 0;
        for (let i = 0; i < input.length; i++) sum += input[i] * input[i];
        viewLevel = Math.sqrt(sum / input.length);
        for (let i = 0; i < input.length; i++) hold.push(input[i]);
        const step = audioCtx.sampleRate / 16000;
        let i = frac;
        while (i + 1 < hold.length) {
          const i0 = Math.floor(i);
          const t = i - i0;
          const s = hold[i0] * (1 - t) + hold[i0 + 1] * t;
          let q = Math.round(s * 32767);
          if (q > 32767) q = 32767;
          if (q < -32768) q = -32768;
          pending.push(q);
          i += step;
        }
        const consumed = Math.floor(i);
        hold.splice(0, consumed);
        frac = i - consumed;
        while (pending.length >= 1280) {
          const frame = pending.splice(0, 1280);
          const rms = D.rmsInt16(frame);
          viewLevel = rms;
          if (engine) engine.noteLevel(rms);
          if (ws && ws.readyState === 1 && ws.bufferedAmount < 256000) {
            ws.send(D.packPcm(frame, 16000, seq));
            seq = (seq + 1) >>> 0;
          }
        }
      };
      src.connect(proc);
      proc.connect(mute);
      mute.connect(audioCtx.destination);
      mic = { stream: stream, src: src, proc: proc };
      document.body.dataset.mic = "on";
    });
  }

  function releaseMic() {
    if (!mic) {
      if (document.body.dataset.mic === "on") document.body.dataset.mic = "off";
      return;
    }
    try { mic.proc.disconnect(); } catch (e) {}
    try { mic.src.disconnect(); } catch (e) {}
    const tracks = mic.stream.getTracks();
    for (let i = 0; i < tracks.length; i++) tracks[i].stop();
    mic = null;
    viewLevel = 0;
    document.body.dataset.mic = "off";
  }

  function stopPlayback() {
    for (let i = 0; i < playNodes.length; i++) {
      try { playNodes[i].stop(); } catch (e) {}
    }
    playNodes = [];
    playAt = 0;
    playEnd = 0;
    playLevel = 0;
  }

  function playIncoming(parsed) {
    gotPcm = true;
    if (notice.textContent === "No voice on this preview.") setNotice("");
    playLevel = D.rmsInt16(parsed.samples);
    if (!audioCtx) return;
    const floats = new Float32Array(parsed.samples.length);
    for (let i = 0; i < parsed.samples.length; i++) floats[i] = parsed.samples[i] / 32768;
    const buf = audioCtx.createBuffer(1, floats.length, parsed.sampleRate);
    buf.getChannelData(0).set(floats);
    const src = audioCtx.createBufferSource();
    src.buffer = buf;
    src.connect(audioCtx.destination);
    const now = audioCtx.currentTime;
    if (playAt < now) playAt = now;
    if (playAt - now > 0.5) return;
    src.start(playAt);
    playAt += floats.length / parsed.sampleRate;
    playEnd = playAt;
    playNodes.push(src);
  }

  async function onTalk() {
    if (arming) return;
    if (session && visualNow() === "speaking") {
      optimistic = true;
      sendControl("barge");
      return;
    }
    if (session) return;
    const gen = ++talkGen;
    arming = true;
    session = true;
    seenActive = false;
    gotPcm = false;
    unlockAudio();
    try {
      await openMic();
    } catch (e) {
      document.body.dataset.mic = "denied";
      setNotice("Microphone stayed off.");
    }
    arming = false;
    if (gen !== talkGen || !session) {
      releaseMic();
      document.body.dataset.mic = "off";
      return;
    }
    optimistic = true;
    sendControl("talk");
  }

  function onStop() {
    talkGen += 1;
    session = false;
    seenActive = false;
    optimistic = false;
    sendControl("stop");
    releaseMic();
    document.body.dataset.mic = "off";
    stopPlayback();
    setState("idle");
    setNotice("");
  }

  function onCaptions() {
    captionsOn = !captionsOn;
    captionsBtn.setAttribute("aria-pressed", captionsOn ? "true" : "false");
    renderCaption();
  }

  talk.addEventListener("click", function () { onTalk(); });
  stopBtn.addEventListener("click", onStop);
  captionsBtn.addEventListener("click", onCaptions);
  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape") {
      e.preventDefault();
      onStop();
    } else if ((e.key === "c" || e.key === "C") && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      onCaptions();
    } else if ((e.key === "Enter" || e.key === " ") && (e.target === document.body || e.target === talk)) {
      e.preventDefault();
      onTalk();
    }
  });

  function wsUrl() {
    const q = params.get("ws");
    if (q) return q;
    if (!location.host) return "";
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    return proto + "//" + location.host + "/v1";
  }

  function schedule() {
    const wait = backoff[Math.min(attempt, backoff.length - 1)];
    attempt += 1;
    setTimeout(connect, wait);
  }

  function connect() {
    const url = wsUrl();
    if (!D.allowedWs(url)) {
      link = "down";
      document.body.dataset.link = link;
      setNotice("The voice link stays on this machine.");
      return;
    }
    let sock;
    try { sock = new WebSocket(url); }
    catch (e) { schedule(); return; }
    sock.binaryType = "arraybuffer";
    sock.onopen = function () {
      attempt = 0;
      link = "open";
      document.body.dataset.link = link;
      ws = sock;
      if (notice.textContent === "The voice link is down.") setNotice("");
    };
    sock.onmessage = function (ev) {
      if (typeof ev.data === "string") {
        const parsed = D.parseEvent(ev.data);
        if (parsed) onEvent(parsed);
      } else {
        const bytes = new Uint8Array(ev.data);
        const pcm = D.parsePcm(bytes);
        if (pcm) playIncoming(pcm);
      }
    };
    sock.onerror = function () { try { sock.close(); } catch (e) {} };
    sock.onclose = function () {
      if (ws === sock) ws = null;
      link = "down";
      document.body.dataset.link = link;
      setNotice("The voice link is down.");
      schedule();
    };
  }

  function paintBg(animMs) {
    const d = img.data;
    for (let y = 0; y < 54; y++) {
      const v = (y + 0.5) / 54;
      for (let x = 0; x < 96; x++) {
        const u = (x + 0.5) / 96;
        const c = D.backgroundAt(u, v, animMs);
        const k = (y * 96 + x) * 4;
        d[k] = c.r;
        d[k + 1] = c.g;
        d[k + 2] = c.b;
        d[k + 3] = 255;
      }
    }
    bx.putImageData(img, 0, 0);
    ctx.imageSmoothingEnabled = true;
    ctx.drawImage(bg, 0, 0, canvas.width, canvas.height);
  }

  function shade(rgb, k) {
    return [Math.round(rgb[0] * k), Math.round(rgb[1] * k), Math.round(rgb[2] * k)];
  }

  function lift(rgb, t) {
    const light = [196, 181, 253];
    return [
      Math.round(rgb[0] + (light[0] - rgb[0]) * t),
      Math.round(rgb[1] + (light[1] - rgb[1]) * t),
      Math.round(rgb[2] + (light[2] - rgb[2]) * t)
    ];
  }

  function paintDot(radiusFrac, brightness, rgb, glow) {
    const w = canvas.width;
    const h = canvas.height;
    const side = Math.min(w, h);
    const rad = Math.max(1, radiusFrac * (0.97 + 0.03 * brightness) * side);
    ctx.save();
    ctx.translate(w * 0.5, h * 0.42);
    if (!reduced()) {
      const blur = rad * (0.12 + 0.28 * glow * brightness);
      ctx.shadowColor = "rgba(" + rgb[0] + "," + rgb[1] + "," + rgb[2] + ",0.55)";
      ctx.shadowBlur = blur;
    }
    const core = lift(rgb, 0.28 * brightness);
    const edge = shade(rgb, 0.82);
    const g = ctx.createRadialGradient(-rad * 0.2, -rad * 0.22, rad * 0.04, 0, 0, rad);
    g.addColorStop(0, "rgb(" + core[0] + "," + core[1] + "," + core[2] + ")");
    g.addColorStop(0.72, "rgb(" + rgb[0] + "," + rgb[1] + "," + rgb[2] + ")");
    g.addColorStop(1, "rgb(" + edge[0] + "," + edge[1] + "," + edge[2] + ")");
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.arc(0, 0, rad, 0, Math.PI * 2);
    ctx.fill();
    ctx.shadowColor = "transparent";
    ctx.lineWidth = Math.max(1, (window.devicePixelRatio || 1));
    ctx.strokeStyle = "rgba(255,255,255,0.35)";
    ctx.stroke();
    ctx.restore();
  }

  function draw(now) {
    const dt = Math.min(50, Math.max(0, now - lastDraw));
    lastDraw = now;
    frames += 1;
    if (dt > 0 && dt < 200) {
      frameMs.push(dt);
      if (frameMs.length > 120) frameMs.shift();
    }
    const anim = Math.max(0, Math.round(now - t0));
    if (session && !mic && visualNow() === "listening") {
      viewLevel = 0.15 + 0.1 * Math.abs(Math.sin(now / 280));
    }
    const visual = visualNow();
    document.body.dataset.visual = visual;
    const face = D.look(mood);
    let targetR;
    let targetB;
    if (visual === "offline") {
      targetR = D.OFFLINE_RADIUS;
      targetB = D.OFFLINE_BRIGHTNESS;
    } else {
      const geom = D.geometry(visual, shownLevel(), anim, mood, reduced());
      targetR = geom.radius;
      targetB = geom.brightness;
    }
    const aim = D.LUTI[visual] || D.LUTI.idle;
    if (reduced()) {
      rP = targetR;
      rV = 0;
      bP = targetB;
      bV = 0;
      wS = aim.w;
      zS = aim.z;
    } else {
      const gk = 1 - Math.exp(-dt / 480);
      wS += (aim.w - wS) * gk;
      zS += (aim.z - zS) * gk;
      const tk = 1 - Math.exp(-dt / 140);
      trLp += (targetR - trLp) * tk;
      tbLp += (targetB - tbLp) * tk;
      const rs = D.spring(rP, rV, trLp, dt, wS, zS);
      rP = rs[0];
      rV = rs[1];
      const bs = D.spring(bP, bV, tbLp, dt, wS * 0.85, Math.min(1.2, zS + 0.08));
      bP = bs[0];
      bV = bs[1];
    }
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = Math.max(2, Math.round(window.innerWidth * dpr));
    const h = Math.max(2, Math.round(window.innerHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    paintBg(anim);
    const bright = Math.max(0, Math.min(1, bP));
    const rad = Math.max(0, Math.min(0.5, rP));
    paintDot(rad, bright, face.rgb, face.glow);
    requestAnimationFrame(draw);
  }

  if (engine) {
    onEvent(engine.hello());
    const clock0 = performance.now();
    let n = 0;
    const clock = function () {
      n += 1;
      onEvent(engine.onTick());
      const wait = Math.max(0, clock0 + n * D.TICK_MS - performance.now());
      setTimeout(clock, wait);
    };
    setTimeout(clock, D.TICK_MS);
    if (fileMode) setNotice("Opened as a file. Preview only.");
  } else {
    connect();
  }

  window.__samantha = {
    get frames() { return frames; },
    get frameMs() { return frameMs.slice(); },
    get gum() { return gumCount; },
    get state() { return document.body.dataset.state; },
    get mic() { return document.body.dataset.mic; },
    get link() { return document.body.dataset.link; },
    get visual() { return document.body.dataset.visual; }
  };

  requestAnimationFrame(draw);
})();
