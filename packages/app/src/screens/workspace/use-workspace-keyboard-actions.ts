import { useCallback, type MutableRefObject } from "react";
import { useKeyboardActionHandler } from "@/hooks/use-keyboard-action-handler";
import { buildWorkspaceKeyboardHandlerId } from "@/keyboard/handler-id";
import type {
  KeyboardActionDefinition,
  WorkspacePanelTarget,
} from "@/keyboard/keyboard-action-dispatcher";
import { openExplorerSidebarView, isExplorerSidebarOpen } from "@/workspace-tabs/explorer-sidebar";
import { openWorkspacePullRequest } from "@/workspace-tabs/open-supporting-view";
import type { PullRequestOpenLocation } from "@/hooks/use-settings";
import { openWorkspaceTargetBeside } from "@/workspace-tabs/open-beside";
import { findAdjacentPane } from "@/utils/split-navigation";
import { toggleDesktopSidebarsWithCheckoutIntent } from "@/utils/desktop-sidebar-toggle";
import { selectIsAgentListOpen, usePanelStore } from "@/stores/panel-store";
import {
  FOCUSED_PANE_PLACEMENT,
  type WorkspaceLayout,
  type WorkspaceTabPlacement,
} from "@/stores/workspace-layout-store";
import type { WorkspaceTabTarget } from "@/workspace-tabs/model";
import type { WorkspaceTabDescriptor } from "./workspace-tabs-types";
import type { WorkspacePaneState } from "./workspace-pane-state";

type PaneDirection = "left" | "right" | "up" | "down";

function parsePaneDirection(actionId: string): PaneDirection | null {
  const direction = actionId.split(".").pop();
  if (direction === "left" || direction === "right" || direction === "up" || direction === "down") {
    return direction;
  }
  return null;
}

function resolveCommandCenterPanelTarget(target: WorkspacePanelTarget): WorkspaceTabTarget {
  switch (target) {
    case "changes":
      return { kind: "changes_tree" };
    case "files":
      return { kind: "files" };
    case "pull-request":
      return { kind: "pull_request" };
  }
}

export interface UseWorkspaceKeyboardActionsProps {
  workspaceActionsEnabled: boolean;
  normalizedServerId: string;
  normalizedWorkspaceId: string;
  persistenceKey: string | null;
  isMobile: boolean;
  pullRequestOpenLocation: PullRequestOpenLocation;
  openWorkspaceTabFocused: (
    workspaceKey: string,
    target: WorkspaceTabTarget,
    placement?: WorkspaceTabPlacement,
  ) => string | null;
  focusedPaneTabState: WorkspacePaneState;
  activeTab: { descriptor: WorkspaceTabDescriptor } | null;
  activeTabId: string | null;
  handleRenameTab: (tab: WorkspaceTabDescriptor) => void;
  handleReloadAgent: (agentId: string) => Promise<void> | void;
  handleCopyResumeCommand: (agentId: string) => Promise<void> | void;
  handleCopyAgentId: (agentId: string) => Promise<void> | void;
  handleCopyTerminalId: (terminalId: string) => Promise<void> | void;
  handleCopyFilePath: (path: string) => Promise<void> | void;
  handleCloseTabsToLeft: (tabId: string) => Promise<void> | void;
  handleCloseTabsToRight: (tabId: string) => Promise<void> | void;
  handleCloseOtherTabs: (tabId: string) => Promise<void> | void;
  handleCreateDraftTab: (input?: { paneId?: string }) => void;
  handleCreateTerminal: (input?: { paneId?: string }) => void;
  handleCreateBrowserTab: (input?: { paneId?: string }) => void;
  handleCreateNewTab: (input?: { paneId?: string }) => void;
  handleCloseTabById: (tabId: string) => Promise<void> | void;
  navigateToTabId: (tabId: string) => void;
  tabs: WorkspaceTabDescriptor[];
  isGitCheckout: boolean;
  handleToggleExplorerSidebar: () => void;
  toggleFocusMode: () => void;
  workspaceLayout: WorkspaceLayout | null;
  handleCreateEmptySplit: (input: {
    targetPaneId: string;
    position: "left" | "right" | "top" | "bottom";
  }) => void;
  focusWorkspacePane: (workspaceKey: string, paneId: string) => void;
  moveWorkspaceTabToPane: (workspaceKey: string, tabId: string, toPaneId: string) => void;
  paneFocusSuppressedRef: MutableRefObject<boolean>;
  handleClosePane: (paneId: string) => Promise<void> | void;
  showWorkspaceSetup: boolean;
  handleOpenSetupTab: () => void;
}

export function useWorkspaceKeyboardActions({
  workspaceActionsEnabled,
  normalizedServerId,
  normalizedWorkspaceId,
  persistenceKey,
  isMobile,
  pullRequestOpenLocation,
  openWorkspaceTabFocused,
  focusedPaneTabState,
  activeTab,
  activeTabId,
  handleRenameTab,
  handleReloadAgent,
  handleCopyResumeCommand,
  handleCopyAgentId,
  handleCopyTerminalId,
  handleCopyFilePath,
  handleCloseTabsToLeft,
  handleCloseTabsToRight,
  handleCloseOtherTabs,
  handleCreateDraftTab,
  handleCreateTerminal,
  handleCreateBrowserTab,
  handleCreateNewTab,
  handleCloseTabById,
  navigateToTabId,
  tabs,
  isGitCheckout,
  handleToggleExplorerSidebar,
  toggleFocusMode,
  workspaceLayout,
  handleCreateEmptySplit,
  focusWorkspacePane,
  moveWorkspaceTabToPane,
  paneFocusSuppressedRef,
  handleClosePane,
  showWorkspaceSetup,
  handleOpenSetupTab,
}: UseWorkspaceKeyboardActionsProps) {
  const handleWorkspacePanelOpenAction = useCallback(
    (action: KeyboardActionDefinition): boolean => {
      if (action.id !== "workspace.tab.open") return false;
      if (!persistenceKey) return true;

      const target = resolveCommandCenterPanelTarget(action.target);
      if (action.placement === "supporting") {
        if (action.target === "files" || action.target === "changes") {
          openExplorerSidebarView({
            isCompact: isMobile,
            workspaceKey: persistenceKey,
            view: action.target === "files" ? "files" : "changes",
          });
          return true;
        }
        if (action.target === "pull-request") {
          openWorkspacePullRequest({
            isCompact: isMobile,
            workspaceKey: persistenceKey,
            destination: pullRequestOpenLocation,
          });
          return true;
        }
        openWorkspaceTabFocused(persistenceKey, target, FOCUSED_PANE_PLACEMENT);
        return true;
      }
      if (action.placement === "side-pane") {
        openWorkspaceTargetBeside({
          workspaceKey: persistenceKey,
          target,
        });
        return true;
      }
      const focusedPaneId = focusedPaneTabState.pane?.id;
      openWorkspaceTabFocused(
        persistenceKey,
        target,
        focusedPaneId ? { mode: "pane", paneId: focusedPaneId } : undefined,
      );
      return true;
    },
    [
      focusedPaneTabState.pane?.id,
      isMobile,
      openWorkspaceTabFocused,
      persistenceKey,
      pullRequestOpenLocation,
    ],
  );

  const handleWorkspaceCurrentTabMetadataAction = useCallback(
    (action: KeyboardActionDefinition): boolean => {
      const descriptor = activeTab?.descriptor;
      switch (action.id) {
        case "workspace.tab.rename-current":
          if (descriptor) handleRenameTab(descriptor);
          return true;
        case "workspace.tab.reload-current":
          if (descriptor?.target.kind === "agent")
            void handleReloadAgent(descriptor.target.agentId);
          return true;
        case "workspace.tab.copy-resume-command":
          if (descriptor?.target.kind === "agent") {
            void handleCopyResumeCommand(descriptor.target.agentId);
          }
          return true;
        case "workspace.tab.copy-id":
          if (descriptor?.target.kind === "agent")
            void handleCopyAgentId(descriptor.target.agentId);
          if (descriptor?.target.kind === "terminal") {
            void handleCopyTerminalId(descriptor.target.terminalId);
          }
          return true;
        case "workspace.tab.copy-file-path":
          if (descriptor?.target.kind === "file") void handleCopyFilePath(descriptor.target.path);
          return true;
        default:
          return false;
      }
    },
    [
      activeTab,
      handleCopyAgentId,
      handleCopyFilePath,
      handleCopyResumeCommand,
      handleCopyTerminalId,
      handleReloadAgent,
      handleRenameTab,
    ],
  );

  const handleWorkspaceCurrentTabCloseAction = useCallback(
    (action: KeyboardActionDefinition): boolean => {
      if (!activeTabId) return true;
      if (action.id === "workspace.tab.close-left") {
        void handleCloseTabsToLeft(activeTabId);
        return true;
      }
      if (action.id === "workspace.tab.close-right") {
        void handleCloseTabsToRight(activeTabId);
        return true;
      }
      if (action.id === "workspace.tab.close-others") {
        void handleCloseOtherTabs(activeTabId);
        return true;
      }
      return false;
    },
    [activeTabId, handleCloseOtherTabs, handleCloseTabsToLeft, handleCloseTabsToRight],
  );

  const handleWorkspaceTabAction = useCallback(
    (action: KeyboardActionDefinition): boolean => {
      switch (action.id) {
        case "workspace.agent.new":
          handleCreateDraftTab();
          return true;
        case "workspace.terminal.new":
          handleCreateTerminal();
          return true;
        case "workspace.browser.new":
          handleCreateBrowserTab();
          return true;
        case "workspace.tab.menu.open":
          handleCreateNewTab({ paneId: focusedPaneTabState.pane?.id });
          return true;
        case "workspace.tab.close-current":
          if (activeTabId) {
            void handleCloseTabById(activeTabId);
          }
          return true;
        case "workspace.tab.navigate-index": {
          const next = tabs[action.index - 1] ?? null;
          if (next?.tabId) {
            navigateToTabId(next.tabId);
          }
          return true;
        }
        case "workspace.tab.navigate-relative": {
          if (tabs.length > 0) {
            const currentIndex = tabs.findIndex((tab) => tab.tabId === activeTabId);
            const fromIndex = currentIndex >= 0 ? currentIndex : 0;
            const nextIndex = (fromIndex + action.delta + tabs.length) % tabs.length;
            const next = tabs[nextIndex] ?? null;
            if (next?.tabId) {
              navigateToTabId(next.tabId);
            }
          }
          return true;
        }
        default:
          return false;
      }
    },
    [
      activeTabId,
      handleCloseTabById,
      handleCreateDraftTab,
      handleCreateBrowserTab,
      handleCreateNewTab,
      handleCreateTerminal,
      focusedPaneTabState.pane?.id,
      navigateToTabId,
      tabs,
    ],
  );

  const handleWorkspaceDirectTargetAction = useCallback(
    (action: KeyboardActionDefinition): boolean => {
      const paneId = focusedPaneTabState.pane?.id;
      switch (action.id) {
        case "workspace.tab.target.agent":
          handleCreateDraftTab({ paneId });
          return true;
        case "workspace.tab.target.browser":
          handleCreateBrowserTab({ paneId });
          return true;
        case "workspace.tab.target.changes":
          if (persistenceKey && isGitCheckout) {
            openExplorerSidebarView({
              isCompact: isMobile,
              workspaceKey: persistenceKey,
              view: "changes",
            });
          }
          return true;
        case "workspace.tab.target.files":
          if (persistenceKey) {
            openExplorerSidebarView({
              isCompact: isMobile,
              workspaceKey: persistenceKey,
              view: "files",
            });
          }
          return true;
        default:
          return false;
      }
    },
    [
      focusedPaneTabState.pane?.id,
      handleCreateBrowserTab,
      handleCreateDraftTab,
      isGitCheckout,
      isMobile,
      persistenceKey,
    ],
  );

  const handleWorkspaceSidebarAction = useCallback(
    (action: KeyboardActionDefinition): boolean => {
      if (action.id === "sidebar.toggle.right") {
        handleToggleExplorerSidebar();
        return true;
      }
      if (action.id !== "sidebar.toggle.both") {
        return false;
      }
      const panel = usePanelStore.getState();
      toggleDesktopSidebarsWithCheckoutIntent({
        isAgentListOpen: selectIsAgentListOpen(panel, { isCompact: isMobile }),
        isExplorerOpen: isExplorerSidebarOpen({
          isCompact: isMobile,
          workspaceKey: persistenceKey,
        }),
        openAgentList: () => panel.openAgentListForLayout({ isCompact: isMobile }),
        closeAgentList: () => panel.closeAgentListForLayout({ isCompact: isMobile }),
        toggleExplorer: handleToggleExplorerSidebar,
      });
      return true;
    },
    [handleToggleExplorerSidebar, isMobile, persistenceKey],
  );

  const handleWorkspacePaneAction = useCallback(
    (action: KeyboardActionDefinition): boolean => {
      if (action.id === "workspace.focus.toggle") {
        toggleFocusMode();
        return true;
      }

      if (!persistenceKey || !workspaceLayout) {
        return true;
      }

      const focusedPane = focusedPaneTabState.pane;
      if (!focusedPane) {
        return true;
      }

      if (action.id === "workspace.pane.split.right") {
        handleCreateEmptySplit({
          targetPaneId: focusedPane.id,
          position: "right",
        });
        return true;
      }

      if (action.id === "workspace.pane.split.down") {
        handleCreateEmptySplit({
          targetPaneId: focusedPane.id,
          position: "bottom",
        });
        return true;
      }

      if (action.id.startsWith("workspace.pane.focus.")) {
        const direction = parsePaneDirection(action.id);
        if (direction) {
          const adjacentPaneId = findAdjacentPane(workspaceLayout.root, focusedPane.id, direction);
          if (adjacentPaneId) {
            focusWorkspacePane(persistenceKey, adjacentPaneId);
          }
        }
        return true;
      }

      if (action.id.startsWith("workspace.pane.move-tab.")) {
        const direction = parsePaneDirection(action.id);
        if (direction) {
          const activePaneTabId = focusedPaneTabState.activeTabId;
          const adjacentPaneId = findAdjacentPane(workspaceLayout.root, focusedPane.id, direction);
          if (activePaneTabId && adjacentPaneId) {
            paneFocusSuppressedRef.current = true;
            moveWorkspaceTabToPane(persistenceKey, activePaneTabId, adjacentPaneId);
            requestAnimationFrame(() => {
              paneFocusSuppressedRef.current = false;
            });
          }
        }
        return true;
      }

      if (action.id === "workspace.pane.close") {
        void handleClosePane(focusedPane.id);
        return true;
      }

      return false;
    },
    [
      focusWorkspacePane,
      handleClosePane,
      handleCreateEmptySplit,
      moveWorkspaceTabToPane,
      paneFocusSuppressedRef,
      persistenceKey,
      focusedPaneTabState.activeTabId,
      focusedPaneTabState.pane,
      toggleFocusMode,
      workspaceLayout,
    ],
  );

  useKeyboardActionHandler({
    handlerId: buildWorkspaceKeyboardHandlerId({
      name: "workspace-tab-actions",
      serverId: normalizedServerId,
      workspaceId: normalizedWorkspaceId,
    }),
    actions: [
      "workspace.agent.new",
      "workspace.tab.close-current",
      "workspace.tab.navigate-index",
      "workspace.tab.navigate-relative",
      "workspace.terminal.new",
      "workspace.browser.new",
      "workspace.tab.menu.open",
    ] as const,
    enabled: workspaceActionsEnabled,
    priority: 100,
    isActive: () => true,
    handle: handleWorkspaceTabAction,
  });

  useKeyboardActionHandler({
    handlerId: buildWorkspaceKeyboardHandlerId({
      name: "workspace-direct-target-actions",
      serverId: normalizedServerId,
      workspaceId: normalizedWorkspaceId,
    }),
    actions: [
      "workspace.tab.target.agent",
      "workspace.tab.target.browser",
      "workspace.tab.target.changes",
      "workspace.tab.target.files",
    ] as const,
    enabled: workspaceActionsEnabled,
    priority: 100,
    isActive: () => true,
    handle: handleWorkspaceDirectTargetAction,
  });

  useKeyboardActionHandler({
    handlerId: buildWorkspaceKeyboardHandlerId({
      name: "workspace-panel-open-actions",
      serverId: normalizedServerId,
      workspaceId: normalizedWorkspaceId,
    }),
    actions: ["workspace.tab.open"] as const,
    enabled: workspaceActionsEnabled,
    priority: 100,
    isActive: () => true,
    handle: handleWorkspacePanelOpenAction,
  });

  useKeyboardActionHandler({
    handlerId: buildWorkspaceKeyboardHandlerId({
      name: "workspace-current-tab-metadata-actions",
      serverId: normalizedServerId,
      workspaceId: normalizedWorkspaceId,
    }),
    actions: [
      "workspace.tab.rename-current",
      "workspace.tab.reload-current",
      "workspace.tab.copy-resume-command",
      "workspace.tab.copy-id",
      "workspace.tab.copy-file-path",
    ] as const,
    enabled: workspaceActionsEnabled,
    priority: 100,
    isActive: () => true,
    handle: handleWorkspaceCurrentTabMetadataAction,
  });

  useKeyboardActionHandler({
    handlerId: buildWorkspaceKeyboardHandlerId({
      name: "workspace-current-tab-close-actions",
      serverId: normalizedServerId,
      workspaceId: normalizedWorkspaceId,
    }),
    actions: [
      "workspace.tab.close-left",
      "workspace.tab.close-right",
      "workspace.tab.close-others",
    ] as const,
    enabled: workspaceActionsEnabled,
    priority: 100,
    isActive: () => true,
    handle: handleWorkspaceCurrentTabCloseAction,
  });

  useKeyboardActionHandler({
    handlerId: buildWorkspaceKeyboardHandlerId({
      name: "workspace-pane-actions",
      serverId: normalizedServerId,
      workspaceId: normalizedWorkspaceId,
    }),
    actions: [
      "workspace.pane.split.right",
      "workspace.pane.split.down",
      "workspace.pane.focus.left",
      "workspace.pane.focus.right",
      "workspace.pane.focus.up",
      "workspace.pane.focus.down",
      "workspace.pane.move-tab.left",
      "workspace.pane.move-tab.right",
      "workspace.pane.move-tab.up",
      "workspace.pane.move-tab.down",
      "workspace.pane.close",
      "workspace.focus.toggle",
    ] as const,
    enabled: workspaceActionsEnabled,
    priority: 100,
    isActive: () => true,
    handle: handleWorkspacePaneAction,
  });

  useKeyboardActionHandler({
    handlerId: buildWorkspaceKeyboardHandlerId({
      name: "workspace-sidebar-actions",
      serverId: normalizedServerId,
      workspaceId: normalizedWorkspaceId,
    }),
    actions: ["sidebar.toggle.right", "sidebar.toggle.both"] as const,
    enabled: workspaceActionsEnabled,
    priority: 100,
    isActive: () => true,
    handle: handleWorkspaceSidebarAction,
  });

  useKeyboardActionHandler({
    handlerId: `workspace-setup-show:${normalizedServerId}:${normalizedWorkspaceId}`,
    actions: ["workspace.setup.show"] as const,
    enabled: workspaceActionsEnabled,
    priority: 100,
    isActive: () => showWorkspaceSetup,
    handle: () => {
      handleOpenSetupTab();
      return true;
    },
  });
}
