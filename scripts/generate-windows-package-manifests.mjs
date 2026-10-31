import { createHash } from "node:crypto";
import { access, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import yaml from "js-yaml";
import { isMainModule } from "./is-main-module.mjs";

const packageIdentifier = "Xenonesis.Kronos";
const manifestVersion = "1.6.0";

function parseArguments(args) {
  const options = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const option = args[index];
    const value = args[index + 1];
    if (!option?.startsWith("--") || !value || value.startsWith("--")) {
      throw new Error(
        "Usage: node scripts/generate-windows-package-manifests.mjs --version <version> --installer <path> --output <directory>",
      );
    }
    if (options.has(option)) throw new Error(`Duplicate option: ${option}`);
    options.set(option, value);
  }

  const version = options.get("--version");
  const installerPath = options.get("--installer");
  const outputDirectory = options.get("--output");
  if (options.size !== 3 || !version || !installerPath || !outputDirectory) {
    throw new Error(
      "Usage: node scripts/generate-windows-package-manifests.mjs --version <version> --installer <path> --output <directory>",
    );
  }
  return { version, installerPath, outputDirectory };
}

function createWingetManifest(type, fields) {
  return yaml.dump(
    {
      $schema: `https://aka.ms/winget-manifest.${type}.1.6.0.schema.json`,
      PackageIdentifier: packageIdentifier,
      PackageVersion: fields.version,
      ManifestType: type,
      ManifestVersion: manifestVersion,
      ...fields.values,
    },
    { lineWidth: -1, noRefs: true },
  );
}

export async function generateWindowsPackageManifests({ version, installerPath, outputDirectory }) {
  if (typeof version !== "string" || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid release version: ${version}`);
  }

  try {
    await access(installerPath);
  } catch {
    throw new Error(`Installer file does not exist: ${installerPath}`);
  }
  const installerStat = await stat(installerPath);
  if (!installerStat.isFile() || installerStat.size === 0) {
    throw new Error(`Installer must be a nonempty file: ${installerPath}`);
  }
  const installerBytes = await readFile(installerPath);
  const installerSha256 = createHash("sha256").update(installerBytes).digest("hex");
  const installerName = `Kronos_${version}_x64-setup.exe`;
  const installerUrl = `https://github.com/Xenonesis/Kronos/releases/download/v${version}/${installerName}`;

  const wingetDirectory = path.join(outputDirectory, "winget");
  const scoopDirectory = path.join(outputDirectory, "scoop");
  await Promise.all([
    mkdir(wingetDirectory, { recursive: true }),
    mkdir(scoopDirectory, { recursive: true }),
  ]);

  const wingetManifests = [
    [
      "Xenonesis.Kronos.yaml",
      createWingetManifest("version", {
        version,
        values: { DefaultLocale: "en-US" },
      }),
    ],
    [
      "Xenonesis.Kronos.installer.yaml",
      createWingetManifest("installer", {
        version,
        values: {
          Installers: [
            {
              Architecture: "x64",
              InstallerType: "nullsoft",
              InstallerUrl: installerUrl,
              InstallerSha256: installerSha256.toUpperCase(),
            },
          ],
        },
      }),
    ],
    [
      "Xenonesis.Kronos.locale.en-US.yaml",
      createWingetManifest("defaultLocale", {
        version,
        values: {
          PackageLocale: "en-US",
          Publisher: "Xenonesis",
          PackageName: "Kronos",
          License: "Apache-2.0",
          ShortDescription: "Kronos desktop application",
          PackageUrl: "https://github.com/Xenonesis/Kronos",
          PublisherUrl: "https://github.com/Xenonesis",
          LicenseUrl: "https://github.com/Xenonesis/Kronos/blob/main/LICENSE",
          ReleaseNotesUrl: `https://github.com/Xenonesis/Kronos/releases/tag/v${version}`,
        },
      }),
    ],
  ];

  const scoopManifest = {
    version,
    description: "Kronos desktop application",
    homepage: "https://github.com/Xenonesis/Kronos",
    license: "Apache-2.0",
    url: installerUrl,
    hash: installerSha256,
    installer: {
      file: installerName,
      args: [],
    },
  };

  await Promise.all([
    ...wingetManifests.map(([name, contents]) =>
      writeFile(path.join(wingetDirectory, name), contents),
    ),
    writeFile(
      path.join(scoopDirectory, "Kronos.json"),
      `${JSON.stringify(scoopManifest, null, 2)}\n`,
    ),
  ]);

  return {
    installerUrl,
    installerSha256,
    files: [
      ...wingetManifests.map(([name]) => path.join(wingetDirectory, name)),
      path.join(scoopDirectory, "Kronos.json"),
    ],
  };
}

async function main() {
  const { version, installerPath, outputDirectory } = parseArguments(process.argv.slice(2));
  const result = await generateWindowsPackageManifests({ version, installerPath, outputDirectory });
  console.log(`Generated ${result.files.length} package-manager manifests for ${version}`);
  console.log(`Installer SHA-256: ${result.installerSha256}`);
  console.log(`Installer URL: ${result.installerUrl}`);
  for (const file of result.files) console.log(file);
}

if (isMainModule(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
