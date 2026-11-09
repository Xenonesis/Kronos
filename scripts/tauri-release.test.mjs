import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import {
  decodeUpdaterPublicKey,
  verifyUpdaterSignature,
  stageRelease,
  mergeReleases,
  requireSigning,
  validateDesktopVersion,
} from "./tauri-release.mjs";

// Ephemeral test-only keys never leave these temporary fixture directories.
function signingFixture(bytes, prehashed = false, keys = generateKeyPairSync("ed25519")) {
  const { publicKey, privateKey } = keys;
  const id = Buffer.from("0102030405060708", "hex");
  const key = publicKey.export({ type: "spki", format: "der" }).subarray(-32);
  const pubkey = Buffer.from(
    `untrusted comment: minisign public key\n${Buffer.concat([Buffer.from("Ed"), id, key]).toString("base64")}\n`,
  ).toString("base64");
  const signature = sign(
    null,
    prehashed ? createHash("blake2b512").update(bytes).digest() : bytes,
    privateKey,
  );
  const trusted = "timestamp:1\tfile:fixture";
  const global = sign(null, Buffer.concat([signature, Buffer.from(trusted)]), privateKey);
  const encoded = Buffer.from(
    [
      "untrusted comment: fixture signature",
      Buffer.concat([Buffer.from(prehashed ? "ED" : "Ed"), id, signature]).toString("base64"),
      `trusted comment: ${trusted}`,
      global.toString("base64"),
      "",
    ].join("\n"),
  ).toString("base64");
  return { pubkey, encoded, keys };
}

function withDirectory(run) {
  const directory = mkdtempSync(path.join(tmpdir(), "kronos-release-"));
  try {
    run(directory);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

for (const prehashed of [false, true]) {
  test(`verifies real updater signatures (${prehashed ? "prehashed" : "legacy"}) and rejects changed bytes`, () => {
    const bytes = Buffer.from("signed installer bytes");
    const { pubkey, encoded } = signingFixture(bytes, prehashed);
    verifyUpdaterSignature(bytes, encoded, pubkey);
    assert.throws(() => verifyUpdaterSignature(Buffer.from("tampered"), encoded, pubkey));
    assert.throws(() => verifyUpdaterSignature(bytes, encoded, signingFixture(bytes).pubkey));
    const lines = Buffer.from(encoded, "base64").toString("utf8").split("\n");
    lines[2] += "tampered";
    assert.throws(() =>
      verifyUpdaterSignature(bytes, Buffer.from(lines.join("\n")).toString("base64"), pubkey),
    );
  });
}

test("rejects missing, placeholder and malformed signing data", () => {
  for (const key of [
    "",
    "placeholder",
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDExMjM1OApSV1RUUUVKVE40...=",
  ]) {
    assert.throws(() => decodeUpdaterPublicKey(key), /public key/i);
  }
  const { pubkey } = signingFixture(Buffer.from("bytes"));
  for (const signature of [
    "",
    "garbage",
    Buffer.from("untrusted comment: invalid").toString("base64"),
  ]) {
    assert.throws(
      () => verifyUpdaterSignature(Buffer.from("bytes"), signature, pubkey),
      /signature/i,
    );
  }
  assert.throws(
    () => requireSigning({ TAURI_SIGNING_PUBLIC_KEY: pubkey }),
    /TAURI_SIGNING_PRIVATE_KEY/,
  );
});

test("stages actual NSIS signature, identical regular aliases and verified metadata", () => {
  withDirectory((directory) => {
    const bundleDir = path.join(directory, "bundle");
    mkdirSync(path.join(bundleDir, "nsis"), { recursive: true });
    const bytes = Buffer.from("installer fixture");
    const { pubkey, encoded } = signingFixture(bytes);
    const installer = path.join(bundleDir, "nsis", "Kronos_1.2.3_x64-setup.exe");
    writeFileSync(installer, bytes);
    writeFileSync(`${installer}.sig`, `${encoded}\n`);
    const outputDir = path.join(directory, "stage");
    stageRelease({
      bundleDir,
      outputDir,
      version: "1.2.3",
      platform: "windows",
      arch: "x86_64",
      publicKey: pubkey,
    });
    const manifest = JSON.parse(
      readFileSync(path.join(outputDir, "latest-windows-x86_64.json"), "utf8"),
    );
    assert.equal(manifest.platforms["windows-x86_64"].signature, encoded);
    assert.deepEqual(readFileSync(path.join(outputDir, "Kronos_x64-setup.exe")), bytes);
    const finalDir = path.join(directory, "final");
    mergeReleases({
      inputDir: outputDir,
      outputDir: finalDir,
      version: "1.2.3",
      publicKey: pubkey,
    });
    assert.equal(
      JSON.parse(readFileSync(path.join(finalDir, "latest.json"), "utf8")).version,
      "1.2.3",
    );
    writeFileSync(path.join(outputDir, "Kronos_x64-setup.exe"), "wrong alias");
    assert.throws(
      () =>
        mergeReleases({
          inputDir: outputDir,
          outputDir: finalDir,
          version: "1.2.3",
          publicKey: pubkey,
        }),
      /alias/i,
    );
    writeFileSync(installer, "corrupted");
    assert.throws(
      () =>
        stageRelease({
          bundleDir,
          outputDir,
          version: "1.2.3",
          platform: "windows",
          arch: "x86_64",
          publicKey: pubkey,
        }),
      /signature/i,
    );
  });
});

test("missing signatures fail closed even if an older stage exists", () => {
  withDirectory((directory) => {
    mkdirSync(path.join(directory, "bundle", "nsis"), { recursive: true });
    writeFileSync(path.join(directory, "bundle", "nsis", "Kronos_1.2.3_x64-setup.exe"), "unsigned");
    assert.throws(
      () =>
        stageRelease({
          bundleDir: path.join(directory, "bundle"),
          outputDir: path.join(directory, "out"),
          version: "1.2.3",
          platform: "windows",
          arch: "x86_64",
          publicKey: signingFixture(Buffer.from("bytes")).pubkey,
        }),
      /sig|ENOENT/i,
    );
  });
});

test("final validation rejects stale versions and metadata signature substitution", () => {
  withDirectory((directory) => {
    const bundleDir = path.join(directory, "bundle");
    mkdirSync(path.join(bundleDir, "nsis"), { recursive: true });
    const bytes = Buffer.from("installer fixture");
    const { pubkey, encoded } = signingFixture(bytes);
    const installer = path.join(bundleDir, "nsis", "Kronos_1.2.3_x64-setup.exe");
    writeFileSync(installer, bytes);
    writeFileSync(`${installer}.sig`, encoded);
    const outputDir = path.join(directory, "stage");
    stageRelease({
      bundleDir,
      outputDir,
      version: "1.2.3",
      platform: "windows",
      arch: "x86_64",
      publicKey: pubkey,
    });
    assert.throws(
      () =>
        mergeReleases({
          inputDir: outputDir,
          outputDir: path.join(directory, "final"),
          version: "9.9.9",
          publicKey: pubkey,
        }),
      /version/i,
    );
    const manifestPath = path.join(outputDir, "latest-windows-x86_64.json");
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    manifest.platforms["windows-x86_64"].signature = "";
    writeFileSync(manifestPath, JSON.stringify(manifest));
    assert.throws(
      () =>
        mergeReleases({
          inputDir: outputDir,
          outputDir: path.join(directory, "final"),
          version: "1.2.3",
          publicKey: pubkey,
        }),
      /signature/i,
    );
  });
});

test("merges both macOS architectures without shipping raw app staging trees", () => {
  withDirectory((directory) => {
    const bytes = Buffer.from("updater archive fixture");
    const { pubkey, encoded } = signingFixture(bytes);
    for (const arch of ["x86_64", "aarch64"]) {
      const bundleDir = path.join(directory, arch, "bundle");
      mkdirSync(path.join(bundleDir, "macos", "Kronos.app"), { recursive: true });
      writeFileSync(
        path.join(bundleDir, "macos", "Kronos.app", "raw-executable"),
        "not a release asset",
      );
      const archive = path.join(bundleDir, "macos", "Kronos.app.tar.gz");
      writeFileSync(archive, bytes);
      writeFileSync(`${archive}.sig`, encoded);
      stageRelease({
        bundleDir,
        outputDir: path.join(directory, "stages", arch),
        version: "1.2.3",
        platform: "darwin",
        arch,
        publicKey: pubkey,
      });
    }
    const outputDir = path.join(directory, "final");
    const manifest = mergeReleases({
      inputDir: path.join(directory, "stages"),
      outputDir,
      version: "1.2.3",
      publicKey: pubkey,
    });
    assert.deepEqual(Object.keys(manifest.platforms).sort(), ["darwin-aarch64", "darwin-x86_64"]);
    assert.match(manifest.platforms["darwin-aarch64"].url, /aarch64-Kronos\.app\.tar\.gz$/);
    assert.match(manifest.platforms["darwin-x86_64"].url, /x86_64-Kronos\.app\.tar\.gz$/);
  });
});

test("desktop version validation rejects a mismatched Cargo version before packaging", () => {
  withDirectory((directory) => {
    const desktop = path.join(directory, "packages", "desktop-tauri");
    mkdirSync(path.join(desktop, "src-tauri"), { recursive: true });
    writeFileSync(path.join(desktop, "package.json"), JSON.stringify({ version: "1.2.3" }));
    writeFileSync(
      path.join(desktop, "src-tauri", "tauri.conf.json"),
      JSON.stringify({ version: "1.2.3" }),
    );
    writeFileSync(path.join(desktop, "src-tauri", "Cargo.toml"), '[package]\nversion = "1.2.3"\n');
    assert.equal(validateDesktopVersion(directory, "1.2.3"), "1.2.3");
    writeFileSync(path.join(desktop, "src-tauri", "Cargo.toml"), '[package]\nversion = "1.2.2"\n');
    assert.throws(() => validateDesktopVersion(directory, "1.2.3"), /version mismatch/i);
    writeFileSync(path.join(desktop, "src-tauri", "Cargo.toml"), '[package]\nversion = "1.2.3"\n');
    writeFileSync(
      path.join(desktop, "src-tauri", "Cargo.lock"),
      'version = 4\n\n[[package]]\nname = "paseo-desktop"\nversion = "1.2.2"\n',
    );
    assert.throws(() => validateDesktopVersion(directory, "1.2.3"), /Cargo\.lock/i);
    writeFileSync(
      path.join(desktop, "src-tauri", "Cargo.lock"),
      'version = 4\n\n[[package]]\nname = "paseo-desktop"\nversion = "1.2.3"\n',
    );
    assert.equal(validateDesktopVersion(directory, "1.2.3"), "1.2.3");
  });
});

test("Linux updater metadata selects and verifies the matching Debian and AppImage formats", () => {
  withDirectory((directory) => {
    const app = signingFixture(Buffer.from("AppImage payload"));
    const deb = signingFixture(Buffer.from("Debian payload"), false, app.keys);
    const bundleDir = path.join(directory, "bundle");
    for (const [folder, filename, bytes, signature] of [
      ["appimage", "Kronos_1.2.3_amd64.AppImage", "AppImage payload", app.encoded],
      ["deb", "Kronos_1.2.3_amd64.deb", "Debian payload", deb.encoded],
    ]) {
      mkdirSync(path.join(bundleDir, folder), { recursive: true });
      writeFileSync(path.join(bundleDir, folder, filename), bytes);
      writeFileSync(path.join(bundleDir, folder, `${filename}.sig`), signature);
    }
    const outputDir = path.join(directory, "stage");
    const options = {
      bundleDir,
      outputDir,
      version: "1.2.3",
      platform: "linux",
      arch: "x86_64",
      publicKey: app.pubkey,
    };
    const manifest = stageRelease(options);
    assert.match(manifest.platforms["linux-x86_64-deb"].url, /\.deb$/);
    assert.equal(manifest.platforms["linux-x86_64-deb"].signature, deb.encoded);
    assert.match(manifest.platforms["linux-x86_64-appimage"].url, /\.AppImage$/);
    assert.equal(manifest.platforms["linux-x86_64-appimage"].signature, app.encoded);
    mergeReleases({
      inputDir: outputDir,
      outputDir: path.join(directory, "final"),
      version: "1.2.3",
      publicKey: app.pubkey,
    });
    const metadataPath = path.join(outputDir, "latest-linux-x86_64.json");
    const substituted = JSON.parse(readFileSync(metadataPath, "utf8"));
    substituted.platforms["linux-x86_64-deb"] = substituted.platforms["linux-x86_64-appimage"];
    writeFileSync(metadataPath, JSON.stringify(substituted));
    assert.throws(
      () =>
        mergeReleases({
          inputDir: outputDir,
          outputDir: path.join(directory, "final"),
          version: "1.2.3",
          publicKey: app.pubkey,
        }),
      /format/i,
    );
    writeFileSync(path.join(bundleDir, "deb", "Kronos_1.2.3_amd64.deb.sig"), app.encoded);
    assert.throws(() => stageRelease(options), /signature/i);
    rmSync(path.join(bundleDir, "deb", "Kronos_1.2.3_amd64.deb.sig"));
    assert.throws(() => stageRelease(options), /sig|ENOENT/i);
    writeFileSync(path.join(bundleDir, "deb", "Kronos_1.2.3_amd64.deb.sig"), deb.encoded);
    rmSync(path.join(bundleDir, "appimage"), { recursive: true });
    assert.deepEqual(Object.keys(stageRelease(options).platforms), ["linux-x86_64-deb"]);
  });
});

test("partial same-version release preserves and revalidates existing platform updater assets", () => {
  withDirectory((directory) => {
    const windows = signingFixture(Buffer.from("Windows payload"));
    const mac = signingFixture(Buffer.from("macOS payload"), false, windows.keys);
    const winBundle = path.join(directory, "windows-bundle");
    mkdirSync(path.join(winBundle, "nsis"), { recursive: true });
    const installer = "Kronos_1.2.3_x64-setup.exe";
    writeFileSync(path.join(winBundle, "nsis", installer), "Windows payload");
    writeFileSync(path.join(winBundle, "nsis", `${installer}.sig`), windows.encoded);
    const winStage = path.join(directory, "windows-stage");
    stageRelease({
      bundleDir: winBundle,
      outputDir: winStage,
      version: "1.2.3",
      platform: "windows",
      arch: "x86_64",
      publicKey: windows.pubkey,
    });
    const previousDir = path.join(directory, "published");
    mergeReleases({
      inputDir: winStage,
      outputDir: previousDir,
      version: "1.2.3",
      publicKey: windows.pubkey,
    });
    const macBundle = path.join(directory, "mac-bundle");
    mkdirSync(path.join(macBundle, "macos"), { recursive: true });
    writeFileSync(path.join(macBundle, "macos", "Kronos.app.tar.gz"), "macOS payload");
    writeFileSync(path.join(macBundle, "macos", "Kronos.app.tar.gz.sig"), mac.encoded);
    const macStage = path.join(directory, "mac-stage");
    stageRelease({
      bundleDir: macBundle,
      outputDir: macStage,
      version: "1.2.3",
      platform: "darwin",
      arch: "aarch64",
      publicKey: windows.pubkey,
    });
    const options = {
      inputDir: macStage,
      outputDir: path.join(directory, "final"),
      version: "1.2.3",
      publicKey: windows.pubkey,
      requiredPlatforms: ["windows-x86_64", "darwin-aarch64"],
    };
    assert.throws(() => mergeReleases(options), /missing.*windows-x86_64/i);
    const manifest = mergeReleases({ ...options, previousDir });
    assert.deepEqual(Object.keys(manifest.platforms).sort(), ["darwin-aarch64", "windows-x86_64"]);
    assert.equal(readFileSync(path.join(options.outputDir, installer), "utf8"), "Windows payload");
    writeFileSync(path.join(previousDir, installer), "tampered");
    assert.throws(() => mergeReleases({ ...options, previousDir }), /signature|alias/i);
  });
});
