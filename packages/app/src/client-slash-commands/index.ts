import type { Agent } from "@/stores/session-store";
import type { WorkspaceDraftTabSetup } from "@/workspace-tabs/model";

export type ClientSlashCommandKind =
  | "archive-agent"
  | "replace-agent-with-draft"
  | "compact-session"
  | "test-loop"
  | "audit-codebase"
  | "consensus-mode"
  | "create-pr"
  | "artifacts-view"
  | "cloud-deploy"
  | "semantic-search"
  | "browser-test";
export type ClientSlashCommandExecution = "immediate" | "insert";

export interface ClientSlashCommand {
  name: string;
  aliases: readonly string[];
  description: string;
  descriptionKey: "composer.clientCommands.archiveAgent" | "composer.clientCommands.freshDraft";
  argumentHint: string;
  kind: ClientSlashCommandKind;
  execution: ClientSlashCommandExecution;
}

export const CLIENT_SLASH_COMMANDS: readonly ClientSlashCommand[] = [
  {
    name: "exit",
    aliases: ["quit", "q"],
    description: "Archive the current agent",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "",
    kind: "archive-agent",
    execution: "immediate",
  },
  {
    name: "clear",
    aliases: ["new"],
    description: "Archive this agent and start a fresh draft",
    descriptionKey: "composer.clientCommands.freshDraft",
    argumentHint: "",
    kind: "replace-agent-with-draft",
    execution: "immediate",
  },
  {
    name: "compact",
    aliases: ["condense", "summarize"],
    description:
      "Compact session context to free tokens while preserving critical file edits and decisions",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "",
    kind: "compact-session",
    execution: "immediate",
  },
  {
    name: "test-loop",
    aliases: ["testloop", "tl"],
    description: "Run autonomous test-driven repair loop until all tests pass",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "<command>",
    kind: "test-loop",
    execution: "immediate",
  },
  {
    name: "audit",
    aliases: ["bug-hunt", "scan"],
    description: "Scan repository & diffs for security vulnerabilities and logic bugs",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "",
    kind: "audit-codebase",
    execution: "immediate",
  },
  {
    name: "consensus",
    aliases: ["debate", "multi-model"],
    description: "Query multiple frontier models in parallel and synthesize consensus patch",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "<prompt>",
    kind: "consensus-mode",
    execution: "immediate",
  },
  {
    name: "pr",
    aliases: ["pull-request", "create-pr"],
    description: "Generate and publish an AI-summarized GitHub Pull Request for current branch",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "",
    kind: "create-pr",
    execution: "immediate",
  },
  {
    name: "artifacts",
    aliases: ["canvas", "diagrams"],
    description: "Open the Visual Artifacts Canvas for architecture diagrams and sandboxes",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "",
    kind: "artifacts-view",
    execution: "immediate",
  },
  {
    name: "deploy",
    aliases: ["publish", "ship"],
    description: "Deploy project to Vercel, Netlify, or Cloudflare Pages in 1 click",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "",
    kind: "cloud-deploy",
    execution: "immediate",
  },
  {
    name: "search-code",
    aliases: ["semantic", "rag"],
    description: "Search codebase using natural language semantic vector retrieval",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "<query>",
    kind: "semantic-search",
    execution: "immediate",
  },
  {
    name: "browse-test",
    aliases: ["test-ui", "verify-ui"],
    description: "Verify localhost web UI rendering, DOM structure, and catch runtime errors",
    descriptionKey: "composer.clientCommands.archiveAgent",
    argumentHint: "<url>",
    kind: "browser-test",
    execution: "immediate",
  },
];

const COMMAND_BY_NAME = new Map<string, ClientSlashCommand>();
for (const command of CLIENT_SLASH_COMMANDS) {
  COMMAND_BY_NAME.set(command.name, command);
  for (const alias of command.aliases) {
    COMMAND_BY_NAME.set(alias, command);
  }
}

export function resolveClientSlashCommand(input: {
  text: string;
  hasAttachments: boolean;
}): ClientSlashCommand | null {
  if (input.hasAttachments) {
    return null;
  }

  const trimmed = input.text.trim();
  if (!trimmed.startsWith("/")) {
    return null;
  }

  const commandName = trimmed.slice(1);
  if (!commandName || /\s/.test(commandName)) {
    return null;
  }

  return COMMAND_BY_NAME.get(commandName) ?? null;
}

export function buildDraftAgentSetup(agent: Agent): WorkspaceDraftTabSetup {
  return {
    provider: agent.provider,
    cwd: agent.cwd,
    modeId: agent.currentModeId ?? agent.runtimeInfo?.modeId ?? null,
    model: agent.model ?? agent.runtimeInfo?.model ?? null,
    thinkingOptionId: agent.thinkingOptionId ?? agent.runtimeInfo?.thinkingOptionId ?? null,
    providerOptions: { ...agent.options },
  };
}
