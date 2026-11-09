import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { copyFile, link, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import net from "node:net";
import http from "node:http";
import { once } from "node:events";
import WebSocket from "ws";
import { WSOutboundMessageSchema } from "@getpaseo/protocol/messages";

const executableArg = process.argv.indexOf("--exe");
if (executableArg < 0 || !process.argv[executableArg + 1]) {
  throw new Error("Usage: node scripts/smoke-native-desktop.mjs --exe <built desktop executable>");
}
const executable = path.resolve(process.argv[executableArg + 1]);
const home = await mkdtemp(path.join(tmpdir(), "kronos-native-smoke-"));
const fixtureId = "ag_native_smoke_history";
const timestamp = "2026-11-07T00:00:00.000Z";
let child;
const sockets = new Set();
let diagnostics = "";

async function allocatePort() {
  const probe = net.createServer();
  probe.listen(0, "127.0.0.1");
  await once(probe, "listening");
  const { port } = probe.address();
  await new Promise((resolve, reject) =>
    probe.close((error) => (error ? reject(error) : resolve())),
  );
  return port;
}

async function waitReady(port) {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (child.exitCode !== null) throw new Error(`Native daemon exited: ${diagnostics}`);
    try {
      const response = await fetch(`http://127.0.0.1:${port}/health`, {
        signal: AbortSignal.timeout(1000),
      });
      if (response.ok) {
        const health = await response.json();
        assert.equal(health.engine, "kronos-rust-kernel");
        return;
      }
    } catch {
      /* Startup may not have bound its socket yet. */
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Native daemon did not become ready: ${diagnostics}`);
}

async function connect(port, hello) {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  sockets.add(ws);
  ws.on("close", () => sockets.delete(ws));
  const frames = [];
  const waiters = [];
  ws.on("message", (bytes) => {
    const frame = JSON.parse(bytes.toString());
    const waiterIndex = waiters.findIndex((waiter) => waiter.match(frame));
    if (waiterIndex >= 0) {
      const [waiter] = waiters.splice(waiterIndex, 1);
      clearTimeout(waiter.timer);
      waiter.resolve(frame);
    } else frames.push(frame);
  });
  ws.on("close", () => {
    for (const waiter of waiters.splice(0)) {
      clearTimeout(waiter.timer);
      waiter.reject(new Error("Socket closed before expected response"));
    }
  });
  const next = (match) => {
    const index = frames.findIndex(match);
    if (index >= 0) return Promise.resolve(frames.splice(index, 1)[0]);
    return new Promise((resolve, reject) => {
      const waiter = { match, resolve, reject, timer: null };
      waiter.timer = setTimeout(() => {
        waiters.splice(waiters.indexOf(waiter), 1);
        reject(new Error(`Native response timeout; queued=${JSON.stringify(frames)}`));
      }, 10000);
      waiters.push(waiter);
    });
  };
  await once(ws, "open");
  if (hello) ws.send(JSON.stringify(hello));
  return { ws, next };
}

async function stopOwnedDaemon() {
  for (const socket of sockets) socket.terminate();
  sockets.clear();
  if (child?.pid && child.exitCode === null) {
    const exited = once(child, "exit");
    child.kill();
    await exited;
  }
}

try {
  await mkdir(path.join(home, "agents", "timelines"), { recursive: true });
  await writeFile(
    path.join(home, "agents", "records.json"),
    JSON.stringify({
      [fixtureId]: {
        id: fixtureId,
        provider: "codex",
        cwd: home,
        createdAt: timestamp,
        updatedAt: timestamp,
        title: "Native history smoke",
        labels: {},
        lastStatus: "idle",
      },
    }),
  );
  const rows = Array.from({ length: 60 }, (_, turn) => [
    { type: "user_message", id: `u${turn}`, text: `prompt ${turn}`, timestamp },
    { type: "assistant_message", id: `a${turn}`, text: `answer ${turn}`, timestamp },
  ]).flat();
  await writeFile(
    path.join(home, "agents", "timelines", `${fixtureId}.json`),
    JSON.stringify(rows),
  );

  const port = await allocatePort();
  // A real local provider process, with no model/network invocation. The native
  // kernel still owns admission, argv, streams, descendants and resume identity.
  const controlledProvider = path.join(home, process.platform === "win32" ? "codex.exe" : "codex");
  try {
    await link(process.execPath, controlledProvider);
  } catch (error) {
    if (error.code !== "EXDEV" && error.code !== "EPERM") throw error;
    await copyFile(process.execPath, controlledProvider);
  }
  await writeFile(
    path.join(home, "exec"),
    String.raw`
const fs = require("node:fs");
const path = require("node:path");
const {spawn} = require("node:child_process");
const args = process.argv.slice(2);
fs.appendFileSync(path.join(process.cwd(), "provider-argv.jsonl"), JSON.stringify(args) + "\n");
console.log(JSON.stringify({type:"thread.started", thread_id:"smoke-session"}));
console.log(JSON.stringify({type:"item.completed", item:{id:"answer",type:"agent_message",
  text:args.includes("resume") ? "Resumed exact smoke session" : "Partial output before cancellation"}}));
if (!args.includes("resume")) {
  const descendant = spawn(process.execPath, ["-e", "setInterval(()=>{},1000)"], {stdio:"ignore", windowsHide:true});
  fs.writeFileSync(path.join(process.cwd(), "provider-descendant.pid"), String(descendant.pid));
  setInterval(()=>{},1000);
}
`,
  );
  const daemonEnv = { ...process.env, KRONOS_HOME: home, PASEO_HOME: home };
  const pathKey = Object.keys(daemonEnv).find((key) => key.toUpperCase() === "PATH") || "PATH";
  daemonEnv[pathKey] = `${home}${path.delimiter}${daemonEnv[pathKey] || ""}`;
  const launch = async () => {
    child = spawn(executable, ["daemon", "--port", String(port)], {
      env: daemonEnv,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    child.on("error", (error) => {
      diagnostics += error.message;
    });
    child.stdout.on("data", (data) => {
      diagnostics = (diagnostics + data).slice(-65536);
    });
    child.stderr.on("data", (data) => {
      diagnostics = (diagnostics + data).slice(-65536);
    });
    await once(child, "spawn");
    await waitReady(port);
  };
  await launch();

  const hostile = await fetch(`http://127.0.0.1:${port}/health`, {
    headers: { Origin: "https://attacker.invalid" },
  });
  assert.equal(hostile.status, 403);
  const rebinding = await new Promise((resolve, reject) => {
    const request = http.get(
      {
        hostname: "127.0.0.1",
        port,
        path: "/health",
        headers: { Host: `attacker.invalid:${port}` },
      },
      resolve,
    );
    request.on("error", reject);
  });
  assert.equal(rebinding.statusCode, 403);
  rebinding.resume();
  assert.equal(
    (await fetch(`http://127.0.0.1:${port}/api/browser-proxy?url=https://example.invalid`)).status,
    401,
  );

  const unauth = await connect(port);
  const unauthorizedClosed = once(unauth.ws, "close");
  unauth.ws.send(
    JSON.stringify({
      type: "session",
      message: { type: "fetch_agents_request", requestId: "must-reject" },
    }),
  );
  const [closeCode] = await unauthorizedClosed;
  assert.equal(closeCode, 4401);

  const credential = (await readFile(path.join(home, "local-credential"), "utf8")).trim();
  const hello = () => ({
    type: "hello",
    protocolVersion: 1,
    clientId: "release-smoke",
    clientType: "cli",
    auth: { kind: "localCredential", token: credential },
  });
  let client = await connect(port, hello());
  const info = await client.next((frame) => frame.message?.payload?.status === "server_info");
  assert.equal(info.message.payload.protocolVersion, 1);
  let sequence = 0;
  const rpc = async (type, fields = {}) => {
    const requestId = `smoke-${++sequence}`;
    client.ws.send(JSON.stringify({ type: "session", message: { type, requestId, ...fields } }));
    const frame = await client.next(
      (frame) =>
        frame.message?.payload?.requestId === requestId || frame.message?.requestId === requestId,
    );
    return frame.message.payload;
  };
  let page = await rpc("fetch_agent_timeline_request", { agentId: fixtureId, limit: 7 });
  assert.equal(page.entries[0].seqStart, 114);
  assert.equal(page.entries.at(-1).item.text, "answer 59");
  assert.equal(page.hasOlder, true);
  assert.equal(page.reset, true);
  const seen = page.entries.map((entry) => entry.seqStart);
  while (page.hasOlder) {
    page = await rpc("fetch_agent_timeline_request", {
      agentId: fixtureId,
      direction: "before",
      cursor: page.startCursor,
      limit: 7,
    });
    assert.equal(page.reset, false);
    seen.unshift(...page.entries.map((entry) => entry.seqStart));
  }
  assert.deepEqual(
    seen,
    Array.from({ length: 120 }, (_, index) => index + 1),
  );
  const stale = await rpc("fetch_agent_timeline_request", {
    agentId: fixtureId,
    direction: "after",
    cursor: { epoch: "old", seq: 1 },
    limit: 4,
  });
  assert.equal(stale.staleCursor, true);
  assert.equal(stale.reset, true);
  const whole = await rpc("fetch_agent_timeline_request", {
    agentId: fixtureId,
    limit: 3,
    wholeTurns: true,
  });
  assert.equal(whole.entries[0].item.type, "user_message");
  assert.equal(whole.entries.at(-1).item.type, "assistant_message");

  const created = await rpc("agent.create.request", {
    provider: "kronos-smoke-unavailable",
    cwd: home,
  });
  assert.ok(created.agent?.id, JSON.stringify(created));
  const agentId = created.agent.id;
  const sent = await rpc("send_agent_message_request", {
    agentId,
    text: "Persist isolated smoke title",
  });
  assert.equal(sent.accepted, true);
  let completed;
  for (let attempt = 0; attempt < 100; attempt++) {
    completed = await rpc("fetch_agent_request", { agentId });
    if (completed.agent.status !== "running") break;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  assert.equal(completed.agent.title, "Persist isolated smoke title");
  assert.notEqual(completed.agent.status, "running");
  const stored = await rpc("fetch_agent_timeline_request", { agentId });
  assert.equal(stored.entries[0].item.text, "Persist isolated smoke title");
  const controlled = await rpc("agent.create.request", { provider: "codex", cwd: home });
  const controlledId = controlled.agent.id;
  assert.equal(
    (
      await rpc("send_agent_message_request", {
        agentId: controlledId,
        text: "Run controlled local turn",
      })
    ).accepted,
    true,
  );
  let owned;
  let descendantPid;
  for (let attempt = 0; attempt < 100; attempt++) {
    owned = await rpc("fetch_agent_request", { agentId: controlledId });
    try {
      descendantPid = Number(await readFile(path.join(home, "provider-descendant.pid"), "utf8"));
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    if (owned.agent.persistence?.sessionId === "smoke-session" && descendantPid > 0) break;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  assert.equal(owned.agent.persistence?.sessionId, "smoke-session");
  assert.ok(descendantPid > 0);
  const overlap = await rpc("send_agent_message_request", {
    agentId: controlledId,
    text: "Must not become an orphan prompt",
  });
  assert.equal(overlap.accepted, false);
  const canceled = client.next(
    (frame) =>
      frame.message?.payload?.agentId === controlledId &&
      frame.message.payload.event?.type === "turn_canceled",
  );
  assert.equal((await rpc("cancel_agent_request", { agentId: controlledId })).accepted, true);
  const canceledFrame = await canceled;
  WSOutboundMessageSchema.parse(canceledFrame);
  assert.equal(typeof canceledFrame.message.payload.event.reason, "string");
  let descendantAlive = true;
  for (let attempt = 0; attempt < 100 && descendantAlive; attempt++) {
    try {
      process.kill(descendantPid, 0);
    } catch (error) {
      if (error.code === "ESRCH") descendantAlive = false;
      else throw error;
    }
    if (descendantAlive) await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.equal(
    descendantAlive,
    false,
    "Cancel must terminate the owned descendant, not just change status",
  );
  const canceledHistory = await rpc("fetch_agent_timeline_request", { agentId: controlledId });
  assert.deepEqual(
    canceledHistory.entries
      .filter((entry) => entry.item.type === "user_message")
      .map((entry) => entry.item.text),
    ["Run controlled local turn"],
  );
  assert.ok(
    canceledHistory.entries.some(
      (entry) => entry.item.text === "Partial output before cancellation",
    ),
  );
  await stopOwnedDaemon();
  await launch();
  const nextCredential = (await readFile(path.join(home, "local-credential"), "utf8")).trim();
  assert.notEqual(nextCredential, credential);
  client = await connect(port, {
    ...hello(),
    auth: { kind: "localCredential", token: nextCredential },
  });
  await client.next((frame) => frame.message?.payload?.status === "server_info");
  const persisted = await rpc("fetch_agent_timeline_request", { agentId });
  assert.equal(persisted.entries[0].item.text, "Persist isolated smoke title");
  const restored = await rpc("fetch_agent_request", { agentId });
  assert.equal(restored.agent.title, "Persist isolated smoke title");
  assert.equal(
    (
      await rpc("send_agent_message_request", {
        agentId: controlledId,
        text: "Resume after daemon restart",
      })
    ).accepted,
    true,
  );
  for (let attempt = 0; attempt < 100; attempt++) {
    owned = await rpc("fetch_agent_request", { agentId: controlledId });
    if (owned.agent.status !== "running") break;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  assert.equal(owned.agent.status, "idle");
  const resumed = await rpc("fetch_agent_timeline_request", { agentId: controlledId });
  assert.ok(resumed.entries.some((entry) => entry.item.text === "Resumed exact smoke session"));
  const invocations = (await readFile(path.join(home, "provider-argv.jsonl"), "utf8"))
    .trim()
    .split("\n")
    .map(JSON.parse);
  assert.equal(invocations.length, 2);
  assert.equal(invocations[1][0], "resume");
  assert.ok(invocations[1].includes("smoke-session"));
  console.log(
    JSON.stringify({
      result: "passed",
      checks: [
        "host-origin-boundary",
        "unauthenticated-rpc-rejection",
        "proxy-authentication",
        "credentialed-hello",
        "paged-history-no-gaps",
        "stale-cursor-reset",
        "whole-turn-pages",
        "native-create-prompt",
        "journal-reload",
        "credential-rotation",
        "single-active-turn",
        "canonical-cancellation-event",
        "real-descendant-cleanup",
        "partial-output-preservation",
        "explicit-provider-resume-after-restart",
      ],
      port,
      productionDaemonTouched: false,
    }),
  );
} finally {
  await stopOwnedDaemon();
  await rm(home, { recursive: true, force: true });
}
