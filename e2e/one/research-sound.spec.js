import { test, expect } from "@playwright/test";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

function readGuide() {
  const candidates = [
    join(process.cwd(), "docs/07-sound-guide/SOUND-LANGUAGE.md"),
    join(process.cwd(), "../../docs/07-sound-guide/SOUND-LANGUAGE.md"),
  ];
  const found = candidates.find((path) => existsSync(path));
  if (!found) throw new Error("sound guide not found from " + process.cwd());
  return readFileSync(found, "utf8");
}

const guide = readGuide();
const shots = process.env.GALION_SHOTS || "/opt/cursor/artifacts";
mkdirSync(shots, { recursive: true });

const allowed = [108, 216, 432, 540, 648, 864];
const desk = [
  { id: "ask-sent", name: "Ask sent", ratio: "1:1", hz: 432 },
  { id: "thinking", name: "Thinking", ratio: "5:4", hz: 540 },
  { id: "answer-ready", name: "Answer ready", ratio: "3:2", hz: 648 },
  { id: "source-attached", name: "Source attached", ratio: "2:1", hz: 864 },
  { id: "error", name: "Error", ratio: "1:2", hz: 216 },
];

async function cueList(page) {
  return ((await page.locator("body").getAttribute("data-cues")) || "").split(",").filter(Boolean);
}

async function tones(page) {
  return page.evaluate(() => window.__tones.slice());
}

test.beforeEach(async ({ page }) => {
  await page.request.post("/__research", { data: { reset: true } });
  await page.addInitScript(() => {
    window.__tones = [];
    window.__spoke = 0;
    if (window.speechSynthesis) {
      window.speechSynthesis.speak = () => {
        window.__spoke += 1;
      };
    }
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
});

test("the sound guide and the desk use the same five cues", async ({ page }) => {
  for (const row of desk) {
    expect(guide).toContain(`| ${row.name} | ${row.id} | ${row.ratio} | ${row.hz} |`);
    expect(allowed).toContain(row.hz);
  }
  expect(guide).not.toContain("288");

  await page.goto("/research");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await expect(page.locator("body")).toHaveAttribute("data-ui", "minimal");
  const plan = await page.evaluate(() => window.GALION_RESEARCH);
  for (const row of desk) {
    expect(plan).toEqual(expect.arrayContaining([{ id: row.id, ratio: row.ratio, hz: row.hz }]));
  }
  for (const row of plan) expect(allowed).toContain(row.hz);
  expect(plan.map((row) => row.hz)).not.toContain(288);
});

test("ask, thinking, and answer ready play on the lattice while the interface is hidden", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/research");
  await page.locator("#prompt").fill("Where is the wash?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-body")).toHaveText("The wash is amber to violet.");
  expect(await cueList(page)).toEqual(["ask-sent", "thinking", "answer-ready"]);
  expect(await tones(page)).toEqual([432, 540, 648]);
  expect(await page.evaluate(() => window.__spoke)).toBe(0);
  const media = await page.evaluate(() =>
    performance.getEntriesByType("resource").map((entry) => entry.name).filter((name) => /\.(wav|mp3|ogg|m4a|flac)(\?|$)/i.test(name)),
  );
  expect(media).toEqual([]);
});

test("showing the desk still cues, and a screenshot keeps the answer", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/research");
  await page.locator("#ui-toggle").click();
  await expect(page.locator("#ui-toggle")).toHaveText("Hide interface");
  await page.locator("#prompt").fill("Where is the wash?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-body")).toHaveText("The wash is amber to violet.");
  await expect(page.locator("#found-list")).toContainText("The wash is amber to violet.");
  expect(await cueList(page)).toEqual(["show", "ask-sent", "thinking", "answer-ready"]);
  expect(await tones(page)).toEqual([864, 432, 540, 648]);
  await expect(page.locator("#silence")).toBeVisible();
  await page.locator("#answer").scrollIntoViewIfNeeded();
  await page.screenshot({ path: join(shots, "research-desk-desktop.png"), fullPage: true });
});

test("a narrow desk keeps the Silence control inside the viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/research");
  await page.locator("#ui-toggle").click();
  const box = await page.locator("#silence").boundingBox();
  expect(box).not.toBeNull();
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(390);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(1);
});

test("attaching a source plays 864 Hz and a refused file plays the error", async ({ page }) => {
  await page.goto("/research");
  await page.locator("#ui-toggle").click();
  await page.locator("#files").setInputFiles({
    name: "wash-note.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("The wash is amber."),
  });
  await expect.poll(() => cueList(page)).toEqual(["show", "source-attached"]);
  expect(await tones(page)).toEqual([864, 864]);
  await expect(page.locator("#local-list")).toContainText("wash-note.txt");

  await page.request.post("/__research", { data: { failSource: true } });
  await page.locator("#files").setInputFiles({
    name: "missing.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("no"),
  });
  await expect.poll(() => cueList(page)).toEqual(["show", "source-attached", "error"]);
  expect(await tones(page)).toEqual([864, 864, 216]);
});

test("an unreachable ask plays the error after ask sent and thinking", async ({ page }) => {
  await page.request.post("/__research", { data: { failAsk: true } });
  await page.goto("/research");
  await page.locator("#prompt").fill("Where is the source?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-body")).toContainText("not reachable from here");
  expect(await cueList(page)).toEqual(["ask-sent", "thinking", "error"]);
  expect(await tones(page)).toEqual([432, 540, 216]);
});

test("Silence stops desk cues and the hide-interface toggle", async ({ page }) => {
  await page.goto("/research?silence=1");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "on");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "off");
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "true");
  await page.locator("#ui-toggle").click();
  await expect(page.locator("#ui-toggle")).toHaveText("Hide interface");
  await page.locator("#prompt").fill("Where is the wash?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-body")).toHaveText("The wash is amber to violet.");
  expect(await cueList(page)).toEqual([]);
  expect(await tones(page)).toEqual([]);

  await page.locator("#silence").click();
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await page.locator("#prompt").fill("What remains local?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-question")).toHaveText("What remains local?");
  expect(await cueList(page)).toEqual(["ask-sent", "thinking", "answer-ready"]);
  expect(await tones(page)).toEqual([432, 540, 648]);

  await page.locator("body").click({ position: { x: 12, y: 12 } });
  await page.keyboard.press("s");
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "true");
  const frozen = await cueList(page);
  await page.locator("#ui-toggle").click();
  await expect(page.locator("body")).toHaveAttribute("data-ui", "minimal");
  await page.locator("#prompt").fill("A quiet follow-up");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#reading-question")).toHaveText("A quiet follow-up");
  expect(await cueList(page)).toEqual(frozen);
  expect(await tones(page)).toEqual([432, 540, 648]);
});
