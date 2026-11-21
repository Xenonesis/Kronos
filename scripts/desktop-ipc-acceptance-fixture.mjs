import { copyFile, link, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

// Real local provider process; native kernel still owns admission/argv/descendants.
export async function seedControlledCodexFixture(home, acceptance = false) {
  const executable = path.join(home, process.platform === "win32" ? "codex.exe" : "codex");
  try {
    await link(process.execPath, executable);
  } catch (error) {
    if (error.code !== "EXDEV" && error.code !== "EPERM") throw error;
    await copyFile(process.execPath, executable);
  }
  const acceptanceScript = acceptance
    ? await readFile(new URL("./fixtures/native-codex-smoke.cjs", import.meta.url), "utf8")
    : null;
  await writeFile(
    path.join(home, "exec"),
    acceptanceScript ??
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
}
