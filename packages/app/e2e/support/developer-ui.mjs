/** @param {import('@playwright/test').Page} page */
export async function installDeveloperToastDismissal(page) {
  // Developer chrome must not cover app actions or move the test's pointer/focus.
  // Console errors remain in the runner's logs.
  //
  // Two things sit over the page and swallow clicks:
  //
  // 1. The app's own toast has no dismiss button; it disappears on a timer.
  // 2. The dev server's error overlay leaves an empty `#error-toast` element
  //    behind once its error is dismissed, and that element still takes pointer
  //    events.
  //
  // A locator handler cannot fix this: it only runs after the click's
  // actionability check has already failed, and that check is what reports the
  // interception. A stylesheet that takes the elements out of the hit-testing
  // order fixes it before the check runs.
  await page.addInitScript(() => {
    const style = document.createElement("style");
    style.textContent =
      '#error-toast, [data-testid="app-toast"] { pointer-events: none !important; }';
    const attach = () => {
      if (!style.isConnected) document.head.appendChild(style);
    };
    attach();
    // The overlay is injected after the page loads, so watch for it.
    new MutationObserver(attach).observe(document.documentElement, {
      childList: true,
      subtree: true,
    });
  });
}
