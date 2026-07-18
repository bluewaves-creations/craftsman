import { expect, test } from "@playwright/test";
import { siteUrl } from "./helpers";

test("home page matches the committed baseline", async ({ page }) => {
  await page.goto(siteUrl("index.html"));
  await expect(page).toHaveScreenshot("home.png");
});
