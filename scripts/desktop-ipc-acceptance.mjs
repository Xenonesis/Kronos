import path from "node:path";
import { createHash, randomBytes } from "node:crypto";
import { isMainModule } from "./is-main-module.mjs";
import childProcess from "node:child_process";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { seedControlledCodexFixture } from "./desktop-ipc-acceptance-fixture.mjs";
import { createReadStream } from "node:fs";
import {
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  open,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";

const repoRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");

async function directoryManifest(root, excludeTarget = false) {
  if (typeof root !== "string" || !path.isAbsolute(root)) {
    throw new Error("Build manifest roots must be absolute");
  }
  const manifest = [];
  async function visit(directory) {
    const metadata = await lstat(directory);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      throw new Error("Build manifest directories cannot be linked");
    }
    for (const entry of (await readdir(directory, { withFileTypes: true })).sort((a, b) =>
      a.name.localeCompare(b.name),
    )) {
      if (excludeTarget && directory === root && entry.name === "target") continue;
      const filename = path.join(directory, entry.name);
      if (entry.isDirectory()) await visit(filename);
      else manifest.push({ path: filename, sha256: await fileHash(filename) });
    }
  }
  await visit(root);
  validateManifest(manifest);
  return manifest;
}

export async function captureBuildInputs({ nativeRoot, assetsRoot, nativeFiles = [] }) {
  const nativeSources = await directoryManifest(nativeRoot, true);
  for (const filename of nativeFiles)
    nativeSources.push({ path: filename, sha256: await fileHash(filename) });
  validateManifest(nativeSources);
  return {
    nativeRoot,
    assetsRoot,
    nativeFiles,
    nativeSources,
    assets: await directoryManifest(assetsRoot),
  };
}

export async function writeBuildReceiptFromCapture({
  executable,
  receiptPath,
  inputs,
  captureReceiptPath,
}) {
  if (typeof captureReceiptPath !== "string" || !path.isAbsolute(captureReceiptPath)) {
    throw new Error("Build receipt requires a native Job capture receipt");
  }
  const metadata = await lstat(captureReceiptPath);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 64 * 1024) {
    throw new Error("Native Job capture receipt must be a bounded regular file");
  }
  const capture = JSON.parse(await readFile(captureReceiptPath, "utf8"));
  return writeBuildReceipt({
    executable,
    receiptPath,
    inputs,
    build: {
      argv: [capture.executable, ...(capture.argv || [])],
      exitCode: capture.exitCode,
      timedOut: capture.timedOut,
      reaped: capture.leaderReaped,
      jobAssignedBeforeResume: capture.jobAssignedBeforeResume,
      treeEmpty: capture.treeEmpty,
      captureReceiptPath,
    },
  });
}

export async function writeBuildReceipt({ executable, receiptPath, inputs, build }) {
  validateBuildCommand(build);
  const captureReceipt = await captureBuildReceipt(build.captureReceiptPath, build);
  const buildRecord = { ...build };
  delete buildRecord.captureReceiptPath;
  const current = await captureBuildInputs(inputs);
  if (JSON.stringify(current) !== JSON.stringify(inputs)) {
    throw new Error("Compiled inputs changed during build; no receipt written");
  }
  const receipt = {
    schemaVersion: 1,
    ...current,
    executable: { path: executable, sha256: await fileHash(executable) },
    build: { ...buildRecord, captureReceipt },
  };
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx" });
  return receipt;
}

async function readBuildCaptureMarker(filename) {
  const metadata = await lstat(filename);
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.nlink !== 1 ||
    metadata.size > 64 * 1024
  ) {
    throw new Error("Native Job capture owner marker must be a bounded single-link file");
  }
  return JSON.parse(await readFile(filename, "utf8"));
}

export function requireShortAcceptanceBuildParent(parent) {
  if (
    typeof parent !== "string" ||
    !path.isAbsolute(parent) ||
    path.basename(parent) !== "builds" ||
    (path.dirname(parent) !== path.join(repoRoot, ".superpowers") &&
      path.dirname(parent) !== path.join(repoRoot, ".superpowers", "sdd", path.basename(repoRoot)))
  )
    throw new Error(
      "Acceptance build root must be short and repo-local: <repo>/.superpowers/builds",
    );
  return parent;
}

async function assertOwnedBuildCapture(filename, capture) {
  const reportDirectory = path.dirname(filename);
  const root = path.dirname(reportDirectory);
  const buildParent = path.dirname(root);
  if (
    path.basename(filename) !== "build-capture.json" ||
    path.basename(reportDirectory) !== "reports" ||
    path.basename(root) !== `kronos-ipc-acceptance-${capture.runId}` ||
    path.basename(buildParent) !== "builds" ||
    (path.dirname(buildParent) !== path.join(repoRoot, ".superpowers") &&
      path.dirname(buildParent) !==
        path.join(repoRoot, ".superpowers", "sdd", path.basename(repoRoot)))
  ) {
    throw new Error("Native Job capture receipt is outside its owned short run root: " + root);
  }
  const [owner, descriptor] = await Promise.all([
    readBuildCaptureMarker(path.join(root, "owner.json")),
    readBuildCaptureMarker(path.join(root, "run.json")),
  ]);
  if (
    owner.schemaVersion !== 1 ||
    owner.runId !== capture.runId ||
    descriptor.schemaVersion !== 1 ||
    descriptor.runId !== capture.runId ||
    descriptor.transport !== "ipc" ||
    descriptor.caseId !== "preflight"
  ) {
    throw new Error("Native Job capture receipt owner mismatch");
  }
}

function validCaptureIdentity(capture) {
  return (
    capture.schemaVersion === 1 &&
    /^[a-f0-9]{32}$/.test(capture.runId) &&
    Number.isSafeInteger(capture.parentPid) &&
    capture.parentPid > 0 &&
    Number.isSafeInteger(capture.childPid) &&
    capture.childPid > 0 &&
    capture.parentPid !== capture.childPid &&
    typeof capture.executable === "string" &&
    path.isAbsolute(capture.executable) &&
    Array.isArray(capture.argv) &&
    capture.argv.every((argument) => typeof argument === "string") &&
    typeof capture.cwd === "string" &&
    path.isAbsolute(capture.cwd)
  );
}

function validCaptureOutcome(capture, build) {
  return (
    capture.exitCode === 0 &&
    capture.timedOut === false &&
    capture.leaderReaped === true &&
    capture.treeEmpty === true &&
    capture.jobAssignedBeforeResume === true &&
    capture.exitCode === build.exitCode &&
    capture.timedOut === build.timedOut &&
    capture.leaderReaped === build.reaped &&
    capture.treeEmpty === build.treeEmpty &&
    capture.jobAssignedBeforeResume === build.jobAssignedBeforeResume
  );
}

async function captureBuildReceipt(filename, build) {
  if (typeof filename !== "string" || !path.isAbsolute(filename)) {
    throw new Error("Build receipt requires a native Job capture receipt");
  }
  const metadata = await lstat(filename);
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.nlink !== 1 ||
    metadata.size > 64 * 1024
  ) {
    throw new Error("Native Job capture receipt must be a bounded single-link regular file");
  }
  const capture = JSON.parse(await readFile(filename, "utf8"));
  if (!capture || typeof capture !== "object" || Array.isArray(capture)) {
    throw new Error("Native Job capture receipt must be an object");
  }
  await assertOwnedBuildCapture(filename, capture);
  if (
    !validCaptureIdentity(capture) ||
    !validCaptureOutcome(capture, build) ||
    JSON.stringify([capture.executable, ...capture.argv]) !== JSON.stringify(build.argv)
  ) {
    throw new Error("Native Job capture receipt does not prove this successful build command");
  }
  return { path: filename, sha256: await fileHash(filename) };
}

async function validateBuildReceiptCapture(captureReceipt, build) {
  if (
    !captureReceipt ||
    typeof captureReceipt.path !== "string" ||
    !path.isAbsolute(captureReceipt.path) ||
    !/^[a-f0-9]{64}$/.test(captureReceipt.sha256) ||
    (await fileHash(captureReceipt.path)) !== captureReceipt.sha256
  ) {
    throw new Error("Native Job capture receipt hash mismatch");
  }
  await captureBuildReceipt(captureReceipt.path, build);
}

async function fileHash(filename) {
  if (typeof filename !== "string" || !path.isAbsolute(filename)) {
    throw new Error("Build manifest paths must be absolute");
  }
  const metadata = await lstat(filename);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error("Build manifest entry must be a regular file");
  }
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(filename)) hash.update(chunk);
  return hash.digest("hex");
}

function validateManifest(manifest) {
  if (!Array.isArray(manifest) || manifest.length === 0 || manifest.length > 100000) {
    throw new Error("Build receipt requires nonempty native and asset manifests");
  }
  const seen = new Set();
  for (const entry of manifest) {
    if (seen.has(entry.path)) throw new Error("Duplicate build manifest entry");
    seen.add(entry.path);
  }
}

function validateBuildCommand(build) {
  if (build?.exitCode !== 0 || build.timedOut !== false || build.reaped !== true) {
    throw new Error("Build receipt requires a successful reaped command");
  }
  if (build.jobAssignedBeforeResume !== true || build.treeEmpty !== true) {
    throw new Error("Build receipt requires verified Windows Job ownership and empty tree");
  }
  const argv = build.argv;
  const featureIndex = Array.isArray(argv) ? argv.indexOf("--features") : -1;
  if (
    featureIndex < 0 ||
    argv[featureIndex + 1] !== "ipc-acceptance" ||
    !argv.every((argument) => typeof argument === "string") ||
    !["build", "--no-bundle", "--ci"].every((argument) => argv.includes(argument)) ||
    argv.includes("--no-default-features")
  ) {
    throw new Error("Build receipt requires the private acceptance feature with default features");
  }
}

export async function validateBuildReceipt(executable, receiptPath) {
  const metadata = await lstat(receiptPath);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 8 * 1024 * 1024) {
    throw new Error("Build receipt must be a bounded regular file");
  }
  const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
  if (receipt.schemaVersion !== 1) throw new Error("Unsupported build receipt schema");
  validateBuildCommand(receipt.build);
  await validateBuildReceiptCapture(receipt.build.captureReceipt, receipt.build);
  if (!path.isAbsolute(executable) || receipt.executable?.path !== executable) {
    throw new Error("Build receipt executable path mismatch");
  }
  validateManifest(receipt.nativeSources);
  validateManifest(receipt.assets);
  for (const entry of [receipt.executable, ...receipt.nativeSources, ...receipt.assets]) {
    if (!/^[a-f0-9]{64}$/.test(entry.sha256) || (await fileHash(entry.path)) !== entry.sha256) {
      throw new Error(`Build receipt hash mismatch: ${entry.path}`);
    }
  }
  const current = await captureBuildInputs(receipt);
  for (const key of ["nativeSources", "assets"]) {
    if (JSON.stringify(current[key]) !== JSON.stringify(receipt[key])) {
      throw new Error(`Build manifest changed: ${key}`);
    }
  }
  return receipt;
}

export function validateObservation(native, renderer, expected) {
  if (!native || !renderer) throw new Error("Missing renderer callback delivery evidence");
  for (const key of ["runId", "caseId", "role"]) {
    if (!expected[key] || native[key] !== expected[key] || renderer[key] !== expected[key]) {
      throw new Error(`Observation identity mismatch: ${key}`);
    }
  }
  for (const key of ["incarnation", "sessionId", "generation"]) {
    if (!native[key] || native[key] !== renderer[key])
      throw new Error(`Observation identity mismatch: ${key}`);
  }
  validateNativeMeasurements(native);
  const sequence = renderer.lastReceivedSequence;
  if (
    !Number.isSafeInteger(native.lastAckSequence) ||
    !Number.isSafeInteger(sequence) ||
    sequence <= 0 ||
    sequence > native.lastAckSequence ||
    native.lastAckSequence > native.lastSentSequence
  )
    throw new Error("Renderer delivery lacks matching native ACK");
  validateRendererState(renderer, expected.caseId);
}

function evidencePath(value) {
  if (typeof value !== "string" || !path.isAbsolute(value))
    throw new Error("Preflight requires absolute owned paths");
  // These are internal canonical observations, not raw descriptor inputs.
  if (
    process.platform === "win32" &&
    value.startsWith("\\\\?\\") &&
    /^[a-z]:\\/i.test(value.slice(4))
  )
    value = value.slice(4);
  return path.resolve(value);
}

function requirePreflightPath(actual, wanted, label) {
  if (path.relative(evidencePath(wanted), evidencePath(actual)) !== "")
    throw new Error(`Preflight path mismatch: ${label}`);
}

function validatePreflightPaths(native, root) {
  const home = path.join(evidencePath(root), "home");
  const paths = {
    home,
    managerHome: home,
    configHome: home,
    tauriConfig: home,
    tauriData: home,
    tauriLocalData: home,
    tauriCache: path.join(home, "caches"),
    tauriLogs: path.join(home, "logs"),
    logs: path.join(root, "logs"),
    schedules: path.join(home, "schedules"),
    mcp: path.join(home, "mcp_servers.json"),
  };
  for (const [name, wanted] of Object.entries(paths))
    requirePreflightPath(native.directories?.[name], wanted, name);
  if (
    !Array.isArray(native.modelRoots) ||
    native.modelRoots.length !== 1 ||
    !Array.isArray(native.profiles) ||
    native.profiles.length !== 3
  )
    throw new Error("Preflight requires bounded model roots and all owned profiles");
  requirePreflightPath(native.modelRoots[0], path.join(home, "models"), "models");
  for (const [index, role] of ["main", "secondary", "guest"].entries())
    requirePreflightPath(native.profiles[index], path.join(root, "profile", role), role);
  if (!native.providerCommands || Object.keys(native.providerCommands).length !== 2)
    throw new Error("Preflight requires both owned fixture commands");
  for (const name of ["codex", "node"])
    requirePreflightPath(
      native.providerCommands[name],
      path.join(root, "bin", name + (process.platform === "win32" ? ".exe" : "")),
      name,
    );
}

function validatePreflight(native, expected) {
  for (const name of ["runId", "caseId", "transport"])
    if (!expected[name] || native[name] !== expected[name])
      throw new Error(`Preflight identity mismatch: ${name}`);
  if (!["ipc", "websocket"].includes(expected.transport))
    throw new Error("Preflight transport is invalid");
  for (const name of ["launcherPid", "childPid"])
    if (
      !Number.isSafeInteger(expected[name]) ||
      expected[name] <= 0 ||
      native.claims?.[name] !== expected[name]
    )
      throw new Error(`Preflight cleanup/claim mismatch: ${name}`);
  if (
    expected.launcherPid === expected.childPid ||
    native.pid !== expected.childPid ||
    native.claims?.ownerRunId !== expected.runId ||
    native.resourceClaimsRunId !== expected.runId ||
    native.identifier !== `sh.paseo.desktop.ipc-acceptance.${expected.runId}`
  )
    throw new Error("Preflight owned identity/claims mismatch");
  validatePreflightInitialization(native);
  validatePreflightPaths(native, expected.root);
}

function validatePreflightInitialization(native) {
  for (const [name, wanted] of Object.entries({
    engineReady: true,
    coreStopped: false,
    schedulerRegistered: true,
    windowCount: 0,
    configuredWindowCount: 0,
    osIntegrationsEnabled: false,
    initialPipeLeaseTransferred: true,
    externalMcpServerCount: 0,
    providerStarts: 0,
    networkError: null,
  }))
    if (native[name] !== wanted) throw new Error(`Preflight initialization mismatch: ${name}`);
  if (
    !Number.isSafeInteger(native.engineEpoch) ||
    native.engineEpoch <= 0 ||
    !Number.isSafeInteger(native.networkPort) ||
    native.networkPort <= 0 ||
    native.networkPort > 65535 ||
    native.networkPort === 6767
  )
    throw new Error("Preflight requires ready owned core/listener measurements");
  if (
    JSON.stringify(native.installedProviders) !== '["codex"]' ||
    !Array.isArray(native.providerPids) ||
    native.providerPids.length !== 0
  )
    throw new Error("Preflight resolved unexpected providers or spawned a provider");
}

function validateBlockedResult(result, expected) {
  const reason = result.renderer.reason;
  if (typeof reason !== "string" || !reason.trim())
    throw new Error("A blocked acceptance case must state its reason");
  for (const key of ["runId", "caseId", "role"]) {
    if (result.native[key] !== expected[key] || result.renderer[key] !== expected[key])
      throw new Error(`Observation identity mismatch: ${key}`);
  }
  for (const key of ["incarnation", "sessionId", "generation"]) {
    if (!result.native[key] || result.native[key] !== result.renderer[key])
      throw new Error(`Observation identity mismatch: ${key}`);
  }
  return { ...result, outcome: "blocked" };
}

function throwScenarioFailure(renderer) {
  throw new Error(`Acceptance scenario failed: ${renderer.reason ?? "no reason recorded"}`);
}

// Backpressure is certified from native close/peak history plus renderer delivery claims.
function validateBackpressure(result) {
  const native = result.native;
  if (native.providerStarts !== 1)
    throw new Error("Backpressure requires exactly one controlled provider start");
  const closed = Array.isArray(native.closedSessions) ? native.closedSessions : [];
  const byCode = (code) => closed.filter((entry) => entry?.closeCode === code);
  for (const code of [1013, 1009, 1011, 1000]) {
    if (byCode(code).length === 0)
      throw new Error(`Backpressure missing native close evidence for ${code}`);
  }
  const overflow = byCode(1013).find((entry) => entry.channelPeakEvents >= 64);
  if (!overflow || overflow.sessionQueuePeakFrames < 2)
    throw new Error("Backpressure missing real high-water evidence at overflow");
  if (result.renderer?.burstDelivered !== true)
    throw new Error("Backpressure requires observed burst delivery after release");
  if (result.renderer?.resyncObserved !== true)
    throw new Error("Backpressure requires an observed resync on a new session");
}

/**
 * Validate the native run result envelope and join native/renderer evidence.
 * This is the only place a run becomes a `pass`; the native side never claims an outcome.
 */
export function validateLaunchResult(result, expected) {
  if (!result || typeof result !== "object" || Array.isArray(result))
    throw new Error("Missing native acceptance result");
  if (result.schemaVersion !== 1) throw new Error("Unsupported native acceptance result schema");
  if (result.runId !== expected.runId || result.caseId !== expected.caseId)
    throw new Error("Native acceptance result identity mismatch");
  if (typeof result.error === "string" && result.error)
    throw new Error(`Native acceptance case failed: ${result.error}`);
  if (!result.native || typeof result.native !== "object")
    throw new Error("Missing native acceptance result observation");
  if (result.renderer?.outcome === "fail") throwScenarioFailure(result.renderer);
  if (result.renderer?.outcome === "blocked" && !IMPLEMENTED_CASES.has(expected.caseId))
    return validateBlockedResult(result, expected);
  if (expected.caseId === "preflight") {
    if (result.renderer !== undefined) throw new Error("Preflight cannot use renderer evidence");
    validatePreflight(result.native, expected);
  } else {
    validateObservation(result.native, result.renderer, expected);
    if (expected.caseId === "trust-reload") validateTrustReload(result, expected);
    if (expected.caseId === "backpressure") validateBackpressure(result);
  }
  return { ...result, outcome: "pass" };
}

export async function readLaunchResult(filename, expected) {
  const maximum = 8 * 1024 * 1024;
  const metadata = await lstat(filename);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.nlink !== 1)
    throw new Error("Acceptance result must be a non-linked regular file");
  if (metadata.size > maximum) throw new Error("Acceptance result exceeds bound");
  const file = await open(filename, "r");
  try {
    const opened = await file.stat();
    if (
      !opened.isFile() ||
      opened.nlink !== 1 ||
      opened.ino !== metadata.ino ||
      opened.dev !== metadata.dev
    )
      throw new Error("Acceptance result file changed or is linked");
    const chunks = [];
    let size = 0;
    for await (const chunk of file.createReadStream({ autoClose: false, end: maximum })) {
      size += chunk.length;
      if (size > maximum) throw new Error("Acceptance result exceeds bound");
      chunks.push(chunk);
    }
    return validateLaunchResult(JSON.parse(Buffer.concat(chunks).toString("utf8")), expected);
  } finally {
    await file.close();
  }
}

function validateRendererState(renderer, caseId) {
  if (
    typeof renderer.visibleState !== "string" ||
    !renderer.visibleState ||
    renderer.outcome !== "pass" ||
    (caseId === "native-smoke" && renderer.visibleState !== "sidebar-visible")
  )
    throw new Error("Missing successful app-visible DOM state");
  if (renderer.serverInfoReceived !== true)
    throw new Error("Missing authenticated server_info callback");
  if (renderer.requestCompleted !== true)
    throw new Error("Missing completed frontend request callback");
}

function validateTrustReload(result, expected) {
  if (result.native.opened !== true)
    throw new Error("Trust-reload requires a delivered native session");
  if (expected.role === "secondary") {
    if (result.renderer.crossOwnerRejected !== true)
      throw new Error("Trust-reload secondary did not prove cross-owner rejection");
    return;
  }
  if (
    expected.role !== "main" ||
    result.renderer.staleGenerationRejected !== true ||
    result.renderer.sameEngineEpoch !== true ||
    result.renderer.guestRejected !== true
  )
    throw new Error(
      "Trust-reload main did not prove guest/stale rejection and unchanged core epoch",
    );
  const secondary = result.secondary;
  if (!secondary || !secondary.native || !secondary.renderer)
    throw new Error("Trust-reload requires native secondary-window delivery evidence");
  validateObservation(secondary.native, secondary.renderer, {
    runId: expected.runId,
    caseId: expected.caseId,
    role: "secondary",
  });
  validateTrustReload(secondary, { ...expected, role: "secondary" });
  if (secondary.native.engineEpoch !== result.native.engineEpoch)
    throw new Error("Trust-reload changed engine epoch across trusted windows");
}

function validateNativeMeasurements(native) {
  for (const key of [
    "engineEpoch",
    "lastSentSequence",
    "sessionQueuePeakFrames",
    "sessionQueuePeakDecodedBytes",
    "channelPeakEvents",
    "channelPeakSerializedBytes",
  ]) {
    if (!Number.isSafeInteger(native[key]) || native[key] <= 0)
      throw new Error(`Missing native measurement: ${key}`);
  }
  if (
    !Number.isSafeInteger(native.providerStarts) ||
    native.providerStarts < 0 ||
    !Array.isArray(native.providerPids) ||
    !native.providerPids.every((pid) => Number.isSafeInteger(pid) && pid > 0)
  )
    throw new Error("Missing native provider measurements");
  if (
    native.sessionQueuePeakFrames > 128 ||
    native.sessionQueuePeakDecodedBytes > 32 * 1024 * 1024 ||
    native.channelPeakEvents > 64 ||
    native.channelPeakSerializedBytes > 32 * 1024 * 1024
  ) {
    throw new Error("Observed queue measurement exceeded production bound");
  }
}

// Cases with a real fixed scenario; everything else must report blocked with a reason.
const IMPLEMENTED_CASES = new Set(["preflight", "native-smoke", "trust-reload", "backpressure"]);

const CASES = new Set([
  "preflight",
  "native-smoke",
  "trust-reload",
  "turn-history",
  "approval",
  "terminal",
  "files-watch-plugins",
  "network-independent",
  "backpressure",
  "remote-recovery",
  "remote-peer",
  "measurements",
]);

function validateCase(caseId, transport) {
  if (!CASES.has(caseId)) throw new Error("Unknown acceptance case");
  if (transport !== "ipc" && transport !== "websocket")
    throw new Error("Unknown acceptance transport");
}

function validateLaunchOptions(options) {
  validateCase(options?.caseId, options?.transport);
  for (const key of ["executable", "buildReceipt", "receiptDir"]) {
    if (typeof options[key] !== "string" || !path.isAbsolute(options[key])) {
      throw new Error(`Acceptance options require absolute ${key}`);
    }
  }
}

export function parseAcceptanceArguments(argv) {
  const names = new Map([
    ["--exe", "executable"],
    ["--build-receipt", "buildReceipt"],
    ["--case", "caseId"],
    ["--transport", "transport"],
    ["--receipt-dir", "receiptDir"],
  ]);
  const options = {};
  let gui = false;
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    if (flag === "--gui-acceptance" && !gui) {
      gui = true;
      continue;
    }
    const name = names.get(flag);
    if (!name || name in options || !argv[i + 1] || argv[i + 1].startsWith("--")) {
      throw new Error(
        "Unknown or duplicate acceptance argument; Usage: --gui-acceptance --exe <path> --build-receipt <path> --case <id> --transport <ipc|websocket> --receipt-dir <path>",
      );
    }
    options[name] = argv[++i];
  }
  if (!gui) throw new Error("Usage requires --gui-acceptance");
  for (const key of ["executable", "buildReceipt", "receiptDir"]) {
    if (options[key]) options[key] = path.resolve(options[key]);
  }
  validateLaunchOptions(options);
  return options;
}

export async function createAcceptanceRun({ caseId, transport }) {
  validateCase(caseId, transport);
  const ownerRoot = await mkdtemp(path.join(tmpdir(), "kronos-owned-supervisor-"));
  const runId = randomBytes(16).toString("hex");
  const root = path.join(ownerRoot, `kronos-ipc-acceptance-${runId}`);
  try {
    for (const directory of [
      "home/.codex",
      "home/AppData/Roaming",
      "home/AppData/Local",
      "bin",
      "logs",
      "tmp",
      "workspace",
      "reports",
      "profile/main",
      "profile/secondary",
      "profile/guest",
    ]) {
      await mkdir(path.join(root, directory), { recursive: true });
    }
    await writeFile(path.join(root, "owner.json"), JSON.stringify({ schemaVersion: 1, runId }), {
      flag: "wx",
    });
    const descriptor = path.join(root, "run.json");
    await writeFile(descriptor, JSON.stringify({ schemaVersion: 1, runId, caseId, transport }), {
      flag: "wx",
    });
    await writeFile(path.join(root, "home/mcp_servers.json"), "{}", { flag: "wx" });
    await seedControlledCodexFixture(path.join(root, "bin"), true);
    await copyFile(
      process.execPath,
      path.join(root, "bin", process.platform === "win32" ? "node.exe" : "node"),
    );
    // Node's codex.exe fixture resolves its exec script relative to provider CWD, not the bin directory.
    await copyFile(path.join(root, "bin/exec"), path.join(root, "workspace/exec"));
    return { root, ownerRoot, descriptor, runId, environment: acceptanceEnvironment(root) };
  } catch (error) {
    await rm(ownerRoot, { recursive: true, force: true }); // No process exists during resource preparation.
    throw error;
  }
}

function within(promise, milliseconds) {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error("Owned launcher deadline expired")), milliseconds);
    }),
  ]).finally(() => clearTimeout(timer));
}

async function captureLauncher(stream, filename) {
  const file = await open(filename, "wx");
  let size = 0;
  try {
    for await (const chunk of stream) {
      size += chunk.length;
      if (size > 4 * 1024 * 1024) throw new Error("Launcher diagnostics exceeded bound");
      await file.writeFile(chunk);
    }
  } finally {
    await file.close();
  }
}

async function launchOwned(executable, run, destination) {
  const child = childProcess.spawn(executable, ["--ipc-acceptance-run", run.descriptor], {
    cwd: path.join(run.root, "workspace"),
    env: run.environment,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  const exit = new Promise((resolve) => {
    child.once("exit", (code, signal) => resolve({ code, signal }));
    child.once("error", (error) => resolve({ error }));
  });
  const captures = ["stdout", "stderr"].map((name) =>
    captureLauncher(child[name], path.join(destination, `launcher-${name}.log`)).catch((error) => {
      child.kill();
      return error;
    }),
  );
  try {
    const status = await within(exit, 430000);
    const diagnostics = await within(Promise.all(captures), 3000);
    if (status.error) throw status.error;
    if (diagnostics.some((error) => error)) throw new Error("Launcher capture failed");
    if (status.code !== 0 || status.signal)
      throw new Error(`Owned launcher exited ${status.code}/${status.signal}`);
    const receipt = JSON.parse(
      await readFile(path.join(run.root, "reports/supervisor-exit.json"), "utf8"),
    );
    if (
      receipt.schemaVersion !== 1 ||
      receipt.runId !== run.runId ||
      receipt.parentPid !== child.pid ||
      !Number.isSafeInteger(receipt.childPid) ||
      receipt.childPid <= 0 ||
      receipt.childPid === receipt.parentPid ||
      receipt.leaderReaped !== true ||
      receipt.treeEmpty !== true ||
      receipt.timedOut !== false ||
      receipt.exitCode !== 0
    ) {
      throw new Error("Owned native cleanup receipt mismatch");
    }
    return receipt;
  } catch (error) {
    if (child.exitCode === null) child.kill(); // Exact owned launcher only; its Job closes with it.
    await within(exit, 3000).catch(() => {});
    throw new Error(`${error.message}; retain ${run.root}`, { cause: error });
  }
}

// Certified evidence must survive a WebView2 file that stays locked after the native
// cleanup receipt: retry briefly, then retain the owned root instead of discarding proof.
export async function releaseOwnedRoot(
  root,
  { remove = (target) => rm(target, { recursive: true, force: true }), delay, attempts = 5 } = {},
) {
  const wait = delay ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));
  let error = null;
  for (let attempt = 1; attempt <= attempts; attempt++) {
    try {
      await remove(root);
      return { removed: true, attempts: attempt };
    } catch (cause) {
      error = cause;
      if (attempt < attempts) await wait(200 * attempt);
    }
  }
  return { removed: false, attempts, error };
}

// A blocked case is recorded evidence, not a successful run.
export function acceptanceExitCode(result) {
  if (result?.outcome === "pass") return 0;
  if (result?.outcome === "blocked") return 3;
  throw new Error("Unsupported acceptance outcome");
}

export async function runDesktopIpcAcceptance(options) {
  validateLaunchOptions(options);
  await validateBuildReceipt(options.executable, options.buildReceipt); // Never spawn or allocate run state before this.
  if (process.platform !== "win32")
    throw new Error("Native acceptance requires Windows ownership checks");
  const scenario = path.join(import.meta.dirname, "desktop-ipc-acceptance-renderer.js");
  if ((await lstat(scenario)).size > 256 * 1024)
    throw new Error("Acceptance scenario exceeds bound");
  const scenarioHash = await fileHash(scenario);
  const run = await createAcceptanceRun(options);
  const destination = path.join(options.receiptDir, run.runId);
  try {
    await mkdir(options.receiptDir, { recursive: true });
    await mkdir(destination);
    // The fixture only bursts for this case, and only after the renderer holds consumption:
    // the provider cannot start before the scenario's own prompt, so ordering is causal.
    if (options.caseId === "backpressure")
      await writeFile(path.join(run.root, "reports/fixture-continue"), "release", { flag: "wx" });
    await copyFile(scenario, path.join(run.root, "scenario.js"));
    if ((await fileHash(path.join(run.root, "scenario.js"))) !== scenarioHash)
      throw new Error("Scenario changed during copy");
    await writeFile(
      path.join(destination, "run.json"),
      JSON.stringify(
        {
          schemaVersion: 1,
          runId: run.runId,
          caseId: options.caseId,
          transport: options.transport,
          root: run.root,
          scenarioSha256: scenarioHash,
          executableSha256: await fileHash(options.executable),
          driverSha256: await fileHash(path.resolve(import.meta.filename)),
          fixtureSha256: await fileHash(path.join(run.root, "workspace/exec")),
        },
        null,
        2,
      ),
      { flag: "wx" },
    );
    const cleanup = await launchOwned(options.executable, run, destination);
    await writeFile(path.join(destination, "cleanup.json"), JSON.stringify(cleanup), {
      flag: "wx",
    });
    const resultPath = path.join(run.root, "reports/result.json");
    const validated = await readLaunchResult(resultPath, {
      runId: run.runId,
      caseId: options.caseId,
      role: "main",
      transport: options.transport,
      root: run.root,
      launcherPid: cleanup.parentPid,
      childPid: cleanup.childPid,
    });
    await writeFile(path.join(destination, "result.json"), JSON.stringify(validated, null, 2), {
      flag: "wx",
    });
    // Native receipt and launcher exit are both verified; only the temp-root unlink is best-effort.
    const released = await releaseOwnedRoot(run.ownerRoot);
    if (!released.removed) {
      await writeFile(
        path.join(destination, "retained-root.json"),
        JSON.stringify(
          { schemaVersion: 1, runId: run.runId, root: run.root, reason: String(released.error) },
          null,
          2,
        ),
        { flag: "wx" },
      );
    }
    return validated;
  } catch (error) {
    throw new Error(`${error.message}; retain ${run.root}`, { cause: error });
  }
}

// Private acceptance tooling. Importing this module never starts a process.
export function acceptanceEnvironment(root, parent = process.env) {
  if (!path.isAbsolute(root)) throw new Error("Acceptance root must be absolute");
  const home = path.join(root, "home");
  const temporary = path.join(root, "tmp");
  const environment = {};
  for (const key of ["SystemRoot", "WINDIR", "COMSPEC", "PATHEXT"]) {
    const entry = Object.entries(parent).find(([name]) => name.toUpperCase() === key.toUpperCase());
    if (entry) environment[key] = entry[1];
  }
  return {
    ...environment,
    KRONOS_HOME: home,
    PASEO_HOME: home,
    CODEX_HOME: path.join(home, ".codex"),
    HOME: home,
    USERPROFILE: home,
    APPDATA: path.join(home, "AppData", "Roaming"),
    LOCALAPPDATA: path.join(home, "AppData", "Local"),
    PATH: path.join(root, "bin"),
    TEMP: temporary,
    TMP: temporary,
    TMPDIR: temporary,
  };
}

if (isMainModule(import.meta.url)) {
  try {
    const [
      mode,
      executable,
      captureReceiptPath,
      nativeRoot,
      assetsRoot,
      receiptPath,
      ...nativeFiles
    ] = process.argv.slice(2);
    if (mode !== "--write-build-receipt" || !receiptPath) {
      throw new Error(
        "Usage: --write-build-receipt <exe> <capture.json> <native-root> <assets-root> <receipt.json> [native-file...]",
      );
    }
    const inputs = await captureBuildInputs({ nativeRoot, assetsRoot, nativeFiles });
    await writeBuildReceiptFromCapture({ executable, captureReceiptPath, receiptPath, inputs });
    await validateBuildReceipt(executable, receiptPath);
    process.stdout.write(`PASS build receipt: ${receiptPath}\\n`);
  } catch (error) {
    console.error(error);
    process.exitCode = 1;
  }
}
