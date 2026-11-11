import { useCallback, type Dispatch, type SetStateAction } from "react";
import * as Clipboard from "expo-clipboard";
import type { TFunction } from "i18next";
import type { DaemonClient } from "@getpaseo/client/internal/daemon-client";
import { useSessionStore } from "@/stores/session-store";
import { getHostRuntimeStore } from "@/runtime/host-runtime";
import { confirmDialog } from "@/utils/confirm-dialog";
import { buildProviderCommand } from "@/utils/provider-command-templates";
import { getOrCreateClientId } from "@/utils/client-id";
import { getOpenAgentTabLabel } from "@getpaseo/protocol/agent-labels";
import { resolveCloseAgentTabPolicy } from "@/subagents";
import { planTimelineTailFetch } from "@/timeline/timeline-sync-plan";
import { getPanelInstanceAttributes } from "@/panels/panel-instance-attributes";
import {
  buildBulkCloseConfirmationMessage,
  type BulkCloseConfirmationLabels,
  classifyBulkClosableTabs,
  closeBulkWorkspaceTabs,
} from "@/screens/workspace/workspace-bulk-close";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import { normalizeWorkspaceTabTarget } from "@/workspace-tabs/identity";
import type { WorkspaceTabTarget } from "@/workspace-tabs/model";
import {
  canDismissPaneInLayout,
  findPaneById,
  type WorkspaceLayout,
  type WorkspaceTabPlacement,
} from "@/stores/workspace-layout-store";
import type { useToast } from "@/contexts/toast-context";

export interface UseWorkspaceTabActionsInput {
  t: TFunction;
  toast: ReturnType<typeof useToast>;
  normalizedServerId: string;
  normalizedWorkspaceId: string;
  persistenceKey: string | null;
  client: DaemonClient | null;
  isConnected: boolean;
  workspaceDirectory: string | null;
  currentBranchName: string | null;
  closeTab: (tabId: string, run: () => Promise<void>) => Promise<void>;
  closeWorkspaceTabWithCleanup: (input: {
    tabId: string;
    target?: WorkspaceTabTarget | null;
  }) => void;
  closeWorkspaceTab: (workspaceKey: string, tabId: string) => void;
  closeWorkspacePane: (workspaceKey: string, paneId: string) => void;
  removeTerminalFromCache: (terminalId: string) => void;
  killTerminalAsync: (terminalId: string) => Promise<unknown>;
  invalidateTerminals: () => void;
  setHoveredCloseTabKey: Dispatch<SetStateAction<string | null>>;
  archiveAgent: (input: { serverId: string; agentId: string }) => Promise<unknown>;
  allTabDescriptorsById: Map<string, WorkspaceTabDescriptor>;
  sheets: readonly { tab: { tabId: string; target: WorkspaceTabTarget } }[];
  tabs: WorkspaceTabDescriptor[];
  workspaceLayout: WorkspaceLayout | null;
  explorerSidebarPaneId: string | null;
  bulkCloseConfirmationLabels: BulkCloseConfirmationLabels;
  openWorkspaceTabFocused: (
    workspaceKey: string,
    target: WorkspaceTabTarget,
    placement?: WorkspaceTabPlacement,
  ) => void | string | null;
  focusedPanePlacement?: WorkspaceTabPlacement;
}

export function useWorkspaceTabActions({
  t,
  toast,
  normalizedServerId,
  normalizedWorkspaceId,
  persistenceKey,
  client,
  isConnected,
  workspaceDirectory,
  currentBranchName,
  closeTab,
  closeWorkspaceTabWithCleanup,
  closeWorkspaceTab,
  closeWorkspacePane,
  removeTerminalFromCache,
  killTerminalAsync,
  invalidateTerminals,
  setHoveredCloseTabKey,
  archiveAgent,
  allTabDescriptorsById,
  sheets,
  tabs,
  workspaceLayout,
  explorerSidebarPaneId,
  bulkCloseConfirmationLabels,
  openWorkspaceTabFocused,
  focusedPanePlacement,
}: UseWorkspaceTabActionsInput) {
  const handleCloseTerminalTab = useCallback(
    async (input: { tabId: string; terminalId: string }) => {
      const { tabId, terminalId } = input;
      await closeTab(tabId, async () => {
        const confirmed = await confirmDialog({
          title: t("workspace.tabs.confirmations.closeTerminalTitle"),
          message: t("workspace.tabs.confirmations.closeTerminalMessage"),
          confirmLabel: t("workspace.tabs.confirmations.close"),
          cancelLabel: t("workspace.tabs.confirmations.cancel"),
          destructive: true,
        });
        if (!confirmed) {
          return;
        }

        removeTerminalFromCache(terminalId);
        setHoveredCloseTabKey((current) => (current === tabId ? null : current));
        if (persistenceKey) {
          closeWorkspaceTabWithCleanup({
            tabId,
            target: { kind: "terminal", terminalId },
          });
        }

        void killTerminalAsync(terminalId).catch(invalidateTerminals);
      });
    },
    [
      closeTab,
      closeWorkspaceTabWithCleanup,
      invalidateTerminals,
      killTerminalAsync,
      persistenceKey,
      removeTerminalFromCache,
      setHoveredCloseTabKey,
      t,
    ],
  );

  const handleCloseAgentTab = useCallback(
    async (input: { tabId: string; agentId: string }) => {
      const { tabId, agentId } = input;
      await closeTab(tabId, async () => {
        if (!normalizedServerId) {
          return;
        }

        const agent =
          useSessionStore.getState().sessions[normalizedServerId]?.agents?.get(agentId) ?? null;
        let closePolicy = resolveCloseAgentTabPolicy(agent);
        const isRunning = agent?.status === "running";

        if (isRunning && closePolicy.kind === "archive-on-close") {
          const confirmed = await confirmDialog({
            title: t("workspace.tabs.confirmations.archiveRunningAgentTitle"),
            message: t("workspace.tabs.confirmations.archiveRunningAgentMessage"),
            confirmLabel: t("workspace.tabs.confirmations.archive"),
            cancelLabel: t("workspace.tabs.confirmations.cancel"),
            destructive: true,
          });
          if (!confirmed) {
            return;
          }
        }

        if (closePolicy.kind === "layout-only") {
          const sessionClient = useSessionStore.getState().sessions[normalizedServerId]?.client;
          if (!sessionClient) {
            toast.error(t("common.errors.daemonClientUnavailable"));
            return;
          }
          try {
            const clientId = await getOrCreateClientId();
            await sessionClient.updateAgent(agentId, {
              labels: { [getOpenAgentTabLabel(clientId)]: "false" },
            });
            const latestAgent =
              useSessionStore.getState().sessions[normalizedServerId]?.agents?.get(agentId) ?? null;
            closePolicy = resolveCloseAgentTabPolicy(latestAgent);
          } catch (error) {
            console.error("[WorkspaceScreen] Failed to close subagent tab", {
              error,
              agentId,
            });
            toast.error(t("workspace.tabs.toasts.failedToCloseAgent"));
            return;
          }
        }

        setHoveredCloseTabKey((current) => (current === tabId ? null : current));
        if (persistenceKey) {
          closeWorkspaceTabWithCleanup({
            tabId,
            target: { kind: "agent", agentId },
          });
        }

        if (closePolicy.kind === "layout-only") {
          return;
        }

        void archiveAgent({ serverId: normalizedServerId, agentId }).catch(() => {});
      });
    },
    [
      archiveAgent,
      closeTab,
      closeWorkspaceTabWithCleanup,
      normalizedServerId,
      persistenceKey,
      setHoveredCloseTabKey,
      t,
      toast,
    ],
  );

  const handleClosePassiveTab = useCallback(
    function handleClosePassiveTab(input: { tabId: string; target?: WorkspaceTabTarget | null }) {
      setHoveredCloseTabKey((current) => (current === input.tabId ? null : current));
      if (persistenceKey) {
        closeWorkspaceTabWithCleanup({
          tabId: input.tabId,
          target: input.target,
        });
      }
    },
    [closeWorkspaceTabWithCleanup, persistenceKey, setHoveredCloseTabKey],
  );

  const confirmDiscardModifiedTab = useCallback(
    async (tabId: string): Promise<boolean> => {
      const attributes = getPanelInstanceAttributes({
        serverId: normalizedServerId,
        workspaceId: normalizedWorkspaceId,
        tabId,
      });
      if (!attributes.modified) return true;
      const resumePendingSave = attributes.suspendPendingSave?.();
      const confirmed = await confirmDialog({
        title: t("workspace.tabs.confirmations.unsavedTitle"),
        message: t("workspace.tabs.confirmations.unsavedMessage"),
        confirmLabel: t("workspace.tabs.confirmations.closeWithoutSaving"),
        cancelLabel: t("workspace.tabs.confirmations.cancel"),
        destructive: true,
      });
      if (!confirmed) resumePendingSave?.();
      return confirmed;
    },
    [normalizedServerId, normalizedWorkspaceId, t],
  );

  const handleCloseTabById = useCallback(
    async (tabId: string) => {
      if (persistenceKey && sheets.some((sheet) => sheet.tab.tabId === tabId)) {
        if (await confirmDiscardModifiedTab(tabId)) {
          closeWorkspaceTab(persistenceKey, tabId);
        }
        return;
      }
      const tab = allTabDescriptorsById.get(tabId);
      if (!tab) {
        return;
      }
      if (!(await confirmDiscardModifiedTab(tabId))) {
        return;
      }
      if (tab.target.kind === "terminal") {
        await handleCloseTerminalTab({
          tabId,
          terminalId: tab.target.terminalId,
        });
        return;
      }
      if (tab.target.kind === "agent") {
        await handleCloseAgentTab({ tabId, agentId: tab.target.agentId });
        return;
      }
      handleClosePassiveTab({ tabId, target: tab.target });
    },
    [
      allTabDescriptorsById,
      closeWorkspaceTab,
      confirmDiscardModifiedTab,
      handleCloseAgentTab,
      handleClosePassiveTab,
      handleCloseTerminalTab,
      persistenceKey,
      sheets,
    ],
  );

  const handleCopyAgentId = useCallback(
    async (agentId: string) => {
      if (!agentId) return;
      try {
        await Clipboard.setStringAsync(agentId);
        toast.copied(t("workspace.tabs.toasts.agentIdCopiedLabel"));
      } catch {
        toast.error(t("workspace.tabs.toasts.copyFailed"));
      }
    },
    [toast, t],
  );

  const handleCopyTerminalId = useCallback(
    async (terminalId: string) => {
      if (!terminalId) return;
      try {
        await Clipboard.setStringAsync(terminalId);
        toast.copied(t("workspace.tabs.toasts.terminalIdCopiedLabel"));
      } catch {
        toast.error(t("workspace.tabs.toasts.copyFailed"));
      }
    },
    [toast, t],
  );

  const handleCopyFilePath = useCallback(
    async (path: string) => {
      if (!path) return;
      try {
        await Clipboard.setStringAsync(path);
        toast.copied(t("workspace.tabs.toasts.filePathCopiedLabel"));
      } catch {
        toast.error(t("workspace.tabs.toasts.copyFailed"));
      }
    },
    [toast, t],
  );

  const handleCopyResumeCommand = useCallback(
    async (agentId: string) => {
      if (!agentId) return;
      const agent =
        useSessionStore.getState().sessions[normalizedServerId]?.agents?.get(agentId) ?? null;
      const providerSessionId =
        agent?.runtimeInfo?.sessionId ?? agent?.persistence?.sessionId ?? null;
      if (!agent || !providerSessionId) {
        toast.error(t("workspace.tabs.toasts.resumeIdUnavailable"));
        return;
      }

      const command =
        buildProviderCommand({
          provider: agent.provider,
          id: "resume",
          sessionId: providerSessionId,
        }) ?? null;
      if (!command) {
        toast.error(t("workspace.tabs.toasts.resumeCommandUnavailable"));
        return;
      }
      try {
        await Clipboard.setStringAsync(command);
        toast.copied(t("workspace.tabs.toasts.resumeCommandCopiedLabel"));
      } catch {
        toast.error(t("workspace.tabs.toasts.copyFailed"));
      }
    },
    [normalizedServerId, toast, t],
  );

  const handleReloadAgent = useCallback(
    async (agentId: string) => {
      if (!client || !isConnected) {
        toast.error(t("workspace.terminal.hostDisconnected"));
        return;
      }

      toast.show(t("workspace.tabs.toasts.reloadingAgent"), {
        durationMs: null,
      });
      try {
        await client.refreshAgent(agentId);
        const sessionState = useSessionStore.getState().sessions[normalizedServerId];
        const currentCursor = sessionState?.agentTimelineCursor.get(agentId);
        await getHostRuntimeStore().fetchAgentTimeline(normalizedServerId, agentId, {
          ...planTimelineTailFetch(),
          ...(currentCursor
            ? {
                cursor: {
                  epoch: currentCursor.epoch,
                  seq: currentCursor.endSeq,
                },
              }
            : {}),
        });
        toast.show(t("workspace.tabs.toasts.reloadedAgent"), {
          variant: "success",
        });
      } catch (error) {
        toast.error(
          error instanceof Error ? error.message : t("workspace.tabs.toasts.failedToReloadAgent"),
        );
      }
    },
    [client, isConnected, normalizedServerId, toast, t],
  );

  const handleCopyWorkspacePath = useCallback(async () => {
    if (!workspaceDirectory) {
      toast.error(t("workspace.header.toasts.workspacePathUnavailable"));
      return;
    }

    try {
      await Clipboard.setStringAsync(workspaceDirectory);
      toast.copied(t("workspace.header.toasts.workspacePathCopiedLabel"));
    } catch {
      toast.error(t("workspace.tabs.toasts.copyFailed"));
    }
  }, [toast, workspaceDirectory, t]);

  const handleCopyBranchName = useCallback(async () => {
    if (!currentBranchName) {
      toast.error(t("workspace.header.toasts.branchNameUnavailable"));
      return;
    }

    try {
      await Clipboard.setStringAsync(currentBranchName);
      toast.copied(t("workspace.header.toasts.branchNameCopiedLabel"));
    } catch {
      toast.error(t("workspace.tabs.toasts.copyFailed"));
    }
  }, [currentBranchName, toast, t]);

  const handleOpenSetupTab = useCallback(() => {
    if (!persistenceKey) {
      return;
    }
    const target = normalizeWorkspaceTabTarget({
      kind: "setup",
      workspaceId: normalizedWorkspaceId,
    });
    if (!target) {
      return;
    }
    openWorkspaceTabFocused(persistenceKey, target, focusedPanePlacement);
  }, [focusedPanePlacement, normalizedWorkspaceId, openWorkspaceTabFocused, persistenceKey]);

  const handleBulkCloseTabs = useCallback(
    async (input: {
      tabsToClose: WorkspaceTabDescriptor[];
      title: string;
      logLabel: string;
    }): Promise<boolean> => {
      const { tabsToClose, title, logLabel } = input;
      if (tabsToClose.length === 0) {
        return true;
      }

      const groups = classifyBulkClosableTabs(tabsToClose, (agentId) => {
        const agent = useSessionStore.getState().sessions[normalizedServerId]?.agents?.get(agentId);
        return resolveCloseAgentTabPolicy(agent).kind === "layout-only" ? "layout-only" : "archive";
      });
      const modifiedCount = tabsToClose.filter(
        (tab) =>
          getPanelInstanceAttributes({
            serverId: normalizedServerId,
            workspaceId: normalizedWorkspaceId,
            tabId: tab.tabId,
          }).modified,
      ).length;
      const bulkMessage = buildBulkCloseConfirmationMessage(groups, bulkCloseConfirmationLabels);
      const confirmed = await confirmDialog({
        title,
        message:
          modifiedCount > 0
            ? `${bulkMessage}\n\n${t("workspace.tabs.confirmations.bulkUnsaved", { count: modifiedCount })}`
            : bulkMessage,
        confirmLabel: t("workspace.tabs.confirmations.close"),
        cancelLabel: t("workspace.tabs.confirmations.cancel"),
        destructive: true,
      });
      if (!confirmed) {
        return false;
      }

      await closeBulkWorkspaceTabs({
        client,
        groups,
        closeTab,
        closeLayoutOnlyAgent: async (agentId) => {
          if (!client) {
            throw new Error(t("common.errors.daemonClientUnavailable"));
          }
          const clientId = await getOrCreateClientId();
          await client.updateAgent(agentId, {
            labels: { [getOpenAgentTabLabel(clientId)]: "false" },
          });
          const latestAgent =
            useSessionStore.getState().sessions[normalizedServerId]?.agents?.get(agentId) ?? null;
          if (resolveCloseAgentTabPolicy(latestAgent).kind === "archive-on-close") {
            await archiveAgent({ serverId: normalizedServerId, agentId });
          }
        },
        closeWorkspaceTabWithCleanup: (cleanupInput) => {
          if (!persistenceKey) {
            return;
          }
          closeWorkspaceTabWithCleanup(cleanupInput);
        },
        logLabel,
        warn: (message, payload) => {
          console.warn(message, payload);
        },
      });

      const closedKeys = new Set(tabsToClose.map((tab) => tab.key));
      setHoveredCloseTabKey((current) => (current && closedKeys.has(current) ? null : current));
      return true;
    },
    [
      archiveAgent,
      bulkCloseConfirmationLabels,
      client,
      closeTab,
      closeWorkspaceTabWithCleanup,
      normalizedServerId,
      normalizedWorkspaceId,
      persistenceKey,
      setHoveredCloseTabKey,
      t,
    ],
  );

  const handleCloseTabsToLeftInPane = useCallback(
    async (tabId: string, paneTabs: WorkspaceTabDescriptor[]) => {
      const index = paneTabs.findIndex((tab) => tab.tabId === tabId);
      if (index < 0) {
        return;
      }
      await handleBulkCloseTabs({
        tabsToClose: paneTabs.slice(0, index),
        title: t("workspace.tabs.confirmations.closeTabsLeftTitle"),
        logLabel: "to the left",
      });
    },
    [handleBulkCloseTabs, t],
  );

  const handleCloseTabsToLeft = useCallback(
    async (tabId: string) => {
      await handleCloseTabsToLeftInPane(tabId, tabs);
    },
    [handleCloseTabsToLeftInPane, tabs],
  );

  const handleCloseTabsToRightInPane = useCallback(
    async (tabId: string, paneTabs: WorkspaceTabDescriptor[]) => {
      const index = paneTabs.findIndex((tab) => tab.tabId === tabId);
      if (index < 0) {
        return;
      }
      await handleBulkCloseTabs({
        tabsToClose: paneTabs.slice(index + 1),
        title: t("workspace.tabs.confirmations.closeTabsRightTitle"),
        logLabel: "to the right",
      });
    },
    [handleBulkCloseTabs, t],
  );

  const handleCloseTabsToRight = useCallback(
    async (tabId: string) => {
      await handleCloseTabsToRightInPane(tabId, tabs);
    },
    [handleCloseTabsToRightInPane, tabs],
  );

  const handleCloseOtherTabsInPane = useCallback(
    async (tabId: string, paneTabs: WorkspaceTabDescriptor[]) => {
      const tabsToClose = paneTabs.filter((tab) => tab.tabId !== tabId);
      await handleBulkCloseTabs({
        tabsToClose,
        title: t("workspace.tabs.confirmations.closeOtherTabsTitle"),
        logLabel: "from close other tabs",
      });
    },
    [handleBulkCloseTabs, t],
  );

  const handleCloseOtherTabs = useCallback(
    async (tabId: string) => {
      await handleCloseOtherTabsInPane(tabId, tabs);
    },
    [handleCloseOtherTabsInPane, tabs],
  );

  const handleClosePane = useCallback(
    async (paneId: string) => {
      if (!persistenceKey || !workspaceLayout) {
        return;
      }
      const pane = findPaneById(workspaceLayout.root, paneId);
      if (!pane || !canDismissPaneInLayout(workspaceLayout, paneId, explorerSidebarPaneId)) {
        return;
      }
      const tabsToClose = pane.tabIds.flatMap((tabId: string) => {
        const tab = allTabDescriptorsById.get(tabId);
        return tab ? [tab] : [];
      });
      const closed = await handleBulkCloseTabs({
        tabsToClose,
        title: t("workspace.tabs.confirmations.closePaneTitle"),
        logLabel: "from pane close",
      });
      if (!closed) {
        return;
      }
      closeWorkspacePane(persistenceKey, paneId);
    },
    [
      allTabDescriptorsById,
      closeWorkspacePane,
      handleBulkCloseTabs,
      persistenceKey,
      t,
      workspaceLayout,
      explorerSidebarPaneId,
    ],
  );

  return {
    handleCloseTerminalTab,
    handleCloseAgentTab,
    handleClosePassiveTab,
    confirmDiscardModifiedTab,
    handleCloseTabById,
    handleCopyAgentId,
    handleCopyTerminalId,
    handleCopyFilePath,
    handleCopyResumeCommand,
    handleReloadAgent,
    handleCopyWorkspacePath,
    handleCopyBranchName,
    handleOpenSetupTab,
    handleBulkCloseTabs,
    handleCloseTabsToLeft,
    handleCloseTabsToRight,
    handleCloseOtherTabs,
    handleClosePane,
    handleCloseTabsToLeftInPane,
    handleCloseTabsToRightInPane,
    handleCloseOtherTabsInPane,
  };
}
