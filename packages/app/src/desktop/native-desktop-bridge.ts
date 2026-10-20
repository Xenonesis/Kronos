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
