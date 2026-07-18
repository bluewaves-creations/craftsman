import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { siteUrl } from "./helpers";

test("index page has no detectable a11y violations", async ({ page }) => {
  await page.goto(siteUrl("index.html"));
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations).toEqual([]);
});
