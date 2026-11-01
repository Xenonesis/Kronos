import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import yaml from "js-yaml";

import {
  computeWingetManifestPath,
  validateWingetManifests,
  submitWingetPackage,
} from "./submit-winget-package.mjs";

test("computeWingetManifestPath computes standard Winget path layout", () => {
  const relPath = computeWingetManifestPath(
    "Xenonesis.Kronos",
    "0.15.0-beta.1",
    "Xenonesis.Kronos.yaml",
  );
  assert.equal(
    relPath,
    "manifests/x/Xenonesis/Kronos/0.15.0-beta.1/Xenonesis.Kronos.yaml",
  );
});

test("validateWingetManifests validates required files and schema values", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "kronos-winget-test-"));
  try {
    const vYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "1.0.0",
      DefaultLocale: "en-US",
      ManifestType: "version",
      ManifestVersion: "1.6.0",
    });
    const iYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "1.0.0",
      ManifestType: "installer",
      ManifestVersion: "1.6.0",
      Installers: [{ Architecture: "x64", InstallerType: "nullsoft" }],
    });
    const lYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "1.0.0",
      PackageLocale: "en-US",
      ManifestType: "defaultLocale",
      ManifestVersion: "1.6.0",
    });

    await writeFile(path.join(dir, "Xenonesis.Kronos.yaml"), vYaml);
    await writeFile(path.join(dir, "Xenonesis.Kronos.installer.yaml"), iYaml);
    await writeFile(path.join(dir, "Xenonesis.Kronos.locale.en-US.yaml"), lYaml);

    const loaded = await validateWingetManifests(dir, "1.0.0");
    assert.equal(loaded.length, 3);
    assert.equal(
      loaded[0].targetPath,
      "manifests/x/Xenonesis/Kronos/1.0.0/Xenonesis.Kronos.yaml",
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("submitWingetPackage runs dry-run with mock fetch or no token", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "kronos-winget-dry-"));
  try {
    const vYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "1.0.0",
      ManifestType: "version",
      ManifestVersion: "1.6.0",
    });
    const iYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "1.0.0",
      ManifestType: "installer",
      ManifestVersion: "1.6.0",
      Installers: [],
    });
    const lYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "1.0.0",
      ManifestType: "defaultLocale",
      ManifestVersion: "1.6.0",
    });

    await writeFile(path.join(dir, "Xenonesis.Kronos.yaml"), vYaml);
    await writeFile(path.join(dir, "Xenonesis.Kronos.installer.yaml"), iYaml);
    await writeFile(path.join(dir, "Xenonesis.Kronos.locale.en-US.yaml"), lYaml);

    const result = await submitWingetPackage({
      version: "1.0.0",
      manifestsDir: dir,
      dryRun: true,
    });

    assert.equal(result.success, true);
    assert.equal(result.dryRun, true);
    assert.equal(result.files.length, 3);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("submitWingetPackage executes full PR workflow with mock fetchImpl", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "kronos-winget-mock-"));
  try {
    const vYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "2.0.0",
      ManifestType: "version",
      ManifestVersion: "1.6.0",
    });
    const iYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "2.0.0",
      ManifestType: "installer",
      ManifestVersion: "1.6.0",
      Installers: [],
    });
    const lYaml = yaml.dump({
      PackageIdentifier: "Xenonesis.Kronos",
      PackageVersion: "2.0.0",
      ManifestType: "defaultLocale",
      ManifestVersion: "1.6.0",
    });

    await writeFile(path.join(dir, "Xenonesis.Kronos.yaml"), vYaml);
    await writeFile(path.join(dir, "Xenonesis.Kronos.installer.yaml"), iYaml);
    await writeFile(path.join(dir, "Xenonesis.Kronos.locale.en-US.yaml"), lYaml);

    const calls = [];
    const mockFetch = async (url, options = {}) => {
      calls.push({ url: String(url), method: options.method || "GET" });
      if (url.includes("/user")) {
        return { ok: true, json: async () => ({ login: "kronos-bot" }) };
      }
      if (url.includes("/forks")) {
        return { ok: true, status: 202, json: async () => ({}) };
      }
      if (url.endsWith("/repos/microsoft/winget-pkgs")) {
        return { ok: true, json: async () => ({ default_branch: "master" }) };
      }
      if (url.includes("/git/ref/heads/master")) {
        return {
          ok: true,
          json: async () => ({ object: { sha: "basesha123" } }),
        };
      }
      if (url.includes("/git/refs")) {
        return { ok: true, status: 201, json: async () => ({}) };
      }
      if (url.includes("/contents/")) {
        if (options.method === "PUT") {
          return { ok: true, status: 201, json: async () => ({}) };
        }
        return { ok: false, status: 404 };
      }
      if (url.includes("/pulls")) {
        return {
          ok: true,
          json: async () => ({
            html_url: "https://github.com/microsoft/winget-pkgs/pull/99999",
            number: 99999,
          }),
        };
      }
      return { ok: true, json: async () => ({}) };
    };

    const result = await submitWingetPackage({
      version: "2.0.0",
      manifestsDir: dir,
      dryRun: false,
      token: "mock-token",
      fetchImpl: mockFetch,
    });

    assert.equal(result.success, true);
    assert.equal(result.dryRun, false);
    assert.equal(
      result.pullRequestUrl,
      "https://github.com/microsoft/winget-pkgs/pull/99999",
    );
    assert.equal(result.pullRequestNumber, 99999);
    assert.ok(calls.some((c) => c.url.includes("/pulls") && c.method === "POST"));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
