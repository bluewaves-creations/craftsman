// The visual RED case: the broken variant renders different pixels but is
// compared against the SAME committed baseline (shared snapshot dir).
import { expect, test } from "@playwright/test";
import { siteUrl } from "./helpers";

test("broken variant matches the committed baseline", async ({ page }) => {
  await page.goto(siteUrl("broken.html"));
  await expect(page).toHaveScreenshot("home.png");
});
