import { listenToDesktopEvent, type DesktopEventUnlisten } from "@/desktop/electron/events";
import { invokeDesktopCommand } from "@/desktop/electron/invoke";
import { getDesktopHost } from "@/desktop/host";

/**
 * Updates the native OS system tray icon with the number of actively running agents.
 */
export async function updateNativeTrayActiveAgents(count: number): Promise<void> {
  const host = getDesktopHost();
  if (!host?.invoke) {
    return;
  }
  try {
    await invokeDesktopCommand("update_active_agents", { count });
  } catch {
    // Graceful no-op in non-Tauri / pure web environments
  }
}

/**
 * Triggers a native OS notification (Windows Action Center banner + sound).
 */
export async function showNativeTaskCompleteNotification(input: {
  taskName: string;
  status?: string;
  details?: string;
}): Promise<void> {
  const host = getDesktopHost();
  if (!host?.invoke) {
    return;
  }
  try {
    await invokeDesktopCommand("notify_task_complete", {
      taskName: input.taskName,
      status: input.status ?? "completed",
      details: input.details ?? null,
    });
  } catch {
    // Graceful no-op if notification permissions not granted
  }
}

export interface QuickTogglePayload {
  timestamp: number;
}

/**
 * Listens for global hotkey quick-toggle events emitted by the native desktop shell.
 */
export async function onQuickToggle(
  callback: (payload: QuickTogglePayload) => void,
): Promise<DesktopEventUnlisten> {
  return listenToDesktopEvent<QuickTogglePayload>("kronos://quick-toggle", callback);
}

export interface WorkspaceSearchResult {
  path: string;
  score: number;
}

/**
 * Searches files in workspace using Tantivy in-memory lexical search engine.
 */
export async function searchWorkspaceFiles(
  query: string,
  root?: string,
  limit: number = 20,
): Promise<WorkspaceSearchResult[]> {
  const host = getDesktopHost();
  if (!host?.invoke) {
    return [];
  }
  try {
    return await invokeDesktopCommand<WorkspaceSearchResult[]>("workspace_search", {
      query,
      root: root ?? ".",
      limit,
    });
  } catch {
    return [];
  }
}

/**
 * Applies or discards a specific diff hunk to a file on disk.
 */
export async function applyDiffHunk(
  filePath: string,
  hunk: unknown,
  reverse: boolean = false,
): Promise<string | null> {
  const host = getDesktopHost();
  if (!host?.invoke) {
    return null;
  }
  try {
    return await invokeDesktopCommand<string>("apply_diff_hunk", {
      filePath,
      hunk,
      reverse,
    });
  } catch {
    return null;
  }
}

/**
 * Creates a shadow git checkpoint before agent turns modify files.
 */
export async function createTurnCheckpoint(
  repoPath: string,
  turnId: string,
): Promise<string | null> {
  const host = getDesktopHost();
  if (!host?.invoke) {
    return null;
  }
  try {
    return await invokeDesktopCommand<string>("create_turn_checkpoint", {
      repoPath,
      turnId,
    });
  } catch {
    return null;
  }
}

/**
 * Restores workspace state to a prior turn checkpoint.
 */
export async function restoreTurnCheckpoint(repoPath: string, turnId: string): Promise<boolean> {
  const host = getDesktopHost();
  if (!host?.invoke) {
    return false;
  }
  try {
    await invokeDesktopCommand("restore_turn_checkpoint", {
      repoPath,
      turnId,
    });
    return true;
  } catch {
    return false;
  }
}
