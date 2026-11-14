export type QuickComposerTarget = "general" | "workspace";

export type QuickComposerAgent = "claude" | "opencode" | "codex" | "pi" | "omp";

export interface QuickComposerOptions {
  initialPrompt?: string;
  target?: QuickComposerTarget;
  agent?: QuickComposerAgent;
}

export interface AgentHarnessOption {
  id: QuickComposerAgent;
  label: string;
  badge: string;
  color: string;
}

export const AGENT_HARNESS_OPTIONS: readonly AgentHarnessOption[] = [
  { id: "claude", label: "Claude Code", badge: "Anthropic", color: "#d97706" },
  { id: "opencode", label: "OpenCode", badge: "Fast", color: "#06b6d4" },
  { id: "codex", label: "Codex", badge: "OpenAI", color: "#10b981" },
  { id: "pi", label: "Pi", badge: "Multi-model", color: "#8b5cf6" },
  { id: "omp", label: "Omp", badge: "Swarm", color: "#ec4899" },
];
