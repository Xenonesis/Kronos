import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const distDir = path.join(rootDir, "dist");
const bundleDir = path.join(rootDir, "packages/desktop-tauri/src-tauri/target/release/bundle");

export function cleanOldBuildArtifacts() {
  let cleanedCount = 0;

  if (fs.existsSync(distDir)) {
    for (const file of fs.readdirSync(distDir)) {
      if (
        file.endsWith(".exe") ||
        file.endsWith(".msi") ||
        file.endsWith(".zip") ||
        file.endsWith(".sig") ||
        file.endsWith(".md") ||
        /^latest.*\.json$/.test(file)
      ) {
        const fullPath = path.join(distDir, file);
        fs.rmSync(fullPath, { force: true, recursive: true });
        console.log(`[Auto-Clean] Removed stale dist artifact: ${file}`);
        cleanedCount++;
      }
    }
  }

  if (fs.existsSync(bundleDir)) {
    fs.rmSync(bundleDir, { recursive: true, force: true });
    console.log(`[Auto-Clean] Removed old Tauri bundle directory`);
    cleanedCount++;
  }

  return cleanedCount;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const count = cleanOldBuildArtifacts();
  console.log(`Done. Cleaned ${count} stale build artifacts.`);
}
