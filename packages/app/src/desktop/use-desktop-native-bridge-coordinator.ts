import { useEffect, useRef } from "react";
import { useSessionStore } from "@/stores/session-store";
import {
  showNativeTaskCompleteNotification,
  updateNativeTrayActiveAgents,
} from "@/desktop/native-desktop-bridge";

/**
 * Global coordinator that tracks active running agents across all connected servers
 * and updates:
 * 1. The native OS system tray icon/tooltip with active count
 * 2. Real Windows OS notifications when an agent's turn completes
 */
export function useDesktopNativeBridgeCoordinator(): void {
  const previousTurnsRef = useRef<Map<string, "open" | "idle">>(new Map());

  useEffect(() => {
    // Subscribe to session store state changes
    const unsubscribe = useSessionStore.subscribe((state) => {
      let activeCount = 0;
      const currentTurns = new Map<string, "open" | "idle">();

      for (const serverSession of Object.values(state.sessions)) {
        if (!serverSession) continue;
        for (const [agentId, agent] of serverSession.agents.entries()) {
          const phase = agent.turn?.phase ?? "idle";
          currentTurns.set(agentId, phase);

          if (phase === "open") {
            activeCount += 1;
          }

          // Check if agent just transitioned from open -> idle (task complete)
          const previousPhase = previousTurnsRef.current.get(agentId);
          if (previousPhase === "open" && phase === "idle") {
            void showNativeTaskCompleteNotification({
              taskName: agent.title || `Agent ${agentId.slice(0, 8)}`,
              status: "completed",
              details: "Assistant finished executing task successfully.",
            });
          }
        }
      }

      previousTurnsRef.current = currentTurns;

      // Update native system tray active agents count
      void updateNativeTrayActiveAgents(activeCount);
    });

    return () => {
      unsubscribe();
    };
  }, []);
}
