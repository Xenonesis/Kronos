import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isMainModule } from "./is-main-module.mjs";

const defaultRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function syncNativeVersions(rootDir, version, touched) {
  const nativeDir = path.join(rootDir, "packages/desktop-tauri/src-tauri");
  if (!existsSync(nativeDir)) return;
  const update = (file, transform) => {
    const source = readFileSync(file, "utf8");
    const next = transform(source);
    if (next !== source) {
      writeFileSync(file, next);
      touched.push(path.relative(rootDir, file));
    }
  };
  update(path.join(nativeDir, "tauri.conf.json"), (source) => {
    const config = JSON.parse(source);
    if (config.version === version) return source;
    // Edit only the root version line, preserving security/updater configuration
    // and the rest of the formatted document byte-for-byte.
    const pattern = /^(  "version"\s*:\s*)"[^"\r\n]*"/m;
    if (!pattern.test(source))
      throw new Error("Expected a root version line in formatted tauri.conf.json");
    return source.replace(pattern, (_, prefix) => `${prefix}${JSON.stringify(version)}`);
  });
  const replacePackageVersion = (section) => {
    const name = section.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
    if (name !== "paseo-desktop") return section;
    if (!/^version\s*=\s*"[^"]+"/m.test(section))
      throw new Error("Native Cargo package version is missing");
    return section.replace(/^(version\s*=\s*)"[^"]+"/m, (_, prefix) => `${prefix}"${version}"`);
  };
  update(path.join(nativeDir, "Cargo.toml"), (source) => {
    const section = source.match(/\[package\][\s\S]*?(?=\n\[|$)/)?.[0];
    if (!section || !/^name\s*=\s*"paseo-desktop"/m.test(section))
      throw new Error("Native Cargo package is missing");
    return source.replace(section, replacePackageVersion(section));
  });
  const lockPath = path.join(nativeDir, "Cargo.lock");
  if (existsSync(lockPath))
    update(lockPath, (source) => {
      let packages = 0;
      const next = source.replace(/\[\[package\]\][\s\S]*?(?=\n\[\[package\]\]|$)/g, (section) => {
        if (/^name\s*=\s*"paseo-desktop"/m.test(section)) packages += 1;
        return replacePackageVersion(section);
      });
      if (packages !== 1) throw new Error("Expected one native root package in Cargo.lock");
      return next;
    });
}

export function syncWorkspaceVersions(rootDir = defaultRoot) {
  const rootPackage = JSON.parse(readFileSync(path.join(rootDir, "package.json"), "utf8"));
  const rootVersion = rootPackage.version;
  const workspacePaths = Array.isArray(rootPackage.workspaces) ? rootPackage.workspaces : [];
  const sharedMetadata = {
    homepage: rootPackage.homepage,
    repository: rootPackage.repository,
    author: rootPackage.author,
    license: rootPackage.license,
  };
  if (typeof rootVersion !== "string" || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(rootVersion)) {
    throw new Error('Root package.json must contain a valid "version"');
  }
  const dependencySections = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
  ];
  const touched = [];
  for (const workspacePath of workspacePaths) {
    const packagePath = path.join(rootDir, workspacePath, "package.json");
    if (!existsSync(packagePath)) continue;
    const pkg = JSON.parse(readFileSync(packagePath, "utf8"));
    let changed = false;
    if (pkg.version !== rootVersion) {
      pkg.version = rootVersion;
      changed = true;
    }
    if (pkg.name === "@getpaseo/desktop") {
      for (const [field, value] of Object.entries(sharedMetadata)) {
        if (JSON.stringify(pkg[field]) !== JSON.stringify(value)) {
          pkg[field] = value;
          changed = true;
        }
      }
    }
    // Private packages resolve local siblings; published packages pin the release.
    const internalDepRange = pkg.private === true ? "*" : rootVersion;
    for (const section of dependencySections) {
      const deps = pkg[section];
      if (!deps || typeof deps !== "object") continue;
      for (const name of Object.keys(deps)) {
        if (name.startsWith("@getpaseo/") && name !== pkg.name && deps[name] !== internalDepRange) {
          deps[name] = internalDepRange;
          changed = true;
        }
      }
    }
    if (changed) {
      writeFileSync(packagePath, `${JSON.stringify(pkg, null, 2)}\n`);
      touched.push(path.relative(rootDir, packagePath));
    }
  }
  syncNativeVersions(rootDir, rootVersion, touched);
  return touched;
}

if (isMainModule(import.meta.url)) {
  const touched = syncWorkspaceVersions();
  const { version } = JSON.parse(readFileSync(path.join(defaultRoot, "package.json"), "utf8"));
  if (touched.length === 0)
    console.log(`Workspace and native versions already synced to ${version}`);
  else {
    console.log(`Synced to ${version}:`);
    for (const file of touched) console.log(`- ${file}`);
  }
}
