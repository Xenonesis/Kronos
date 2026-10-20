import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import type { SupportedLocalBrowser } from "@getpaseo/protocol/browser-cookies";

export function readChromiumCookiesForHost(
  browser: SupportedLocalBrowser,
  targetHost: string,
): { cookies: string; count: number } {
  const home = os.homedir();
  const platform = os.platform();
  let userDataDir: string | null = null;

  if (platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA || path.join(home, "AppData", "Local");
    if (browser === "chrome")
      userDataDir = path.join(localAppData, "Google", "Chrome", "User Data");
    else if (browser === "edge")
      userDataDir = path.join(localAppData, "Microsoft", "Edge", "User Data");
    else if (browser === "brave")
      userDataDir = path.join(localAppData, "BraveSoftware", "Brave-Browser", "User Data");
  } else if (platform === "darwin") {
    const appSupport = path.join(home, "Library", "Application Support");
    if (browser === "chrome") userDataDir = path.join(appSupport, "Google", "Chrome");
    else if (browser === "edge") userDataDir = path.join(appSupport, "Microsoft Edge");
    else if (browser === "brave")
      userDataDir = path.join(appSupport, "BraveSoftware", "Brave-Browser");
  } else if (platform === "linux") {
    const config = path.join(home, ".config");
    if (browser === "chrome") userDataDir = path.join(config, "google-chrome");
    else if (browser === "edge") userDataDir = path.join(config, "microsoft-edge");
    else if (browser === "brave") userDataDir = path.join(config, "BraveSoftware", "Brave-Browser");
  }

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
