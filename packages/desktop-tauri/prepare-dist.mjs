import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const targetDir = path.join(__dirname, "server-dist");

console.log("Preparing lean server-dist for Tauri bundle...");

if (fs.existsSync(targetDir)) {
  fs.rmSync(targetDir, { recursive: true, force: true });
}
fs.mkdirSync(targetDir, { recursive: true });

// Copy server dist
console.log("Copying server dist (1,843 files)...");
fs.cpSync(path.join(root, "packages/server/dist"), path.join(targetDir, "dist"), { recursive: true });

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
fs.copyFileSync(path.join(root, "packages/server/package.json"), path.join(targetDir, "package.json"));

// Install production dependencies directly inside server-dist if missing
const serverDistNodeModules = path.join(targetDir, "node_modules");
if (!fs.existsSync(serverDistNodeModules)) {
  console.log("Installing production node_modules into server-dist...");
  const { execSync } = await import("node:child_process");
  execSync("npm install --omit=dev --no-package-lock --ignore-scripts", {
    cwd: targetDir,
    stdio: "inherit",
  });
}

// Copy internal workspace packages into node_modules/@getpaseo
const getpaseoDir = path.join(serverDistNodeModules, "@getpaseo");
fs.mkdirSync(getpaseoDir, { recursive: true });
const internalPackages = ["client", "highlight", "plugin", "protocol", "relay"];
for (const pkg of internalPackages) {
  const src = path.join(root, "packages", pkg);
  const dest = path.join(getpaseoDir, pkg);
  if (fs.existsSync(dest)) fs.rmSync(dest, { recursive: true, force: true });
  fs.cpSync(src, dest, {
    recursive: true,
    filter: (srcPath) => !srcPath.includes("node_modules") && !srcPath.includes(".git"),
  });
}

console.log("Lean server-dist prepared with bundled dependencies!");
