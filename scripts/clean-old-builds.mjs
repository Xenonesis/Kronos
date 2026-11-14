import { execFileSync } from "node:child_process";
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
        try {
          fs.rmSync(fullPath, { force: true, recursive: true });
          console.log(`[Auto-Clean] Removed stale dist artifact: ${file}`);
          cleanedCount++;
        } catch (err) {
          if (process.platform === "win32" && (err.code === "EPERM" || err.code === "EBUSY")) {
            try {
              execFileSync("taskkill", ["/F", "/IM", file, "/T"], { stdio: "ignore" });
              fs.rmSync(fullPath, { force: true, recursive: true });
              console.log(`[Auto-Clean] Terminated running instance and removed stale artifact: ${file}`);
              cleanedCount++;
            } catch {
              console.warn(`[Auto-Clean] Could not remove locked file (currently running): ${file}`);
            }
          } else {
            console.warn(`[Auto-Clean] Skipping ${file}: ${err.message}`);
          }
        }
      }
    }
  }

  if (fs.existsSync(bundleDir)) {
    try {
      fs.rmSync(bundleDir, { recursive: true, force: true });
      console.log(`[Auto-Clean] Removed old Tauri bundle directory`);
      cleanedCount++;
    } catch {
      // ignore
    }
  }

  return cleanedCount;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const count = cleanOldBuildArtifacts();
  console.log(`Done. Cleaned ${count} stale build artifacts.`);
}
