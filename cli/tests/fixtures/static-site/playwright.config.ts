import { defineConfig } from "@playwright/test";

// Deterministic on one machine: fixed viewport, chromium only, animations
// off, and a shared snapshot dir so the broken-variant spec compares
// against the SAME committed baseline as the green spec.
export default defineConfig({
  testDir: "tests",
  workers: 1,
  retries: 0,
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}-{platform}{ext}",
  use: {
    browserName: "chromium",
    viewport: { width: 800, height: 600 },
  },
  expect: {
    toHaveScreenshot: { animations: "disabled", maxDiffPixelRatio: 0.01 },
  },
});
