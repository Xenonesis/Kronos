import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const serverDist = path.join(__dirname, "server-dist");

function getDirectorySize(dir) {
  let size = 0;
  if (!fs.existsSync(dir)) return 0;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      size += getDirectorySize(fullPath);
    } else {
      size += fs.statSync(fullPath).size;
    }
  }
  return size;
}

describe("prepare-dist --without-node", () => {
  it("creates a lean standalone payload without node.exe or bulky server-dist", () => {
    const scriptPath = path.join(__dirname, "prepare-dist.mjs");
    const output = execFileSync(process.execPath, [scriptPath, "--without-node"], {
      cwd: __dirname,
      encoding: "utf8",
    });

    assert.match(output, /Standalone Native Mode/i);

    const nodeExe = path.join(serverDist, "node.exe");
    assert.strictEqual(
      fs.existsSync(nodeExe),
      false,
      "server-dist/node.exe must not exist in standalone mode",
    );

    const standaloneJson = path.join(serverDist, "dist", "standalone.json");
    assert.strictEqual(
      fs.existsSync(standaloneJson),
      true,
      "server-dist/dist/standalone.json marker must exist",
    );

    const marker = JSON.parse(fs.readFileSync(standaloneJson, "utf8"));
    assert.strictEqual(marker.native, true);
    assert.strictEqual(marker.engine, "rust");

    const distEntries = fs.readdirSync(path.join(serverDist, "dist"));
    assert.ok(
      distEntries.length <= 5,
      `standalone dist should only contain lean metadata, found ${distEntries.length} entries`,
    );

    const totalBytes = getDirectorySize(serverDist);
    assert.ok(
      totalBytes < 500 * 1024,
      `standalone server-dist size should be <500KB, got ${totalBytes} bytes`,
    );
  });
  it("defaults to standalone native mode without any arguments", () => {
    const scriptPath = path.join(__dirname, "prepare-dist.mjs");
    const output = execFileSync(process.execPath, [scriptPath], {
      cwd: __dirname,
      encoding: "utf8",
    });

    assert.match(output, /Standalone Native Mode active/i);

    const nodeExe = path.join(serverDist, "node.exe");
    assert.strictEqual(
      fs.existsSync(nodeExe),
      false,
      "server-dist/node.exe must not exist in default standalone mode",
    );

    const standaloneJson = path.join(serverDist, "dist", "standalone.json");
    assert.strictEqual(
      fs.existsSync(standaloneJson),
      true,
      "server-dist/dist/standalone.json marker must exist in default mode",
    );
  });
});
