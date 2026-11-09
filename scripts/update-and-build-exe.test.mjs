import assert from "node:assert/strict";
import { linkSync, lstatSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { copyInstallerArtifact } from "./update-and-build-exe.mjs";

test("local aliases are independent regular identical installer copies, not old linked destinations", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "kronos-installer-alias-"));
  try {
    const installer = path.join(directory, "versioned-setup.exe");
    const previous = path.join(directory, "previous.exe");
    const aliases = ["Kronos_x64-setup.exe", "latest.exe"].map((name) =>
      path.join(directory, name),
    );
    const alias = aliases[0];
    writeFileSync(installer, "current installer");
    writeFileSync(previous, "previous installer");
    linkSync(previous, alias);
    for (const destination of aliases) copyInstallerArtifact(installer, destination);
    for (const destination of aliases) {
      assert.equal(lstatSync(destination).isFile(), true);
      assert.equal(lstatSync(destination).isSymbolicLink(), false);
      assert.deepEqual(readFileSync(destination), readFileSync(installer));
    }
    assert.equal(readFileSync(previous, "utf8"), "previous installer");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
