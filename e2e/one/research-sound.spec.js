import { test, expect } from "@playwright/test";

const hum = [108, 216];
const allowed = [108, 216, 432, 540, 648, 864];

const researchHz = {
  "question-sent": [432, 648, 864],
  thinking: [540],
  "answer-ready": [432, 540, 648],
  "source-attached": [216, 432],
  "source-cited": [648, 540],
  error: [216, 108],
};

const bellHz = {
  "window-open": [864, 432],
  "window-close": [432, 216],
  "window-minimise": [540, 216],
  "window-settle": [648, 432],
};

function withHum(notes) {
  return notes.concat(hum);
}

async function cues(page) {
  return (await page.locator("body").getAttribute("data-cues")) || "";
}

async function tones(page) {
  return page.evaluate(() => window.__tones.slice());
}

async function shot(page, name) {
  if (!process.env.GALION_SHOTS) return;
  await page.screenshot({ path: `${process.env.GALION_SHOTS}/${name}.png`, fullPage: true });
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
  await page.request.post("/__research", {
    data: {
      reset: true,
      delayMs: 1100,
      fail: false,
      status: "Found",
      reply: "The wash is amber to violet.",
      citations: [{ n: 1, text: "wash-note.txt" }],
      found: ["The wash is amber to violet."],
    },
  });
  await page.setViewportSize({ width: 1440, height: 900 });
});

test("the research cue table is the 432 Hz lattice at quiet peaks", async ({ page }) => {
  await page.goto("/research");
  await expect(page.locator("body")).toHaveAttribute("data-product", "research");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");

  const plan = await page.evaluate(() => ({
    research: window.GALION_RESEARCH,
    bells: window.GALION_BELLS,
  }));

  expect(plan.research.map((row) => row.id)).toEqual(Object.keys(researchHz));
  expect(plan.bells.map((row) => row.id)).toEqual(Object.keys(bellHz));

  for (const row of plan.research) {
    expect(row.kind).toBe("resonance");
    expect(row.notes).toEqual(researchHz[row.id]);
    expect(row.hum).toEqual(hum);
    expect(row.peak).toBe(0.03);
    expect(row.humPeak).toBe(0.01);
    expect(row.partial).toBe(0);
    for (const hz of row.notes.concat(row.hum)) expect(allowed).toContain(hz);
  }
  for (const row of plan.bells) {
    expect(row.kind).toBe("bell");
    expect(row.notes).toEqual(bellHz[row.id]);
    expect(row.hum).toEqual(hum);
    expect(row.peak).toBe(0.03);
    expect(row.humPeak).toBe(0.01);
    expect(row.partial).toBe(0.01);
    for (const hz of row.notes.concat(row.hum)) expect(allowed).toContain(hz);
  }
});

test("a question plays sent, thinking, ready, and cited at those frequencies", async ({ page }) => {
  await page.goto("/research");
  await expect.poll(() => cues(page)).toBe("window-open");
  await shot(page, "research-desk");

  await page.locator("#files").setInputFiles({
    name: "wash-note.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("The wash is amber."),
  });
  await expect.poll(() => cues(page)).toBe("window-open,source-attached");

  await page.locator("#prompt").fill("What colour is the wash?");
  await page.locator("#prompt").press("Enter");
  await expect(page.locator("#hermes-status")).toHaveText("Hermes · Running");
  await expect.poll(() => cues(page), { timeout: 8000 }).toBe(
    "window-open,source-attached,question-sent,thinking,answer-ready,source-cited",
  );

  expect(await tones(page)).toEqual(
    []
      .concat(withHum(bellHz["window-open"]))
      .concat(withHum(researchHz["source-attached"]))
      .concat(withHum(researchHz["question-sent"]))
      .concat(withHum(researchHz.thinking))
      .concat(withHum(researchHz["answer-ready"]))
      .concat(withHum(researchHz["source-cited"])),
  );

  await expect(page.locator("#reading-body")).toContainText("The wash is amber to violet.");
  await expect(page.locator("#citation-list")).toContainText("[1] wash-note.txt");
  await page.locator("#ui-toggle").click();
  await expect(page.locator("#ui-toggle")).toHaveText("Hide interface");
  expect(await cues(page)).toBe(
    "window-open,source-attached,question-sent,thinking,answer-ready,source-cited",
  );
  await expect(page.locator("#citation-list")).toBeVisible();
  await shot(page, "research-answer-cited");
  await page.setViewportSize({ width: 390, height: 844 });
  await shot(page, "research-answer-mobile");
});

test("attaching a source plays source-attached", async ({ page }) => {
  await page.goto("/research");
  await expect.poll(() => cues(page)).toBe("window-open");
  await page.locator("#files").setInputFiles({
    name: "wash-note.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("The wash is amber."),
  });
  await expect.poll(() => cues(page)).toBe("window-open,source-attached");
  expect(await tones(page)).toEqual(
    withHum(bellHz["window-open"]).concat(withHum(researchHz["source-attached"])),
  );
  await page.locator("#ui-toggle").click();
  await expect(page.locator("#local-list")).toContainText("wash-note.txt");
  await shot(page, "research-source-attached");
});

test("a failed ask plays error and does not play answer-ready", async ({ page }) => {
  await page.request.post("/__research", { data: { fail: true, delayMs: 1100, citations: [] } });
  await page.goto("/research");
  await page.locator("#prompt").fill("Where is the source?");
  await page.locator("#prompt").press("Enter");
  await expect.poll(() => cues(page), { timeout: 8000 }).toBe("window-open,question-sent,thinking,error");
  expect(await tones(page)).toEqual(
    []
      .concat(withHum(bellHz["window-open"]))
      .concat(withHum(researchHz["question-sent"]))
      .concat(withHum(researchHz.thinking))
      .concat(withHum(researchHz.error)),
  );
  await expect(page.locator("#reading-body")).toContainText("The research path is not reachable from here.");
  await page.locator("#ui-toggle").click();
  await shot(page, "research-error");
});

test("Silence blocks the window bell and later research cues", async ({ page }) => {
  await page.goto("/research?silence=1");
  await expect(page.locator("body")).toHaveAttribute("data-silence", "on");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "off");
  await expect(page.locator("#silence")).toHaveAttribute("aria-pressed", "true");
  await page.waitForTimeout(400);
  expect(await cues(page)).toBe("");
  expect(await tones(page)).toEqual([]);

  await page.evaluate(() => document.getElementById("silence").click());
  await expect(page.locator("body")).toHaveAttribute("data-silence", "off");
  await expect(page.locator("body")).toHaveAttribute("data-resonance", "on");
  await page.request.post("/__research", { data: { delayMs: 0, citations: [], status: "Unknown", reply: "Unknown." } });
  await page.locator("#prompt").fill("A short question");
  await page.locator("#prompt").press("Enter");
  await expect.poll(() => cues(page)).toContain("question-sent");
  const heard = await tones(page);
  expect(heard.slice(0, 5)).toEqual(withHum(researchHz["question-sent"]));
  for (const hz of heard) expect(allowed).toContain(hz);
});

test("bells fire only for window open, close, minimise, and settle", async ({ page }) => {
  await page.goto("/research");
  await expect.poll(() => cues(page)).toBe("window-open");

  await page.locator("#voice").click();
  await page.locator("#ui-toggle").click();
  await page.waitForTimeout(200);
  expect(await cues(page)).toBe("window-open");
  expect(await tones(page)).toEqual(withHum(bellHz["window-open"]));

  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect.poll(() => cues(page)).toBe("window-open,window-minimise");

  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "visible" });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect.poll(() => cues(page)).toBe("window-open,window-minimise,window-settle");

  await page.evaluate(() => {
    window.dispatchEvent(new Event("pagehide"));
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect.poll(() => cues(page)).toBe("window-open,window-minimise,window-settle,window-close");
  expect(await tones(page)).toEqual(
    []
      .concat(withHum(bellHz["window-open"]))
      .concat(withHum(bellHz["window-minimise"]))
      .concat(withHum(bellHz["window-settle"]))
      .concat(withHum(bellHz["window-close"])),
  );
});
