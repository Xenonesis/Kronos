/** @param {import('@playwright/test').Page} page */
export async function installDeveloperToastDismissal(page) {
  // Developer chrome must not cover app actions or move the test's pointer/focus.
  // Console errors remain in the runner's logs.
  //
  // Two things sit over the page and swallow clicks:
  //
  // 1. The app's own toast has no dismiss button; it disappears on a timer.
  // 2. Expo's LogBox renders its error toast into a shadow root hosted on
  //    `<div id="error-toast">`. The host is absolutely positioned, and the
  //    shadow's own `:host { all: initial }` puts `pointer-events` back to
  //    `auto`, so a rule on the host does not stop it intercepting clicks. The
  //    host itself has to go.
  //
  // A locator handler cannot fix this: it only runs after the click's
  // actionability check has already failed, and that check is what reports the
  // interception, so the handler never gets the chance to remove the element.
  await page.addInitScript(() => {
    const OVERLAY_SELECTORS = ["#error-toast", '[data-testid="app-toast"]'];

    const removeOverlays = () => {
      for (const selector of OVERLAY_SELECTORS) {
        for (const node of Array.from(document.querySelectorAll(selector))) {
          node.remove();
        }
      }
    };

    const attachStyle = () => {
      // The init script can run before there is a head to append to, so the
      // style is attached whenever a document element exists and the observer
      // below keeps trying until it does.
      const host = document.head ?? document.documentElement;
      if (!host) return false;
      if (!document.getElementById("e2e-disable-dev-overlays")) {
        const style = document.createElement("style");
        style.id = "e2e-disable-dev-overlays";
        style.textContent =
          '#error-toast, [data-testid="app-toast"] { pointer-events: none !important; }';
        host.appendChild(style);
      }
      return true;
    };

    const apply = () => {
      attachStyle();
      removeOverlays();
    };

    apply();
    document.addEventListener("DOMContentLoaded", apply);
    const observer = new MutationObserver(apply);
    if (document.documentElement) {
      observer.observe(document.documentElement, { childList: true, subtree: true });
    } else {
      document.addEventListener("readystatechange", () => {
        if (document.documentElement) {
          observer.observe(document.documentElement, { childList: true, subtree: true });
        }
      });
    }
  });
}
