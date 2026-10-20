import { expect, test } from "playwright/test";

// The homepage is the Kronos landing page, which has no sponsors section. The
// sponsors band lives in the site footer, which every other page renders.
const PAGES = ["/docs", "/changelog", "/download/thanks"];

for (const path of PAGES) {
  test(`${path} shows the sponsors and invites the next one`, async ({ page }) => {
    await page.goto(path);

    const sponsors = page.getByRole("region", { name: "Sponsors", exact: true });
    await expect(sponsors.getByRole("link", { name: "Hoplite" })).toHaveAttribute(
      "href",
      "https://hoplite.sh/?utm_source=paseo&utm_medium=website&utm_campaign=sponsor",
    );
    await expect(sponsors.getByText("Cloud coding agents", { exact: true })).toBeVisible();
    await expect(sponsors.getByRole("link", { name: "Become a sponsor" })).toHaveAttribute(
      "href",
      "/sponsor#spot",
    );
  });
}
