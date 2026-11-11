import { useCallback } from "react";
import type { TerminalProfile } from "@getpaseo/protocol/messages";
import { createWorkspaceBrowser } from "@/desktop/browser/store";
import { useDesktopBrowserNewTabRequests } from "@/desktop/browser/new-tab-requests";
import { generateDraftId } from "@/stores/draft-keys";
import type { NewTabSelection } from "@/workspace-tabs/new-tab";
import type { WorkspaceTabLaunchDestination } from "@/workspace-tabs/launcher";
import type { WorkspaceTabTarget } from "@/workspace-tabs/model";
import type { WorkspaceLayout, WorkspaceTabPlacement } from "@/stores/workspace-layout-store";

export interface UseWorkspaceTabLaunchersInput {
  persistenceKey: string | null;
  workspaceLayout: WorkspaceLayout | null;
  openWorkspaceTabFocused: (
    workspaceKey: string,
    target: WorkspaceTabTarget,
    placement?: WorkspaceTabPlacement,
  ) => void;
  createWorkspaceTab: (
    workspaceKey: string,
    target: WorkspaceTabTarget,
    placement?: WorkspaceTabPlacement,
  ) => void;
  replaceWorkspaceTabTarget: (
    workspaceKey: string,
    tabId: string,
    target: WorkspaceTabTarget,
  ) => void;
  splitWorkspacePaneEmpty: (
    workspaceKey: string,
    input: { targetPaneId: string; position: "left" | "right" | "top" | "bottom" },
  ) => void;
  createTerminal: (input: {
    profile?: TerminalProfile;
    destination: WorkspaceTabLaunchDestination;
  }) => void;
  openWorkspaceDraftTab: (input?: { paneId?: string }) => void;
  navigateToTabId: (tabId: string) => void;
  paneLocalPlacement: (paneId?: string) => WorkspaceTabPlacement | undefined;
  focusedPanePlacement?: WorkspaceTabPlacement;
}

export function useWorkspaceTabLaunchers({
  persistenceKey,
  workspaceLayout,
  openWorkspaceTabFocused,
  createWorkspaceTab,
  replaceWorkspaceTabTarget,
  splitWorkspacePaneEmpty,
  createTerminal,
  openWorkspaceDraftTab,
  navigateToTabId,
  paneLocalPlacement,
  focusedPanePlacement,
}: UseWorkspaceTabLaunchersInput) {
  const handleCreateDraftTab = useCallback(
    (input?: { paneId?: string }) => {
      openWorkspaceDraftTab({ paneId: input?.paneId });
    },
    [openWorkspaceDraftTab],
  );

  const handleCreateTerminal = useCallback(
    (input?: { paneId?: string }) => {
      createTerminal({
        destination: input?.paneId ? { kind: "open", paneId: input.paneId } : { kind: "open" },
      });
    },
    [createTerminal],
  );

  const handleCreateTerminalWithProfile = useCallback(
    (profile: TerminalProfile) => {
      createTerminal({ profile, destination: { kind: "open" } });
    },
    [createTerminal],
  );

  const handleCreateBrowserTab = useCallback(
    (input?: { paneId?: string }) => {
      if (!persistenceKey) {
        return;
      }
      const { browserId } = createWorkspaceBrowser();
      openWorkspaceTabFocused(
        persistenceKey,
        { kind: "browser", browserId },
        paneLocalPlacement(input?.paneId),
      );
    },
    [openWorkspaceTabFocused, paneLocalPlacement, persistenceKey],
  );

  const handleCreateNewTab = useCallback(
    (input?: { paneId?: string }) => {
      if (!persistenceKey) {
        return;
      }
      createWorkspaceTab(persistenceKey, { kind: "new_tab" }, paneLocalPlacement(input?.paneId));
    },
    [createWorkspaceTab, paneLocalPlacement, persistenceKey],
  );

  const launchWorkspaceTab = useCallback(
    (selection: NewTabSelection, destination: WorkspaceTabLaunchDestination) => {
      if (!persistenceKey) {
        return;
      }
      const openTarget = (target: WorkspaceTabTarget) => {
        if (destination.kind === "replace") {
          replaceWorkspaceTabTarget(persistenceKey, destination.tabId, target);
        } else {
          createWorkspaceTab(persistenceKey, target, paneLocalPlacement(destination.paneId));
        }
      };
      if (selection.kind === "target") {
        openTarget(selection.target);
        return;
      }
      if (selection.kind === "agent") {
        openTarget({
          kind: "draft",
          draftId: generateDraftId(),
        });
        return;
      }
      if (selection.kind === "terminal") {
        createTerminal({
          profile: selection.profile,
          destination,
        });
        return;
      }
      const { browserId } = createWorkspaceBrowser();
      openTarget({ kind: "browser", browserId });
    },
    [
      createTerminal,
      createWorkspaceTab,
      paneLocalPlacement,
      persistenceKey,
      replaceWorkspaceTabTarget,
    ],
  );

  const handleOpenUrlInBrowserTab = useCallback(
    (url: string) => {
      if (!persistenceKey) {
        return;
      }
      const { browserId } = createWorkspaceBrowser({ initialUrl: url });
      openWorkspaceTabFocused(persistenceKey, { kind: "browser", browserId }, focusedPanePlacement);
    },
    [focusedPanePlacement, openWorkspaceTabFocused, persistenceKey],
  );

  useDesktopBrowserNewTabRequests({
    enabled: Boolean(persistenceKey),
    workspaceLayout,
    openUrl: handleOpenUrlInBrowserTab,
  });

  const handleSelectSwitcherTab = useCallback(
    (key: string) => {
      navigateToTabId(key);
    },
    [navigateToTabId],
  );

  const handleCreateEmptySplit = useCallback(
    (input: { targetPaneId: string; position: "left" | "right" | "top" | "bottom" }) => {
      if (!persistenceKey) {
        return;
      }
      splitWorkspacePaneEmpty(persistenceKey, input);
    },
    [persistenceKey, splitWorkspacePaneEmpty],
  );

  return {
    handleCreateDraftTab,
    handleCreateTerminal,
    handleCreateTerminalWithProfile,
    handleCreateBrowserTab,
    handleCreateNewTab,
    launchWorkspaceTab,
    handleOpenUrlInBrowserTab,
    handleSelectSwitcherTab,
    handleCreateEmptySplit,
  };
}
