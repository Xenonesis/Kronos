import { useEffect, useState, useMemo, useCallback } from "react";
import { Platform } from "react-native";
import { useRouter } from "expo-router";
import type { PaletteItem } from "./types";
import { useSessionStore } from "@/stores/session-store";
import { useHosts } from "@/runtime/host-runtime";

export function useCommandPalette() {
  const [isOpen, setIsOpen] = useState(false);
  const router = useRouter();
  const hosts = useHosts();
  const sessions = useSessionStore((state) => state.sessions);

  // Global keyboard listener for Ctrl+K / Cmd+K
  useEffect(() => {
    if (Platform.OS !== "web") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setIsOpen((prev) => !prev);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const open = useCallback(() => setIsOpen(true), []);
  const close = useCallback(() => setIsOpen(false), []);
  const toggle = useCallback(() => setIsOpen((prev) => !prev), []);

  // Built-in actions and active agent items
  const items = useMemo<PaletteItem[]>(() => {
    const list: PaletteItem[] = [
      {
        id: "action-new-agent",
        title: "New Agent Chat",
        subtitle: "Start a fresh agent session in current workspace",
        category: "actions",
        shortcut: "Ctrl+N",
        keywords: ["new", "agent", "chat", "create"],
        run: () => {
          router.push("/new");
        },
      },
      {
        id: "action-open-browser",
        title: "Open Browser Pane",
        subtitle: "Launch internal browser for localhost or web preview",
        category: "navigation",
        keywords: ["browser", "web", "preview", "html", "localhost"],
        run: () => {
          // Navigates or triggers browser pane
        },
      },
      {
        id: "action-settings",
        title: "Open Settings",
        subtitle: "Configure models, providers, plugins, and keys",
        category: "actions",
        shortcut: "Ctrl+,",
        keywords: ["settings", "preferences", "config", "keys"],
        run: () => {
          router.push("/settings");
        },
      },
      {
        id: "action-schedules",
        title: "View Background Schedules",
        subtitle: "Manage recurring tasks and automations",
        category: "navigation",
        keywords: ["schedules", "cron", "automation", "background"],
        run: () => {
          router.push("/schedules");
        },
      },
      {
        id: "action-omp-sessions",
        title: "OMP Sessions Explorer",
        subtitle: "Inspect past OMP agent history, tool calls, and diffs",
        category: "navigation",
        shortcut: "Ctrl+Shift+O",
        keywords: ["omp", "sessions", "history", "timeline", "diff", "explorer"],
        run: () => {
          router.push("/sessions");
        },
      },
    ];

    // Collect active sessions / agents across hosts
    for (const [serverId, sessionData] of Object.entries(sessions)) {
      const hostLabel = hosts.find((h) => h.serverId === serverId)?.label ?? serverId;
      if (sessionData.agents) {
        for (const [agentId, agent] of sessionData.agents.entries()) {
          list.push({
            id: `agent-${agentId}`,
            title: agent.title || `Agent ${agentId.slice(0, 8)}`,
            subtitle: `${hostLabel} • ${agent.status || "active"}`,
            category: "agents",
            keywords: ["agent", agentId, agent.title || ""],
            run: () => {
              router.push(`/h/${serverId}/agent/${agentId}`);
            },
          });
        }
      }
    }

    return list;
  }, [hosts, router, sessions]);

  return {
    isOpen,
    open,
    close,
    toggle,
    items,
  };
}
