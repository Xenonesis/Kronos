import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isMainModule } from "../../scripts/is-main-module.mjs";
import {
  requireSigning,
  validateDesktopVersion,
  validateSignedBundle,
} from "../../scripts/tauri-release.mjs";

const desktopDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(desktopDir, "../..");

function defaultBundlesForPlatform(platform) {
  if (platform === "win32") return ["nsis"];
  if (platform === "darwin") return ["app", "dmg"];
  return ["deb", "appimage"];
}

function resolveTauriPlatform(platform) {
  if (platform === "win32") return "windows";
  if (platform === "darwin") return "darwin";
  return "linux";
}

function resolveTauriArch(arch) {
  if (arch === "arm64") return "aarch64";
  if (arch === "x64") return "x86_64";
  return arch;
}

export function buildOptions(argv, env = process.env, platform = process.platform) {
  let signed = false;
  let withNode = env.KRONOS_WITH_NODE === "1";
  let bundles = defaultBundlesForPlatform(platform);
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--signed" || arg === "--release") signed = true;
    else if (arg === "--with-node") withNode = true;
    else if (arg === "--without-node") withNode = false;
    else if (arg === "--force") {
      /* Every bundle already exports fresh sources. */
    } else if (arg === "--bundles") {
      bundles = (argv[++index] || "").split(",");
      const supported = defaultBundlesForPlatform(platform);
      if (!bundles.length || bundles.some((bundle) => !supported.includes(bundle)))
        throw new Error("Unsupported installer bundle target");
    } else throw new Error(`Unsupported desktop build option: ${arg}`);
  }
  const publicKey = signed ? requireSigning(env) : "";
  return {
    signed,
    withNode,
    bundles,
    config: {
      bundle: { createUpdaterArtifacts: signed },
      plugins: { updater: { pubkey: publicKey } },
    },
  };
}

export function buildDesktop(argv = [], env = process.env) {
  const options = buildOptions(argv, env);
  const version = validateDesktopVersion(rootDir, env.DESKTOP_VERSION);
  const targetDir = env.CARGO_TARGET_DIR
    ? path.resolve(env.CARGO_TARGET_DIR)
    : path.join(desktopDir, "src-tauri/target");
  const bundleDir = path.join(targetDir, "release/bundle");
  // Successful output must belong to this invocation, not an older version or mode.
  fs.rmSync(bundleDir, { recursive: true, force: true });
  const childEnv = {
    ...env,
    CARGO_TARGET_DIR: targetDir,
    KRONOS_WITH_NODE: options.withNode ? "1" : "0",
  };
  if (!options.signed) {
    delete childEnv.TAURI_SIGNING_PRIVATE_KEY;
    delete childEnv.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;
  }
  const result = spawnSync(
    process.execPath,
    [
      path.join(rootDir, "node_modules/@tauri-apps/cli/tauri.js"),
      "build",
      "--bundles",
      options.bundles.join(","),
      "--config",
      JSON.stringify(options.config),
    ],
    { cwd: desktopDir, env: childEnv, stdio: "inherit" },
  );
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Tauri installer build failed (exit ${result.status})`);
  if (options.signed)
    validateSignedBundle({
      bundleDir,
      version,
      publicKey: env.TAURI_SIGNING_PUBLIC_KEY,
      platform: resolveTauriPlatform(process.platform),
      arch: resolveTauriArch(process.arch),
    });
  if (process.platform === "win32") {
    const versionedInstaller = path.join(bundleDir, "nsis", `Kronos_${version}_x64-setup.exe`);
    const alias = path.join(bundleDir, "nsis", "Kronos_x64-setup.exe");
    if (fs.existsSync(versionedInstaller)) {
      fs.rmSync(alias, { force: true });
      fs.copyFileSync(versionedInstaller, alias);
      if (options.signed && fs.existsSync(`${versionedInstaller}.sig`)) {
        fs.rmSync(`${alias}.sig`, { force: true });
        fs.copyFileSync(`${versionedInstaller}.sig`, `${alias}.sig`);
      }
    }
  }
  return { ...options, version, bundleDir };
}

if (isMainModule(import.meta.url)) buildDesktop(process.argv.slice(2));
