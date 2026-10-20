export function isLocalhostUrl(urlStr: string): boolean {
  try {
    const parsed = new URL(urlStr.startsWith("http") ? urlStr : `https://${urlStr}`);
    const host = parsed.hostname.toLowerCase();
    return (
      host === "localhost" ||
      host === "127.0.0.1" ||
      host === "0.0.0.0" ||
      host.endsWith(".local") ||
      host.endsWith(".internal")
    );
  } catch {
    return false;
  }
}

const LOOKS_LIKE_DOMAIN_PATTERN = /^[^\s/:]+\.[a-z]{2,}(?::\d+)?(?:[/?#].*)?$/i;
const LOCAL_ADDRESS_PATTERN =
  /^(?:localhost|127(?:\.\d{1,3}){3}|0\.0\.0\.0)(?::\d+)?(?:[/?#].*)?$/i;

export function normalizeAddressBarInput(input: string, searchEngine = "google"): string {
  const trimmed = input.trim();
  if (!trimmed || trimmed === "about:blank") return "about:blank";

  // Already a full URL
  if (
    trimmed.startsWith("http://") ||
    trimmed.startsWith("https://") ||
    trimmed.startsWith("about:") ||
    trimmed.startsWith("file://")
  ) {
    return trimmed;
  }

  // Localhost or loopback IP
  if (LOCAL_ADDRESS_PATTERN.test(trimmed)) {
    return `http://${trimmed}`;
  }

  // Plain domain or domain with port/path (e.g. "google.com", "example.com:8080/test")
  if (LOOKS_LIKE_DOMAIN_PATTERN.test(trimmed)) {
    return `https://${trimmed}`;
  }

  // Plain text query or search keywords (e.g. "google", "weather tomorrow", "react hooks")
  const query = encodeURIComponent(trimmed);
  switch (searchEngine) {
    case "duckduckgo":
      return `https://duckduckgo.com/?q=${query}`;
    case "bing":
      return `https://www.bing.com/search?q=${query}`;
    case "google":
    default:
      return `https://www.google.com/search?q=${query}`;
  }
}

export function resolveIframeTargetUrl(rawUrl: string): string {
  if (!rawUrl || rawUrl === "about:blank") return "about:blank";

  let url = rawUrl.trim();
  if (!url.startsWith("http://") && !url.startsWith("https://")) {
    url = normalizeAddressBarInput(url);
  }

  // If it's a local development server, load it directly so hot reloading works
  if (isLocalhostUrl(url)) {
    return url;
  }

  // For external websites (e.g. google.com, github.com, etc.), route through
  // our daemon browser reverse proxy to strip X-Frame-Options and frame-ancestors!
  let daemonBase = "http://127.0.0.1:6767";
  if (typeof window !== "undefined" && window.location?.origin) {
    if (window.location.hostname !== "tauri.localhost") {
      daemonBase = window.location.origin;
    }
  }

  return `${daemonBase}/api/browser-proxy?url=${encodeURIComponent(url)}`;
}
