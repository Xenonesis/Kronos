// Installed in the page to take the developer overlays out of the way.
//
// Two things sit over the page:
//
// 1. Expo's LogBox renders its error toast into a shadow root hosted on
//    `<div id="error-toast">`. The host is absolutely positioned, and the
//    shadow's own `:host { all: initial }` resets `pointer-events` back to
//    `auto`. A rule from the outer document still wins for the host element,
//    so the host can be made non-interactive without touching the shadow.
//
// 2. The app's own toast is a real UI element that tests read their text
//    from, so it stays in the document; it only has to stop swallowing clicks.
//
// Neither overlay is removed: tests assert on the text they carry. They are
// only taken out of the hit-testing order.
//
// A locator handler cannot fix the interception: it only runs after the
// click's actionability check has already failed, and that check is what
// reports the interception, so the handler never gets the chance to act.
export const DEVELOPER_OVERLAY_GUARD = () => {
  // The guard can run before there is a head to append to. The style is
  // attached again on DOMContentLoaded, and the rule applies to any matching
  // element that shows up later, so no observer over the document is needed.
  const attachStyle = () => {
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

  attachStyle();
  document.addEventListener("DOMContentLoaded", attachStyle);
};

/**
 * Keeps the dev server's error overlay and the app's own toast from covering
 * app actions or moving the test's pointer/focus.
 *
 * The guard is installed on the current document and on every later
 * navigation, because a page that is already loaded when this runs — an
 * Electron shell reached over CDP, for instance — would otherwise never see
 * the init script.
 *
 * @param {import('@playwright/test').Page} page
 */
export async function installDeveloperToastDismissal(page) {
  await page.addInitScript(DEVELOPER_OVERLAY_GUARD);
  await page.evaluate(DEVELOPER_OVERLAY_GUARD);
}
