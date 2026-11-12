import { describe, expect, it } from "vitest";
import {
  CLIENT_SLASH_COMMANDS,
  buildDraftAgentSetup,
  resolveClientSlashCommand,
} from "@/client-slash-commands";
import type { Agent } from "@/stores/session-store";

function createAgent(overrides: Partial<Agent> = {}): Agent {
  const now = new Date("2026-05-15T00:00:00.000Z");
  return {
    serverId: "server-1",
    id: "agent-1",
    provider: "codex",
    status: "idle",
    createdAt: now,
    updatedAt: now,
    lastUserMessageAt: now,
    lastActivityAt: now,
    capabilities: {
      supportsStreaming: true,
      supportsSessionPersistence: true,
      supportsDynamicModes: true,
      supportsMcpServers: false,
      supportsReasoningStream: false,
      supportsToolInvocations: true,
    },
    currentModeId: "mode-current",
    availableModes: [],
    pendingPermissions: [],
    persistence: null,
    runtimeInfo: {
      provider: "codex",
      sessionId: "session-1",
      model: "runtime-model",
      modeId: "runtime-mode",
      thinkingOptionId: "runtime-thinking",
    },
    title: "Agent",
    cwd: "/repo",
    model: "agent-model",
    thinkingOptionId: "think-hard",
    options: { "web-search": true, effort: "high" },
    parentAgentId: null,
    labels: {},
    ...overrides,
    turn: overrides.turn ?? { phase: "idle", cancellationRequestId: null },
  };
}

describe("resolveClientSlashCommand", () => {
  it("declares the exact canonical client commands with their aliases", () => {
    expect(
      CLIENT_SLASH_COMMANDS.map((command) => [
        command.name,
        [...command.aliases],
        command.execution,
      ]),
    ).toEqual([
      ["exit", ["quit", "q"], "immediate"],
      ["clear", ["new"], "immediate"],
      ["compact", ["condense", "summarize"], "immediate"],
      ["test-loop", ["testloop", "tl"], "immediate"],
      ["audit", ["bug-hunt", "scan"], "immediate"],
      ["consensus", ["debate", "multi-model"], "immediate"],
      ["pr", ["pull-request", "create-pr"], "immediate"],
      ["artifacts", ["canvas", "diagrams"], "immediate"],
      ["deploy", ["publish", "ship"], "immediate"],
      ["search-code", ["semantic", "rag"], "immediate"],
      ["browse-test", ["test-ui", "verify-ui"], "immediate"],
      ["web", ["search-web", "google"], "immediate"],
      ["fetch", ["scrape", "read-url"], "immediate"],
      ["repomap", ["map", "symbols"], "immediate"],
      ["preview", ["live", "web-preview", "split-preview"], "immediate"],
      ["mode", ["switch-mode", "persona"], "immediate"],
      ["yolo", ["danger", "auto-approve", "permissions"], "immediate"],
      ["voice", ["dictate", "speak", "mic"], "immediate"],
      ["swarm", ["subagent", "delegate", "spawn-agent"], "immediate"],
      ["review", ["diff-review", "review-all"], "immediate"],
      ["rewind", ["checkpoint", "rollback", "undo-turn"], "immediate"],
      ["commit", ["git-commit", "conventional-commit"], "immediate"],
      ["cost", ["budget", "usage", "tokens"], "immediate"],
      ["mcp", ["mcp-tools", "tools"], "immediate"],
      ["notepad", ["notes", "scratchpad"], "immediate"],
      ["playbook", ["playbooks", "lessons"], "immediate"],
      ["workflows", ["custom-commands", "prompts"], "immediate"],
      ["diagram", ["arch", "dependencies", "dep-graph"], "immediate"],
      ["db", ["sql", "sqlite", "database"], "immediate"],
      ["compare", ["arena", "ab-test"], "immediate"],
    ]);
  });

  it("resolves canonical names and aliases after trimming", () => {
    expect(resolveClientSlashCommand({ text: " /quit ", hasAttachments: false })).toMatchObject({
      name: "exit",
      kind: "archive-agent",
      execution: "immediate",
    });
    expect(resolveClientSlashCommand({ text: "/exit", hasAttachments: false })).toMatchObject({
      name: "exit",
      kind: "archive-agent",
    });
    expect(resolveClientSlashCommand({ text: "/q", hasAttachments: false })).toMatchObject({
      name: "exit",
      kind: "archive-agent",
    });
    expect(resolveClientSlashCommand({ text: "/clear", hasAttachments: false })).toMatchObject({
      name: "clear",
      kind: "replace-agent-with-draft",
    });
    expect(resolveClientSlashCommand({ text: "/new", hasAttachments: false })).toMatchObject({
      name: "clear",
      kind: "replace-agent-with-draft",
    });
    expect(resolveClientSlashCommand({ text: "/compact", hasAttachments: false })).toMatchObject({
      name: "compact",
      kind: "compact-session",
    });
    expect(resolveClientSlashCommand({ text: "/condense", hasAttachments: false })).toMatchObject({
      name: "compact",
      kind: "compact-session",
    });
  });

  it("leaves provider commands, arguments, ordinary messages, and attachment submits alone", () => {
    expect(resolveClientSlashCommand({ text: "/clear now", hasAttachments: false })).toBeNull();
    expect(resolveClientSlashCommand({ text: "/quit now", hasAttachments: false })).toBeNull();
    expect(
      resolveClientSlashCommand({ text: "/provider-command", hasAttachments: false }),
    ).toBeNull();
    expect(resolveClientSlashCommand({ text: "hello /quit", hasAttachments: false })).toBeNull();
    expect(resolveClientSlashCommand({ text: "/quit", hasAttachments: true })).toBeNull();
  });
});

describe("buildDraftAgentSetup", () => {
  it("builds draft setup from the active agent snapshot", () => {
    expect(buildDraftAgentSetup(createAgent())).toEqual({
      provider: "codex",
      cwd: "/repo",
      modeId: "mode-current",
      model: "agent-model",
      thinkingOptionId: "think-hard",
      providerOptions: {
        "web-search": true,
        effort: "high",
      },
    });
  });

  it("resolves /pr slash command correctly", () => {
    const resolved = resolveClientSlashCommand({ text: "/pr", hasAttachments: false });
    expect(resolved).toMatchObject({
      name: "pr",
      kind: "create-pr",
      execution: "immediate",
    });
  });

  it("resolves /artifacts slash command correctly", () => {
    const resolved = resolveClientSlashCommand({ text: "/artifacts", hasAttachments: false });
    expect(resolved).toMatchObject({
      name: "artifacts",
      kind: "artifacts-view",
      execution: "immediate",
    });
  });

  it("resolves /deploy slash command correctly", () => {
    const resolved = resolveClientSlashCommand({ text: "/deploy", hasAttachments: false });
    expect(resolved).toMatchObject({
      name: "deploy",
      kind: "cloud-deploy",
      execution: "immediate",
    });
  });

  it("resolves /search-code slash command correctly", () => {
    const resolved = resolveClientSlashCommand({ text: "/search-code", hasAttachments: false });
    expect(resolved).toMatchObject({
      name: "search-code",
      kind: "semantic-search",
      execution: "immediate",
    });
  });

  it("resolves /browse-test slash command correctly", () => {
    const resolved = resolveClientSlashCommand({ text: "/browse-test", hasAttachments: false });
    expect(resolved).toMatchObject({
      name: "browse-test",
      kind: "browser-test",
      execution: "immediate",
    });
  });

  it("resolves /web and /fetch slash commands correctly", () => {
    const resolvedWeb = resolveClientSlashCommand({ text: "/web", hasAttachments: false });
    expect(resolvedWeb).toMatchObject({
      name: "web",
      kind: "web-search",
      execution: "immediate",
    });

    const resolvedGoogle = resolveClientSlashCommand({ text: "/google", hasAttachments: false });
    expect(resolvedGoogle).toMatchObject({
      name: "web",
      kind: "web-search",
      execution: "immediate",
    });

    const resolvedFetch = resolveClientSlashCommand({ text: "/fetch", hasAttachments: false });
    expect(resolvedFetch).toMatchObject({
      name: "fetch",
      kind: "web-fetch",
      execution: "immediate",
    });

    const resolvedRepoMap = resolveClientSlashCommand({ text: "/repomap", hasAttachments: false });
    expect(resolvedRepoMap).toMatchObject({
      name: "repomap",
      kind: "repo-map",
      execution: "immediate",
    });

    const resolvedPreview = resolveClientSlashCommand({ text: "/preview", hasAttachments: false });
    expect(resolvedPreview).toMatchObject({
      name: "preview",
      kind: "web-preview",
      execution: "immediate",
    });

    const resolvedMode = resolveClientSlashCommand({ text: "/mode", hasAttachments: false });
    expect(resolvedMode).toMatchObject({
      name: "mode",
      kind: "agent-mode",
      execution: "immediate",
    });

    const resolvedYolo = resolveClientSlashCommand({ text: "/yolo", hasAttachments: false });
    expect(resolvedYolo).toMatchObject({
      name: "yolo",
      kind: "yolo-mode",
      execution: "immediate",
    });

    const resolvedVoice = resolveClientSlashCommand({ text: "/voice", hasAttachments: false });
    expect(resolvedVoice).toMatchObject({
      name: "voice",
      kind: "voice-dictation",
      execution: "immediate",
    });

    const resolvedSwarm = resolveClientSlashCommand({ text: "/swarm", hasAttachments: false });
    expect(resolvedSwarm).toMatchObject({
      name: "swarm",
      kind: "agent-swarm",
      execution: "immediate",
    });
  });

  it("falls back to runtime model setup when top-level fields are absent", () => {
    expect(
      buildDraftAgentSetup(
        createAgent({
          currentModeId: null,
          model: null,
          thinkingOptionId: null,
        }),
      ),
    ).toMatchObject({
      modeId: "runtime-mode",
      model: "runtime-model",
      thinkingOptionId: "runtime-thinking",
    });
  });
});
