import { defineConfig } from "@playwright/test";

const origin = process.env.CG_UI_TEST_ORIGIN;
const edition = process.env.CG_UI_TEST_EDITION;
if (!origin || !["community", "enterprise"].includes(edition ?? "")) {
  throw new Error("Use python3 scripts/verify.py --suite ui-browser to start disposable APIs.");
}
if (new URL(origin).hostname !== "127.0.0.1") {
  throw new Error("Browser regressions require an isolated loopback server.");
}

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  maxFailures: 1,
  forbidOnly: true,
  timeout: 30_000,
  globalTimeout: 180_000,
  expect: { timeout: 8_000 },
  outputDir: `./test-results/${edition}/artifacts`,
  reporter: [["list"], ["json", { outputFile: `./test-results/${edition}/results.json` }]],
  use: {
    browserName: "chromium",
    baseURL: origin,
    viewport: { width: 1280, height: 800 },
    actionTimeout: 8_000,
    screenshot: "only-on-failure",
    // Traces can contain login credentials and bearer headers.
    trace: "off",
    video: "off",
  },
});
