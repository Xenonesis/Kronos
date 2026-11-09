#!/usr/bin/env node
import fs from "fs";
import path from "path";
import zlib from "zlib";

const distDir = path.resolve("packages/app/dist");
const indexPath = path.join(distDir, "index.html");

if (!fs.existsSync(indexPath)) {
  console.error("Error: index.html not found at", indexPath);
  process.exit(1);
}

const html = fs.readFileSync(indexPath, "utf8");

// Extract all script src attributes
const scriptRegex = /<script\s+[^>]*src=["']([^"']+)["'][^>]*>/gi;
let match;
const scripts = [];

while ((match = scriptRegex.exec(html)) !== null) {
  const src = match[1];
  // Ignore external URLs if any
  if (!src.startsWith("http://") && !src.startsWith("https://")) {
    scripts.push(src);
  }
}

if (scripts.length === 0) {
  console.error("Error: No script tags found in", indexPath);
  process.exit(1);
}

console.log("\n=======================================================");
console.log("             KRONOS WEB ENTRY BUNDLE AUDIT             ");
console.log("=======================================================\n");

let totalRaw = 0;
let totalGzip = 0;

console.log(`${"Script".padEnd(52)} ${"Uncompressed".padStart(14)} ${"Gzipped".padStart(12)}`);
console.log("-".repeat(80));

for (const scriptSrc of scripts) {
  // Strip leading slash
  const relativePath = scriptSrc.replace(/^\/+/, "");
  const filePath = path.join(distDir, relativePath);

  if (!fs.existsSync(filePath)) {
    console.warn(`Warning: script file not found on disk: ${filePath}`);
    continue;
  }

  const content = fs.readFileSync(filePath);
  const rawBytes = content.length;
  const gzipBytes = zlib.gzipSync(content).length;

  totalRaw += rawBytes;
  totalGzip += gzipBytes;

  const rawStr = `${(rawBytes / 1024).toFixed(1)} KB`;
  const gzipStr = `${(gzipBytes / 1024).toFixed(1)} KB`;
  const name = path.basename(filePath);

  console.log(`${name.padEnd(52)} ${rawStr.padStart(14)} ${gzipStr.padStart(12)}`);
}

console.log("-".repeat(80));
const totalRawMB = (totalRaw / (1024 * 1024)).toFixed(2);
const totalGzipMB = (totalGzip / (1024 * 1024)).toFixed(2);

console.log(
  `${"TOTAL STARTUP SCRIPTS".padEnd(52)} ${(totalRawMB + " MB").padStart(14)} ${(totalGzipMB + " MB").padStart(12)}`,
);
console.log("=======================================================\n");

// Budget thresholds
const MAX_RAW_MB = parseFloat(process.env.MAX_STARTUP_MB || "10.0");
if (totalRaw / (1024 * 1024) > MAX_RAW_MB) {
  console.error(
    `❌ BUDGET VIOLATION: Total startup bundle is ${totalRawMB} MB, exceeding budget limit of ${MAX_RAW_MB} MB!`,
  );
  process.exit(1);
}

console.log(`✓ Bundle budget check passed (Total: ${totalRawMB} MB <= ${MAX_RAW_MB} MB limit).\n`);
