import { expect, test } from "@playwright/test";

test("the research page points at the one web app", async ({ page }) => {
  await page.goto("/research");
  await expect(page).toHaveTitle("Galion Research");
  await expect(page.getByRole("heading", { name: "This page is no longer the research desk." })).toBeVisible();
  await expect(page.locator("#prompt")).toHaveCount(0);
  await expect(page.locator("#voice")).toHaveCount(0);
  await expect(page.getByRole("link", { name: "Open the Galion Research app" })).toHaveAttribute(
    "href",
    "https://github.com/galion-studio/hermes-research",
  );
  const text = await page.locator("body").innerText();
  expect(text).not.toMatch(/\bHermes\b/);
});
