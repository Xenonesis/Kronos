import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { syncWorkspaceVersions } from "./sync-workspace-versions.mjs";
import { validateDesktopVersion } from "./tauri-release.mjs";

test("normal workspace version synchronization keeps native builds releasable without changing security configuration", () => {
  const root = mkdtempSync(path.join(tmpdir(), "kronos-version-sync-"));
  try {
    const desktop = path.join(root, "packages/desktop-tauri");
    const native = path.join(desktop, "src-tauri");
    mkdirSync(native, { recursive: true });
    writeFileSync(
      path.join(root, "package.json"),
      JSON.stringify({ version: "1.3.0-beta.2", workspaces: ["packages/desktop-tauri"] }),
    );
    writeFileSync(
      path.join(desktop, "package.json"),
      JSON.stringify({ name: "@getpaseo/desktop-tauri", version: "1.2.3", private: true }),
    );
    const config = {
      version: "1.2.3",
      app: { security: { csp: "default-src 'self'" } },
      plugins: {
        updater: { pubkey: "owner-public-key", endpoints: ["https://updates.example/latest.json"] },
      },
    };
    writeFileSync(path.join(native, "tauri.conf.json"), `${JSON.stringify(config, null, 2)}\n`);
    writeFileSync(
      path.join(native, "Cargo.toml"),
      '[package]\nname = "paseo-desktop"\nversion = "1.2.3"\n\n[dependencies]\nexample = { version = "9.8.7" }\n',
    );
    writeFileSync(
      path.join(native, "Cargo.lock"),
      'version = 4\n\n[[package]]\nname = "dependency"\nversion = "1.2.3"\n\n[[package]]\nname = "paseo-desktop"\nversion = "1.2.3"\ndependencies = ["dependency"]\n',
    );
    const originalConfig = readFileSync(path.join(native, "tauri.conf.json"), "utf8");
    const touched = syncWorkspaceVersions(root);
    assert.equal(touched.length, 4);
    assert.equal(validateDesktopVersion(root, "1.3.0-beta.2"), "1.3.0-beta.2");
    const updatedConfig = readFileSync(path.join(native, "tauri.conf.json"), "utf8");
    assert.equal(
      updatedConfig,
      originalConfig.replace('"version": "1.2.3"', '"version": "1.3.0-beta.2"'),
    );
    const lock = readFileSync(path.join(native, "Cargo.lock"), "utf8");
    assert.match(lock, /name = "dependency"\nversion = "1\.2\.3"/);
    assert.match(lock, /name = "paseo-desktop"\nversion = "1\.3\.0-beta\.2"/);
    assert.match(
      readFileSync(path.join(native, "Cargo.toml"), "utf8"),
      /example = \{ version = "9\.8\.7" \}/,
    );
    assert.deepEqual(syncWorkspaceVersions(root), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
