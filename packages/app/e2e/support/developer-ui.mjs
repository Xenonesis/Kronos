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

  // The LogBox toast carries `role="button"`, and its accessible name can
  // match a control the test is looking for — an error mentioning "Failed to
  // send" collides with the composer's "Send message" button and turns a role
  // query into a strict-mode violation.
  //
  // The overlay is hidden from the accessibility tree rather than removed, so
  // the text it carries is still readable by the text queries that assert on
  // it.
  const hideDevOverlayFromRoles = () => {
    for (const host of Array.from(document.querySelectorAll("#error-toast"))) {
      if (host.getAttribute("aria-hidden") !== "true") {
        host.setAttribute("aria-hidden", "true");
      }
    }
  };

  const apply = () => {
    attachStyle();
    hideDevOverlayFromRoles();
  };

  apply();
  document.addEventListener("DOMContentLoaded", apply);
  // The LogBox host is injected after the first paint, so it still needs
  // watching for. The observer is scoped to the body's direct children so it
  // does not run on every mutation in the app.
  const observer = new MutationObserver(hideDevOverlayFromRoles);
  const observeBody = () => {
    if (document.body) observer.observe(document.body, { childList: true });
  };
  if (document.body) {
    observeBody();
  } else {
    document.addEventListener("DOMContentLoaded", observeBody);
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

export const hideDeveloperToasts = installDeveloperToastDismissal;
