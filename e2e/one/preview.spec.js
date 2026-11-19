import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }, testInfo) => {
  if (testInfo.project.name !== "webkit-mobile") {
    await page.setViewportSize({ width: 1440, height: 900 });
  }
  await page.addInitScript(() => {
    window.__gumCalls = 0;
    const devices = navigator.mediaDevices;
    if (!devices || !devices.getUserMedia) return;
    devices.getUserMedia = function () {
      window.__gumCalls += 1;
      return Promise.reject(new DOMException("blocked-in-preview", "NotAllowedError"));
    };
  });
});

test.describe("orb voice preview", () => {
  test.describe.configure({ timeout: 60_000 });

  test("one tap plays the preview through with no mic", async ({ page }) => {
    test.setTimeout(60_000);
    await page.goto("/one");
    await expect.poll(async () => (await page.locator("#s").getAttribute("data-cues")) || "").toContain("boot");
    await expect(page.locator("body")).toHaveAttribute("data-preview-only", "1");
    await expect(page.locator("body")).toHaveAttribute("data-live-voice", "off");
    await page.waitForFunction(() => {
      const el = document.getElementById("voice-preview");
      return el && el.readyState >= 3;
    });
    // A /dot round-trip paints the mark so the canvas click hits it, not the shell.
    await page.request.post("/__state", { data: { mode: "speaking", approval: true } });
    await expect(page.locator("#s")).toHaveAttribute("data-mode", "speaking");
    await page.request.post("/__state", { data: { mode: "idle", approval: false } });
    await expect(page.locator("#s")).toHaveAttribute("data-mode", "idle");

    await page.locator("canvas").click();

    await expect.poll(async () => page.evaluate(() => {
      const el = document.getElementById("voice-preview");
      if (!el) return 0;
      return el.paused ? 0 : el.currentTime;
    }), { timeout: 8000 }).toBeGreaterThan(0.02);

    await page.waitForTimeout(1500);
    const mid = await page.evaluate(() => {
      const el = document.getElementById("voice-preview");
      return { paused: el.paused, t: el.currentTime, ended: el.ended, gum: window.__gumCalls };
    });
    expect(mid.paused).toBe(false);
    expect(mid.ended).toBe(false);
    expect(mid.t).toBeGreaterThan(0.5);
    expect(mid.gum).toBe(0);

    await expect.poll(async () => page.evaluate(() => {
      const el = document.getElementById("voice-preview");
      if (!el) return 0;
      if (el.ended) return 26;
      return el.paused ? 0 : el.currentTime;
    }), { timeout: 45_000 }).toBeGreaterThan(25);

    expect(await page.evaluate(() => window.__gumCalls)).toBe(0);
    await expect(page.locator("#status")).not.toContainText("Microphone");
    await expect(page.locator("#ai-note")).toHaveCount(0);
    await expect(page.locator("body")).not.toContainText("You are talking to an AI");
    await expect(page.locator("body")).not.toContainText("Galion is an AI assistant, not a person");
    await expect(page.locator("body")).not.toContainText("Razgovarate s umjetnom inteligencijom");
    expect(await page.locator("#caption").innerText()).not.toContain("You are talking to an AI");
  });
});
