// Installed in the page to take the developer overlays out of the way.
//
// Expo's LogBox renders its error toast into a shadow root hosted on
// `<div id="error-toast">`. The host is absolutely positioned, and the shadow's
// own `:host { all: initial }` puts `pointer-events` back to `auto`, so a rule on
// the host does not stop it intercepting clicks. The host has to be removed, and
// it carries no content a test asserts on.
//
// The app's own toast is a real UI element that tests read their text from, so
// it stays in the document; it only has to stop swallowing clicks.
//
// A locator handler cannot fix the interception: it only runs after the click's
// actionability check has already failed, and that check is what reports the
// interception, so the handler never gets the chance to act.
export const DEVELOPER_OVERLAY_GUARD = () => {
  const attachStyle = () => {
    // The guard can run before there is a head to append to, so the style is
    // attached whenever a document element exists and the observer below keeps
    // trying until it does.
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

  const removeDevOverlay = () => {
    for (const node of Array.from(document.querySelectorAll("#error-toast"))) {
      node.remove();
    }
  };

  const apply = () => {
    attachStyle();
    removeDevOverlay();
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
