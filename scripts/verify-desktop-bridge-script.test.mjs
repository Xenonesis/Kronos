import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import test from "node:test";

const repoRoot = join(import.meta.dirname, "..");

test("desktop main.rs bridge_script has 100% valid JavaScript syntax", async () => {
  const mainRsPath = join(repoRoot, "packages/desktop-tauri/src-tauri/src/main.rs");
  const content = await readFile(mainRsPath, "utf8");

  const startMarker = 'let bridge_script = format!(\n        r#"';
  const endMarker = '"#\n    );';

  const startIdx = content.indexOf(startMarker);
  assert.ok(startIdx !== -1, "bridge_script start marker must be present in main.rs");

  const codeStart = startIdx + startMarker.length;
  const endIdx = content.indexOf(endMarker, codeStart);
  assert.ok(endIdx !== -1, "bridge_script end marker must be present in main.rs");

  let script = content.substring(codeStart, endIdx);

  // Replace rust template formatting {{ and }} with { and }
  script = script.replace(/\{\{/g, "{").replace(/\}\}/g, "}");
  // Replace {status_json} with a mock status object
  script = script.replace(
    "{status_json}",
    JSON.stringify({
      status: "running",
      listen: "127.0.0.1:6767",
      server_id: "srv_test_123",
      credential: "token",
    }),
  );

  assert.doesNotThrow(() => {
    new Function(script);
  }, "bridge_script must be completely valid JavaScript without syntax errors");

  assert.match(
    script,
    /window\.paseoDesktop\s*=\s*\{/,
    "bridge_script must assign window.paseoDesktop",
  );
  assert.match(
    script,
    /getKernelTelemetry:\s*async function/,
    "bridge_script must define getKernelTelemetry",
  );
});
