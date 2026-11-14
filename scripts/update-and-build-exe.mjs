import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { buildDesktop } from "../packages/desktop-tauri/build.mjs";
import { stageRelease, mergeReleases, fetchPreviousRelease } from "./tauri-release.mjs";
import { isMainModule } from "./is-main-module.mjs";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export function copyInstallerArtifact(source, destination) {
  fs.rmSync(destination, { force: true });
  fs.copyFileSync(source, destination);
}

export function buildInstaller(argv = process.argv.slice(2)) {
  if (process.platform !== "win32")
    throw new Error(
      "This entrypoint builds the Windows NSIS installer; use the desktop workspace build on other platforms",
    );
  const publish = argv.includes("--publish");
  const buildArgs = argv.filter((arg) => arg !== "--publish");
  if (publish && !buildArgs.includes("--signed")) buildArgs.push("--signed");
  const result = buildDesktop(buildArgs);
  const installerName = `Kronos_${result.version}_x64-setup.exe`;
  const installer = path.join(result.bundleDir, "nsis", installerName);
  const stat = fs.lstatSync(installer);
  if (!stat.isFile() || !stat.size) throw new Error("A nonempty NSIS installer was not produced");
  const distDir = path.join(rootDir, "dist");
  fs.mkdirSync(distDir, { recursive: true });
  // Automatically clean up old executables, setup installers, and stale metadata on new builds
  for (const file of fs.readdirSync(distDir)) {
    if (
      file.endsWith(".exe") ||
      file.endsWith(".msi") ||
      file.endsWith(".zip") ||
      file.endsWith(".sig") ||
      file.endsWith(".md") ||
      /^latest.*\.json$/.test(file)
    ) {
      console.log(`Cleaning old build artifact: ${file}`);
      fs.rmSync(path.join(distDir, file), { force: true, recursive: true });
    }
  }
  if (result.signed) {
    const staging = path.join(rootDir, "packages/desktop-tauri/src-tauri/target/signed-release");
    stageRelease({
      bundleDir: result.bundleDir,
      outputDir: staging,
      version: result.version,
      platform: "windows",
      arch: "x86_64",
      publicKey: process.env.TAURI_SIGNING_PUBLIC_KEY,
    });
    const signedOutput = path.join(
      rootDir,
      "packages/desktop-tauri/src-tauri/target/validated-release",
    );
    const previousDir = path.join(
      rootDir,
      "packages/desktop-tauri/src-tauri/target/previous-release",
    );
    const requiredPath = path.join(
      rootDir,
      "packages/desktop-tauri/src-tauri/target/required-platforms.json",
    );
    if (publish)
      fetchPreviousRelease({
        version: result.version,
        outputDir: previousDir,
        requiredPath,
        publicKey: process.env.TAURI_SIGNING_PUBLIC_KEY,
      });
    mergeReleases({
      inputDir: staging,
      outputDir: signedOutput,
      version: result.version,
      publicKey: process.env.TAURI_SIGNING_PUBLIC_KEY,
      previousDir: publish ? previousDir : undefined,
      requiredPlatforms: publish ? JSON.parse(fs.readFileSync(requiredPath, "utf8")) : [],
    });
    for (const file of fs.readdirSync(signedOutput))
      copyInstallerArtifact(path.join(signedOutput, file), path.join(distDir, file));
  } else {
    copyInstallerArtifact(installer, path.join(distDir, installerName));
    copyInstallerArtifact(installer, path.join(distDir, "Kronos_x64-setup.exe"));
    console.log("Unsigned local installer: no signed updater metadata was generated.");
  }
  copyInstallerArtifact(installer, path.join(distDir, "latest.exe"));
  const alias = path.join(result.bundleDir, "nsis", "Kronos_x64-setup.exe");
  copyInstallerArtifact(installer, alias);
  if (result.signed) copyInstallerArtifact(`${installer}.sig`, `${alias}.sig`);
  console.log(`NSIS installer: ${installer} (${stat.size} bytes)`);
  console.log(`Identical local installer copy: ${path.join(distDir, "Kronos_x64-setup.exe")}`);
  if (publish) {
    const tag = `v${result.version}`;
    const gh = (args, stdio = "inherit") => execFileSync("gh", args, { cwd: rootDir, stdio });
    try {
      gh(["release", "view", tag, "--repo", "Xenonesis/Kronos"], "ignore");
    } catch {
      gh([
        "release",
        "create",
        tag,
        "--draft",
        "--title",
        `Kronos ${tag}`,
        "--notes",
        "",
        "--repo",
        "Xenonesis/Kronos",
      ]);
    }
    const assets = [
      installerName,
      `${installerName}.sig`,
      "Kronos_x64-setup.exe",
      "Kronos_x64-setup.exe.sig",
      "latest.json",
    ];
    gh([
      "release",
      "upload",
      tag,
      ...assets.map((file) => path.join(distDir, file)),
      "--clobber",
      "--repo",
      "Xenonesis/Kronos",
    ]);
    console.log(
      `Validated signed assets uploaded to ${tag}; publication remains an explicit release decision.`,
    );
  }
}

if (isMainModule(import.meta.url)) {
  try {
    buildInstaller();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
