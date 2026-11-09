import { createHash, createPublicKey, verify } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { isMainModule } from "./is-main-module.mjs";
import { parseReleaseVersion } from "./release-version-utils.mjs";
import { getGitHubRelease } from "./github-release.mjs";

const repository = "Xenonesis/Kronos";

function decodeBase64(value, label) {
  if (
    typeof value !== "string" ||
    !value ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)
  ) {
    throw new Error(`Missing or invalid ${label}`);
  }
  return Buffer.from(value, "base64");
}

export function decodeUpdaterPublicKey(value) {
  const lines = decodeBase64(value?.trim(), "updater public key")
    .toString("utf8")
    .trimEnd()
    .split(/\r?\n/);
  if (lines.length !== 2 || !lines[0].startsWith("untrusted comment: ")) {
    throw new Error("Invalid updater public key: expected base64-encoded minisign public key file");
  }
  const key = decodeBase64(lines[1], "updater public key");
  if (key.length !== 42 || !["Ed", "ED"].includes(key.subarray(0, 2).toString())) {
    throw new Error("Invalid updater public key format");
  }
  return key;
}

// Mirrors tauri-plugin-updater's minisign verification, including the trusted
// comment signature. Tauri accepts both legacy Ed and prehashed ED signatures.
export function verifyUpdaterSignature(bytes, encodedSignature, publicKey) {
  const key = decodeUpdaterPublicKey(publicKey);
  const lines = decodeBase64(encodedSignature?.trim(), "updater signature")
    .toString("utf8")
    .trimEnd()
    .split(/\r?\n/);
  if (
    lines.length !== 4 ||
    !lines[0].startsWith("untrusted comment: ") ||
    !lines[2].startsWith("trusted comment: ")
  ) {
    throw new Error("Invalid updater signature format");
  }
  const packet = decodeBase64(lines[1], "updater signature");
  const global = decodeBase64(lines[3], "updater signature");
  if (
    packet.length !== 74 ||
    global.length !== 64 ||
    !["Ed", "ED"].includes(packet.subarray(0, 2).toString())
  ) {
    throw new Error("Invalid updater signature packet");
  }
  if (!packet.subarray(2, 10).equals(key.subarray(2, 10))) {
    throw new Error("Updater signature uses a different public key");
  }
  const ed25519 = createPublicKey({
    key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), key.subarray(10)]),
    type: "spki",
    format: "der",
  });
  const signature = packet.subarray(10);
  const data =
    packet.subarray(0, 2).toString() === "ED"
      ? createHash("blake2b512").update(bytes).digest()
      : bytes;
  if (
    !verify(null, data, ed25519, signature) ||
    !verify(null, Buffer.concat([signature, Buffer.from(lines[2].slice(17))]), ed25519, global)
  ) {
    throw new Error("Updater signature verification failed");
  }
}

export function requireSigning(env = process.env) {
  const publicKey = env.TAURI_SIGNING_PUBLIC_KEY?.trim();
  decodeUpdaterPublicKey(publicKey);
  if (!env.TAURI_SIGNING_PRIVATE_KEY?.trim()) {
    throw new Error(
      "Signed releases require TAURI_SIGNING_PRIVATE_KEY (and TAURI_SIGNING_PRIVATE_KEY_PASSWORD when encrypted)",
    );
  }
  return publicKey;
}

function regularFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(directory, entry.name);
    if (entry.isDirectory()) return regularFiles(full);
    if (!entry.isFile()) throw new Error(`Release artifact must be a regular file: ${full}`);
    return [full];
  });
}

function nonemptyFile(file) {
  const stat = fs.lstatSync(file);
  if (!stat.isFile() || stat.size === 0)
    throw new Error(`Missing, empty or non-regular release artifact: ${file}`);
}

function checkVersion(version) {
  parseReleaseVersion(version);
  return version;
}

export function validateDesktopVersion(rootDir, expectedVersion) {
  const config = JSON.parse(
    fs.readFileSync(path.join(rootDir, "packages/desktop-tauri/src-tauri/tauri.conf.json"), "utf8"),
  );
  const desktop = JSON.parse(
    fs.readFileSync(path.join(rootDir, "packages/desktop-tauri/package.json"), "utf8"),
  );
  const cargo = fs
    .readFileSync(path.join(rootDir, "packages/desktop-tauri/src-tauri/Cargo.toml"), "utf8")
    .match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const version = checkVersion(expectedVersion || config.version);
  for (const actual of [config.version, desktop.version, cargo]) {
    if (actual !== version)
      throw new Error(
        `Desktop version mismatch: expected ${version}; synchronize Tauri, Cargo and desktop package versions`,
      );
  }
  const lockPath = path.join(rootDir, "packages/desktop-tauri/src-tauri/Cargo.lock");
  if (fs.existsSync(lockPath)) {
    const lockSource = fs.readFileSync(lockPath, "utf8");
    const targetBlock = lockSource
      .split(/\[\[package\]\]/)
      .find((block) => /^\s*name\s*=\s*"paseo-desktop"/m.test(block));
    const lockVersion = targetBlock?.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1];
    if (lockVersion && lockVersion !== version) {
      throw new Error(
        `Desktop version mismatch in Cargo.lock: expected ${version}, found ${lockVersion}; synchronize Tauri, Cargo and desktop package versions`,
      );
    }
  }
  return version;
}

function artifactUrl(version, filename) {
  return `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(filename)}`;
}

export function validateSignedBundle({ bundleDir, version, platform, arch, publicKey }) {
  checkVersion(version);
  decodeUpdaterPublicKey(publicKey);
  const extensions = {
    windows: [".exe"],
    linux: [".AppImage", ".deb"],
    darwin: [".app.tar.gz", ".dmg"],
  };
  const formats = {
    windows: [["-setup.exe", ""]],
    linux: [
      [".AppImage", "-appimage"],
      [".deb", "-deb"],
    ],
    darwin: [[".app.tar.gz", ""]],
  }[platform];
  if (!formats || !["x86_64", "aarch64"].includes(arch))
    throw new Error("Unsupported release platform/architecture");
  // Inspect only final bundle files, never the AppDir/.app staging trees (which
  // legitimately contain symlinks and are not distributed as standalone assets).
  const directories = { windows: ["nsis"], linux: ["appimage", "deb"], darwin: ["macos", "dmg"] }[
    platform
  ];
  const files = directories.flatMap((directory) => {
    const full = path.join(bundleDir, directory);
    return fs.existsSync(full)
      ? fs
          .readdirSync(full, { withFileTypes: true })
          .filter((entry) => !entry.isDirectory())
          .map((entry) => path.join(full, entry.name))
      : [];
  });
  const updaters = formats.flatMap(([extension, suffix]) => {
    const candidates = files.filter(
      (file) =>
        file.endsWith(extension) &&
        (platform === "darwin" || path.basename(file).includes(`_${version}_`)),
    );
    if (candidates.length > 1 || (platform !== "linux" && candidates.length !== 1))
      throw new Error(`Expected one current ${platform}${suffix} updater artifact`);
    return candidates.map((file) => {
      nonemptyFile(file);
      nonemptyFile(`${file}.sig`);
      const signature = fs.readFileSync(`${file}.sig`, "utf8").trim();
      verifyUpdaterSignature(fs.readFileSync(file), signature, publicKey);
      return { file, signature, target: `${platform}-${arch}${suffix}` };
    });
  });
  if (!updaters.length) throw new Error(`No current ${platform} updater artifacts found`);
  const releaseFiles = files.filter(
    (file) =>
      extensions[platform].some((extension) => file.endsWith(extension)) &&
      (platform !== "windows" || path.basename(file) !== "Kronos_x64-setup.exe"),
  );
  for (const file of releaseFiles) {
    nonemptyFile(file);
    if (platform !== "darwin" && !path.basename(file).includes(`_${version}_`)) {
      throw new Error(`Stale artifact in release bundle: ${path.basename(file)}`);
    }
  }
  return { updaters, releaseFiles };
}

export function stageRelease(options) {
  const { outputDir, version, platform, arch } = options;
  const { updaters, releaseFiles } = validateSignedBundle(options);
  fs.rmSync(outputDir, { recursive: true, force: true });
  fs.mkdirSync(outputDir, { recursive: true });
  // macOS updater archives omit the architecture; distinguish the two runners.
  const assetName = (file) =>
    platform === "darwin" ? `${arch}-${path.basename(file)}` : path.basename(file);
  for (const file of releaseFiles) fs.copyFileSync(file, path.join(outputDir, assetName(file)));
  const platforms = {};
  for (const { file, signature, target } of updaters) {
    fs.copyFileSync(`${file}.sig`, path.join(outputDir, `${assetName(file)}.sig`));
    platforms[target] = { signature, url: artifactUrl(version, assetName(file)) };
    if (target.endsWith("-appimage")) platforms[`linux-${arch}`] = platforms[target];
  }
  if (platform === "windows") {
    fs.copyFileSync(updaters[0].file, path.join(outputDir, "Kronos_x64-setup.exe"));
    fs.copyFileSync(`${updaters[0].file}.sig`, path.join(outputDir, "Kronos_x64-setup.exe.sig"));
  }
  const manifest = {
    version,
    notes: `Kronos v${version}`,
    pub_date: new Date().toISOString(),
    platforms,
  };
  fs.writeFileSync(
    path.join(outputDir, `latest-${platform}-${arch}.json`),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  return manifest;
}

function updaterFilename(entry, version) {
  const filename = decodeURIComponent(new URL(entry.url).pathname.split("/").at(-1));
  if (filename !== path.basename(filename) || entry.url !== artifactUrl(version, filename))
    throw new Error("Unexpected updater asset URL");
  return filename;
}

function validatePlatformEntries(manifest, directory, version, publicKey) {
  if (manifest.version !== version) throw new Error("Release metadata version mismatch");
  const entries = Object.entries(manifest.platforms || {});
  if (!entries.length) throw new Error("Invalid platform metadata");
  for (const [platform, entry] of entries) {
    if (!/^(?:windows|darwin|linux)-(?:x86_64|aarch64)(?:-(?:appimage|deb))?$/.test(platform))
      throw new Error(`Unsupported updater platform: ${platform}`);
    const artifact = path.join(directory, updaterFilename(entry, version));
    nonemptyFile(artifact);
    nonemptyFile(`${artifact}.sig`);
    if (
      (platform.endsWith("-deb") && !entry.url.endsWith(".deb")) ||
      (platform.endsWith("-appimage") && !entry.url.endsWith(".AppImage"))
    )
      throw new Error("Updater artifact format does not match platform target");
    if (fs.readFileSync(`${artifact}.sig`, "utf8").trim() !== entry.signature)
      throw new Error("Metadata signature differs from built sidecar");
    if (platform === "windows-x86_64") {
      const alias = path.join(directory, "Kronos_x64-setup.exe");
      nonemptyFile(alias);
      if (
        !fs.readFileSync(alias).equals(fs.readFileSync(artifact)) ||
        fs.readFileSync(`${alias}.sig`, "utf8").trim() !== entry.signature
      )
        throw new Error("Stable installer alias differs from signed installer");
    }
    verifyUpdaterSignature(fs.readFileSync(artifact), entry.signature, publicKey);
  }
}

export function mergeReleases({
  inputDir,
  outputDir,
  version,
  publicKey,
  previousDir,
  requiredPlatforms = [],
}) {
  checkVersion(version);
  decodeUpdaterPublicKey(publicKey);
  const manifests = regularFiles(inputDir).filter((file) =>
    /^latest-(windows|darwin|linux)-.*\.json$/.test(path.basename(file)),
  );
  if (!manifests.length) throw new Error("No signed platform metadata found");
  const platforms = {};
  const assets = new Map();
  const addAssets = (directory, replace = false) => {
    for (const file of regularFiles(directory).filter((f) => !f.endsWith(".json"))) {
      nonemptyFile(file);
      const name = path.basename(file);
      if (!replace && assets.has(name)) throw new Error(`Duplicate release asset: ${name}`);
      assets.set(name, file);
    }
  };
  if (previousDir && fs.existsSync(path.join(previousDir, "latest.json"))) {
    const manifest = JSON.parse(fs.readFileSync(path.join(previousDir, "latest.json"), "utf8"));
    validatePlatformEntries(manifest, previousDir, version, publicKey);
    Object.assign(platforms, manifest.platforms);
    addAssets(previousDir);
  }
  const freshPlatforms = new Set();
  const freshAssets = new Set();
  for (const manifestPath of manifests) {
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    const directory = path.dirname(manifestPath);
    validatePlatformEntries(manifest, directory, version, publicKey);
    for (const [platform, entry] of Object.entries(manifest.platforms)) {
      if (freshPlatforms.has(platform)) throw new Error(`Duplicate platform metadata: ${platform}`);
      freshPlatforms.add(platform);
      platforms[platform] = entry;
    }
    for (const file of regularFiles(directory).filter((f) => !f.endsWith(".json"))) {
      const name = path.basename(file);
      if (freshAssets.has(name)) throw new Error(`Duplicate release asset: ${name}`);
      freshAssets.add(name);
    }
    addAssets(directory, true);
  }
  const missing = requiredPlatforms.filter((platform) => !platforms[platform]);
  if (missing.length)
    throw new Error(
      `Missing previously published platforms: ${missing.join(", ")}. Build those platforms for this version before publication.`,
    );
  fs.rmSync(outputDir, { recursive: true, force: true });
  fs.mkdirSync(outputDir, { recursive: true });
  for (const [name, file] of assets) fs.copyFileSync(file, path.join(outputDir, name));
  const manifest = {
    version,
    notes: `Kronos v${version}`,
    pub_date: new Date().toISOString(),
    platforms,
  };
  fs.writeFileSync(path.join(outputDir, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

// Hydrate only updater assets named by this same-version release's manifest.
// A newer single-version manifest cannot safely advertise older platform binaries:
// the published latest release therefore also supplies a no-platform-loss guard.
export function fetchPreviousRelease({ version, outputDir, requiredPath, publicKey }) {
  checkVersion(version);
  decodeUpdaterPublicKey(publicKey);
  const gh = (args, options = {}) =>
    execFileSync("gh", args, { stdio: ["ignore", "pipe", "pipe"], ...options });
  const download = (release, name, destination) => {
    const asset = release.assets?.find((candidate) => candidate.name === name);
    if (!asset) throw new Error(`Published updater asset is missing: ${name}`);
    const descriptor = fs.openSync(destination, "w");
    try {
      gh(
        [
          "api",
          `repos/${repository}/releases/assets/${asset.id}`,
          "-H",
          "Accept: application/octet-stream",
        ],
        { stdio: ["ignore", descriptor, "pipe"] },
      );
    } finally {
      fs.closeSync(descriptor);
    }
  };
  const target = getGitHubRelease(repository, `v${version}`);
  let latest;
  try {
    latest = JSON.parse(gh(["api", `repos/${repository}/releases/latest`], { encoding: "utf8" }));
  } catch (error) {
    if (!String(error.stderr || "").includes("HTTP 404")) throw error;
  }
  fs.rmSync(outputDir, { recursive: true, force: true });
  fs.mkdirSync(outputDir, { recursive: true });
  const requiredPlatforms = new Set();
  if (latest?.assets?.some((asset) => asset.name === "latest.json")) {
    const latestPath = path.join(outputDir, "published-platforms.json");
    download(latest, "latest.json", latestPath);
    const manifest = JSON.parse(fs.readFileSync(latestPath, "utf8"));
    checkVersion(manifest.version);
    for (const [platform, entry] of Object.entries(manifest.platforms || {})) {
      updaterFilename(entry, manifest.version);
      requiredPlatforms.add(platform);
    }
  }
  if (target?.assets?.some((asset) => asset.name === "latest.json")) {
    download(target, "latest.json", path.join(outputDir, "latest.json"));
    const manifest = JSON.parse(fs.readFileSync(path.join(outputDir, "latest.json"), "utf8"));
    if (manifest.version !== version) throw new Error("Existing release manifest version mismatch");
    const names = new Set();
    for (const [platform, entry] of Object.entries(manifest.platforms || {})) {
      requiredPlatforms.add(platform);
      const filename = updaterFilename(entry, version);
      names.add(filename);
      names.add(`${filename}.sig`);
      if (platform === "windows-x86_64") {
        names.add("Kronos_x64-setup.exe");
        names.add("Kronos_x64-setup.exe.sig");
      }
    }
    for (const name of names) download(target, name, path.join(outputDir, name));
    validatePlatformEntries(manifest, outputDir, version, publicKey);
  }
  fs.writeFileSync(requiredPath, `${JSON.stringify([...requiredPlatforms])}\n`);
}

if (isMainModule(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  const options = {};
  for (let index = 0; index < args.length; index += 2) {
    if (!args[index]?.startsWith("--") || !args[index + 1])
      throw new Error("Expected --option value");
    options[args[index].slice(2)] = args[index + 1];
  }
  if (command === "check-signing") requireSigning();
  else if (command === "stage")
    stageRelease({
      bundleDir: options.bundle,
      outputDir: options.output,
      version: options.version,
      platform: options.platform,
      arch: options.arch,
      publicKey: process.env.TAURI_SIGNING_PUBLIC_KEY,
    });
  else if (command === "fetch-previous")
    fetchPreviousRelease({
      version: options.version,
      outputDir: options.output,
      requiredPath: options.required,
      publicKey: process.env.TAURI_SIGNING_PUBLIC_KEY,
    });
  else if (command === "merge")
    mergeReleases({
      inputDir: options.input,
      outputDir: options.output,
      version: options.version,
      publicKey: process.env.TAURI_SIGNING_PUBLIC_KEY,
      previousDir: options.previous,
      requiredPlatforms: options.required
        ? JSON.parse(fs.readFileSync(options.required, "utf8"))
        : [],
    });
  else throw new Error("Expected check-signing, stage, fetch-previous or merge command");
}
