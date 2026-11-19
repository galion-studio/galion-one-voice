import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.js",
  timeout: 30_000,
  expect: { timeout: 8_000 },
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4173",
    colorScheme: "dark",
  },
  projects: [
    {
      name: "chromium",
      use: {
        browserName: "chromium",
        viewport: { width: 1440, height: 900 },
        deviceScaleFactor: 1,
        launchOptions: {
          args: ["--autoplay-policy=no-user-gesture-required"],
        },
      },
    },
    {
      name: "webkit-mobile",
      testMatch: "preview.spec.js",
      use: {
        ...devices["iPhone 13"],
      },
    },
  ],
  webServer: {
    command: "node server.mjs",
    url: "http://127.0.0.1:4173/one",
    reuseExistingServer: !process.env.CI,
    timeout: 20_000,
  },
});
