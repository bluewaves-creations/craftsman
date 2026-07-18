// The a11y RED case: the seeded-issue variant (no lang, image without
// alt, low-contrast text) must produce axe violations.
import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { siteUrl } from "./helpers";

test("broken variant has no detectable a11y violations", async ({ page }) => {
  await page.goto(siteUrl("broken.html"));
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations).toEqual([]);
});
