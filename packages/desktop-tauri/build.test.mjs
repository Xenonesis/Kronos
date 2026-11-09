import assert from "node:assert/strict";
import test from "node:test";
import { buildOptions } from "./build.mjs";

test("native unsigned bundles omit the optional Node server and signed updater artifacts", () => {
  const options = buildOptions([], {}, "win32");
  assert.equal(options.withNode, false);
  assert.equal(options.signed, false);
  assert.equal(options.config.bundle.createUpdaterArtifacts, false);
  assert.equal(options.config.plugins.updater.pubkey, "");
  assert.deepEqual(options.bundles, ["nsis"]);
});

test("Node payload is explicit, signed mode requires owner material, and raw executable flags are rejected", () => {
  assert.equal(buildOptions(["--with-node"], {}, "win32").withNode, true);
  assert.equal(buildOptions([], { KRONOS_WITH_NODE: "1" }, "win32").withNode, true);
  assert.equal(
    buildOptions(["--without-node"], { KRONOS_WITH_NODE: "1" }, "win32").withNode,
    false,
  );
  assert.throws(() => buildOptions(["--signed"], {}, "win32"), /public key/i);
  for (const arg of ["--quick", "--portable", "--config", "--no-bundle"]) {
    assert.throws(() => buildOptions([arg], {}, "win32"), /Unsupported/);
  }
});
