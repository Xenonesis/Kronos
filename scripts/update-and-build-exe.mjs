import { execSync } from "node:child_process";
import {
  statSync,
  existsSync,
  readdirSync,
  copyFileSync,
  readFileSync,
  writeFileSync,
  mkdirSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import os from "node:os";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

function run(command, cwd = rootDir) {
  console.log(
    `\n\x1b[36m▶ Running:\x1b[0m ${command} \x1b[90m(in ${path.relative(rootDir, cwd) || "."})\x1b[0m`,
  );
  execSync(command, { cwd, stdio: "inherit", shell: true });
}

function getLatestExe(nsisDir) {
  if (!existsSync(nsisDir)) return "";
  const files = readdirSync(nsisDir).filter((f) => f.endsWith("-setup.exe"));
  if (files.length === 0) return "";
  files.sort(
    (a, b) => statSync(path.join(nsisDir, b)).mtimeMs - statSync(path.join(nsisDir, a)).mtimeMs,
  );
  return path.join(nsisDir, files[0]);
}
function getAppVersion() {
  try {
    const tauriConfPath = path.join(
      rootDir,
      "packages",
      "desktop-tauri",
      "src-tauri",
      "tauri.conf.json",
    );
    if (existsSync(tauriConfPath)) {
      const conf = JSON.parse(readFileSync(tauriConfPath, "utf8"));
      if (conf.version) return conf.version;
    }
  } catch {}
  try {
    const pkgPath = path.join(rootDir, "package.json");
    const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
    return pkg.version || "0.13.0";
  } catch {
    return "0.13.0";
  }
}

function getChangedFiles(oldHead, newHead) {
  if (!oldHead || !newHead || oldHead === newHead) return [];
  try {
    const files = execSync(`git diff --name-only ${oldHead} ${newHead}`, { cwd: rootDir })
      .toString()
      .trim()
      .split(/\r?\n/)
      .filter(Boolean);
    console.log(`\x1b[36mℹ ${files.length} file(s) changed in this update.\x1b[0m`);
    return files;
  } catch {
    return [];
  }
}

const SERVER_SOURCE_PREFIXES = [
  "packages/protocol/",
  "packages/client/",
  "packages/relay/",
  "packages/highlight/",
  "packages/plugin/",
  "packages/server/",
];

const SHARED_BUILD_FILES = new Set(["package.json", "package-lock.json"]);

function touchesSources(changedFiles, prefixes) {
  return changedFiles.some(
    (file) => prefixes.some((prefix) => file.startsWith(prefix)) || SHARED_BUILD_FILES.has(file),
  );
}

function needsServerBuild({ force, serverDistExists, changedFiles }) {
  if (!serverDistExists) return true;
  if (force) return true;
  return touchesSources(changedFiles, SERVER_SOURCE_PREFIXES);
}

function needsAppBuild({ force, appDistExists, changedFiles }) {
  if (!appDistExists) return true;
  if (force) return true;
  return touchesSources(changedFiles, ["packages/app/"]);
}

async function main() {
  console.log("\x1b[1m\x1b[32m=== Kronos Desktop Auto-Pull & .EXE Builder ===\x1b[0m");

  const args = new Set(process.argv.slice(2));
  const isRelease = args.has("--release");
  const force = args.has("--force") || isRelease;
  const quick = (args.has("--quick") || args.has("--portable")) && !isRelease;
  const publish = args.has("--publish");
  const nsisDir = path.join(
    rootDir,
    "packages",
    "desktop-tauri",
    "src-tauri",
    "target",
    "release",
    "bundle",
    "nsis",
  );
  const existingExe = getLatestExe(nsisDir);

  // Step 1: Check Git Repository & Pull
  console.log("\n\x1b[33m[1/5] Checking git repository status & pulling updates...\x1b[0m");
  let oldHead = "";
  try {
    oldHead = execSync("git rev-parse HEAD", { cwd: rootDir }).toString().trim();
  } catch {}

  try {
    const pullOutput = execSync("git pull origin main", { cwd: rootDir }).toString();
    console.log(pullOutput.trim());
  } catch {
    console.warn(
      "\x1b[33m⚠️ Git pull encountered an issue, proceeding with local changes...\x1b[0m",
    );
  }

  let newHead = "";
  try {
    newHead = execSync("git rev-parse HEAD", { cwd: rootDir }).toString().trim();
  } catch {}

  const changedFiles = getChangedFiles(oldHead, newHead);

  const serverDistExists = existsSync(
    path.join(rootDir, "packages/desktop-tauri/server-dist/dist"),
  );
  const appDistExists = existsSync(path.join(rootDir, "packages/app/dist"));
  const isUpToDate = oldHead && newHead && oldHead === newHead;

  if (isUpToDate && !force && existingExe && existsSync(existingExe)) {
    console.log("\n\x1b[32m✔ Repository already up to date! Existing installer found:\x1b[0m");
    console.log(`  \x1b[37mPath:\x1b[0m ${existingExe}`);
    console.log(
      "\x1b[90m(Run with '--force' to rebuild anyway: update-desktop-exe.bat --force)\x1b[0m",
    );
    return;
  }

  const serverNeedsBuild = needsServerBuild({ force, serverDistExists, changedFiles });
  const appNeedsBuild = needsAppBuild({ force, appDistExists, changedFiles });

  // Step 2: Build internal packages
  console.log("\n\x1b[33m[2/5] Core packages & server build...\x1b[0m");
  if (serverNeedsBuild) {
    run("npm run build --workspace=@getpaseo/protocol");
    run("npm run build --workspace=@getpaseo/client");
    run("npm run build --workspace=@getpaseo/relay");
    run("npm run build --workspace=@getpaseo/highlight");
    run("npm run build --workspace=@getpaseo/plugin");
    run("npm run build --workspace=@getpaseo/server");
  } else {
    console.log("\x1b[32m⏩ [SKIP] Core packages & server unchanged. Reusing cached build.\x1b[0m");
  }

  // Step 3: Export Web Bundle
  console.log("\n\x1b[33m[3/5] Frontend web bundle export...\x1b[0m");
  if (appNeedsBuild) {
    run(
      "npx cross-env NODE_ENV=production PASEO_WEB_PLATFORM=electron npx expo export --platform web --clear",
      path.join(rootDir, "packages", "app"),
    );
  } else {
    console.log("\x1b[32m⏩ [SKIP] Frontend files unchanged. Reusing cached web export.\x1b[0m");
  }

  // Step 4: Prepare Server Distribution Assets
  console.log("\n\x1b[33m[4/5] Preparing lean server-dist assets...\x1b[0m");
  run("node prepare-dist.mjs", path.join(rootDir, "packages", "desktop-tauri"));

  // Step 5: Compile Tauri Executable / Installer
  const builtQuick = compileTauriBundle(quick, nsisDir);
  if (builtQuick && !isRelease) return;
  // Verification & Packaging
  const exePath = getLatestExe(nsisDir);
  if (exePath && existsSync(exePath)) {
    const stats = statSync(exePath);
    const sizeMb = (stats.size / (1024 * 1024)).toFixed(2);
    console.log("\n\x1b[1m\x1b[32m✔ SUCCESS! Updated installer created:\x1b[0m");
    console.log(`  \x1b[37mPath:\x1b[0m ${exePath}`);
    console.log(`  \x1b[37mSize:\x1b[0m ${sizeMb} MB (${stats.size.toLocaleString()} bytes)`);
    console.log(`  \x1b[37mLast Modified:\x1b[0m ${stats.mtime.toLocaleString()}`);

    const persistentPath = path.join(nsisDir, "Kronos_x64-setup.exe");
    if (exePath !== persistentPath) {
      copyFileSync(exePath, persistentPath);
      console.log(`  \x1b[37mPersistent Latest Asset:\x1b[0m ${persistentPath}`);
    }

    const appVersion = getAppVersion();
    const releaseTag = `v${appVersion}`;

    // Create root dist/ directory and deploy release artifacts
    const distRootDir = path.join(rootDir, "dist");
    mkdirSync(distRootDir, { recursive: true });
    const distInstallerPath = path.join(distRootDir, `Kronos_${appVersion}_x64-setup.exe`);
    copyFileSync(exePath, distInstallerPath);
    console.log(`  \x1b[32m✔ Dist Installer Deployed:\x1b[0m ${distInstallerPath}`);

    // Locate or build standalone release binary for portable target
    const releaseBinary = path.join(
      rootDir,
      "packages",
      "desktop-tauri",
      "src-tauri",
      "target",
      "release",
      "paseo-desktop.exe",
    );
    const tempReleaseBinary = path.join(
      os.tmpdir(),
      "kronos_target",
      "release",
      "paseo-desktop.exe",
    );
    const sourcePortableExe = existsSync(releaseBinary)
      ? releaseBinary
      : existsSync(tempReleaseBinary)
        ? tempReleaseBinary
        : null;

    if (sourcePortableExe) {
      const distPortablePath = path.join(distRootDir, "kronos-portable.exe");
      copyFileSync(sourcePortableExe, distPortablePath);
      const portableStats = statSync(distPortablePath);
      console.log(
        `  \x1b[32m✔ Portable Executable Deployed:\x1b[0m ${distPortablePath} (${(portableStats.size / (1024 * 1024)).toFixed(2)} MB)`,
      );
    } else {
      console.warn("  \x1b[33m⚠️ Release binary not found for portable copy\x1b[0m");
    }

    // Generate latest.json manifest for Tauri v2 updater
    const latestJsonPath = path.join(distRootDir, "latest.json");
    generateUpdateManifest(appVersion, latestJsonPath);

    if (publish) {
      publishToGitHub(exePath, persistentPath, latestJsonPath, releaseTag);
    } else {
      console.log(
        "\n\x1b[36mℹ Note: Built locally. To auto-upload to GitHub Releases and update the live website, run with --publish:\x1b[0m",
      );
      console.log("  \x1b[37mnode scripts/update-and-build-exe.mjs --publish\x1b[0m");
    }
  } else {
    console.error("\n\x1b[31m✖ Error: Installer .exe was not found at expected path.\x1b[0m");
    process.exit(1);
  }
}

function generateUpdateManifest(appVersion, targetPath) {
  const tag = `v${appVersion}`;
  const manifest = {
    version: tag,
    notes: `Kronos ${tag} Release: Instant boot, Windows 11 Mica backdrop, native Myers diff, and dual global hotkeys.`,
    pub_date: new Date().toISOString(),
    platforms: {
      "windows-x86_64": {
        signature: "",
        url: `https://github.com/Xenonesis/Kronos/releases/download/${tag}/Kronos_${appVersion}_x64-setup.exe`,
      },
    },
  };
  writeFileSync(targetPath, JSON.stringify(manifest, null, 2), "utf8");
  console.log(`  \x1b[32m✔ Tauri Updater Manifest Generated:\x1b[0m ${targetPath}`);
}

function publishToGitHub(exePath, persistentPath, latestJsonPath, releaseTag) {
  console.log(
    `\n\x1b[33m[*] Uploading latest .exe and manifest to GitHub Releases (${releaseTag})...\x1b[0m`,
  );
  try {
    try {
      execSync(`gh release view ${releaseTag} --repo Xenonesis/Kronos`, { stdio: "ignore" });
    } catch {
      console.log(`\x1b[36mℹ Release ${releaseTag} does not exist yet. Creating release...\x1b[0m`);
      execSync(
        `gh release create ${releaseTag} --title "Kronos ${releaseTag}" --notes "Automated build of Kronos ${releaseTag}" --repo Xenonesis/Kronos`,
        { stdio: "inherit" },
      );
    }
    const uploadFiles = [exePath, persistentPath];
    if (latestJsonPath && existsSync(latestJsonPath)) {
      uploadFiles.push(latestJsonPath);
    }
    const uploadArgs = uploadFiles.map((f) => `"${f}"`).join(" ");
    execSync(`gh release upload ${releaseTag} ${uploadArgs} --clobber --repo Xenonesis/Kronos`, {
      stdio: "inherit",
    });
    console.log(
      `\x1b[32m✔ Uploaded latest installer and updater manifest to GitHub Releases (${releaseTag})!\x1b[0m`,
    );

    console.log("\x1b[33m[*] Triggering GitHub Pages website deployment...\x1b[0m");
    try {
      execSync(`gh workflow run "Deploy to GitHub Pages" --repo Xenonesis/Kronos`, {
        stdio: "inherit",
      });
      console.log("\x1b[32m✔ GitHub Pages website deployment triggered!\x1b[0m");
    } catch (wfErr) {
      console.warn(
        "\x1b[33m⚠ Note: Could not trigger GitHub Pages workflow automatically:\x1b[0m",
        wfErr.message,
      );
    }
  } catch (e) {
    console.warn(
      "\x1b[33m⚠ Warning: Could not upload to GitHub Releases automatically.\x1b[0m",
      e.message,
    );
  }
}
function compileTauriBundle(quick, nsisDir) {
  if (quick) {
    console.log(
      "\n\x1b[33m[5/5] Compiling Fast Release Binary (skipping NSIS compression)...\x1b[0m",
    );
    run(
      "cargo build --release --manifest-path src-tauri/Cargo.toml",
      path.join(rootDir, "packages", "desktop-tauri"),
    );
    const releaseExe = path.join(
      rootDir,
      "packages",
      "desktop-tauri",
      "src-tauri",
      "target",
      "release",
      "paseo-desktop.exe",
    );
    if (existsSync(releaseExe)) {
      const stats = statSync(releaseExe);
      console.log("\n\x1b[1m\x1b[32m✔ SUCCESS! Fast release executable created:\x1b[0m");
      console.log(`  \x1b[37mPath:\x1b[0m ${releaseExe}`);
      console.log(`  \x1b[37mSize:\x1b[0m ${(stats.size / (1024 * 1024)).toFixed(2)} MB`);
      return true;
    }
  }

  console.log("\n\x1b[33m[5/5] Compiling Tauri NSIS installer .exe...\x1b[0m");
  const tempTargetDir = path.join(os.tmpdir(), "kronos_target");
  try {
    run("npx tauri build --bundles nsis", path.join(rootDir, "packages", "desktop-tauri"));
  } catch {
    console.log(
      "\x1b[33mTarget directory locked by running process; compiling with CARGO_TARGET_DIR fallback...\x1b[0m",
    );
    run(
      `npx cross-env CARGO_TARGET_DIR="${tempTargetDir}" npx tauri build --bundles nsis`,
      path.join(rootDir, "packages", "desktop-tauri"),
    );
    const appVersion = getAppVersion();
    const versionedName = `Kronos_${appVersion}_x64-setup.exe`;
    const tempBundle = path.join(tempTargetDir, "release", "bundle", "nsis", versionedName);
    if (existsSync(tempBundle)) {
      copyFileSync(tempBundle, path.join(nsisDir, versionedName));
      copyFileSync(tempBundle, path.join(nsisDir, "Kronos_x64-setup.exe"));
    }
  }
  return false;
}

main().catch((err) => {
  console.error("\n\x1b[31m✖ Build failed:\x1b[0m", err);
  process.exit(1);
});
