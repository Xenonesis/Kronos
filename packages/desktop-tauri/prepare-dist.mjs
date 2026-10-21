import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const targetDir = path.join(__dirname, "server-dist");

console.log("Preparing lean server-dist for Tauri bundle...");

const distDir = path.join(targetDir, "dist");
if (fs.existsSync(distDir)) {
  fs.rmSync(distDir, { recursive: true, force: true });
}
fs.mkdirSync(targetDir, { recursive: true });
// Copy server dist
console.log("Copying server dist (1,843 files)...");
fs.cpSync(path.join(root, "packages/server/dist"), path.join(targetDir, "dist"), {
  recursive: true,
});

// Copy node runtime directly so Tauri installed app always has Node available
console.log("Copying node runtime...");
const nodeBinName = process.platform === "win32" ? "node.exe" : "node";
fs.copyFileSync(process.execPath, path.join(targetDir, nodeBinName));
if (process.platform !== "win32") {
  try {
    fs.copyFileSync(process.execPath, path.join(targetDir, "node.exe"));
  } catch {}
}
// Copy server package.json
fs.copyFileSync(
  path.join(root, "packages/server/package.json"),
  path.join(targetDir, "package.json"),
);

// Install production dependencies directly inside server-dist if missing
const serverDistNodeModules = path.join(targetDir, "node_modules");
const internalPackages = ["client", "highlight", "plugin", "protocol", "relay"];

// Reports the registries npm would resolve against, so a failed install
// carries the context that explains it.
function npmRegistryConfig(cwd, execFileSync) {
  try {
    return execFileSync(
      process.platform === "win32" ? "npm.cmd" : "npm",
      ["config", "get", "registry"],
      { cwd, encoding: "utf8" },
    ).trim();
  } catch {
    return "unknown";
  }
}

if (!fs.existsSync(serverDistNodeModules)) {
  console.log("Installing production node_modules into server-dist...");
  const { runWithRetry } = await import("../../scripts/npm-retry.mjs");
  const { execFileSync } = await import("node:child_process");
  const os = await import("node:os");

  // The install runs in a directory outside the repository. server-dist sits
  // inside a workspace member, and npm resolves a nested project against the
  // workspace it belongs to, which is where the resolver crash comes from:
  // "Cannot read properties of null (reading 'edgesOut')".
  const stagingDir = fs.mkdtempSync(path.join(os.tmpdir(), "kronos-server-dist-"));

  // The install resolves from the registry rather than from a lockfile, so it
  // has to be told which registry to use. The CI runner configures the
  // repository to resolve through GitHub Packages, which does not carry the
  // public packages this bundle needs.
  fs.writeFileSync(
    path.join(stagingDir, ".npmrc"),
    ["registry=https://registry.npmjs.org/", ""].join("\n"),
  );

  // The internal workspace packages are copied in below, so they are removed
  // from the manifest for the install. Leaving them in makes npm try to
  // resolve them from the registry, which is a second source of truth for
  // npm to disagree with.
  const serverPackage = JSON.parse(
    fs.readFileSync(path.join(root, "packages", "server", "package.json"), "utf8"),
  );
  for (const name of internalPackages) {
    delete serverPackage.dependencies?.[`@getpaseo/${name}`];
  }
  fs.writeFileSync(
    path.join(stagingDir, "package.json"),
    `${JSON.stringify(serverPackage, null, 2)}\n`,
  );

  // A stale npm cache makes the install fail inside npm's resolver as well,
  // so the cache is cleared before the first attempt.
  try {
    execFileSync(process.platform === "win32" ? "npm.cmd" : "npm", ["cache", "clean", "--force"], {
      stdio: "inherit",
    });
  } catch {
    // A cache that cannot be cleaned is not fatal on its own.
  }
  const exitCode = await runWithRetry(
    [
      "install",
      "--omit=dev",
      "--no-package-lock",
      "--ignore-scripts",
      // The registry has to be given on the command line, not in an .npmrc:
      // the CI runner points npm at GitHub Packages through
      // NPM_CONFIG_USERCONFIG, and that user config outranks a project
      // .npmrc.
      "--registry=https://registry.npmjs.org/",
      // npm's peer resolution crashes with "Cannot read properties of null
      // (reading 'edgesOut')" on this dependency tree when it resolves peers
      // itself. The bundle does not need peer resolution — the internal
      // packages are copied in and the rest are direct dependencies — so the
      // legacy resolver is used instead.
      "--legacy-peer-deps",
    ],
    { cwd: stagingDir },
  );
  if (exitCode !== 0) {
    // npm's own error goes to its debug log under the cache directory, which
    // is not in the build output. The resolver configuration is printed here
    // so a failure carries the context it needs.
    console.error(`npm registry config: ${npmRegistryConfig(stagingDir, execFileSync)}`);
    throw new Error(
      `Failed to install production dependencies into server-dist (exit ${exitCode})`,
    );
  }

  fs.cpSync(path.join(stagingDir, "node_modules"), serverDistNodeModules, { recursive: true });
  fs.rmSync(stagingDir, { recursive: true, force: true });
}

// Copy internal workspace packages into node_modules/@getpaseo
const getpaseoDir = path.join(serverDistNodeModules, "@getpaseo");
fs.mkdirSync(getpaseoDir, { recursive: true });
for (const pkg of internalPackages) {
  const src = path.join(root, "packages", pkg);
  const dest = path.join(getpaseoDir, pkg);
  if (fs.existsSync(dest)) fs.rmSync(dest, { recursive: true, force: true });
  fs.cpSync(src, dest, {
    recursive: true,
    filter: (srcPath) => !srcPath.includes("node_modules") && !srcPath.includes(".git"),
  });
}
// Remove bloated unused claude binary to stay within installer limits
const heavyClaude = path.join(
  serverDistNodeModules,
  "@anthropic-ai/claude-agent-sdk-win32-x64/claude.exe",
);
if (fs.existsSync(heavyClaude)) {
  fs.unlinkSync(heavyClaude);
  console.log("Stripped bloated claude.exe binary");
}

// Ensure typescript compiler is available for runtime plugin compilation
const tsDest = path.join(serverDistNodeModules, "typescript");
if (!fs.existsSync(tsDest)) {
  const tsSrc = path.join(root, "node_modules/typescript");
  if (fs.existsSync(tsSrc)) {
    fs.cpSync(tsSrc, tsDest, { recursive: true });
    console.log("Copied typescript to server-dist/node_modules");
  }
}
// Strip sourcemap and buildinfo files to speed up packaging and reduce size
function cleanExtraFiles(dir) {
  try {
    for (const f of fs.readdirSync(dir)) {
      const p = path.join(dir, f);
      const st = fs.statSync(p);
      if (st.isDirectory()) {
        cleanExtraFiles(p);
      } else if (f.endsWith(".map") || f.endsWith(".tsbuildinfo")) {
        fs.unlinkSync(p);
      }
    }
  } catch {}
}
cleanExtraFiles(targetDir);
console.log("Stripped sourcemap and buildinfo files");

console.log("Lean server-dist prepared with bundled dependencies!");
