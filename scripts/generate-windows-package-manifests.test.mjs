import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import yaml from "js-yaml";

import { generateWindowsPackageManifests } from "./generate-windows-package-manifests.mjs";

test("generates consistent Winget and Scoop manifests from installer bytes", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "kronos-manifests-"));
  try {
    const installerPath = path.join(directory, "installer.exe");
    const outputDirectory = path.join(directory, "out");
    const fixtureBytes = Buffer.from("verified fixture installer bytes");
    await writeFile(installerPath, fixtureBytes);

    await generateWindowsPackageManifests({
      version: "1.2.3-beta.1",
      installerPath,
      outputDirectory,
    });

    const hash = createHash("sha256").update(fixtureBytes).digest("hex").toUpperCase();
    const versionManifest = yaml.load(
      await readFile(path.join(outputDirectory, "winget", "Xenonesis.Kronos.yaml"), "utf8"),
    );
    const installerManifest = yaml.load(
      await readFile(
        path.join(outputDirectory, "winget", "Xenonesis.Kronos.installer.yaml"),
        "utf8",
      ),
    );
    const localeManifest = yaml.load(
      await readFile(
        path.join(outputDirectory, "winget", "Xenonesis.Kronos.locale.en-US.yaml"),
        "utf8",
      ),
    );
    const scoopManifest = JSON.parse(
      await readFile(path.join(outputDirectory, "scoop", "Kronos.json"), "utf8"),
    );
    const url =
      "https://github.com/Xenonesis/Kronos/releases/download/v1.2.3-beta.1/Kronos_1.2.3-beta.1_x64-setup.exe";

    assert.equal(versionManifest.PackageIdentifier, "Xenonesis.Kronos");
    assert.equal(versionManifest.DefaultLocale, localeManifest.PackageLocale);
    assert.equal(versionManifest.ManifestVersion, "1.6.0");
    assert.equal(versionManifest.PackageVersion, "1.2.3-beta.1");
    assert.equal(versionManifest.ManifestType, "version");
    assert.equal(installerManifest.PackageVersion, versionManifest.PackageVersion);
    assert.equal(installerManifest.ManifestType, "installer");
    assert.equal(installerManifest.Installers[0].InstallerUrl, url);
    assert.equal(installerManifest.Installers[0].InstallerSha256, hash);
    assert.equal(installerManifest.Installers[0].Architecture, "x64");
    assert.equal(installerManifest.Installers[0].InstallerType, "nullsoft");
    assert.equal(localeManifest.ManifestType, "defaultLocale");
    assert.equal(scoopManifest.version, versionManifest.PackageVersion);
    assert.equal(scoopManifest.url, url);
    assert.equal(scoopManifest.hash, hash.toLowerCase());
    assert.equal(scoopManifest.installer.file, path.basename(url));
    assert.deepEqual(scoopManifest.installer.args, []);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("fails when the installer does not exist", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "kronos-manifests-"));
  try {
    await assert.rejects(
      generateWindowsPackageManifests({
        version: "1.2.3",
        installerPath: path.join(directory, "missing.exe"),
        outputDirectory: path.join(directory, "out"),
      }),
      /Installer file does not exist/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
