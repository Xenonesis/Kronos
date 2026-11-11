import React, { memo, type Dispatch, type ReactNode, type SetStateAction } from "react";
import { SplitContainer } from "@/components/split-container";
import type { WorkspaceLayout } from "@/stores/workspace-layout-store";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import type { WorkspacePaneContentModel } from "@/screens/workspace/workspace-pane-content";
import type { WorkspaceTab } from "@/workspace-tabs/model";
import type { WorkspaceCompactHeaderTabs } from "@/screens/workspace/adaptive-workspace-chrome";

interface WorkspaceDesktopSplitViewProps {
  layout: WorkspaceLayout;
  renderMainHeader?: () => ReactNode;
  renderCompactHeader: (paneTabs: WorkspaceCompactHeaderTabs | null) => ReactNode;
  focusModeEnabled?: boolean;
  onExitFocusMode: () => void;
  workspaceKey: string;
  normalizedServerId: string;
  normalizedWorkspaceId: string;
  isWorkspaceFocused: boolean;
  uiTabs: WorkspaceTab[];
  hoveredCloseTabKey: string | null;
  setHoveredCloseTabKey: Dispatch<SetStateAction<string | null>>;
  closingTabIds: Set<string>;
  onNavigateTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => Promise<void> | void;
  onCopyResumeCommand: (agentId: string) => Promise<void> | void;
  onCopyAgentId: (agentId: string) => Promise<void> | void;
  onCopyTerminalId: (terminalId: string) => Promise<void> | void;
  onCopyFilePath: (path: string) => Promise<void> | void;
  onReloadAgent: (agentId: string) => Promise<void> | void;
  onRenameTab: (tab: WorkspaceTabDescriptor) => void;
  onCloseTabsToLeft: (tabId: string, paneTabs: WorkspaceTabDescriptor[]) => Promise<void> | void;
  onCloseTabsToRight: (tabId: string, paneTabs: WorkspaceTabDescriptor[]) => Promise<void> | void;
  onCloseOtherTabs: (tabId: string, paneTabs: WorkspaceTabDescriptor[]) => Promise<void> | void;
  onCreateNewTab: (input: { paneId?: string }) => void;
  buildPaneContentModel: (input: {
    paneId: string;
    tab: WorkspaceTabDescriptor;
  }) => WorkspacePaneContentModel;
  onFocusPane: (paneId: string) => void;
  onSplitPane: (input: {
    tabId: string;
    targetPaneId: string;
    position: "left" | "right" | "top" | "bottom";
  }) => void;
  onSplitPaneEmpty: (input: {
    targetPaneId: string;
    position: "left" | "right" | "top" | "bottom";
  }) => void;
  onMoveTabToPane: (tabId: string, toPaneId: string) => void;
  onSelectTabInPane: (paneId: string, tabId: string) => void;
  onResizeSplit: (groupId: string, sizes: number[]) => void;
  onReorderTabsInPane: (paneId: string, tabIds: string[]) => void;
}

export const WorkspaceDesktopSplitView = memo(function WorkspaceDesktopSplitView({
  layout,
  renderMainHeader,
  renderCompactHeader,
  focusModeEnabled,
  onExitFocusMode,
  workspaceKey,
  normalizedServerId,
  normalizedWorkspaceId,
  isWorkspaceFocused,
  uiTabs,
  hoveredCloseTabKey,
  setHoveredCloseTabKey,
  closingTabIds,
  onNavigateTab,
  onCloseTab,
  onCopyResumeCommand,
  onCopyAgentId,
  onCopyTerminalId,
  onCopyFilePath,
  onReloadAgent,
  onRenameTab,
  onCloseTabsToLeft,
  onCloseTabsToRight,
  onCloseOtherTabs,
  onCreateNewTab,
  buildPaneContentModel,
  onFocusPane,
  onSplitPane,
  onSplitPaneEmpty,
  onMoveTabToPane,
  onSelectTabInPane,
  onResizeSplit,
  onReorderTabsInPane,
}: WorkspaceDesktopSplitViewProps) {
  return (
    <SplitContainer
      layout={layout}
      renderMainHeader={renderMainHeader}
      renderCompactHeader={renderCompactHeader}
      focusModeEnabled={focusModeEnabled}
      onExitFocusMode={onExitFocusMode}
      workspaceKey={workspaceKey}
      normalizedServerId={normalizedServerId}
      normalizedWorkspaceId={normalizedWorkspaceId}
      isWorkspaceFocused={isWorkspaceFocused}
      uiTabs={uiTabs}
      hoveredCloseTabKey={hoveredCloseTabKey}
      setHoveredCloseTabKey={setHoveredCloseTabKey}
      closingTabIds={closingTabIds}
      onNavigateTab={onNavigateTab}
      onCloseTab={onCloseTab}
      onCopyResumeCommand={onCopyResumeCommand}
      onCopyAgentId={onCopyAgentId}
      onCopyTerminalId={onCopyTerminalId}
      onCopyFilePath={onCopyFilePath}
      onReloadAgent={onReloadAgent}
      onRenameTab={onRenameTab}
      onCloseTabsToLeft={onCloseTabsToLeft}
      onCloseTabsToRight={onCloseTabsToRight}
      onCloseOtherTabs={onCloseOtherTabs}
      onCreateNewTab={onCreateNewTab}
      buildPaneContentModel={buildPaneContentModel}
      onFocusPane={onFocusPane}
      onSplitPane={onSplitPane}
      onSplitPaneEmpty={onSplitPaneEmpty}
      onMoveTabToPane={onMoveTabToPane}
      onSelectTabInPane={onSelectTabInPane}
      onResizeSplit={onResizeSplit}
      onReorderTabsInPane={onReorderTabsInPane}
    />
  );
});
