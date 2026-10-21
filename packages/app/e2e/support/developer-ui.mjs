/** @param {import('@playwright/test').Page} page */
export async function installDeveloperToastDismissal(page) {
  // Developer chrome must not cover app actions or move the test's pointer/focus.
  // Console errors remain in the runner's logs.
  //
  // Two things sit over the page and intercept clicks:
  //
  // 1. The app's own toast has no dismiss button; it disappears on a timer.
  // 2. The dev server's error overlay leaves an empty `#error-toast` element
  //    behind once its error is dismissed, and that element still swallows
  //    pointer events.
  //
  // Both are removed rather than clicked, so a click that was meant for the
  // page underneath lands there.
  //
  // A locator handler only fires when the locator matches, so a page with
  // neither element pays nothing for this.
  for (const selector of ['[data-testid="app-toast"]', "#error-toast"]) {
    const toast = page.locator(selector);
    await page.addLocatorHandler(toast, () => toast.evaluate((node) => node.remove()), {
      noWaitAfter: true,
    });
  }
}
