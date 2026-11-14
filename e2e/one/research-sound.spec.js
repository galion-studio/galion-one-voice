import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";

const shots = process.env.GALION_SHOTS || "/opt/cursor/artifacts";
mkdirSync(shots, { recursive: true });

const allowed = [108, 216, 288, 432, 540, 648, 864];
const oneNotes = [
  [108, 216, 432],
  [432, 216, 108],
  [432, 540],
  [216, 432, 648],
  [540, 648, 864],
  [432, 648],
  [216, 108],
];

async function cues(page) {
  return (await page.locator("#voice-dot").getAttribute("data-cues")) || "";
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__tones = [];
    window.__types = [];
    window.__ramps = [];
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
      const typeProto = Object.getPrototypeOf(osc);
      const typeDesc = Object.getOwnPropertyDescriptor(typeProto, "type");
      Object.defineProperty(osc, "type", {
        configurable: true,
        enumerable: true,
        get() {
          return typeDesc && typeDesc.get ? typeDesc.get.call(osc) : "sine";
        },
        set(v) {
          window.__types.push(v);
          if (typeDesc && typeDesc.set) typeDesc.set.call(osc, v);
        },
      });
      return osc;
    };
    const ramp = AudioParam.prototype.exponentialRampToValueAtTime;
    AudioParam.prototype.exponentialRampToValueAtTime = function (value, time) {
      window.__ramps.push({ value, time });
      return ramp.apply(this, arguments);
    };
  });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.request.post("/__state", { data: { researchFail: false, resetSources: true } });
});

test.afterEach(async ({ page }) => {
  await page.request.post("/__state", { data: { researchFail: false, resetSources: true } }).catch(() => {});
});

test("Research cues stay on the 432 Hz lattice and are not Galion One's", async ({ page }) => {
  await page.goto("/research");
  await expect(page.locator("body")).toHaveAttribute("data-product", "research");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await expect(page.locator("body")).toHaveAttribute("data-motion", "full");
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "false");

  const plan = await page.evaluate(() => window.GALION_RESEARCH);
  expect(plan.map((row) => row.id)).toEqual([
    "ask",
    "answer",
    "attached",
    "show",
    "hide",
    "error",
    "listen",
    "leave",
  ]);
  for (const row of plan) {
    expect(row.hum).toEqual([108, 216]);
    expect(row.decay).toBe(2.8);
    expect(row.bell).toBe(false);
    expect(oneNotes.some((seq) => JSON.stringify(seq) === JSON.stringify(row.notes))).toBe(false);
    for (const hz of row.notes) expect(allowed).toContain(hz);
  }
  expect(plan.find((row) => row.id === "ask").notes).toEqual([432, 648, 540]);
  expect(plan.find((row) => row.id === "answer").notes).toEqual([648, 540, 432]);
  expect(plan.find((row) => row.id === "attached").notes).toEqual([288, 432]);
  expect(plan.find((row) => row.id === "show").notes).toEqual([216, 540, 864]);
  expect(plan.find((row) => row.id === "hide").notes).toEqual([864, 540, 216]);
  expect(plan.find((row) => row.id === "error").notes).toEqual([648, 432, 288]);
  const oneKey = await page.evaluate(() => localStorage.getItem("galion-one-silence"));
  expect(oneKey).toBeNull();
  await page.screenshot({ path: join(shots, "research-desk.png") });
});

test("ask, answer, file, show, and hide each cue on the lattice with a long decay", async ({ page }) => {
  await page.goto("/research");
  await page.screenshot({ path: join(shots, "research-minimal.png") });
  await page.locator("#ui-toggle").click();
  await expect(page.locator("#rail")).toBeVisible();
  await expect.poll(() => cues(page)).toContain("show");
  const showRamps = await page.evaluate(() => window.__ramps.slice());
  expect(showRamps.length).toBe(10);
  const gaps = [];
  for (let i = 0; i < showRamps.length; i += 2) {
    expect(showRamps[i + 1].value).toBe(0.0001);
    gaps.push(showRamps[i + 1].time - showRamps[i].time);
  }
  expect(gaps.filter((gap) => Math.abs(gap - 2.76) < 0.05)).toHaveLength(3);
  expect(gaps.filter((gap) => Math.abs(gap - 3.32) < 0.05)).toHaveLength(2);
  await page.screenshot({ path: join(shots, "research-interface.png") });

  await page.locator("#prompt").fill("What colour is the wash?");
  await page.locator("#prompt").press("Enter");
  await expect.poll(() => cues(page)).toBe("show,ask,answer");
  await expect(page.locator("#reading-body")).toHaveText("The wash is amber.");
  await page.screenshot({ path: join(shots, "research-answer.png") });

  await page.setInputFiles("#files", {
    name: "wash-note.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("The wash is amber."),
  });
  await expect.poll(() => cues(page)).toContain("attached");
  await expect(page.locator("#local-list")).toContainText("wash-note.txt");

  await page.locator("#voice").click();
  await expect.poll(() => cues(page)).toContain("listen");
  await page.locator("#voice").click();
  await expect.poll(() => cues(page)).toContain("leave");

  await page.locator("#ui-toggle").click();
  await expect.poll(() => cues(page)).toContain("hide");

  const heard = await page.evaluate(() => ({
    tones: window.__tones.slice(),
    types: window.__types.slice(),
    ramps: window.__ramps.slice(),
  }));
  expect(heard.tones.length).toBeGreaterThan(0);
  expect(heard.types.length).toBeGreaterThan(0);
  for (const hz of heard.tones) expect(allowed).toContain(hz);
  for (const type of heard.types) expect(type).toBe("sine");
  expect(heard.tones).toEqual(expect.arrayContaining([108, 216, 288, 432, 540, 648, 864]));
});

test("a failed ask and a rejected file play the research error", async ({ page }) => {
  await page.request.post("/__state", { data: { researchFail: true } });
  await page.goto("/research");
  await page.locator("#prompt").fill("Where is the source?");
  await page.locator("#prompt").press("Enter");
  await expect.poll(() => cues(page)).toBe("ask,error");
  await expect(page.locator("#reading-body")).toContainText("The research path is not reachable from here.");
  await page.screenshot({ path: join(shots, "research-error.png") });

  await page.setInputFiles("#files", {
    name: "missing.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("not attached"),
  });
  await expect.poll(() => cues(page)).toBe("ask,error,error");
  await expect(page.locator("#source-empty")).toHaveText("The file was not attached.");
  const notes = await page.evaluate(() => window.GALION_RESEARCH.find((row) => row.id === "error").notes);
  expect(notes).toEqual([648, 432, 288]);
  expect(notes).not.toEqual([216, 108]);
});

test("Silence is opt-in, persisted, and blocks later cues", async ({ page }) => {
  await page.goto("/research?silence=1");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "on");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "off");
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "true");
  await page.locator("#prompt").fill("What colour is the wash?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-body")).toHaveText("The wash is amber.");
  expect(await cues(page)).toBe("");
  expect(await page.evaluate(() => window.__tones.length)).toBe(0);
  await page.screenshot({ path: join(shots, "research-silence.png") });

  await page.locator("#silence").click();
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await page.locator("#prompt").fill("What colour is the wash again?");
  await page.locator("#prompt").press("Enter");
  await expect.poll(() => cues(page)).toBe("ask,answer");

  await page.locator("#silence").click();
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "true");
  const frozen = await cues(page);
  await page.locator("#ui-toggle").click();
  await page.locator("#prompt").fill("And once more?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-body")).toHaveText("The wash is amber.");
  expect(await cues(page)).toBe(frozen);

  await page.reload();
  await expect(page.locator("body")).toHaveAttribute("data-silence", "on");
  const stored = await page.evaluate(() => localStorage.getItem("galion-research-silence"));
  expect(stored).toBe("1");
});

test("prefers-reduced-motion plays no cue and holds the disc", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/research");
  await expect(page.locator("body")).toHaveAttribute("data-motion", "reduce");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "off");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await page.waitForFunction(() => document.getElementById("voice-dot").width > 0);
  await page.waitForTimeout(80);
  const first = await page.evaluate(() => {
    const cv = document.getElementById("voice-dot");
    return cv.getContext("2d").getImageData(cv.width / 2, cv.height / 2, 1, 1).data.join(",");
  });
  await page.waitForTimeout(400);
  const second = await page.evaluate(() => {
    const cv = document.getElementById("voice-dot");
    return cv.getContext("2d").getImageData(cv.width / 2, cv.height / 2, 1, 1).data.join(",");
  });
  expect(second).toBe(first);
  await page.locator("#prompt").fill("What colour is the wash?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-body")).toHaveText("The wash is amber.");
  await page.locator("#ui-toggle").click();
  expect(await cues(page)).toBe("");
  expect(await page.evaluate(() => window.__tones.length)).toBe(0);
  const stored = await page.evaluate(() => localStorage.getItem("galion-research-silence"));
  expect(stored).toBe("0");
});

test("the Silence control stays inside a phone width", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/research");
  const box = await page.locator("#silence").boundingBox();
  expect(box).toBeTruthy();
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(390);
  await expect(page.locator("#silence")).toBeVisible();
  await page.screenshot({ path: join(shots, "research-phone.png") });
});
