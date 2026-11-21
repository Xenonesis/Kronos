// Acceptance-only local process fixture; no provider credentials or network access.
const fs = require("node:fs");
const path = require("node:path");
const { createHash } = require("node:crypto");
const { spawn } = require("node:child_process");

const descriptorPath = process.env.KRONOS_IPC_ACCEPTANCE_DESCRIPTOR;
if (!descriptorPath || !path.isAbsolute(descriptorPath))
  throw new Error("Owned descriptor required");
const root = path.dirname(descriptorPath);
const descriptor = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
if (
  descriptor.schemaVersion !== 1 ||
  !/^[a-f0-9]{32}$/.test(descriptor.runId) ||
  path.basename(root) !== `kronos-ipc-acceptance-${descriptor.runId}` ||
  fs.realpathSync(process.cwd()) !== fs.realpathSync(path.join(root, "workspace"))
) {
  throw new Error("Fixture must run in the exact owned workspace");
}
const args = process.argv.slice(2);
const reports = path.join(root, "reports");
const events = path.join(reports, "provider-events.jsonl");
function record(stage, extra = {}) {
  if (fs.existsSync(events) && fs.statSync(events).size > 1024 * 1024)
    throw new Error("Fixture event bound exceeded");
  fs.appendFileSync(
    events,
    JSON.stringify({ runId: descriptor.runId, stage, pid: process.pid, ...extra }) + "\n",
  );
}
function frame(value) {
  process.stdout.write(JSON.stringify(value) + "\n");
}
record("started", {
  argvSha256: createHash("sha256").update(JSON.stringify(args)).digest("hex"),
  resumed: args.includes("resume"),
});
frame({ type: "thread.started", thread_id: "smoke-session" });
frame({
  type: "item.completed",
  item: {
    id: "answer",
    type: "agent_message",
    text: args.includes("resume")
      ? "Resumed exact smoke session"
      : "Partial output before cancellation",
  },
});
if (args.includes("resume")) {
  record("completed");
} else {
  // Finite even if a faulty parent loses its gate; normal cleanup remains owned by native Job.
  const descendant = spawn(process.execPath, ["-e", "setTimeout(()=>{},25000)"], {
    stdio: "ignore",
    windowsHide: true,
  });
  record("descendant", { descendantPid: descendant.pid });
  fs.writeFileSync(path.join(reports, "provider-descendant.pid"), String(descendant.pid));
  const deadline = setTimeout(() => {
    record("expired");
    process.exitCode = 2;
    clearInterval(poll);
  }, 25000);
  const poll = setInterval(() => {
    if (!fs.existsSync(path.join(reports, "fixture-continue"))) return;
    clearInterval(poll);
    clearTimeout(deadline);
    if (descriptor.caseId === "backpressure") {
      for (let i = 0; i < 256; i++)
        frame({
          type: "item.completed",
          item: { id: `burst-${i}`, type: "agent_message", text: "x".repeat(4096) },
        });
    }
    frame({
      type: "item.completed",
      item: { id: "finished", type: "agent_message", text: "Controlled fixture completed" },
    });
    record("completed");
    // Child remains in the parent's Job until the native runner confirms cancellation/exit cleanup.
    descendant.unref();
  }, 10);
}
