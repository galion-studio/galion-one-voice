import { test, expect } from "@playwright/test";

const allowed = [108, 216, 432, 540, 648, 864];

function happyLine() {
  return {
    schema: "galion-nebula/1",
    voice: "galion",
    timestamp: 1.25,
    vad: 1,
    energy: 0.4,
    valence: 0.6,
    arousal: 0.7,
    dominance: 0.3,
    weights: [{ emotion: "happy", weight: 0.82, element: "NII-575.5" }],
  };
}

async function cues(page) {
  return (await page.locator("#s").getAttribute("data-cues")) || "";
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__tones = [];
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    const orig = AC.prototype.createOscillator;
    AC.prototype.createOscillator = function () {
      const osc = orig.apply(this, arguments);
      const freq = osc.frequency;
      const proto = Object.getPrototypeOf(freq);
      const desc = Object.getOwnPropertyDescriptor(proto, "value");
      let current = freq.value;
      Object.defineProperty(freq, "value", {
        configurable: true,
        enumerable: true,
        get() {
          return current;
        },
        set(v) {
          current = v;
          window.__tones.push(v);
          if (desc && desc.set) desc.set.call(freq, v);
        },
      });
      return osc;
    };
  });
  await page.setViewportSize({ width: 1440, height: 900 });
});

test.afterEach(async ({ page }) => {
  await page.request.post("/__release").catch(() => {});
  await page.request.post("/__state", {
    data: {
      mode: "idle",
      approval: false,
      nebula: {
        schema: "galion-nebula/1",
        voice: "galion",
        timestamp: 0,
        vad: 0,
        energy: 0,
        valence: 0,
        arousal: 0.1,
        dominance: 0.2,
        weights: [{ emotion: "neutral", weight: 1, element: "HeII-541.2" }],
      },
    },
  }).catch(() => {});
});

test("resonance is on and the seven cues stay on the 432 Hz lattice", async ({ page }) => {
  await page.goto("/one");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "false");
  await expect.poll(() => cues(page)).toContain("boot");

  const plan = await page.evaluate(() => window.GALION_OS);
  expect(plan.map((row) => row.id)).toEqual([
    "boot",
    "shutdown",
    "notification",
    "wake",
    "done",
    "approval",
    "error",
  ]);
  for (const row of plan) {
    expect(row.hum).toEqual([108, 216]);
    for (const hz of row.notes) expect(allowed).toContain(hz);
  }
  const tones = await page.evaluate(() => window.__tones.slice());
  expect(tones.length).toBeGreaterThan(0);
  for (const hz of tones) expect(allowed).toContain(hz);
  expect(tones).toEqual(expect.arrayContaining([108, 216, 432]));

  const colors = await page.evaluate(() => {
    const style = getComputedStyle(document.getElementById("silence"));
    return { color: style.color, background: style.backgroundColor };
  });
  expect(colors.color).toBe("rgb(245, 245, 247)");
  expect(colors.background).toBe("rgb(10, 10, 12)");
});

test("feed, wake, task done, approval, and shutdown each cue once", async ({ page }) => {
  await page.goto("/one");
  await expect.poll(() => cues(page)).toContain("boot");

  await page.request.post("/__state", { data: { mode: "idle", nebula: happyLine() } });
  await expect(page.locator("#s")).toHaveAttribute("data-nebula", "gas");
  await expect.poll(() => cues(page)).toContain("notification");

  await page.request.post("/__state", { data: { mode: "speaking", approval: true } });
  await expect(page.locator("#s")).toHaveAttribute("data-mode", "speaking");
  await expect.poll(() => cues(page)).toContain("approval");

  await page.request.post("/__state", { data: { mode: "idle", approval: false } });
  await expect.poll(() => cues(page)).toContain("done");

  await page.evaluate(() => {
    navigator.mediaDevices.getUserMedia = () => Promise.reject(new DOMException("no", "NotAllowedError"));
  });
  await page.locator("canvas").click();
  await expect.poll(() => cues(page)).toContain("wake");
  await expect.poll(() => cues(page)).toContain("error");

  await page.request.post("/__hold");
  await expect(page.locator("#s")).toHaveAttribute("data-mode", "offline", { timeout: 5000 });
  await expect.poll(() => cues(page), { timeout: 5000 }).toContain("shutdown");
  await page.request.post("/__release");
});

test("Silence is opt-in and stops later cues", async ({ page }) => {
  await page.goto("/one?silence=1");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "on");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "off");
  await page.waitForTimeout(400);
  expect(await cues(page)).toBe("");
  expect(await page.evaluate(() => window.__tones.length)).toBe(0);

  await page.keyboard.press("s");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await page.request.post("/__state", { data: { nebula: happyLine(), mode: "idle" } });
  await expect.poll(() => cues(page)).toContain("notification");

  await page.evaluate(() => document.getElementById("silence").click());
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "true");
  const frozen = await cues(page);
  await page.request.post("/__state", { data: { mode: "speaking" } });
  await page.request.post("/__state", { data: { mode: "idle" } });
  await page.waitForTimeout(700);
  expect(await cues(page)).toBe(frozen);
});
