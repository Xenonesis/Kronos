/** @param {import('@playwright/test').Page} page */
export async function installDeveloperToastDismissal(page) {
  // Developer chrome must not cover app actions or move the test's pointer/focus.
  // Console errors remain in the runner's logs.
  //
  // The app's toast has no dismiss button: it disappears on a timer. While it
  // is up it sits over the page and intercepts clicks, which strands a test that
  // is trying to reach a control underneath it. Taking it out of the way lets the
  // click land.
  //
  // A locator handler only fires when the locator matches, so this costs
  // nothing on a page with no toast. The earlier handler on a `#error-toast`
  // element the app no longer renders spent the test's whole timeout waiting
  // for a dismissal that had nothing to dismiss.
  const appToast = page.locator('[data-testid="app-toast"]');
  await page.addLocatorHandler(appToast, () => appToast.evaluate((node) => node.remove()), {
    noWaitAfter: true,
  });
}
