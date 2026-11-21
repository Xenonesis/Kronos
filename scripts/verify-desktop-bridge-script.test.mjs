import assert from "node:assert/strict";
import childProcess from "node:child_process";
import { createRequire } from "node:module";
import { link, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { seedControlledCodexFixture } from "./desktop-ipc-acceptance-fixture.mjs";
import {
  acceptanceEnvironment,
  captureBuildInputs,
  createAcceptanceRun,
  parseAcceptanceArguments,
  requireShortAcceptanceBuildParent,
  runDesktopIpcAcceptance,
  validateBuildReceipt,
  validateObservation,
  writeBuildReceipt,
  writeBuildReceiptFromCapture,
} from "./desktop-ipc-acceptance.mjs";
import { join } from "node:path";
import test from "node:test";

import vm from "node:vm";
const repoRoot = join(import.meta.dirname, "..");

async function saveBuildCaptureFixture(parent, capture) {
  const root = join(parent, "builds", `kronos-ipc-acceptance-${capture.runId}`);
  const reports = join(root, "reports");
  await mkdir(reports, { recursive: true });
  await writeFile(
    join(root, "owner.json"),
    JSON.stringify({ schemaVersion: 1, runId: capture.runId }),
  );
  await writeFile(
    join(root, "run.json"),
    JSON.stringify({
      schemaVersion: 1,
      runId: capture.runId,
      transport: "ipc",
      caseId: "preflight",
    }),
  );
  const path = join(reports, "build-capture.json");
  const bytes = JSON.stringify(capture);
  await writeFile(path, bytes);
  return { path, bytes };
}

test("private acceptance build parent is short and repo-local", () => {
  assert.equal(
    requireShortAcceptanceBuildParent(join(repoRoot, ".superpowers", "builds")),
    join(repoRoot, ".superpowers", "builds"),
  );
  for (const parent of [
    join(repoRoot, "builds"),
    join(repoRoot, "unowned", "builds"),
    join(repoRoot, ".superpowers", "builds", "nested"),
  ])
    assert.throws(() => requireShortAcceptanceBuildParent(parent), /short/);
});

test("controlled Codex fixture seeds a real executable and preserves smoke protocol", async () => {
  const home = await mkdtemp(join(tmpdir(), "kronos-fixture-contract-"));
  const environmentHash = () =>
    createHash("sha256").update(JSON.stringify(process.env)).digest("hex");
  const environment = environmentHash();
  try {
    await seedControlledCodexFixture(home);
    const executable = join(home, process.platform === "win32" ? "codex.exe" : "codex");
    assert.deepEqual(await readFile(executable), await readFile(process.execPath));
    const script = await readFile(join(home, "exec"), "utf8");
    assert.match(script, /Resumed exact smoke session/);
    assert.match(script, /Partial output before cancellation/);
    assert.match(script, /smoke-session/);
    assert.equal(environmentHash(), environment);
  } finally {
    await rm(home, { recursive: true, force: true });
  }
});

test("acceptance child environment owns every home and never inherits provider secrets", () => {
  const parent = {
    SystemRoot: "C:\\Windows",
    PATH: "unowned-provider-bin",
    OPENAI_API_KEY: "test-only-never-forward",
    NODE_OPTIONS: "--require unowned.js",
    WEBVIEW2_USER_DATA_FOLDER: "unowned-profile",
    CODEX_HOME: "unowned-codex",
    KRONOS_DAEMON_ENGINE: "node",
  };
  const before = { ...parent };
  const root = join(tmpdir(), "kronos-owned-environment");
  const environment = acceptanceEnvironment(root, parent);
  assert.equal(environment.KRONOS_HOME, join(root, "home"));
  assert.equal(environment.PASEO_HOME, join(root, "home"));
  assert.equal(environment.CODEX_HOME, join(root, "home", ".codex"));
  assert.equal(environment.PATH, join(root, "bin"));
  assert.equal(environment.TEMP, join(root, "tmp"));
  assert.equal(environment.TMP, join(root, "tmp"));
  assert.equal(environment.HOME, join(root, "home"));
  assert.equal(environment.USERPROFILE, join(root, "home"));
  assert.equal(environment.SystemRoot, parent.SystemRoot);
  for (const key of [
    "OPENAI_API_KEY",
    "NODE_OPTIONS",
    "WEBVIEW2_USER_DATA_FOLDER",
    "KRONOS_DAEMON_ENGINE",
  ]) {
    assert.equal(environment[key], undefined);
  }
  assert.deepEqual(parent, before);
  assert.throws(() => acceptanceEnvironment("relative-root", parent), /absolute/);
});

test("acceptance receipt rejects stale binary, stale sources, missing assets and wrong feature before launch", async () => {
  const root = await mkdtemp(join(repoRoot, ".superpowers", "kronos-receipt-contract-"));
  await mkdir(join(repoRoot, ".superpowers", "builds"), { recursive: true });
  const digest = (data) => createHash("sha256").update(data).digest("hex");
  try {
    const executable = join(root, "private.exe");
    const nativeRoot = join(root, "native");
    const assetsRoot = join(root, "assets");
    await mkdir(nativeRoot);
    await mkdir(assetsRoot);
    const source = join(nativeRoot, "main.rs");
    const asset = join(assetsRoot, "index.html");
    const receiptPath = join(root, "build.json");
    await writeFile(executable, "never executed fixture");
    await writeFile(source, "native source fixture");
    await writeFile(asset, "exported asset fixture");
    const capture = {
      schemaVersion: 1,
      runId: "0123456789abcdef0123456789abcdef",
      parentPid: 11,
      childPid: 12,
      executable: process.execPath,
      argv: [
        join(root, "tauri.js"),
        "build",
        "--no-bundle",
        "--features",
        "ipc-acceptance",
        "--ci",
      ],
      cwd: root,
      exitCode: 0,
      timedOut: false,
      leaderReaped: true,
      treeEmpty: true,
      jobAssignedBeforeResume: true,
    };
    const captureFile = await saveBuildCaptureFixture(join(repoRoot, ".superpowers"), capture);
    const capturePath = captureFile.path;
    const captureBytes = captureFile.bytes;
    const receipt = {
      schemaVersion: 1,
      nativeRoot,
      assetsRoot,
      executable: { path: executable, sha256: digest("never executed fixture") },
      nativeSources: [{ path: source, sha256: digest("native source fixture") }],
      assets: [{ path: asset, sha256: digest("exported asset fixture") }],
      build: {
        argv: [process.execPath, ...capture.argv],
        exitCode: 0,
        timedOut: false,
        reaped: true,
        jobAssignedBeforeResume: true,
        treeEmpty: true,
        captureReceipt: { path: capturePath, sha256: digest(captureBytes) },
      },
    };
    const save = () => writeFile(receiptPath, JSON.stringify(receipt));
    await save();
    await validateBuildReceipt(executable, receiptPath);
    await writeFile(executable, "stale executable");
    await assert.rejects(validateBuildReceipt(executable, receiptPath), /hash/);
    await writeFile(executable, "never executed fixture");
    receipt.build.argv = ["node", "tauri.js", "build", "--no-bundle", "--ci"];
    await save();
    await assert.rejects(validateBuildReceipt(executable, receiptPath), /feature/);
    receipt.build.argv = [process.execPath, ...capture.argv];
    await save();
    await writeFile(source, "changed native source");
    await assert.rejects(validateBuildReceipt(executable, receiptPath), /hash/);
    await writeFile(source, "native source fixture");
    await writeFile(join(nativeRoot, "added.rs"), "new compiled input");
    await assert.rejects(validateBuildReceipt(executable, receiptPath), /manifest/);
    await rm(join(nativeRoot, "added.rs"));
    receipt.assets = [];
    await save();
    await assert.rejects(validateBuildReceipt(executable, receiptPath), /manifest/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("build receipt writer binds pre-build inputs and refuses unsuccessful or changed builds", async () => {
  const root = await mkdtemp(join(repoRoot, ".superpowers", "kronos-build-writer-"));
  await mkdir(join(repoRoot, ".superpowers", "builds"), { recursive: true });
  try {
    const nativeRoot = join(root, "native");
    const assetsRoot = join(root, "assets");
    await mkdir(nativeRoot);
    await mkdir(assetsRoot);
    await writeFile(join(nativeRoot, "main.rs"), "controlled native source");
    await writeFile(join(assetsRoot, "index.html"), "controlled exported asset");
    const executable = join(root, "private.exe");
    await writeFile(executable, "fake binary never launched");
    const cargoConfig = join(root, "cargo-config.toml");
    await writeFile(cargoConfig, "[build]\njobs = 2\n");
    const inputs = await captureBuildInputs({ nativeRoot, assetsRoot, nativeFiles: [cargoConfig] });
    assert.ok(inputs.nativeSources.some((entry) => entry.path === cargoConfig));
    const capture = {
      schemaVersion: 1,
      runId: "fedcba9876543210fedcba9876543210",
      parentPid: 21,
      childPid: 22,
      executable: process.execPath,
      argv: [
        join(root, "tauri.js"),
        "build",
        "--no-bundle",
        "--features",
        "ipc-acceptance",
        "--ci",
      ],
      cwd: root,
      exitCode: 0,
      timedOut: false,
      leaderReaped: true,
      treeEmpty: true,
      jobAssignedBeforeResume: true,
    };
    const captureFile = await saveBuildCaptureFixture(join(repoRoot, ".superpowers"), capture);
    const capturePath = captureFile.path;
    const build = {
      argv: [process.execPath, ...capture.argv],
      exitCode: 0,
      timedOut: false,
      reaped: true,
      jobAssignedBeforeResume: false,
      treeEmpty: true,
    };
    const receiptPath = join(root, "build.json");
    await assert.rejects(
      writeBuildReceipt({ executable, receiptPath, inputs, build }),
      /Job ownership/,
    );
    build.jobAssignedBeforeResume = true;
    await assert.rejects(
      writeBuildReceipt({ executable, receiptPath, inputs, build }),
      /native Job capture receipt/,
    );
    build.captureReceiptPath = capturePath;
    const written = await writeBuildReceiptFromCapture({
      executable,
      receiptPath,
      inputs,
      captureReceiptPath: capturePath,
    });
    assert.equal(written.build.jobAssignedBeforeResume, true);
    assert.ok((await readdir(root)).includes("build.json"), "writer did not save receipt");
    await validateBuildReceipt(executable, receiptPath);
    await assert.rejects(writeBuildReceipt({ executable, receiptPath, inputs, build }), /EEXIST/);
    const rejectedPath = join(root, "rejected.json");
    await assert.rejects(
      writeBuildReceipt({
        executable,
        receiptPath: rejectedPath,
        inputs,
        build: { ...build, reaped: false },
      }),
      /reaped/,
    );
    await writeFile(join(nativeRoot, "main.rs"), "source changed during build");
    await assert.rejects(
      writeBuildReceipt({ executable, receiptPath: rejectedPath, inputs, build }),
      /changed/,
    );
    assert.ok(!(await readdir(root)).includes("rejected.json"));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("build receipt CLI creates exactly one validated receipt", async () => {
  const root = await mkdtemp(join(repoRoot, ".superpowers", "kronos-build-cli-"));
  await mkdir(join(repoRoot, ".superpowers", "builds"), { recursive: true });
  try {
    const nativeRoot = join(root, "native");
    const assetsRoot = join(root, "assets");
    await mkdir(nativeRoot);
    await mkdir(assetsRoot);
    await writeFile(join(nativeRoot, "main.rs"), "controlled native source");
    await writeFile(join(assetsRoot, "index.html"), "controlled exported asset");
    const executable = join(root, "private.exe");
    const receiptPath = join(root, "build.json");
    await writeFile(executable, "fixture executable");
    const capture = {
      schemaVersion: 1,
      runId: "a1b2c3d4e5f67890123456789abcdef0",
      parentPid: 31,
      childPid: 32,
      executable: process.execPath,
      argv: [
        join(root, "tauri.js"),
        "build",
        "--no-bundle",
        "--features",
        "ipc-acceptance",
        "--ci",
      ],
      cwd: root,
      exitCode: 0,
      timedOut: false,
      leaderReaped: true,
      treeEmpty: true,
      jobAssignedBeforeResume: true,
    };
    const { path: capturePath } = await saveBuildCaptureFixture(
      join(repoRoot, ".superpowers"),
      capture,
    );
    const result = childProcess.spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/desktop-ipc-acceptance.mjs"),
        "--write-build-receipt",
        executable,
        capturePath,
        nativeRoot,
        assetsRoot,
        receiptPath,
      ],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    assert.equal((result.stdout.match(/PASS build receipt/g) ?? []).length, 1);
    await validateBuildReceipt(executable, receiptPath);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("acceptance imports and invalid launch options never spawn a process", async (context) => {
  let starts = 0;
  context.mock.method(childProcess, "spawn", () => {
    starts++;
    throw new Error("Forbidden spawn");
  });
  await import(`./desktop-ipc-acceptance.mjs?import-check=${Date.now()}`);
  for (const argv of [[], ["--gui-acceptance"], ["--exe", "a", "--unknown", "b"]]) {
    assert.throws(() => parseAcceptanceArguments(argv), /acceptance|Usage|Unknown/);
  }
  await assert.rejects(
    runDesktopIpcAcceptance({ caseId: "unknown", transport: "ipc" }),
    /case|options/,
  );
  assert.equal(starts, 0);
});

test("owned acceptance resource factory creates strict descriptor, homes and fixtures without spawning", async (context) => {
  context.mock.method(childProcess, "spawn", () => assert.fail("Resource creation must not spawn"));
  const run = await createAcceptanceRun({ caseId: "preflight", transport: "ipc" });
  assert.ok(run?.root, "owned resource factory missing");
  try {
    const descriptor = JSON.parse(await readFile(join(run.root, "run.json"), "utf8"));
    assert.deepEqual(Object.keys(descriptor).sort(), [
      "caseId",
      "runId",
      "schemaVersion",
      "transport",
    ]);
    assert.equal(descriptor.runId, run.runId);
    assert.equal(descriptor.schemaVersion, 1);
    assert.equal(run.root, join(run.ownerRoot, `kronos-ipc-acceptance-${run.runId}`));
    assert.equal(run.environment.KRONOS_HOME, join(run.root, "home"));
    for (const directory of [
      "bin",
      "logs",
      "tmp",
      "workspace",
      "reports",
      "profile/main",
      "profile/secondary",
      "profile/guest",
    ]) {
      await readdir(join(run.root, directory));
    }
    assert.deepEqual(
      await readFile(join(run.root, "bin", process.platform === "win32" ? "node.exe" : "node")),
      await readFile(process.execPath),
    );
    const script = await readFile(join(run.root, "workspace", "exec"), "utf8");
    assert.match(script, /fixture-continue/);
    assert.match(script, /provider-events/);
  } finally {
    await rm(run.ownerRoot, { recursive: true, force: true });
  }
});

test("acceptance launcher rejects wrong-feature receipt before spawning or creating run state", async (context) => {
  context.mock.method(childProcess, "spawn", () => assert.fail("Receipt must reject before spawn"));
  const root = await mkdtemp(join(tmpdir(), "kronos-driver-reject-"));
  try {
    const buildReceipt = join(root, "wrong.json");
    await writeFile(
      buildReceipt,
      JSON.stringify({
        schemaVersion: 1,
        build: {
          exitCode: 0,
          timedOut: false,
          reaped: true,
          jobAssignedBeforeResume: true,
          treeEmpty: true,
          argv: ["build", "--no-bundle", "--ci"],
        },
      }),
    );
    await assert.rejects(
      runDesktopIpcAcceptance({
        executable: join(root, "unverified.exe"),
        buildReceipt,
        caseId: "preflight",
        transport: "ipc",
        receiptDir: root,
      }),
      /feature/,
    );
    assert.deepEqual(await readdir(root), ["wrong.json"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("acceptance fixture VM characterization holds output until owned file gate and records no prompt", async (context) => {
  const run = await createAcceptanceRun({ caseId: "preflight", transport: "ipc" });
  try {
    let poll;
    let detached = false;
    context.mock.method(childProcess, "spawn", () => ({
      pid: 7788,
      unref() {
        detached = true;
      },
    }));
    const output = [];
    vm.runInNewContext(await readFile(join(run.root, "workspace/exec"), "utf8"), {
      require: createRequire(import.meta.url),
      process: {
        env: { KRONOS_IPC_ACCEPTANCE_DESCRIPTOR: run.descriptor },
        argv: [process.execPath, "exec", "controlled private prompt"],
        pid: 7787,
        execPath: process.execPath,
        cwd: () => join(run.root, "workspace"),
        stdout: { write: (chunk) => output.push(JSON.parse(chunk)) },
      },
      setTimeout: () => 1,
      clearTimeout() {},
      setInterval: (callback) => {
        poll = callback;
        return 2;
      },
      clearInterval() {},
    });
    assert.equal(output.length, 2);
    poll();
    assert.equal(output.length, 2, "completion escaped unopened gate");
    await writeFile(join(run.root, "reports/fixture-continue"), "release");
    poll();
    assert.equal(output.at(-1).item.text, "Controlled fixture completed");
    assert.equal(detached, true);
    const recorded = await readFile(join(run.root, "reports/provider-events.jsonl"), "utf8");
    assert.ok(!recorded.includes("controlled private prompt"));
    assert.match(recorded, /argvSha256/);
    assert.match(recorded, /descendantPid/);
  } finally {
    await rm(run.ownerRoot, { recursive: true, force: true });
  }
});

test("observer validation rejects native send-only, missing ACK/DOM and mismatched identities", () => {
  const expected = { runId: "run", caseId: "native-smoke", role: "main" };
  const native = {
    ...expected,
    incarnation: 1,
    engineEpoch: 1,
    sessionId: "s",
    generation: "g",
    lastSentSequence: 3,
    lastAckSequence: 3,
    sessionQueuePeakFrames: 2,
    sessionQueuePeakDecodedBytes: 20,
    channelPeakEvents: 2,
    channelPeakSerializedBytes: 200,
    closeCode: null,
    providerPids: [],
    providerStarts: 0,
  };
  const renderer = {
    ...expected,
    incarnation: 1,
    sessionId: "s",
    generation: "g",
    lastReceivedSequence: 3,
    visibleState: "sidebar-visible",
    outcome: "pass",
    serverInfoReceived: true,
    requestCompleted: true,
  };
  validateObservation(native, renderer, expected);
  assert.throws(() => validateObservation(native, null, expected), /renderer|delivery/);
  assert.throws(
    () => validateObservation({ ...native, lastAckSequence: 0 }, renderer, expected),
    /ACK/,
  );
  for (const visibleState of ["", "booting", true, { sidebar: true }]) {
    assert.throws(
      () => validateObservation(native, { ...renderer, visibleState }, expected),
      /visible|DOM/,
    );
  }
  for (const field of ["sessionId", "generation", "role", "incarnation", "runId", "caseId"]) {
    assert.throws(
      () => validateObservation(native, { ...renderer, [field]: "other" }, expected),
      /identity|mismatch/,
    );
  }
  assert.throws(
    () => validateObservation(native, { ...renderer, serverInfoReceived: false }, expected),
    /server_info/,
  );
  assert.throws(
    () => validateObservation({ ...native, channelPeakEvents: undefined }, renderer, expected),
    /measurement/,
  );
});

async function acceptanceObserverVm(environment = {}) {
  const callbacks = new Map();
  const internals = {};
  Object.defineProperties(internals, {
    callbacks: { value: callbacks },
    runCallback: {
      value: (id, data) => {
        const callback = callbacks.get(id);
        if (callback) callback(data);
      },
    },
    invoke: {
      value:
        environment.invoke ??
        (() => assert.fail("Observer must not issue synthetic business requests")),
    },
  });
  const window = { __TAURI_INTERNALS__: internals, ...environment.window };
  vm.runInNewContext(
    await readFile(join(import.meta.dirname, "desktop-ipc-acceptance-renderer.js"), "utf8"),
    { window, Map, Set, WeakSet },
  );
  return { window, callbacks, internals };
}

test("a failed scenario surfaces its own reason instead of a generic state error", async () => {
  const { validateLaunchResult } = await import("./desktop-ipc-acceptance.mjs");
  const expected = {
    runId: "0123456789abcdef0123456789abcdef",
    caseId: "backpressure",
    role: "main",
    transport: "ipc",
  };
  const result = {
    schemaVersion: 1,
    runId: expected.runId,
    caseId: expected.caseId,
    native: { runId: expected.runId, caseId: expected.caseId, role: "main", incarnation: 1 },
    renderer: {
      runId: expected.runId,
      caseId: expected.caseId,
      role: "main",
      incarnation: 1,
      outcome: "fail",
      reason: "No run-owned workspace is visible to the frontend",
    },
  };
  assert.throws(
    () => validateLaunchResult(result, expected),
    /No run-owned workspace is visible to the frontend/,
  );
});

test("backpressure passes only on real overflow, resync and separate close evidence", async () => {
  const { validateLaunchResult } = await import("./desktop-ipc-acceptance.mjs");
  const expected = {
    runId: "0123456789abcdef0123456789abcdef",
    caseId: "backpressure",
    role: "main",
    transport: "ipc",
  };
  const closed = (code, extra = {}) => ({
    sessionId: `closed-${code}`,
    generation: "one",
    closeCode: code,
    opened: true,
    sessionQueuePeakFrames: 2,
    sessionQueuePeakDecodedBytes: 4096,
    channelPeakEvents: 64,
    channelPeakSerializedBytes: 262144,
    ...extra,
  });
  const base = {
    schemaVersion: 1,
    runId: expected.runId,
    caseId: expected.caseId,
    native: {
      runId: expected.runId,
      caseId: expected.caseId,
      role: "main",
      incarnation: 1,
      sessionId: "current",
      generation: "four",
      engineEpoch: 1,
      opened: true,
      closeCode: null,
      lastSentSequence: 9,
      lastAckSequence: 9,
      sessionQueuePeakFrames: 2,
      sessionQueuePeakDecodedBytes: 4096,
      channelPeakEvents: 3,
      channelPeakSerializedBytes: 2048,
      providerPids: [4242],
      providerStarts: 1,
      closedSessions: [closed(1013), closed(1009), closed(1011), closed(1000)],
    },
    renderer: {
      runId: expected.runId,
      caseId: expected.caseId,
      role: "main",
      incarnation: 1,
      sessionId: "current",
      generation: "four",
      outcome: "pass",
      visibleState: "burst-drained",
      lastReceivedSequence: 9,
      serverInfoReceived: true,
      requestCompleted: true,
      burstDelivered: true,
      resyncObserved: true,
    },
  };
  const clone = (mutate) => {
    const copy = structuredClone(base);
    mutate(copy);
    return copy;
  };
  assert.equal(validateLaunchResult(base, expected).outcome, "pass");
  assert.throws(
    () =>
      validateLaunchResult(
        clone((c) => (c.native.providerStarts = 2)),
        expected,
      ),
    /provider/i,
  );
  assert.throws(
    () =>
      validateLaunchResult(
        clone((c) => (c.native.closedSessions = [closed(1013), closed(1009), closed(1011)])),
        expected,
      ),
    /1000/,
  );
  assert.throws(
    () =>
      validateLaunchResult(
        clone((c) => (c.native.closedSessions[0].channelPeakEvents = 8)),
        expected,
      ),
    /high-water/i,
  );
  assert.throws(
    () =>
      validateLaunchResult(
        clone((c) => (c.renderer.resyncObserved = false)),
        expected,
      ),
    /resync/i,
  );
  assert.throws(
    () =>
      validateLaunchResult(
        clone((c) => (c.renderer.burstDelivered = false)),
        expected,
      ),
    /burst/i,
  );
});

test("a blocked acceptance run exits non-zero so it cannot read as success", async () => {
  const { acceptanceExitCode } = await import("./desktop-ipc-acceptance.mjs");
  assert.equal(acceptanceExitCode({ outcome: "pass" }), 0);
  assert.equal(acceptanceExitCode({ outcome: "blocked" }), 3);
  assert.throws(() => acceptanceExitCode({ outcome: "unknown" }), /outcome/i);
});

test("blocked acceptance cases are recorded as blocked, never as pass", async () => {
  const { validateLaunchResult } = await import("./desktop-ipc-acceptance.mjs");
  const expected = { runId: "0123456789abcdef0123456789abcdef", caseId: "approval", role: "main" };
  const base = {
    schemaVersion: 1,
    runId: expected.runId,
    caseId: expected.caseId,
    native: {
      runId: expected.runId,
      caseId: expected.caseId,
      role: "main",
      incarnation: 1,
      sessionId: "local-session-blocked",
      generation: "generation-blocked",
    },
    renderer: {
      runId: expected.runId,
      caseId: expected.caseId,
      role: "main",
      incarnation: 1,
      sessionId: "local-session-blocked",
      generation: "generation-blocked",
      outcome: "blocked",
      reason: "native-unsupported: pendingPermissions is always empty",
    },
  };
  const blocked = validateLaunchResult(base, expected);
  assert.equal(blocked.outcome, "blocked");
  assert.equal(blocked.renderer.reason, base.renderer.reason);
  assert.throws(
    () => validateLaunchResult({ ...base, renderer: { ...base.renderer, reason: "" } }, expected),
    /reason/i,
    "a blocked case must state why",
  );
  assert.throws(
    () =>
      validateLaunchResult(
        {
          ...base,
          caseId: "native-smoke",
          native: { ...base.native, caseId: "native-smoke" },
          renderer: { ...base.renderer, caseId: "native-smoke" },
        },
        { ...expected, caseId: "native-smoke" },
      ),
    "an implemented case cannot be downgraded to blocked",
  );
  assert.throws(
    () => validateLaunchResult({ ...base, runId: "ffffffffffffffffffffffffffffffff" }, expected),
    /identity mismatch/i,
  );
});

test("owned root release retries a transient lock and retains the root when it stays locked", async () => {
  const { releaseOwnedRoot } = await import("./desktop-ipc-acceptance.mjs");
  let attempts = 0;
  const busy = () => {
    attempts++;
    return Promise.reject(Object.assign(new Error("locked"), { code: "EBUSY" }));
  };
  const flaky = await releaseOwnedRoot("C:/owned/root", {
    remove: () => (attempts < 3 ? busy() : Promise.resolve()),
    delay: () => Promise.resolve(),
  });
  assert.equal(flaky.removed, true, "a transient lock was not retried");
  assert.equal(attempts, 3);
  attempts = 0;
  const stuck = await releaseOwnedRoot("C:/owned/root", {
    remove: busy,
    delay: () => Promise.resolve(),
  });
  assert.equal(stuck.removed, false, "an unresolved lock must retain the owned root");
  assert.equal(stuck.attempts, 5, "cleanup retry must stay bounded");
  assert.equal(stuck.error?.code, "EBUSY");
});

test("renderer observer VM characterization delegates real registered callbacks without replacing immutable Tauri methods", async () => {
  const { window, callbacks, internals } = await acceptanceObserverVm();
  const marker = Promise.resolve("original callback result");
  let called = 0;
  callbacks.set(7, () => {
    called++;
    return marker;
  });
  const message = {
    sessionId: "session",
    generation: "generation",
    sequence: 2,
    kind: "message",
    text: JSON.stringify({ type: "session", message: { payload: { status: "server_info" } } }),
  };
  assert.equal(callbacks.get(7)({ index: 0, message }), marker);
  assert.equal(called, 1);
  const observed = window.__KRONOS_IPC_ACCEPTANCE_METADATA__();
  assert.equal(observed.length, 1, "renderer callback was not observed");
  assert.equal(observed[0].lastReceivedSequence, 2);
  assert.equal(observed[0].serverInfoReceived, true);
  assert.equal(Object.getOwnPropertyDescriptor(internals, "runCallback").writable, false);
  assert.equal(Object.getOwnPropertyDescriptor(internals, "invoke").writable, false);
});

test("renderer ACK-gate VM characterization holds one callback and releases it without fabricating delivery", async () => {
  const { window, callbacks } = await acceptanceObserverVm();
  let called = 0;
  callbacks.set(8, () => {
    called++;
  });
  window.__KRONOS_IPC_ACCEPTANCE_ACK_GATE__(true);
  try {
    callbacks.get(8)({
      index: 0,
      message: { sessionId: "s", generation: "g", sequence: 2, kind: "message", text: "{}" },
    });
    assert.equal(called, 0, "held callback executed before release");
    assert.equal(window.__KRONOS_IPC_ACCEPTANCE_METADATA__().length, 0);
  } finally {
    window.__KRONOS_IPC_ACCEPTANCE_ACK_GATE__(false);
  }
  assert.equal(called, 1);
  assert.equal(window.__KRONOS_IPC_ACCEPTANCE_METADATA__()[0].lastReceivedSequence, 2);
});

test("renderer fixed scenario entry dispatches closed commands and releases held callbacks on failure", async () => {
  const { window, callbacks } = await acceptanceObserverVm();
  const entry = window.__KRONOS_IPC_ACCEPTANCE_START__;
  assert.equal(typeof entry, "function", "fixed scenario entry missing");
  const deliveries = [];
  callbacks.set(9, (data) => {
    deliveries.push(data.index);
  });
  const message = (sequence) => ({
    index: sequence - 1,
    message: { sessionId: "s", generation: "g", sequence, kind: "message", text: "{}" },
  });

  const started = await entry({ type: "ack-gate", enabled: true });
  assert.equal(started.applied, true);
  callbacks.get(9)(message(1));
  assert.deepEqual(deliveries, [], "held callback executed before release");

  // A rejected scenario command must release the held callback, not strand it.
  await assert.rejects(entry({ type: "ack-gate", enabled: "yes" }), /boolean/);
  assert.deepEqual(deliveries, [0], "failed scenario command left a callback held");

  const released = await entry({ type: "release" });
  assert.equal(released.released, true);
  callbacks.get(9)(message(2));
  assert.deepEqual(deliveries, [0, 1], "release did not clear the gate");

  const metadata = await entry({ type: "metadata" });
  assert.equal(metadata.length, 1);
  assert.equal(metadata[0].lastReceivedSequence, 2);

  for (const command of [
    null,
    [],
    { type: "execute" },
    { type: "ack-gate" },
    { type: "ack-gate", enabled: true, extra: 1 },
    { type: "metadata", extra: 1 },
  ]) {
    await assert.rejects(entry(command), /scenario command|boolean/i);
  }
});

test("renderer bootstrap VM characterization freezes native identity and rejects replacement or malformed handoffs", async () => {
  const { window } = await acceptanceObserverVm();
  const entry = window.__KRONOS_IPC_ACCEPTANCE_START__;
  const command = {
    type: "bootstrap",
    runId: "0123456789abcdef0123456789abcdef",
    caseId: "native-smoke",
    role: "main",
    surfaceGeneration: 1,
  };
  await assert.doesNotReject(() => entry(command), "Native bootstrap handoff was rejected");
  for (const invalid of [
    { ...command, runId: "other" },
    { ...command, role: "guest" },
    { ...command, surfaceGeneration: 0 },
    { ...command, surfaceGeneration: 1.5 },
    { ...command, surfaceGeneration: Number.MAX_SAFE_INTEGER + 1 },
    { ...command, extra: true },
    { ...command, caseId: "execute" },
  ])
    await assert.rejects(entry(invalid), /bootstrap|handoff/i);
  const result = await entry(command);
  assert.equal(result.bootstrapped, true);
  assert.equal(result.surfaceGeneration, 1);
  result.surfaceGeneration = 99;
  assert.equal(
    (await entry(command)).surfaceGeneration,
    1,
    "Returned metadata mutated captured generation",
  );
  await assert.rejects(entry({ ...command, surfaceGeneration: 2 }), /replacement/i);
  await assert.rejects(entry({ ...command, role: "secondary" }), /replacement/i);
  assert.equal((await entry(command)).role, "main");
});

test("trust-reload initial-main VM records held open before creating secondary and waits for reload", async () => {
  const runId = "0123456789abcdef0123456789abcdef";
  const calls = [];
  const storage = new Map();
  const native = {
    runId,
    caseId: "trust-reload",
    role: "main",
    incarnation: 1,
    sessionId: "held-main-session",
    generation: "held-main-generation",
    engineEpoch: 7,
    lastSentSequence: 0,
    lastAckSequence: 0,
    opened: false,
    sessionQueuePeakFrames: 0,
    sessionQueuePeakDecodedBytes: 0,
    channelPeakEvents: 0,
    channelPeakSerializedBytes: 0,
    closeCode: null,
    providerPids: [],
    providerStarts: 0,
  };
  const { window } = await acceptanceObserverVm({
    window: {
      performance: { now: () => 0 },
      setTimeout: (callback) => setTimeout(callback, 0),
      clearTimeout,
      sessionStorage: {
        getItem: (key) => storage.get(key) ?? null,
        setItem: (key, value) => storage.set(key, value),
      },
    },
    invoke: async (command, args) => {
      const input = args.input;
      calls.push({ command, input });
      if (
        input.action.type === "observe" &&
        calls.filter((item) => item.input.action?.type === "observe").length === 1
      )
        return native;
      if (input.action.type === "create" && input.action.role === "guest")
        return { applied: true, guestUnregistered: true };
      if (input.action.type === "create" && input.action.role === "secondary")
        return { applied: true };
      if (input.action.type === "observe") throw new Error("Stale desktop surface");
      assert.fail(`Unexpected initial-main command: ${command}/${JSON.stringify(input.action)}`);
    },
  });
  const entry = window.__KRONOS_IPC_ACCEPTANCE_START__;
  await entry({
    type: "bootstrap",
    runId,
    caseId: "trust-reload",
    role: "main",
    surfaceGeneration: 1,
  });
  await assert.doesNotReject(entry({ type: "run" }), "Initial held-open scenario was rejected");
  assert.deepEqual(
    calls.map(({ input }) => input.action.type),
    ["observe", "create", "create", "observe"],
  );
  assert.equal(calls[1].input.action.role, "guest");
  assert.equal(calls[2].input.action.role, "secondary");
  assert.deepEqual(
    JSON.parse(storage.get(`__KRONOS_IPC_ACCEPTANCE_TRUST_RELOAD__:${runId}:main`)),
    {
      generation: 1,
      engineEpoch: 7,
      sessionId: "held-main-session",
      guestRejected: true,
    },
  );
  assert.ok(
    !calls.some(({ command }) => command === "ipc_acceptance_report"),
    "Held main session reported before reload",
  );
});

test("trust-reload secondary VM characterizes cross-owner rejection, delivery, report-before-reload ordering", async () => {
  const runId = "0123456789abcdef0123456789abcdef";
  const calls = [];
  let clock = 0;
  const native = {
    runId,
    caseId: "trust-reload",
    role: "secondary",
    incarnation: 2,
    sessionId: "secondary-session",
    generation: "secondary-generation",
    engineEpoch: 7,
    lastSentSequence: 2,
    lastAckSequence: 2,
    opened: true,
    sessionQueuePeakFrames: 1,
    sessionQueuePeakDecodedBytes: 10,
    channelPeakEvents: 2,
    channelPeakSerializedBytes: 100,
    closeCode: null,
    providerPids: [],
    providerStarts: 0,
  };
  const { window, callbacks } = await acceptanceObserverVm({
    window: {
      performance: { now: () => clock },
      setTimeout: (callback, delay) => {
        if (delay === 50) clock += 50;
        return setTimeout(callback, 0);
      },
      clearTimeout,
      document: { querySelector: () => ({ getClientRects: () => [{ width: 100, height: 100 }] }) },
      getComputedStyle: () => ({ visibility: "visible", display: "flex" }),
    },
    invoke: async (command, args) => {
      const input = args.input;
      calls.push({ command, input });
      if (
        command === "ipc_acceptance_control" &&
        input.action.type === "open-gate" &&
        input.surfaceGeneration === 1
      )
        throw new Error("Acceptance surface generation mismatch");
      if (command === "ipc_acceptance_control" && input.action.type === "open-gate")
        return { applied: true };
      if (command === "ipc_acceptance_control" && input.action.type === "observe") return native;
      if (command === "ipc_acceptance_report")
        return {
          schemaVersion: 1,
          runId,
          caseId: "trust-reload",
          native,
          renderer: {
            ...input.report,
            runId,
            caseId: "trust-reload",
            role: "secondary",
            incarnation: 2,
          },
        };
      if (command === "ipc_acceptance_control" && input.action.type === "reload")
        return { applied: true };
      assert.fail(`Unexpected trust-reload command: ${command}/${JSON.stringify(input.action)}`);
    },
  });
  callbacks.set(30, () => {});
  for (const [index, payload] of [
    { status: "server_info" },
    { requestId: "request-secondary", success: true },
  ].entries())
    callbacks.get(30)({
      index,
      message: {
        sessionId: native.sessionId,
        generation: native.generation,
        sequence: index + 1,
        kind: "message",
        text: JSON.stringify({ type: "session", message: { payload } }),
      },
    });
  const entry = window.__KRONOS_IPC_ACCEPTANCE_START__;
  await entry({
    type: "bootstrap",
    runId,
    caseId: "trust-reload",
    role: "secondary",
    surfaceGeneration: 2,
    primaryGeneration: 1,
  });
  await assert.doesNotReject(entry({ type: "run" }), "Trusted secondary scenario was rejected");
  const crossOwner = calls.findIndex(
    ({ input }) => input.action?.type === "open-gate" && input.surfaceGeneration === 1,
  );
  const release = calls.findIndex(
    ({ input }) =>
      input.action?.type === "open-gate" &&
      input.surfaceGeneration === 2 &&
      input.action.enabled === false,
  );
  const report = calls.findIndex(({ command }) => command === "ipc_acceptance_report");
  const reload = calls.findIndex(({ input }) => input.action?.type === "reload");
  assert.ok(crossOwner >= 0 && crossOwner < release);
  assert.ok(
    release < report && report < reload,
    "Secondary report must persist before reloading main",
  );
  assert.equal(calls[reload].input.action.role, "main");
});

test("trust-reload reloaded-main VM rejects prior generation and confirms same core before reporting", async () => {
  const runId = "0123456789abcdef0123456789abcdef";
  const calls = [];
  let clock = 0;
  const storage = new Map([
    [
      `__KRONOS_IPC_ACCEPTANCE_TRUST_RELOAD__:${runId}:main`,
      JSON.stringify({
        generation: 1,
        engineEpoch: 7,
        sessionId: "old-session",
        guestRejected: true,
      }),
    ],
  ]);
  const native = {
    runId,
    caseId: "trust-reload",
    role: "main",
    incarnation: 3,
    sessionId: "successor-session",
    generation: "successor-generation",
    engineEpoch: 7,
    lastSentSequence: 2,
    lastAckSequence: 2,
    opened: true,
    sessionQueuePeakFrames: 1,
    sessionQueuePeakDecodedBytes: 10,
    channelPeakEvents: 2,
    channelPeakSerializedBytes: 100,
    closeCode: null,
    providerPids: [],
    providerStarts: 0,
  };
  const { window, callbacks } = await acceptanceObserverVm({
    window: {
      performance: { now: () => clock },
      sessionStorage: {
        getItem: (key) => storage.get(key) ?? null,
        setItem: (key, value) => storage.set(key, value),
      },
      setTimeout: (callback, delay) => {
        if (delay === 50) clock += 50;
        return setTimeout(callback, 0);
      },
      clearTimeout,
      document: { querySelector: () => ({ getClientRects: () => [{ width: 100, height: 100 }] }) },
      getComputedStyle: () => ({ visibility: "visible", display: "flex" }),
    },
    invoke: async (command, args) => {
      const input = args.input;
      calls.push({ command, input });
      if (
        command === "ipc_acceptance_control" &&
        input.action.type === "open-gate" &&
        input.surfaceGeneration === 1
      )
        throw new Error("Acceptance surface generation mismatch");
      if (command === "ipc_acceptance_control" && input.action.type === "open-gate")
        return { applied: true };
      if (command === "ipc_acceptance_control" && input.action.type === "observe") return native;
      if (command === "ipc_acceptance_report")
        return {
          schemaVersion: 1,
          runId,
          caseId: "trust-reload",
          native,
          renderer: {
            ...input.report,
            runId,
            caseId: "trust-reload",
            role: "main",
            incarnation: 3,
          },
        };
      if (command === "ipc_acceptance_control" && input.action.type === "exit")
        return { exiting: true };
      assert.fail(`Unexpected trust-reload command: ${command}/${JSON.stringify(input.action)}`);
    },
  });
  callbacks.set(31, () => {});
  for (const [index, payload] of [
    { status: "server_info" },
    { requestId: "request-main", success: true },
  ].entries())
    callbacks.get(31)({
      index,
      message: {
        sessionId: native.sessionId,
        generation: native.generation,
        sequence: index + 1,
        kind: "message",
        text: JSON.stringify({ type: "session", message: { payload } }),
      },
    });
  const entry = window.__KRONOS_IPC_ACCEPTANCE_START__;
  await entry({
    type: "bootstrap",
    runId,
    caseId: "trust-reload",
    role: "main",
    surfaceGeneration: 3,
  });
  await assert.doesNotReject(entry({ type: "run" }), "Reloaded main scenario was rejected");
  const stale = calls.findIndex(
    ({ input }) => input.action?.type === "open-gate" && input.surfaceGeneration === 1,
  );
  const release = calls.findIndex(
    ({ input }) =>
      input.action?.type === "open-gate" &&
      input.surfaceGeneration === 3 &&
      input.action.enabled === false,
  );
  assert.ok(
    stale >= 0 && stale < release,
    "Stale generation was not rejected before successor gate release",
  );
  const report = calls.find(({ command }) => command === "ipc_acceptance_report");
  assert.equal(report.input.report.staleGenerationRejected, true);
  assert.equal(report.input.report.sameEngineEpoch, true);
  assert.equal(report.input.report.guestRejected, true);
  assert.equal(
    storage.get(`__KRONOS_IPC_ACCEPTANCE_TRUST_RELOAD__:${runId}:main`),
    JSON.stringify({
      generation: 3,
      engineEpoch: 7,
      sessionId: "successor-session",
      guestRejected: true,
    }),
  );
});

test("native-smoke scenario VM characterization waits for DOM, real callback metadata and matching native ACK before reporting", async () => {
  const calls = [];
  let clock = 0;
  let checks = 0;
  const runId = "0123456789abcdef0123456789abcdef";
  const native = {
    runId,
    caseId: "native-smoke",
    role: "main",
    incarnation: 1,
    sessionId: "s",
    generation: "g",
    engineEpoch: 1,
    lastSentSequence: 2,
    lastAckSequence: 1,
    sessionQueuePeakFrames: 1,
    sessionQueuePeakDecodedBytes: 10,
    channelPeakEvents: 2,
    channelPeakSerializedBytes: 100,
    closeCode: null,
    providerPids: [],
    providerStarts: 0,
  };
  const { window, callbacks } = await acceptanceObserverVm({
    window: {
      performance: { now: () => clock },
      setTimeout: (callback, delay) => {
        if (delay === 50) {
          clock += 50;
          return setTimeout(callback, 0);
        }
        return setTimeout(callback, delay);
      },
      clearTimeout,
      document: {
        querySelector: (selector) => {
          assert.equal(selector, '[data-testid="sidebar-project-workspace-list-scroll"]');
          return { getClientRects: () => (checks >= 2 ? [{ width: 100, height: 100 }] : []) };
        },
      },
      getComputedStyle: () => ({ visibility: "visible", display: "flex" }),
    },
    invoke: async (command, args) => {
      calls.push({ command, args });
      assert.equal(args.input.runId, runId);
      assert.equal(args.input.surfaceGeneration, 1);
      if (command === "ipc_acceptance_control" && args.input.action.type === "observe") {
        checks++;
        return { ...native, lastAckSequence: checks >= 2 ? 2 : 1 };
      }
      if (command === "ipc_acceptance_report")
        return {
          schemaVersion: 1,
          runId,
          caseId: "native-smoke",
          native: { ...native, lastAckSequence: 2 },
          renderer: {
            ...args.input.report,
            runId,
            caseId: "native-smoke",
            role: "main",
            incarnation: 1,
          },
        };
      assert.equal(command, "ipc_acceptance_control");
      assert.equal(args.input.action.type, "exit");
      return { exiting: true };
    },
  });
  callbacks.set(10, () => {});
  callbacks.get(10)({
    index: 0,
    message: {
      sessionId: "s",
      generation: "g",
      sequence: 1,
      kind: "message",
      text: JSON.stringify({ type: "session", message: { payload: { status: "server_info" } } }),
    },
  });
  callbacks.get(10)({
    index: 1,
    message: {
      sessionId: "s",
      generation: "g",
      sequence: 2,
      kind: "message",
      text: JSON.stringify({
        type: "session",
        message: { payload: { requestId: "request-1", success: true } },
      }),
    },
  });
  const entry = window.__KRONOS_IPC_ACCEPTANCE_START__;
  await entry({
    type: "bootstrap",
    runId,
    caseId: "native-smoke",
    role: "main",
    surfaceGeneration: 1,
  });
  await assert.doesNotReject(
    () => entry({ type: "run" }),
    "Fixed native-smoke scenario was rejected",
  );
  const reports = calls.filter((call) => call.command === "ipc_acceptance_report");
  assert.equal(reports.length, 1);
  assert.equal(reports[0].args.input.report.visibleState, "sidebar-visible");
  assert.equal(reports[0].args.input.report.lastReceivedSequence, 2);
  assert.equal(reports[0].args.input.report.serverInfoReceived, true);
  assert.equal(reports[0].args.input.report.requestCompleted, true);
  assert.equal(calls.at(-1).args.input.action.type, "exit");
  assert.ok(checks >= 2, "Scenario ignored missing native ACK / DOM layout");
  const count = calls.length;
  await entry({ type: "run" });
  assert.equal(calls.length, count, "Repeated page-finished dispatch ran scenario twice");
});

test("native-smoke scenario VM characterization cannot pass native-send-only and releases held callback at deadline", async () => {
  let clock = 0;
  let deliveries = 0;
  const calls = [];
  const outcomes = [];
  const { window, callbacks } = await acceptanceObserverVm({
    window: {
      performance: { now: () => clock },
      setTimeout: (callback, delay) => {
        if (delay === 50) {
          clock += 120000;
          return setTimeout(callback, 0);
        }
        return setTimeout(callback, delay);
      },
      clearTimeout,
      document: { querySelector: () => ({ getClientRects: () => [{ width: 100, height: 100 }] }) },
      getComputedStyle: () => ({ visibility: "visible", display: "flex" }),
    },
    invoke: async (command, args) => {
      calls.push(command);
      if (command === "ipc_acceptance_report") {
        outcomes.push(args.input.report.outcome);
        return { reported: true };
      }
      assert.equal(command, "ipc_acceptance_control");
      assert.ok(["observe", "exit"].includes(args.input.action.type));
      if (args.input.action.type === "exit") return { exited: true };
      return {
        runId: "0123456789abcdef0123456789abcdef",
        caseId: "native-smoke",
        role: "main",
        incarnation: 1,
        sessionId: "s",
        generation: "g",
        engineEpoch: 1,
        lastSentSequence: 2,
        lastAckSequence: 2,
        sessionQueuePeakFrames: 1,
        sessionQueuePeakDecodedBytes: 10,
        channelPeakEvents: 2,
        channelPeakSerializedBytes: 100,
        closeCode: null,
        providerPids: [],
        providerStarts: 0,
      };
    },
  });
  const entry = window.__KRONOS_IPC_ACCEPTANCE_START__;
  await entry({
    type: "bootstrap",
    runId: "0123456789abcdef0123456789abcdef",
    caseId: "native-smoke",
    role: "main",
    surfaceGeneration: 1,
  });
  callbacks.set(11, () => {
    deliveries++;
  });
  await entry({ type: "ack-gate", enabled: true });
  callbacks.get(11)({
    index: 0,
    message: { sessionId: "s", generation: "g", sequence: 1, kind: "message", text: "{}" },
  });
  assert.equal(deliveries, 0);
  await assert.rejects(entry({ type: "run" }), /deadline/i);
  assert.equal(deliveries, 1, "Failure stranded a real callback");
  assert.ok(!outcomes.includes("pass"), "Native send alone certified delivery");
});

test("native-smoke scenario VM characterization rejects missing DOM, wrong session, stale identity and hung observation", async () => {
  const runId = "0123456789abcdef0123456789abcdef";
  for (const scenario of [
    { name: "missing DOM", sidebar: false },
    { name: "other session", native: { sessionId: "other" } },
    { name: "other session generation", native: { generation: "other" } },
    { name: "missing ACK", native: { lastAckSequence: 1 } },
    { name: "impossible ACK", native: { lastAckSequence: 3 } },
    { name: "closed session", native: { closeCode: 1000 } },
    { name: "stale incarnation", native: { incarnation: 2 }, error: /identity/i },
    { name: "wrong run", native: { runId: "other" }, error: /identity/i },
    { name: "wrong role", native: { role: "secondary" }, error: /identity/i },
    { name: "hung native observation", pending: true },
  ]) {
    let clock = 0;
    const calls = [];
    const outcomes = [];
    const { window, callbacks } = await acceptanceObserverVm({
      window: {
        performance: { now: () => clock },
        setTimeout: (callback, delay) =>
          setTimeout(() => {
            clock += delay === 50 ? 120000 : delay;
            callback();
          }, 0),
        clearTimeout,
        document: {
          querySelector: () =>
            scenario.sidebar === false
              ? null
              : { getClientRects: () => [{ width: 100, height: 100 }] },
        },
        getComputedStyle: () => ({ visibility: "visible", display: "flex" }),
      },
      invoke: async (command, args) => {
        calls.push(command);
        if (command === "ipc_acceptance_report") {
          outcomes.push(args.input.report.outcome);
          return { reported: true };
        }
        assert.equal(command, "ipc_acceptance_control", scenario.name);
        assert.ok(["observe", "exit"].includes(args.input.action.type), scenario.name);
        if (args.input.action.type === "exit") return { exited: true };
        if (scenario.pending) return new Promise(() => {});
        return {
          runId,
          caseId: "native-smoke",
          role: "main",
          incarnation: 1,
          sessionId: "s",
          generation: "g",
          engineEpoch: 1,
          lastSentSequence: 2,
          lastAckSequence: 2,
          sessionQueuePeakFrames: 1,
          sessionQueuePeakDecodedBytes: 10,
          channelPeakEvents: 2,
          channelPeakSerializedBytes: 100,
          closeCode: null,
          providerPids: [],
          providerStarts: 0,
          ...scenario.native,
        };
      },
    });
    callbacks.set(12, () => {});
    for (const [index, payload] of [
      { status: "server_info" },
      { requestId: "request-1", success: true },
    ].entries()) {
      callbacks.get(12)({
        index,
        message: {
          sessionId: "s",
          generation: "g",
          sequence: index + 1,
          kind: "message",
          text: JSON.stringify({ type: "session", message: { payload } }),
        },
      });
    }
    const entry = window.__KRONOS_IPC_ACCEPTANCE_START__;
    await entry({
      type: "bootstrap",
      runId,
      caseId: "native-smoke",
      role: "main",
      surfaceGeneration: 1,
    });
    await assert.rejects(entry({ type: "run" }), scenario.error ?? /deadline/i, scenario.name);
    assert.ok(!outcomes.includes("pass"), scenario.name);
  }
});

test("preflight result validation requires owned runtime paths, initialization and cleanup-bound claims without renderer", async () => {
  const path = await import("node:path");
  const { validateLaunchResult } = await import("./desktop-ipc-acceptance.mjs");
  const root = path.resolve("preflight-validator-fixture");
  const home = path.join(root, "home");
  const runId = "0123456789abcdef0123456789abcdef";
  const expected = {
    runId,
    caseId: "preflight",
    transport: "ipc",
    root,
    launcherPid: 101,
    childPid: 102,
  };
  const native = {
    runId,
    caseId: "preflight",
    transport: "ipc",
    pid: 102,
    identifier: `sh.paseo.desktop.ipc-acceptance.${runId}`,
    claims: { ownerRunId: runId, launcherPid: 101, childPid: 102 },
    resourceClaimsRunId: runId,
    initialPipeLeaseTransferred: true,
    engineEpoch: 1,
    engineReady: true,
    coreStopped: false,
    schedulerRegistered: true,
    windowCount: 0,
    configuredWindowCount: 0,
    osIntegrationsEnabled: false,
    networkPort: 49152,
    networkError: null,
    installedProviders: ["codex"],
    externalMcpServerCount: 0,
    providerPids: [],
    providerStarts: 0,
    directories: {
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
    },
    modelRoots: [path.join(home, "models")],
    profiles: ["main", "secondary", "guest"].map((role) => path.join(root, "profile", role)),
    providerCommands: Object.fromEntries(
      ["codex", "node"].map((name) => [
        name,
        path.join(root, "bin", name + (process.platform === "win32" ? ".exe" : "")),
      ]),
    ),
  };
  const envelope = (patch = {}) => ({
    schemaVersion: 1,
    runId,
    caseId: "preflight",
    native: { ...native, ...patch },
  });
  let valid;
  assert.doesNotThrow(() => {
    valid = validateLaunchResult(envelope(), expected);
  }, "Valid native-only preflight evidence was rejected");
  assert.equal(valid.outcome, "pass");
  assert.equal(valid.renderer, undefined);
  for (const patch of [
    { engineEpoch: 0 },
    { engineReady: false },
    { coreStopped: true },
    { schedulerRegistered: false },
    { windowCount: 1 },
    { configuredWindowCount: 1 },
    { osIntegrationsEnabled: true },
    { resourceClaimsRunId: "other" },
    { initialPipeLeaseTransferred: false },
    { pid: 103 },
    { claims: { ...native.claims, childPid: 103 } },
    { claims: { ...native.claims, launcherPid: 103 } },
    { installedProviders: ["codex", "claude"] },
    { providerStarts: 1 },
    { providerPids: [500] },
    { externalMcpServerCount: 1 },
    { networkPort: 6767 },
    { networkError: "failed" },
    { modelRoots: [path.join(root, "decoy-models")] },
    { profiles: native.profiles.slice(0, 2) },
    { providerCommands: { ...native.providerCommands, codex: path.join(root, "outside.exe") } },
    { directories: { ...native.directories, tauriData: path.join(root, "outside") } },
    { directories: { ...native.directories, managerHome: path.join(root, "outside") } },
  ])
    assert.throws(
      () => validateLaunchResult(envelope(patch), expected),
      /preflight/i,
      JSON.stringify(patch),
    );
  assert.throws(
    () => validateLaunchResult(envelope(), { ...expected, root: undefined }),
    /preflight/i,
  );
  assert.throws(
    () => validateLaunchResult({ ...envelope(), renderer: { outcome: "pass" } }, expected),
    /preflight/i,
  );
  if (process.platform === "win32") {
    const directories = Object.fromEntries(
      Object.entries(native.directories).map(([name, value]) => [name, `\\\\?\\${value}`]),
    );
    assert.doesNotThrow(
      () => validateLaunchResult(envelope({ directories }), expected),
      "Canonical internal Windows paths were rejected",
    );
  }
});

test("trust-reload result requires stale rejection, unchanged core epoch and trusted secondary delivery", async () => {
  const { validateLaunchResult } = await import("./desktop-ipc-acceptance.mjs");
  const make = (role, incarnation, sessionId) => ({
    schemaVersion: 1,
    runId: "run",
    caseId: "trust-reload",
    native: {
      runId: "run",
      caseId: "trust-reload",
      role,
      incarnation,
      sessionId,
      generation: `generation-${sessionId}`,
      engineEpoch: 7,
      lastSentSequence: 2,
      lastAckSequence: 2,
      opened: true,
      sessionQueuePeakFrames: 1,
      sessionQueuePeakDecodedBytes: 10,
      channelPeakEvents: 2,
      channelPeakSerializedBytes: 100,
      closeCode: null,
      providerPids: [],
      providerStarts: 0,
    },
    renderer: {
      runId: "run",
      caseId: "trust-reload",
      role,
      incarnation,
      sessionId,
      generation: `generation-${sessionId}`,
      lastReceivedSequence: 2,
      visibleState: role === "main" ? "sidebar-visible" : "secondary-sidebar-visible",
      outcome: "pass",
      serverInfoReceived: true,
      requestCompleted: true,
      ...(role === "main"
        ? { staleGenerationRejected: true, sameEngineEpoch: true, guestRejected: true }
        : { crossOwnerRejected: true }),
    },
  });
  const main = make("main", 3, "main-session");
  const secondary = make("secondary", 2, "secondary-session");
  const result = { ...main, secondary };
  const expected = { runId: "run", caseId: "trust-reload", role: "main" };
  assert.equal(validateLaunchResult(result, expected).outcome, "pass");
  for (const invalid of [
    { ...result, secondary: undefined },
    { ...result, secondary: { ...secondary, native: { ...secondary.native, role: "guest" } } },
    { ...result, secondary: { ...secondary, native: { ...secondary.native, engineEpoch: 8 } } },
    { ...result, renderer: { ...main.renderer, staleGenerationRejected: false } },
    { ...result, renderer: { ...main.renderer, sameEngineEpoch: false } },
    { ...result, renderer: { ...main.renderer, guestRejected: false } },
    {
      ...result,
      secondary: { ...secondary, renderer: { ...secondary.renderer, crossOwnerRejected: false } },
    },
  ])
    assert.throws(
      () => validateLaunchResult(invalid, expected),
      /trust|stale|epoch|cross-owner|identity mismatch/i,
    );
});

test("launch result validation requires matching identity and the full renderer/native join", async () => {
  const { validateLaunchResult, readLaunchResult } = await import("./desktop-ipc-acceptance.mjs");
  const expected = { runId: "r1", caseId: "native-smoke", role: "main" };
  const native = {
    ...expected,
    incarnation: 3,
    sessionId: "s",
    generation: "g",
    engineEpoch: 1,
    lastSentSequence: 2,
    lastAckSequence: 2,
    sessionQueuePeakFrames: 1,
    sessionQueuePeakDecodedBytes: 16,
    channelPeakEvents: 1,
    channelPeakSerializedBytes: 16,
    providerStarts: 0,
    providerPids: [],
  };
  const renderer = {
    ...expected,
    incarnation: 3,
    sessionId: "s",
    generation: "g",
    lastReceivedSequence: 2,
    visibleState: "sidebar-visible",
    outcome: "pass",
    serverInfoReceived: true,
    requestCompleted: true,
  };
  const envelope = (patch) => ({
    schemaVersion: 1,
    runId: "r1",
    caseId: "native-smoke",
    native,
    renderer,
    ...patch,
  });

  const validated = validateLaunchResult(envelope({}), expected);
  assert.equal(validated.outcome, "pass");

  for (const [patch, pattern] of [
    [{ native: null }, /native acceptance result|native result/i],
    [{ renderer: null }, /renderer/i],
    [{ runId: "r2" }, /identity/i],
    [{ caseId: "preflight" }, /identity/i],
    [{ schemaVersion: 2 }, /schema/i],
    [{ error: "core never bound" }, /core never bound/],
    [{ outcome: "pass", renderer: { ...renderer, outcome: "blocked" } }, /app-visible/],
    [{ native: { ...native, lastAckSequence: 0 } }, /ACK/],
    [{ native: { ...native, channelPeakEvents: undefined } }, /measurement/],
    [{ renderer: { ...renderer, serverInfoReceived: false } }, /server_info/],
  ]) {
    assert.throws(() => validateLaunchResult(envelope(patch), expected), pattern);
  }
  assert.throws(() => validateLaunchResult(null, expected), /native acceptance result/i);
  // The join is the real gate: a self-consistent envelope with mismatched identities still rejects.
  assert.throws(
    () => validateLaunchResult(envelope({ renderer: { ...renderer, incarnation: 4 } }), expected),
    /incarnation/,
  );

  const root = await mkdtemp(join(tmpdir(), "kronos-result-test-"));
  const filename = join(root, "result.json");
  try {
    await writeFile(filename, JSON.stringify(envelope({})));
    const fromDisk = await readLaunchResult(filename, expected);
    assert.equal(fromDisk.outcome, "pass");
    await writeFile(filename, " ".repeat(8 * 1024 * 1024) + JSON.stringify(envelope({})));
    await assert.rejects(readLaunchResult(filename, expected), /bound/);
    await writeFile(filename, JSON.stringify(envelope({})));
    await link(filename, join(root, "alias.json"));
    await assert.rejects(readLaunchResult(filename, expected), /linked/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

async function desktopBridgeScript() {
  const content = await readFile(
    join(repoRoot, "packages/desktop-tauri/src-tauri/src/main.rs"),
    "utf8",
  );
  const startMarker = 'let bridge_script = format!(\n        r#"';
  const endMarker = '"#\n    );';
  const start = content.indexOf(startMarker);
  const end = content.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0) throw new Error("Native bridge template is unavailable");
  return content
    .slice(start + startMarker.length, end)
    .replace(/\{\{/g, "{")
    .replace(/\}\}/g, "}")
    .replace("{status_json}", JSON.stringify({ status: "running", listen: "127.0.0.1:6767" }));
}

async function trustedBridge(invoke = async () => null) {
  const registry = JSON.stringify([
    { serverId: "external", connections: [{ type: "directTcp", endpoint: "localhost:6767" }] },
  ]);
  const remembered = JSON.stringify({ serverId: "external", workspaceId: "kept" });
  const storage = new Map([
    ["@paseo:daemon-registry", registry],
    ["paseo:last-workspace-route-selection", remembered],
  ]);
  const window = {
    location: { protocol: "http:", hostname: "tauri.localhost", port: "" },
    __TAURI_INTERNALS__: { invoke },
    addEventListener() {},
  };
  window.top = window;
  vm.runInNewContext(await desktopBridgeScript(), {
    window,
    setInterval() {},
    localStorage: {
      getItem: (key) => storage.get(key) ?? null,
      setItem: (key, value) => storage.set(key, value),
    },
  });
  assert.ok(window.paseoDesktop, "trusted bridge was not installed");
  return { bridge: window.paseoDesktop, storage, registry, remembered };
}

test("native lifecycle/status errors never downgrade to the bootstrap snapshot", async () => {
  const { bridge } = await trustedBridge(async () => {
    throw new Error("native rejected operation");
  });
  for (const command of [
    "desktop_daemon_status",
    "start_desktop_daemon",
    "restart_desktop_daemon",
    "stop_desktop_daemon",
  ]) {
    await assert.rejects(bridge.invoke(command), /native rejected operation/);
  }
});

test("credential lookup forwards the intended daemon identity with the target", async () => {
  const received = [];
  const { bridge } = await trustedBridge(async (command, args) => {
    received.push({ command, ...args });
    return "test-only-token";
  });
  assert.equal(
    await bridge.invoke("desktop_local_credential", {
      listen: "ipc://embedded",
      serverId: "managed",
    }),
    "test-only-token",
  );
  assert.deepEqual(received, [
    { command: "get_live_local_credential", listen: "ipc://embedded", serverId: "managed" },
  ]);
});

test("bridge initialization preserves saved unrelated hosts and remembered workspace", async () => {
  const { storage, registry, remembered } = await trustedBridge();
  assert.equal(storage.get("@paseo:daemon-registry"), registry);
  assert.equal(storage.get("paseo:last-workspace-route-selection"), remembered);
});

test("remote and opaque documents never acquire a native bridge", async () => {
  const source = await desktopBridgeScript();
  for (const location of [
    { protocol: "http:", hostname: "127.0.0.1", port: "6767" },
    { protocol: "https:", hostname: "tauri.localhost.attacker.invalid", port: "" },
    { protocol: "about:", hostname: "", port: "" },
  ]) {
    const window = { location };
    window.top = window;
    vm.runInNewContext(source, { window });
    assert.equal(window.paseoDesktop, undefined);
  }
});

test("a trusted-URL subframe cannot inherit the top-level native bridge", async () => {
  const window = {
    top: {},
    location: { protocol: "http:", hostname: "tauri.localhost", port: "" },
  };
  vm.runInNewContext(await desktopBridgeScript(), { window });
  assert.equal(window.paseoDesktop, undefined);
});
