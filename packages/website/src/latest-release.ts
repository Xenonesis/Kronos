import { getBlockingColdCache, type WebsiteCacheContext } from "./github-cache";

interface GitHubAsset {
  name: string;
}

export interface GitHubRelease {
  tag_name: string;
  assets: GitHubAsset[];
  prerelease: boolean;
  draft: boolean;
}

export interface ReleaseInfo {
  version: string;
  linuxAppImageAsset: string;
  windowsX64Asset: string | null;
  windowsArm64Asset: string | null;
}

export interface ReleaseChannels {
  stable: ReleaseInfo;
  /** The newest prerelease, or null when stable has caught up with it. */
  beta: ReleaseInfo | null;
}

const GITHUB_RELEASES_URL = "https://api.github.com/repos/Xenonesis/Kronos/releases?per_page=10";
const RELEASE_CACHE_KEY = "github-release:kronos:v1";
const ANDROID_RELEASE_CACHE_KEY = "github-android-release:kronos:v1";

function pickWindowsAssets(assets: GitHubAsset[]) {
  const nsisSetup = assets.find((asset) => /Kronos.*-setup\.exe$/i.test(asset.name));
  const genericKronos = assets.find((asset) => /Kronos.*\.exe$/i.test(asset.name));
  const fallback = assets.find((asset) => asset.name.endsWith(".exe"));
  return {
    x64: (nsisSetup ?? genericKronos ?? fallback)?.name ?? null,
    arm64: null,
  };
}

function versionFromTag(tag: string): string {
  return tag.replace(/^v/, "");
}

async function fetchGitHubReleases(): Promise<GitHubRelease[]> {
  const token =
    typeof process !== "undefined" ? process.env?.GITHUB_TOKEN || process.env?.GH_TOKEN : undefined;
  const response = await fetch(GITHUB_RELEASES_URL, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "kronos-website",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    cf: {
      cacheEverything: true,
      cacheTtl: 60,
      cacheKey: "github-releases-latest",
    },
  } as RequestInit);
  if (!response.ok) throw new Error(`github releases ${response.status}`);

  return (await response.json()) as GitHubRelease[];
}

function toReleaseInfo(release: GitHubRelease): ReleaseInfo | null {
  const hasExe = release.assets.some((asset) => asset.name.endsWith(".exe"));
  if (release.draft || !hasExe) return null;

  const version = release.tag_name.replace(/^v/, "");
  const linuxAsset = release.assets.find((asset) => /AppImage$/i.test(asset.name))?.name;
  const linuxAppImageAsset = linuxAsset ?? `Kronos-${version}-x86_64.AppImage`;
  const windowsAssets = pickWindowsAssets(release.assets);
  return {
    version,
    linuxAppImageAsset,
    windowsX64Asset: windowsAssets.x64,
    windowsArm64Asset: windowsAssets.arm64,
  };
}

function coreVersion(version: string): number[] {
  return version.split("-")[0].split(".").map(Number);
}

/**
 * A beta is only worth offering while its core version is ahead of stable.
 * Promotion ships the same core as a stable release, which retires the beta
 * channel until the next beta line opens.
 */
function leadsStable(betaVersion: string, stableVersion: string): boolean {
  const beta = coreVersion(betaVersion);
  const stable = coreVersion(stableVersion);
  for (let index = 0; index < Math.max(beta.length, stable.length); index++) {
    const betaPart = beta[index] ?? 0;
    const stablePart = stable[index] ?? 0;
    if (betaPart !== stablePart) return betaPart > stablePart;
  }
  return false;
}

export function selectReleaseChannels(releases: GitHubRelease[]): ReleaseChannels {
  const stable = releases
    .filter((release) => !release.prerelease)
    .map(toReleaseInfo)
    .find((release) => release !== null);
  if (!stable) return FALLBACK_RELEASE_CHANNELS;

  const beta = releases
    .filter((release) => release.prerelease)
    .map(toReleaseInfo)
    .find((release) => release !== null);

  return { stable, beta: beta && leadsStable(beta.version, stable.version) ? beta : null };
}

const FALLBACK_RELEASE_CHANNELS: ReleaseChannels = {
  stable: {
    version: "0.12.0",
    linuxAppImageAsset: "Kronos-0.12.0-x86_64.AppImage",
    windowsX64Asset: "Kronos_0.12.0_x64-setup.exe",
    windowsArm64Asset: null,
  },
  beta: null,
};

async function fetchReleaseChannels(): Promise<ReleaseChannels> {
  try {
    const releases = await fetchGitHubReleases();
    return selectReleaseChannels(releases);
  } catch {
    return FALLBACK_RELEASE_CHANNELS;
  }
}

export function getLatestAndroidVersionFromReleases(releases: GitHubRelease[]): string {
  const release = releases.find((candidate) => {
    if (candidate.prerelease || candidate.draft) return false;
    const version = versionFromTag(candidate.tag_name);
    if (!/^\d+\.\d+\.\d+$/.test(version)) return false;
    return candidate.assets.some(
      (asset) => asset.name === `paseo-${candidate.tag_name}-android.apk`,
    );
  });
  if (!release) throw new Error("no stable GitHub release with an Android APK found");
  return versionFromTag(release.tag_name);
}

async function fetchLatestAndroidVersion(): Promise<string> {
  return getLatestAndroidVersionFromReleases(await fetchGitHubReleases());
}

function isAndroidVersion(value: unknown): value is string {
  return typeof value === "string" && /^\d+\.\d+\.\d+$/.test(value);
}

function isReleaseInfo(value: unknown): value is ReleaseInfo {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return (
    typeof record.version === "string" &&
    typeof record.linuxAppImageAsset === "string" &&
    (typeof record.windowsX64Asset === "string" || record.windowsX64Asset === null) &&
    (typeof record.windowsArm64Asset === "string" || record.windowsArm64Asset === null)
  );
}

function isReleaseChannels(value: unknown): value is ReleaseChannels {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return isReleaseInfo(record.stable) && (record.beta === null || isReleaseInfo(record.beta));
}

export async function getReleaseChannels(context: WebsiteCacheContext): Promise<ReleaseChannels> {
  return getBlockingColdCache({
    context,
    key: RELEASE_CACHE_KEY,
    isValue: isReleaseChannels,
    fetchFresh: fetchReleaseChannels,
  });
}

export async function getLatestAndroidVersion(context: WebsiteCacheContext): Promise<string> {
  return getBlockingColdCache({
    context,
    key: ANDROID_RELEASE_CACHE_KEY,
    isValue: isAndroidVersion,
    fetchFresh: fetchLatestAndroidVersion,
  });
}
