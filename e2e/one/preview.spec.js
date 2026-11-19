import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }, testInfo) => {
  if (testInfo.project.name !== "webkit-mobile") {
    await page.setViewportSize({ width: 1440, height: 900 });
  }
});

test.describe("orb voice preview", () => {
  test("one tap plays preview audio and hides the disclosure line", async ({ page }) => {
    await page.goto("/one");
    await expect.poll(async () => (await page.locator("#s").getAttribute("data-cues")) || "").toContain("boot");
    await page.waitForFunction(() => {
      const el = document.getElementById("voice-preview");
      return el && el.readyState >= 3;
    });
    // A /dot round-trip paints the mark so the canvas click hits it, not the shell.
    await page.request.post("/__state", { data: { mode: "speaking", approval: true } });
    await expect(page.locator("#s")).toHaveAttribute("data-mode", "speaking");
    await page.request.post("/__state", { data: { mode: "idle", approval: false } });
    await expect(page.locator("#s")).toHaveAttribute("data-mode", "idle");
    await page.evaluate(() => {
      navigator.mediaDevices.getUserMedia = () => Promise.reject(new DOMException("no", "NotAllowedError"));
    });
    await page.locator("canvas").click();

    await expect.poll(async () => page.evaluate(() => {
      const el = document.getElementById("voice-preview");
      if (!el) return 0;
      return el.paused ? 0 : el.currentTime;
    }), { timeout: 8000 }).toBeGreaterThan(0.02);

    const first = await page.evaluate(() => document.getElementById("voice-preview").currentTime);
    await page.waitForTimeout(400);
    const second = await page.evaluate(() => document.getElementById("voice-preview").currentTime);
    expect(second).toBeGreaterThan(first);

    await expect(page.locator("#ai-note")).toHaveCount(0);
    await expect(page.locator("body")).not.toContainText("You are talking to an AI");
    await expect(page.locator("body")).not.toContainText("Galion is an AI assistant, not a person");
    await expect(page.locator("body")).not.toContainText("Razgovarate s umjetnom inteligencijom");
    expect(await page.locator("#caption").innerText()).not.toContain("You are talking to an AI");
  });
});
