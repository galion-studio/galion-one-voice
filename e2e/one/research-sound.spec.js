// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
// Research desk cues. The table is 432 Hz just intonation only.
import { test, expect } from "@playwright/test";

function onLattice(hz) {
  if (!Number.isInteger(hz) || hz <= 0) return false;
  for (let c = -4; c <= 4; c += 1) {
    for (let b = -4; b <= 4; b += 1) {
      for (let a = -6; a <= 6; a += 1) {
        let num = 432n;
        let den = 1n;
        if (a >= 0) num <<= BigInt(a);
        else den <<= BigInt(-a);
        if (b >= 0) {
          num *= 3n ** BigInt(b);
          den <<= BigInt(b);
        } else {
          den *= 3n ** BigInt(-b);
          num <<= BigInt(-b);
        }
        if (c >= 0) {
          num *= 5n ** BigInt(c);
          den <<= BigInt(2 * c);
        } else {
          den *= 5n ** BigInt(-c);
          num <<= BigInt(2 * -c);
        }
        if (num === BigInt(hz) * den) return true;
      }
    }
  }
  return false;
}

async function cueList(page) {
  const raw = await page.locator("body").getAttribute("data-cues");
  return (raw || "").split(",").filter(Boolean);
}

async function tones(page) {
  return page.evaluate(() => (window.__tones || []).slice());
}

async function ask(page, text) {
  await page.locator("#prompt").fill(text);
  await page.locator("#prompt").press("Enter");
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__tones = [];
    window.__audioStarted = false;
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

test("the research cue table uses only 432 Hz just ratios", async ({ page }) => {
  await page.goto("/research");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-standard", "432");
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "false");

  const cues = await page.evaluate(() => window.GALION_RESEARCH);
  expect(cues.map((row) => row.id)).toEqual([
    "question",
    "thinking",
    "found",
    "inferred",
    "unknown",
    "file",
    "hide",
    "show",
    "window-open",
    "window-close",
    "window-minimise",
    "window-settle",
  ]);
  const bells = cues.filter((row) => row.kind === "bell").map((row) => row.id);
  expect(bells).toEqual(["window-open", "window-close", "window-minimise", "window-settle"]);
  const thinking = cues.find((row) => row.id === "thinking");
  expect(thinking.soft).toBe(true);
  expect(thinking.gain).toBeLessThan(0.02);
  for (const row of cues) {
    expect(["resonance", "bell"]).toContain(row.kind);
    const hz = [...row.notes, ...row.hum];
    expect(hz.length).toBeGreaterThan(0);
    for (const tone of hz) expect(onLattice(tone)).toBe(true);
    if (row.kind === "resonance") {
      expect(hz).toEqual(expect.arrayContaining([108, 216]));
    }
    if (row.id !== "thinking") expect(thinking.gain).toBeLessThan(row.gain);
  }
  for (const row of cues.filter((item) => item.kind === "bell" && item.id !== "window-settle")) {
    expect(row.hum).toEqual([]);
  }
  const notes = (id) => cues.find((row) => row.id === id).notes.join(",");
  expect(notes("found")).not.toBe(notes("inferred"));
  expect(notes("found")).not.toBe(notes("unknown"));
  expect(notes("inferred")).not.toBe(notes("unknown"));
  expect(onLattice(440)).toBe(false);
  expect(onLattice(880)).toBe(false);
  expect(onLattice(432)).toBe(true);
  expect(onLattice(324)).toBe(true);

  const ground = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  expect(ground).toBe("rgb(11, 15, 20)");
});

test("no cue plays before a gesture", async ({ page }) => {
  await page.goto("/research");
  await page.waitForTimeout(500);
  expect(await cueList(page)).toEqual([]);
  expect(await tones(page)).toEqual([]);
  expect(await page.evaluate(() => window.__audioStarted)).toBe(false);
});

test("question, thinking, and the three answers are distinct", async ({ page }) => {
  await page.goto("/research");
  await ask(page, "Where was the source found");
  await expect.poll(() => cueList(page)).toEqual(["question", "thinking", "found"]);
  let heard = await tones(page);
  expect(heard.length).toBeGreaterThan(0);
  for (const hz of heard) expect(onLattice(hz)).toBe(true);
  expect(heard).toContain(864);
  expect(heard).not.toContain(324);

  await page.goto("/research");
  await ask(page, "What can we infer from the wash");
  await expect.poll(() => cueList(page)).toEqual(["question", "thinking", "inferred"]);
  heard = await tones(page);
  for (const hz of heard) expect(onLattice(hz)).toBe(true);
  expect(heard).toContain(648);
  expect(heard).not.toContain(864);

  await page.goto("/research");
  await ask(page, "What is the hex");
  await expect.poll(() => cueList(page)).toEqual(["question", "thinking", "unknown"]);
  await expect(page.locator("#unknown-list")).toContainText("hex");
  heard = await tones(page);
  for (const hz of heard) expect(onLattice(hz)).toBe(true);
  expect(heard).toContain(288);
  expect(heard).not.toContain(864);
  expect(heard).not.toContain(648);
});

test("a file attachment cues the fifth, not a bell", async ({ page }) => {
  await page.goto("/research");
  await page.locator("#files").setInputFiles({
    name: "wash-note.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("The wash is amber."),
  });
  await expect.poll(() => cueList(page)).toEqual(["file"]);
  await expect(page.locator("#local-list")).toContainText("wash-note.txt");
  const heard = await tones(page);
  expect(heard).toContain(540);
  expect(heard).toContain(108);
  expect(heard).toContain(216);
  expect(heard).not.toContain(432);
  expect(heard).not.toContain(324);
  for (const hz of heard) expect(onLattice(hz)).toBe(true);
});

test("showing and hiding the interface stays resonance", async ({ page }) => {
  await page.goto("/research");
  await page.locator("#ui-toggle").click();
  await page.locator("#ui-toggle").click();
  await expect.poll(() => cueList(page)).toEqual(["show", "hide"]);
  const heard = await tones(page);
  expect(heard).toContain(864);
  expect(heard).toContain(216);
  expect(heard).not.toContain(324);
  for (const hz of heard) expect(onLattice(hz)).toBe(true);
});

test("window open, close, minimise, and settle are the only bells", async ({ page }) => {
  await page.goto("/research");
  await page.locator("#ui-toggle").click();
  await page.locator("#sources .win-min").click();
  await expect(page.locator("#sources")).toHaveAttribute("data-window", "minimised");
  await expect.poll(() => cueList(page)).toEqual(["show", "window-minimise", "window-settle"]);
  let heard = await tones(page);
  expect(heard).toContain(324);
  for (const hz of heard) expect(onLattice(hz)).toBe(true);

  await page.locator("#sources .win-close").click();
  await expect(page.locator("#sources")).toHaveAttribute("data-window", "closed");
  await expect.poll(() => cueList(page)).toEqual([
    "show",
    "window-minimise",
    "window-settle",
    "window-close",
    "window-settle",
  ]);

  await page.locator("#sources .win-close").click();
  await expect(page.locator("#sources")).toHaveAttribute("data-window", "open");
  await expect.poll(() => cueList(page)).toEqual([
    "show",
    "window-minimise",
    "window-settle",
    "window-close",
    "window-settle",
    "window-open",
    "window-settle",
  ]);
  heard = await tones(page);
  for (const hz of heard) expect(onLattice(hz)).toBe(true);
  expect(heard).toEqual(expect.arrayContaining([108, 216, 288, 324, 432, 540, 648, 864]));
});

test("Silence is opt-in and remembered", async ({ page }) => {
  await page.goto("/research?silence=1");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "on");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "off");
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "true");
  await ask(page, "Where was the source found");
  await page.waitForTimeout(400);
  expect(await cueList(page)).toEqual([]);
  expect(await tones(page)).toEqual([]);

  await page.locator("#silence").click();
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await ask(page, "Where was the source found");
  await expect.poll(() => cueList(page)).toContain("found");

  await page.locator("#silence").click();
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "true");
  const frozen = await cueList(page);
  await ask(page, "What can we infer from the wash");
  await page.waitForTimeout(400);
  expect(await cueList(page)).toEqual(frozen);
  expect(await page.evaluate(() => localStorage.getItem("galion-research-silence"))).toBe("1");

  await page.reload();
  await expect(page.locator("body")).toHaveAttribute("data-silence", "on");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "off");
});

test("reduced motion keeps the mark still and plays nothing", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/research");
  await expect(page.locator("body")).toHaveAttribute("data-motion", "reduced");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await page.waitForTimeout(200);
  const first = await page.locator("#voice-dot").screenshot();
  await page.waitForTimeout(500);
  const second = await page.locator("#voice-dot").screenshot();
  expect(Buffer.compare(first, second)).toBe(0);

  await ask(page, "Where was the source found");
  await page.locator("#ui-toggle").click();
  await page.locator("#sources .win-min").click();
  await page.waitForTimeout(400);
  expect(await cueList(page)).toEqual([]);
  expect(await tones(page)).toEqual([]);
  expect(await page.evaluate(() => localStorage.getItem("galion-research-silence"))).toBe("0");
});
