import { execSync } from "node:child_process";
import { statSync, existsSync, readdirSync, copyFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
  if (force || !serverDistExists) return true;
  if (changedFiles.length === 0) return true;
  return touchesSources(changedFiles, SERVER_SOURCE_PREFIXES);
}

function needsAppBuild({ force, appDistExists, changedFiles }) {
  if (force || !appDistExists) return true;
  if (changedFiles.length === 0) return true;
  return touchesSources(changedFiles, ["packages/app/"]);
}

async function main() {
  console.log("\x1b[1m\x1b[32m=== Kronos Desktop Auto-Pull & .EXE Builder ===\x1b[0m");

  const args = new Set(process.argv.slice(2));
  const force = args.has("--force");
  const quick = args.has("--quick") || args.has("--portable");
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
    run("npx expo export --platform web", path.join(rootDir, "packages", "app"));
  } else {
    console.log("\x1b[32m⏩ [SKIP] Frontend files unchanged. Reusing cached web export.\x1b[0m");
  }

  // Step 4: Prepare Server Distribution Assets
  console.log("\n\x1b[33m[4/5] Preparing lean server-dist assets...\x1b[0m");
  run("node prepare-dist.mjs", path.join(rootDir, "packages", "desktop-tauri"));

  // Step 5: Compile Tauri Executable / Installer
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
      return;
    }
  }

  console.log("\n\x1b[33m[5/5] Compiling Tauri NSIS installer .exe...\x1b[0m");
  run("npx tauri build --bundles nsis", path.join(rootDir, "packages", "desktop-tauri"));

  // Verification
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

    if (publish) {
      console.log("\n\x1b[33m[*] Uploading latest .exe to GitHub Releases...\x1b[0m");
      try {
        execSync(
          `gh release upload v0.12.0 "${exePath}" "${persistentPath}" --clobber --repo Xenonesis/Kronos`,
          { stdio: "inherit" },
        );
        console.log("\x1b[32m✔ Uploaded latest installer to GitHub Releases!\x1b[0m");
      } catch (e) {
        console.warn(
          "\x1b[33m⚠ Warning: Could not upload to GitHub Releases automatically.\x1b[0m",
          e.message,
        );
      }
    }
  } else {
    console.error("\n\x1b[31m✖ Error: Installer .exe was not found at expected path.\x1b[0m");
    process.exit(1);
  }
}

main().catch((err) => {
  console.error("\n\x1b[31m✖ Build failed:\x1b[0m", err);
  process.exit(1);
});
