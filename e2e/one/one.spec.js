import { test, expect } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const shots = process.env.GALION_SHOTS || "/opt/cursor/artifacts";
mkdirSync(shots, { recursive: true });

function neutralLine() {
  return {
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
}

function happyLine(weight = 0.82, element = "NII-575.5") {
  return {
    schema: "galion-nebula/1",
    voice: "galion",
    timestamp: 1.25,
    vad: 1,
    energy: 0.45,
    valence: 0.6,
    arousal: 0.7,
    dominance: 0.3,
    weights: [{ emotion: "happy", weight, element }],
  };
}

function relLum(r, g, b) {
  const channel = (c) => {
    const x = c / 255;
    return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function contrast(a, b) {
  const hi = Math.max(relLum(...a), relLum(...b));
  const lo = Math.min(relLum(...a), relLum(...b));
  return (hi + 0.05) / (lo + 0.05);
}

async function idleState(page) {
  await page.request.post("/__state", {
    data: {
      mode: "idle",
      approval: false,
      persona: "galion",
      emotion: { sense: true, label: "neutral", valence: 0, arousal: 0.1, lean: 0, confidence: 1 },
      nebula: neutralLine(),
    },
  });
}

async function openOne(page) {
  await idleState(page);
  await page.goto("/one");
  await page.waitForFunction(
    () => typeof window.drawNebula === "function" && document.querySelector("#s")?.dataset.mode === "idle",
  );
}

async function probe(page) {
  return page.evaluate(() => {
    const cv = document.querySelector("#s");
    const ctx = cv.getContext("2d");
    const w = cv.width;
    const h = cv.height;
    const data = ctx.getImageData(0, 0, w, h).data;
    const cx = Math.round(w / 2);
    const cy = Math.round(h / 2);
    const at = (x, y) => {
      const i = (y * w + x) * 4;
      return [data[i], data[i + 1], data[i + 2]];
    };
    const bright = (p) => p[0] > 210 && p[1] > 210 && p[2] > 210;
    let radius = 0;
    for (let x = cx; x < w; x += 1) {
      if (!bright(at(x, cy))) break;
      radius = x - cx;
    }
    const limit = Math.max(8, radius * 1.45);
    let count = 0;
    let far = 0;
    let minX = w;
    let minY = h;
    let maxX = 0;
    let maxY = 0;
    for (let y = 0; y < h; y += 1) {
      for (let x = 0; x < w; x += 1) {
        if (!bright(at(x, y))) continue;
        count += 1;
        if (x < minX) minX = x;
        if (y < minY) minY = y;
        if (x > maxX) maxX = x;
        if (y > maxY) maxY = y;
        if (Math.hypot(x - cx, y - cy) > limit) far += 1;
      }
    }
    const ax = Math.min(w - 1, cx + Math.round(radius * 1.35));
    return {
      w,
      h,
      radius,
      count,
      far,
      box: [minX, minY, maxX, maxY],
      center: at(cx, cy),
      corner: at(8, 8),
      adjacent: at(ax, cy),
    };
  });
}

async function stamp(page) {
  await page.evaluate(() => {
    const cv = document.querySelector("#s");
    window.__stamp = cv.getContext("2d").getImageData(0, 0, cv.width, cv.height).data;
  });
}

async function annulusChanged(page) {
  return page.evaluate(() => {
    const cv = document.querySelector("#s");
    const ctx = cv.getContext("2d");
    const w = cv.width;
    const h = cv.height;
    const now = ctx.getImageData(0, 0, w, h).data;
    const prev = window.__stamp;
    const cx = w / 2;
    const cy = h / 2;
    const at = (buf, x, y) => {
      const i = (y * w + x) * 4;
      return [buf[i], buf[i + 1], buf[i + 2]];
    };
    const bright = (p) => p[0] > 210 && p[1] > 210 && p[2] > 210;
    let radius = 0;
    const mid = Math.round(cy);
    for (let x = Math.round(cx); x < w; x += 1) {
      if (!bright(at(now, x, mid))) break;
      radius = x - cx;
    }
    let changed = 0;
    let seen = 0;
    for (let y = 0; y < h; y += 2) {
      for (let x = 0; x < w; x += 2) {
        const d = Math.hypot(x - cx, y - cy);
        if (d < radius * 1.2 || d > radius * 2.5) continue;
        seen += 1;
        const i = (y * w + x) * 4;
        const delta = Math.abs(now[i] - prev[i]) + Math.abs(now[i + 1] - prev[i + 1]) + Math.abs(now[i + 2] - prev[i + 2]);
        if (delta > 18) changed += 1;
      }
    }
    return { changed, seen, radius };
  });
}

test.describe.configure({ mode: "serial" });

test("desktop /one is one dot in idle and listening", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openOne(page);
  await expect(page).toHaveTitle("Galion One");
  await expect(page.locator("canvas")).toHaveCount(1);
  await expect(page.locator("#s")).toHaveAttribute("data-mark", "orb");
  await expect(page.locator("#s")).toHaveAttribute("data-persona", "galion");
  await expect(page.locator("#decide")).toBeHidden();
  const bg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  expect(bg).toBe("rgb(10, 10, 12)");

  await page.waitForTimeout(600);
  const idle = await probe(page);
  expect(idle.far).toBe(0);
  expect(idle.count).toBeGreaterThan(2000);
  const [minX, minY, maxX, maxY] = idle.box;
  const bw = maxX - minX;
  const bh = maxY - minY;
  expect(Math.abs(bw - bh)).toBeLessThan(idle.radius * 0.35);
  expect(minX).toBeGreaterThan(idle.w * 0.3);
  expect(maxX).toBeLessThan(idle.w * 0.7);
  await page.screenshot({ path: join(shots, "one-idle-desktop.png") });

  await page.request.post("/__state", { data: { mode: "listening" } });
  await expect(page.locator("#s")).toHaveAttribute("data-mode", "listening");
  await page.waitForTimeout(1000);
  const listening = await probe(page);
  expect(listening.radius).toBeGreaterThan(idle.radius * 1.12);
  expect(listening.far).toBe(0);
  expect(listening.count).toBeGreaterThan(idle.count);
  await page.screenshot({ path: join(shots, "one-listening-desktop.png") });
});

test("halo reacts to galion-nebula/1 feed lines", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openOne(page);
  await expect(page.locator("#s")).toHaveAttribute("data-nebula", "hold");
  await page.waitForTimeout(300);
  await stamp(page);
  await page.screenshot({ path: join(shots, "one-halo-hold-desktop.png") });

  await page.request.post("/__state", { data: { nebula: happyLine(), mode: "idle" } });
  await expect(page.locator("#s")).toHaveAttribute("data-nebula", "gas");
  await expect(page.locator("#s")).toHaveAttribute("data-halo", "happy");
  await page.waitForTimeout(200);
  const delta = await annulusChanged(page);
  expect(delta.seen).toBeGreaterThan(50);
  expect(delta.changed).toBeGreaterThan(8);
  const joy = await probe(page);
  expect(Math.min(...joy.center)).toBeGreaterThan(220);
  await page.screenshot({ path: join(shots, "one-halo-joy-desktop.png") });

  await page.request.post("/__state", {
    data: { nebula: happyLine(0.82, "H-alpha") },
  });
  await expect(page.locator("#s")).toHaveAttribute("data-nebula", "hold");

  await page.request.post("/__state", { data: { nebula: happyLine(0.1) } });
  await expect(page.locator("#s")).toHaveAttribute("data-nebula", "hold");

  await page.request.post("/__state", {
    data: {
      emotion: { sense: true, label: "joy", confidence: 1, valence: 0.7, arousal: 0.6, lean: 0 },
      nebula: {
        schema: "galion-voice-v2-emotion-feed/1",
        estimate: "none",
        present: false,
        distribution: false,
        reason: null,
      },
    },
  });
  await expect(page.locator("#s")).toHaveAttribute("data-nebula", "hold");
  await expect(page.locator("#s")).toHaveAttribute("data-halo", "hold");
});

test("dot against the ground meets WCAG AA contrast", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openOne(page);
  await page.waitForTimeout(400);
  const sample = await probe(page);
  const dotOnGround = contrast(sample.center, sample.corner);
  const dotOnWash = contrast(sample.center, sample.adjacent);
  writeFileSync(
    join(shots, "one-contrast.json"),
    JSON.stringify(
      {
        viewport: "1440x900",
        center: sample.center,
        corner: sample.corner,
        adjacent: sample.adjacent,
        dotOnGround,
        dotOnWash,
        aaNormalText: 4.5,
        aaNonText: 3,
      },
      null,
      2,
    ),
  );
  expect(dotOnGround).toBeGreaterThanOrEqual(4.5);
  expect(dotOnWash).toBeGreaterThanOrEqual(3);
  await page.screenshot({ path: join(shots, "one-contrast-desktop.png") });
});
