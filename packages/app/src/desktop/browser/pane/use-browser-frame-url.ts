import { useEffect, useState } from "react";
import { getDesktopHost } from "@/desktop/host";
import { isWeb } from "@/constants/platform";
import { isLocalhostUrl, resolveIframeTargetUrl } from "./url-utils";

export function browserFrameSandbox(url: string): string {
  // Proxy documents share a daemon URL, not the upstream site's origin. Give
  // each one an opaque origin so hostile sites cannot read other proxied tabs.
  return isLocalhostUrl(url)
    ? "allow-scripts allow-same-origin allow-forms allow-popups allow-modals"
    : "allow-scripts allow-forms allow-popups allow-modals";
}

// Both the browser and navigated file previews must obtain capabilities from
// the listening daemon. A stale resolution must never navigate a newer tab.
export function useBrowserFrameUrl(url: string, revision: number, enabled = true) {
  const resolveUrl = getDesktopHost()?.nativeBrowser?.resolveUrl;
  const native = enabled && Boolean(resolveUrl) && url !== "about:blank" && !isLocalhostUrl(url);
  const [resolved, setResolved] = useState<{
    url: string;
    revision: number;
    src: string | null;
    error: Error | null;
  } | null>(null);
  useEffect(() => {
    if (!native || !resolveUrl) return;
    let disposed = false;
    void resolveUrl(url).then(
      (src) => {
        if (!disposed) setResolved({ url, revision, src, error: null });
      },
      (cause) => {
        if (!disposed)
          setResolved({
            url,
            revision,
            src: null,
            error: cause instanceof Error ? cause : new Error(String(cause)),
          });
      },
    );
    return () => {
      disposed = true;
    };
  }, [native, resolveUrl, url, revision]);
  if (!enabled) return { src: null, error: null };
  if (!native) return { src: resolveIframeTargetUrl(url), error: null };
  return resolved?.url === url && resolved.revision === revision
    ? resolved
    : { src: null, error: null };
}

export async function importBrowserCookies(host: string, cookies: string): Promise<void> {
  const native = getDesktopHost()?.nativeBrowser;
  if (native) return native.importCookies(host, cookies);
  if (!isWeb) throw new Error("Browser session import is unavailable on this platform");
  const response = await fetch(`${window.location.origin}/api/browser-proxy/cookies`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ host, cookies }),
  });
  if (!response.ok) throw new Error("Cookie import rejected by the host");
}
