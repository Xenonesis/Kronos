import { useCallback, type MutableRefObject } from "react";
import { useStableEvent } from "@/hooks/use-stable-event";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";

export interface UseWorkspacePaneLayoutInput {
  persistenceKey: string;
  focusedPaneId: string | null;
  paneFocusSuppressedRef: MutableRefObject<boolean>;
  focusWorkspacePane: (workspaceKey: string, paneId: string) => void;
  splitWorkspacePane: (
    workspaceKey: string,
    input: {
      tabId: string;
      targetPaneId: string;
      position: "left" | "right" | "top" | "bottom";
    },
  ) => void;
  moveWorkspaceTabToPane: (workspaceKey: string, tabId: string, toPaneId: string) => void;
  resizeWorkspaceSplit: (workspaceKey: string, groupId: string, sizes: number[]) => void;
  reorderWorkspaceTabsInPane: (workspaceKey: string, paneId: string, tabIds: string[]) => void;
}

export function useWorkspacePaneLayout({
  persistenceKey,
  focusedPaneId,
  paneFocusSuppressedRef,
  focusWorkspacePane,
  splitWorkspacePane,
  moveWorkspaceTabToPane,
  resizeWorkspaceSplit,
  reorderWorkspaceTabsInPane,
}: UseWorkspacePaneLayoutInput) {
  const handleFocusPane = useStableEvent(function handleFocusPane(paneId: string) {
    if (!persistenceKey || paneFocusSuppressedRef.current) {
      return;
    }
    focusWorkspacePane(persistenceKey, paneId);
  });

  const handleSplitPane = useCallback(
    function handleSplitPane(input: {
      tabId: string;
      targetPaneId: string;
      position: "left" | "right" | "top" | "bottom";
    }) {
      if (!persistenceKey) {
        return;
      }
      splitWorkspacePane(persistenceKey, input);
    },
    [persistenceKey, splitWorkspacePane],
  );

  const handleMoveTabToPane = useCallback(
    function handleMoveTabToPane(tabId: string, toPaneId: string) {
      if (!persistenceKey) {
        return;
      }
      moveWorkspaceTabToPane(persistenceKey, tabId, toPaneId);
    },
    [moveWorkspaceTabToPane, persistenceKey],
  );

  const handleResizePaneSplit = useCallback(
    function handleResizePaneSplit(groupId: string, sizes: number[]) {
      if (!persistenceKey) {
        return;
      }
      resizeWorkspaceSplit(persistenceKey, groupId, sizes);
    },
    [persistenceKey, resizeWorkspaceSplit],
  );

  const handleReorderTabsInPane = useCallback(
    function handleReorderTabsInPane(paneId: string, tabIds: string[]) {
      if (!persistenceKey) {
        return;
      }
      reorderWorkspaceTabsInPane(persistenceKey, paneId, tabIds);
    },
    [persistenceKey, reorderWorkspaceTabsInPane],
  );

  const handleReorderTabsInFocusedPane = useCallback(
    (nextTabs: WorkspaceTabDescriptor[]) => {
      if (!focusedPaneId) {
        return;
      }
      handleReorderTabsInPane(
        focusedPaneId,
        nextTabs.map((tab) => tab.tabId),
      );
    },
    [focusedPaneId, handleReorderTabsInPane],
  );

  return {
    handleFocusPane,
    handleSplitPane,
    handleMoveTabToPane,
    handleResizePaneSplit,
    handleReorderTabsInPane,
    handleReorderTabsInFocusedPane,
  };
}
