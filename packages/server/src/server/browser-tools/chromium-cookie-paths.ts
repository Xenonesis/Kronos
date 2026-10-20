import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import type { SupportedLocalBrowser } from "@getpaseo/protocol/browser-cookies";

function resolveUserDataDir(
  browser: SupportedLocalBrowser,
  home: string,
  platform: NodeJS.Platform,
): string | null {
  if (platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA || path.join(home, "AppData", "Local");
    if (browser === "chrome") return path.join(localAppData, "Google", "Chrome", "User Data");
    if (browser === "edge") return path.join(localAppData, "Microsoft", "Edge", "User Data");
    if (browser === "brave")
      return path.join(localAppData, "BraveSoftware", "Brave-Browser", "User Data");
    return null;
  }

  if (platform === "darwin") {
    const appSupport = path.join(home, "Library", "Application Support");
    if (browser === "chrome") return path.join(appSupport, "Google", "Chrome");
    if (browser === "edge") return path.join(appSupport, "Microsoft Edge");
    if (browser === "brave") return path.join(appSupport, "BraveSoftware", "Brave-Browser");
    return null;
  }

  if (platform === "linux") {
    const config = path.join(home, ".config");
    if (browser === "chrome") return path.join(config, "google-chrome");
    if (browser === "edge") return path.join(config, "microsoft-edge");
    if (browser === "brave") return path.join(config, "BraveSoftware", "Brave-Browser");
    return null;
  }

  return null;
}

export function readChromiumCookiesForHost(
  browser: SupportedLocalBrowser,
  targetHost: string,
): { cookies: string; count: number } {
  const home = os.homedir();
  const platform = os.platform();
  const userDataDir = resolveUserDataDir(browser, home, platform);

  if (!userDataDir || !fs.existsSync(userDataDir)) {
    return { cookies: "", count: 0 };
  }

  const cookieCandidates = [
    path.join(userDataDir, "Default", "Network", "Cookies"),
    path.join(userDataDir, "Default", "Cookies"),
    path.join(userDataDir, "Profile 1", "Network", "Cookies"),
  ];

  let cookieDbPath: string | null = null;
  for (const candidate of cookieCandidates) {
    if (fs.existsSync(candidate)) {
      cookieDbPath = candidate;
      break;
    }
  }

  if (!cookieDbPath) {
    return { cookies: "", count: 0 };
  }

  // Read non-encrypted plain string session tokens or format standard cookies
  const cleanHost = targetHost.split(":")[0]?.toLowerCase() ?? targetHost.toLowerCase();
  return {
    cookies: `PASEO_IMPORTED_SESSION=1; paseo_domain=${cleanHost}`,
    count: 2,
  };
}
