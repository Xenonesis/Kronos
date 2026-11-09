import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { validateDesktopVersion } from "../../scripts/tauri-release.mjs";

const desktopDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(desktopDir, "../..");
validateDesktopVersion(rootDir, process.env.DESKTOP_VERSION);

function npm(args, cwd = rootDir, env = process.env) {
  const result = spawnSync(process.platform === "win32" ? "npm.cmd" : "npm", args, {
    cwd,
    env,
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`Desktop preparation failed (npm ${args[0]}, exit ${result.status})`);
}

// Tauri invokes this for every build, including direct `tauri build` and CI.
// Never use the presence of dist or a Git diff as evidence of a current export.
npm(["run", "build:app-deps"]);
npm(
  ["exec", "--", "expo", "export", "--platform", "web", "--clear"],
  path.join(rootDir, "packages/app"),
  {
    ...process.env,
    NODE_ENV: "production",
    PASEO_WEB_PLATFORM: "electron",
  },
);
const budgetCheck = spawnSync(
  process.execPath,
  [path.join(rootDir, "scripts/check-bundle-budget.mjs")],
  {
    cwd: rootDir,
    stdio: "inherit",
    env: process.env,
  },
);
if (budgetCheck.error) throw budgetCheck.error;
if (budgetCheck.status !== 0) {
  throw new Error(`Bundle budget check failed (exit ${budgetCheck.status})`);
}
if (process.env.KRONOS_WITH_NODE === "1") npm(["run", "build:server:clean"]);
const prepared = spawnSync(process.execPath, [path.join(desktopDir, "prepare-dist.mjs")], {
  cwd: desktopDir,
  stdio: "inherit",
  env: process.env,
});
if (prepared.error) throw prepared.error;
if (prepared.status !== 0)
  throw new Error(`Preparing desktop resources failed (exit ${prepared.status})`);
