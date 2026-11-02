import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import test from "node:test";

const repoRoot = join(import.meta.dirname, "..");

test("server.rs contains handlers for all core subscriptions and directory sync requests", async () => {
  const serverRsPath = join(
    repoRoot,
    "packages/desktop-tauri/src-tauri/src/kernel/server.rs",
  );
  const content = await readFile(serverRsPath, "utf8");

  const requiredHandlers = [
    '"session.events.set_subscription.request"',
    '"subscription.release.request"',
    '"fetch_workspaces_request"',
    '"fetch_agents_request"',
    '"workspace.label.list.request"',
    '"workspace.setup.subscribe.request"',
    '"agent.timeline.set_subscription.request"',
    '"subscribe_terminals_request"',
    '"creation.subscribe.request"',
    '"get_providers_snapshot_request"',
    '"list_available_providers_request"',
    '"browser.host.register.request"',
    '"get_daemon_config_request"',
    '"daemon.get_status.request"',
    '"workspace.create.request"',
    '"agent.create.request"',
    '"refresh_providers_snapshot_request"',
    '"provider_diagnostic_request"',
  ];
  for (const handler of requiredHandlers) {
    assert.ok(
      content.includes(handler),
      `server.rs must contain handler for ${handler}`,
    );
  }

  // Ensure fetch_workspaces_response includes entries, emptyProjects, and pageInfo
  assert.ok(
    content.includes('"fetch_workspaces_response"'),
    "server.rs must respond with fetch_workspaces_response",
  );
  assert.ok(
    content.includes('"emptyProjects"'),
    "fetch_workspaces_response must contain emptyProjects",
  );
  assert.ok(
    content.includes('"pageInfo"'),
    "fetch_workspaces_response must contain pageInfo",
  );
  assert.ok(
    !content.includes('"projectIconRevision": 1,'),
    "projectIconRevision must never be a numeric literal (schema requires string)",
  );

  const protocolRsPath = join(
    repoRoot,
    "packages/desktop-tauri/src-tauri/src/kernel/protocol.rs",
  );
  const protocolContent = await readFile(protocolRsPath, "utf8");
  assert.ok(
    protocolContent.includes('"providersSnapshot": true'),
    "protocol.rs must advertise providersSnapshot: true in server_info features",
  );
});
