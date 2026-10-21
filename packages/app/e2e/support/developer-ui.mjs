/** @param {import('@playwright/test').Page} page */
export async function installDeveloperToastDismissal(page) {
  // Developer chrome must not cover app actions or move the test's pointer/focus.
  // Console errors remain in the runner's logs.
  const dismiss = page.locator("#error-toast").getByRole("button", { name: "", exact: true });
  await page.addLocatorHandler(
    dismiss,
    async () => {
      // The handler fires whenever the toast might match, including when no
      // toast is there at all. Waiting for the element to appear would spend
      // the test's whole timeout on a dismissal that has nothing to dismiss, so
      // the click gives up quickly instead.
      await dismiss.dispatchEvent("click", { timeout: 1_000 }).catch(() => undefined);
    },
    {
      // A new logged error can replace the toast before the old one disappears.
      noWaitAfter: true,
    },
  );
}
