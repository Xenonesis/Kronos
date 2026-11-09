import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import test from "node:test";

import vm from "node:vm";
const repoRoot = join(import.meta.dirname, "..");

async function desktopBridgeScript() {
  const content = await readFile(
    join(repoRoot, "packages/desktop-tauri/src-tauri/src/main.rs"),
    "utf8",
  );
  const startMarker = 'let bridge_script = format!(\n        r#"';
  const endMarker = '"#\n    );';
  const start = content.indexOf(startMarker);
  const end = content.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0) throw new Error("Native bridge template is unavailable");
  return content
    .slice(start + startMarker.length, end)
    .replace(/\{\{/g, "{")
    .replace(/\}\}/g, "}")
    .replace("{status_json}", JSON.stringify({ status: "running", listen: "127.0.0.1:6767" }));
}

test("remote and opaque documents never acquire a native bridge", async () => {
  const source = await desktopBridgeScript();
  for (const location of [
    { protocol: "http:", hostname: "127.0.0.1", port: "6767" },
    { protocol: "https:", hostname: "tauri.localhost.attacker.invalid", port: "" },
    { protocol: "about:", hostname: "", port: "" },
  ]) {
    const window = { location };
    window.top = window;
    vm.runInNewContext(source, { window });
    assert.equal(window.paseoDesktop, undefined);
  }
});

test("a trusted-URL subframe cannot inherit the top-level native bridge", async () => {
  const window = {
    top: {},
    location: { protocol: "http:", hostname: "tauri.localhost", port: "" },
  };
  vm.runInNewContext(await desktopBridgeScript(), { window });
  assert.equal(window.paseoDesktop, undefined);
});
