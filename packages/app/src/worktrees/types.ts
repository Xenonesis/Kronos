/**
 * Types and data models for the Visual Worktree Fleet Manager.
 */

export interface PaseoWorktreeRaw {
  worktreePath: string;
  createdAt: string;
  branchName?: string | null;
  head?: string | null;
}

export interface PaseoWorktreeListResult {
  worktrees: PaseoWorktreeRaw[];
  error?: unknown;
  requestId?: string;
}

export interface CreatePaseoWorktreeParams {
  cwd: string;
  worktreeSlug: string;
  refName?: string;
  action?: "checkout" | "branch";
  projectId?: string;
  checkoutSource?: string;
  githubPrNumber?: number;
}

export interface CreatePaseoWorktreeResult {
  workspace?: unknown;
  error?: string | null;
  errorCode?: string;
  setupTerminalId?: string | null;
  setupSkippedReason?: string;
  requestId?: string;
}

export interface ArchivePaseoWorktreeParams {
  worktreePath?: string;
  repoRoot?: string;
  branchName?: string;
  workspaceId?: string;
  scope?: "workspace" | "worktree";
}

export interface ArchivePaseoWorktreeResult {
  success: boolean;
  error?: unknown;
  removedAgents?: string[];
  requestId?: string;
}

export interface WorktreeClient {
  listPaseoWorktrees?: (
    input: { cwd?: string; repoRoot?: string },
    requestId?: string,
  ) => Promise<PaseoWorktreeListResult>;
  getPaseoWorktreeList?: (
    input: { cwd?: string; repoRoot?: string },
    requestId?: string,
  ) => Promise<PaseoWorktreeListResult>;
  createPaseoWorktree: (
    input: CreatePaseoWorktreeParams,
    requestId?: string,
  ) => Promise<CreatePaseoWorktreeResult>;
  archivePaseoWorktree: (
    input: ArchivePaseoWorktreeParams,
    requestId?: string,
  ) => Promise<ArchivePaseoWorktreeResult>;
}

export type WorktreeFleetStatus = "active" | "idle" | "running" | "ready" | "dirty" | "error";

export interface WorktreeAgentSummary {
  id: string;
  title: string | null;
  status: string;
  provider?: string;
  turn?: string;
  model?: string | null;
}

export interface WorktreeFleetItem {
  worktreePath: string;
  branchName: string;
  createdAt: string;
  head: string | null;
  status: WorktreeFleetStatus;
  isCurrent: boolean;
  workspaceId?: string;
  agents: WorktreeAgentSummary[];
}

export interface WorkspaceSummaryInput {
  id: string;
  workspaceDirectory: string;
  status?: string;
}

export interface AgentSummaryInput {
  id: string;
  cwd: string;
  workspaceId?: string;
  status: string;
  title?: string | null;
  provider?: string;
  turn?: string;
  model?: string | null;
}

export interface TransformWorktreesOptions {
  rawWorktrees: PaseoWorktreeRaw[];
  currentCwd: string;
  workspaces?: Iterable<WorkspaceSummaryInput>;
  agents?: Iterable<AgentSummaryInput>;
}

export function normalizePath(pathStr?: string | null): string {
  if (!pathStr) return "";
  return pathStr.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

/**
 * Transforms raw worktree payloads from the daemon into enriched WorktreeFleetItem models
 * by cross-referencing active workspaces and agents.
 */
export function transformWorktrees(options: TransformWorktreesOptions): WorktreeFleetItem[] {
  const normalizedCurrent = normalizePath(options.currentCwd);
  const workspacesList = options.workspaces ? Array.from(options.workspaces) : [];
  const agentsList = options.agents ? Array.from(options.agents) : [];

  return options.rawWorktrees.map((raw) => {
    const normalizedPath = normalizePath(raw.worktreePath);
    const isCurrent = normalizedPath.length > 0 && normalizedPath === normalizedCurrent;

    // Match workspace by directory path
    const matchingWorkspace = workspacesList.find(
      (w) => normalizePath(w.workspaceDirectory) === normalizedPath,
    );
    const workspaceId = matchingWorkspace?.id;

    // Match agents by workspace ID or cwd path
    const matchingAgents = agentsList.filter((a) => {
      if (workspaceId && a.workspaceId === workspaceId) return true;
      return normalizePath(a.cwd) === normalizedPath;
    });

    const agentSummaries: WorktreeAgentSummary[] = matchingAgents.map((a) => ({
      id: a.id,
      title: a.title ?? null,
      status: a.status,
      provider: a.provider,
      turn: a.turn,
      model: a.model ?? null,
    }));

    // Status resolution
    let status: WorktreeFleetStatus = "ready";
    const hasRunningAgent = agentSummaries.some(
      (a) => a.status === "running" || (a.turn && a.turn !== "idle"),
    );

    if (hasRunningAgent) {
      status = "running";
    } else if (agentSummaries.length > 0) {
      status = "idle";
    } else if (isCurrent) {
      status = "active";
    } else if (matchingWorkspace?.status === "error") {
      status = "error";
    }

    // A named branch wins; a detached head falls back to the short commit hash, and a
    // worktree with neither is detached.
    let branchName = "detached";
    if (raw.branchName && raw.branchName.length > 0) {
      branchName = raw.branchName;
    } else if (raw.head) {
      branchName = raw.head.slice(0, 7);
    }

    return {
      worktreePath: raw.worktreePath,
      branchName,
      createdAt: raw.createdAt,
      head: raw.head ?? null,
      status,
      isCurrent,
      workspaceId,
      agents: agentSummaries,
    };
  });
}
