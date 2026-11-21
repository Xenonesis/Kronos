import { getDesktopHost } from "@/desktop/host";

/**
 * Returns the active daemon HTTP base URL (e.g. "http://127.0.0.1:52341").
 * In desktop (Tauri) mode, this resolves dynamically from the initial connection hint,
 * live window.location, or fallback.
 */
export function getDaemonBaseUrl(): string {
  if (typeof window !== "undefined") {
    // 1. Check window.__PASEO_INITIAL_DAEMON_CONNECTION__ populated by Tauri bridge
    const hint = (
      window as unknown as { __PASEO_INITIAL_DAEMON_CONNECTION__?: { listen?: string } | string }
    ).__PASEO_INITIAL_DAEMON_CONNECTION__;
    if (hint) {
      const listen = typeof hint === "string" ? hint : hint.listen;
      if (listen) {
        const clean = listen.replace(/^http:\/\//, "").replace(/\/.*$/, "");
        return `http://${clean}`;
      }
    }

    // 2. If running inside web / browser context (not tauri.localhost)
    if (window.location?.origin && window.location.hostname !== "tauri.localhost") {
      return window.location.origin;
    }
  }

  // 3. Fallback
  return "http://127.0.0.1:6767";
}
