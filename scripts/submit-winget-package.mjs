import { access, readFile } from "node:fs/promises";
import path from "node:path";
import yaml from "js-yaml";
import { isMainModule } from "./is-main-module.mjs";

const DEFAULT_TARGET_REPO = "microsoft/winget-pkgs";
const PACKAGE_IDENTIFIER = "Xenonesis.Kronos";

function parseArguments(args) {
  const options = new Map();
  for (let index = 0; index < args.length; index += 1) {
    const current = args[index];
    if (current === "--dry-run") {
      options.set("--dry-run", "true");
      continue;
    }
    if (current.startsWith("--")) {
      const next = args[index + 1];
      if (!next || next.startsWith("--")) {
        throw new Error(`Option ${current} requires a value.`);
      }
      options.set(current, next);
      index += 1;
    }
  }

  const version = options.get("--version");
  const manifestsDir = options.get("--manifests-dir");
  const dryRun = options.get("--dry-run") === "true";
  const token = options.get("--token") || process.env.WINGET_GITHUB_TOKEN || process.env.GITHUB_TOKEN;
  const targetRepo = options.get("--repo") || DEFAULT_TARGET_REPO;

  if (!version || !manifestsDir) {
    throw new Error(
      "Usage: node scripts/submit-winget-package.mjs --version <version> --manifests-dir <path> [--dry-run] [--token <token>] [--repo <repo>]",
    );
  }

  return { version, manifestsDir, dryRun, token, targetRepo };
}

export function computeWingetManifestPath(packageId, version, fileName) {
  const [publisher, packageName] = packageId.split(".");
  const initial = publisher.charAt(0).toLowerCase();
  return path.posix.join("manifests", initial, publisher, packageName, version, fileName);
}

export async function validateWingetManifests(manifestsDir, version) {
  const requiredFiles = [
    `${PACKAGE_IDENTIFIER}.yaml`,
    `${PACKAGE_IDENTIFIER}.installer.yaml`,
    `${PACKAGE_IDENTIFIER}.locale.en-US.yaml`,
  ];

  const loaded = [];
  for (const file of requiredFiles) {
    const fullPath = path.join(manifestsDir, file);
    try {
      await access(fullPath);
    } catch {
      throw new Error(`Missing required Winget manifest file: ${fullPath}`);
    }
    const contents = await readFile(fullPath, "utf8");
    if (!contents.trim()) {
      throw new Error(`Winget manifest file is empty: ${fullPath}`);
    }
    let parsed;
    try {
      parsed = yaml.load(contents);
    } catch (parseError) {
      throw new Error(`Failed to parse YAML in ${file}: ${parseError.message}`);
    }
    if (!parsed || parsed.PackageIdentifier !== PACKAGE_IDENTIFIER) {
      throw new Error(
        `Invalid PackageIdentifier in ${file}. Expected ${PACKAGE_IDENTIFIER}, got ${parsed?.PackageIdentifier}`,
      );
    }
    if (parsed.PackageVersion !== version) {
      throw new Error(
        `Version mismatch in ${file}. Expected ${version}, got ${parsed?.PackageVersion}`,
      );
    }
    loaded.push({
      fileName: file,
      targetPath: computeWingetManifestPath(PACKAGE_IDENTIFIER, version, file),
      rawContent: contents,
      parsed,
    });
  }

  return loaded;
}

export async function submitWingetPackage({
  version,
  manifestsDir,
  dryRun = false,
  token,
  targetRepo = DEFAULT_TARGET_REPO,
  fetchImpl = globalThis.fetch,
}) {
  if (typeof version !== "string" || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid package version format: ${version}`);
  }

  // Support manifests directory pointing directly to winget folder or parent
  const resolvedDir = (await access(path.join(manifestsDir, `${PACKAGE_IDENTIFIER}.yaml`)).then(() => true).catch(() => false))
    ? manifestsDir
    : path.join(manifestsDir, "winget");

  const validatedManifests = await validateWingetManifests(resolvedDir, version);

  if (dryRun || !token) {
    return {
      success: true,
      dryRun: true,
      version,
      targetRepo,
      reason: dryRun ? "Explicit --dry-run requested" : "No GitHub authentication token supplied",
      files: validatedManifests.map((m) => ({
        fileName: m.fileName,
        targetPath: m.targetPath,
      })),
    };
  }

  // GitHub REST API Submission
  const authHeaders = {
    Authorization: `Bearer ${token}`,
    Accept: "application/vnd.github+json",
    "User-Agent": "Kronos-Winget-Automation",
    "X-GitHub-Api-Version": "2022-11-28",
  };

  // 1. Get authenticated user
  const userRes = await fetchImpl("https://api.github.com/user", { headers: authHeaders });
  if (!userRes.ok) {
    throw new Error(`GitHub user authentication failed: ${userRes.status} ${userRes.statusText}`);
  }
  const user = await userRes.json();
  const authUser = user.login;

  // 2. Check if fork exists or fork target repo
  const [targetOwner, targetRepoName] = targetRepo.split("/");
  let forkOwner = authUser;
  if (authUser.toLowerCase() !== targetOwner.toLowerCase()) {
    const forkRes = await fetchImpl(`https://api.github.com/repos/${targetRepo}/forks`, {
      method: "POST",
      headers: authHeaders,
    });
    if (!forkRes.ok && forkRes.status !== 202) {
      throw new Error(`Failed to fork target repo: ${forkRes.status} ${forkRes.statusText}`);
    }
  } else {
    forkOwner = targetOwner;
  }

  // 3. Get base repo default branch SHA
  const repoRes = await fetchImpl(`https://api.github.com/repos/${targetRepo}`, { headers: authHeaders });
  if (!repoRes.ok) {
    throw new Error(`Failed to get target repo info: ${repoRes.status} ${repoRes.statusText}`);
  }
  const repoData = await repoRes.json();
  const defaultBranch = repoData.default_branch || "master";

  const refRes = await fetchImpl(
    `https://api.github.com/repos/${targetRepo}/git/ref/heads/${defaultBranch}`,
    { headers: authHeaders },
  );
  if (!refRes.ok) {
    throw new Error(`Failed to resolve ref for ${defaultBranch}: ${refRes.status}`);
  }
  const refData = await refRes.json();
  const baseSha = refData.object.sha;

  // 4. Create branch in fork
  const branchName = `kronos-${version}`;
  const createBranchRes = await fetchImpl(`https://api.github.com/repos/${forkOwner}/${targetRepoName}/git/refs`, {
    method: "POST",
    headers: { ...authHeaders, "Content-Type": "application/json" },
    body: JSON.stringify({
      ref: `refs/heads/${branchName}`,
      sha: baseSha,
    }),
  });
  // If branch already exists (422), continue updating files on that branch

  // 5. Commit/upload each manifest file
  for (const manifest of validatedManifests) {
    const contentBase64 = Buffer.from(manifest.rawContent, "utf8").toString("base64");
    const filePath = manifest.targetPath;

    // Check if file already exists in branch to supply sha if updating
    let existingSha;
    const existingFileRes = await fetchImpl(
      `https://api.github.com/repos/${forkOwner}/${targetRepoName}/contents/${filePath}?ref=${branchName}`,
      { headers: authHeaders },
    );
    if (existingFileRes.ok) {
      const existingData = await existingFileRes.json();
      existingSha = existingData.sha;
    }

    const putRes = await fetchImpl(
      `https://api.github.com/repos/${forkOwner}/${targetRepoName}/contents/${filePath}`,
      {
        method: "PUT",
        headers: { ...authHeaders, "Content-Type": "application/json" },
        body: JSON.stringify({
          message: `Add ${PACKAGE_IDENTIFIER} version ${version}`,
          content: contentBase64,
          branch: branchName,
          ...(existingSha ? { sha: existingSha } : {}),
        }),
      },
    );

    if (!putRes.ok && putRes.status !== 200 && putRes.status !== 201) {
      throw new Error(`Failed to write file ${filePath}: ${putRes.status} ${putRes.statusText}`);
    }
  }

  // 6. Open Pull Request to target repo
  const prRes = await fetchImpl(`https://api.github.com/repos/${targetRepo}/pulls`, {
    method: "POST",
    headers: { ...authHeaders, "Content-Type": "application/json" },
    body: JSON.stringify({
      title: `New package: ${PACKAGE_IDENTIFIER} version ${version}`,
      head: `${forkOwner}:${branchName}`,
      base: defaultBranch,
      body: `Automated package manifest submission for ${PACKAGE_IDENTIFIER} version ${version}.\n\nInstaller and checksums verified via Kronos release automation.`,
      maintainer_can_modify: true,
    }),
  });

  if (!prRes.ok) {
    const errorBody = await prRes.text();
    throw new Error(`Failed to create pull request on ${targetRepo}: ${prRes.status} - ${errorBody}`);
  }

  const prData = await prRes.json();
  return {
    success: true,
    dryRun: false,
    version,
    pullRequestUrl: prData.html_url,
    pullRequestNumber: prData.number,
  };
}

async function runCli() {
  try {
    const options = parseArguments(process.argv.slice(2));
    console.log(
      `\x1b[36m▶ Winget Package Submission:\x1b[0m ${PACKAGE_IDENTIFIER} v${options.version} (dry-run: ${options.dryRun})`,
    );
    const result = await submitWingetPackage(options);
    if (result.dryRun) {
      console.log(`\x1b[33mℹ Validation Passed (Dry Run):\x1b[0m ${result.reason}`);
      for (const file of result.files) {
        console.log(`  ✓ ${file.fileName} → ${file.targetPath}`);
      }
    } else {
      console.log(`\x1b[32m✔ Pull request opened successfully:\x1b[0m ${result.pullRequestUrl}`);
    }
  } catch (err) {
    console.error(`\x1b[31m✖ Error submitting Winget package:\x1b[0m ${err.message}`);
    process.exit(1);
  }
}

if (isMainModule(import.meta.url)) {
  runCli();
}
