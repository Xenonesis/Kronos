import React, { memo, type Dispatch, type ReactNode, type SetStateAction } from "react";
import { findPaneById, type WorkspaceLayout } from "@/stores/workspace-layout-store";
import { ExplorerSidebarDock } from "@/screens/workspace/explorer-sidebar";
import { ExplorerFrame } from "../explorer-sidebar/frame";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import type { WorkspacePaneContentModel } from "@/screens/workspace/workspace-pane-content";
import type { WorkspaceTab } from "@/workspace-tabs/model";

interface WorkspaceExplorerViewProps {
  workspaceLayout: WorkspaceLayout | null;
  explorerSidebarPaneId: string | null;
  persistenceKey: string | null;
  isMobile: boolean;
  uiTabs: WorkspaceTab[];
  normalizedServerId: string;
  normalizedWorkspaceId: string;
  isRouteFocused: boolean;
  paneFocusSuspended: boolean;
  closingTabIds: Set<string>;
  hoveredCloseTabKey: string | null;
  setHoveredCloseTabKey: Dispatch<SetStateAction<string | null>>;
  selectTabInPane: (paneId: string, tabId: string) => void;
  handleCloseTabById: (tabId: string) => Promise<void> | void;
  handleCreateNewTab: (input?: { paneId?: string }) => void;
  handleCopyResumeCommand: (agentId: string) => Promise<void> | void;
  handleCopyAgentId: (agentId: string) => Promise<void> | void;
  handleCopyTerminalId: (terminalId: string) => Promise<void> | void;
  handleCopyFilePath: (path: string) => Promise<void> | void;
  handleReloadAgent: (agentId: string) => Promise<void> | void;
  handleRenameTab: (tab: WorkspaceTabDescriptor) => void;
  handleCloseTabsToLeftInPane: (
    tabId: string,
    paneTabs: WorkspaceTabDescriptor[],
  ) => Promise<void> | void;
  handleCloseTabsToRightInPane: (
    tabId: string,
    paneTabs: WorkspaceTabDescriptor[],
  ) => Promise<void> | void;
  handleCloseOtherTabsInPane: (
    tabId: string,
    paneTabs: WorkspaceTabDescriptor[],
  ) => Promise<void> | void;
  toggleFocusMode: () => void;
  handleReorderTabsInPane: (paneId: string, tabIds: string[]) => void;
  buildDesktopPaneContentModel: (input: {
    paneId: string;
    tab: WorkspaceTabDescriptor;
  }) => WorkspacePaneContentModel;
  renderExplorerSidebarHeaderAction: () => ReactNode;
  workspaceViewportWidth: number;
  isExplorerSidebarShowing: boolean;
  desktopFocusModeEnabled: boolean;
}

export const WorkspaceExplorerView = memo(function WorkspaceExplorerView({
  workspaceLayout,
  explorerSidebarPaneId,
  persistenceKey,
  isMobile,
  uiTabs,
  normalizedServerId,
  normalizedWorkspaceId,
  isRouteFocused,
  paneFocusSuspended,
  closingTabIds,
  hoveredCloseTabKey,
  setHoveredCloseTabKey,
  selectTabInPane,
  handleCloseTabById,
  handleCreateNewTab,
  handleCopyResumeCommand,
  handleCopyAgentId,
  handleCopyTerminalId,
  handleCopyFilePath,
  handleReloadAgent,
  handleRenameTab,
  handleCloseTabsToLeftInPane,
  handleCloseTabsToRightInPane,
  handleCloseOtherTabsInPane,
  toggleFocusMode,
  handleReorderTabsInPane,
  buildDesktopPaneContentModel,
  renderExplorerSidebarHeaderAction,
  workspaceViewportWidth,
  isExplorerSidebarShowing,
  desktopFocusModeEnabled,
}: WorkspaceExplorerViewProps) {
  if (!workspaceLayout || !explorerSidebarPaneId || !persistenceKey) return null;
  const explorerPane = findPaneById(workspaceLayout.root, explorerSidebarPaneId);
  if (!explorerPane) return null;

  const dock = (
    <ExplorerSidebarDock
      pane={explorerPane}
      uiTabs={uiTabs}
      normalizedServerId={normalizedServerId}
      normalizedWorkspaceId={normalizedWorkspaceId}
      isWorkspaceFocused={isRouteFocused}
      paneFocusSuspended={paneFocusSuspended}
      closingTabIds={closingTabIds}
      hoveredCloseTabKey={hoveredCloseTabKey}
      setHoveredCloseTabKey={setHoveredCloseTabKey}
      onSelectTab={selectTabInPane}
      onCloseTab={handleCloseTabById}
      onCreateNewTab={handleCreateNewTab}
      onCopyResumeCommand={handleCopyResumeCommand}
      onCopyAgentId={handleCopyAgentId}
      onCopyTerminalId={handleCopyTerminalId}
      onCopyFilePath={handleCopyFilePath}
      onReloadAgent={handleReloadAgent}
      onRenameTab={handleRenameTab}
      onCloseTabsToLeft={handleCloseTabsToLeftInPane}
      onCloseTabsToRight={handleCloseTabsToRightInPane}
      onCloseOtherTabs={handleCloseOtherTabsInPane}
      onExitFocusMode={toggleFocusMode}
      onReorderTabsInPane={handleReorderTabsInPane}
      buildPaneContentModel={buildDesktopPaneContentModel}
      headerAction={renderExplorerSidebarHeaderAction()}
    />
  );

  return (
    <ExplorerFrame
      presentation={isMobile ? "overlay" : "dock"}
      isWorkspaceActive={isRouteFocused}
      persistenceKey={persistenceKey}
      containerWidth={workspaceViewportWidth}
      isOpen={isExplorerSidebarShowing && !desktopFocusModeEnabled}
    >
      {dock}
    </ExplorerFrame>
  );
});
