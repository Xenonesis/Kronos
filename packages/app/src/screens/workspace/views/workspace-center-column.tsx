import { memo, useCallback, type Dispatch, type ReactNode, type SetStateAction } from "react";
import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { FloatingHeaderLayout } from "@/components/floating-header";
import { StreamTopOverlayProvider } from "@/agent-stream/top-overlay";
import { SheetTabs } from "@/screens/workspace/sheet-tabs";
import {
  WorkspaceDesktopTabsRow,
  type WorkspaceDesktopTabRowItem,
} from "@/screens/workspace/workspace-desktop-tabs-row";
import { NewTabLauncherProvider, type NewTabLauncher } from "@/workspace-tabs/launcher";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";

interface WorkspaceCenterColumnProps {
  isMobile: boolean;
  compactHeaderLayout: {
    header: ReactNode;
    floating: boolean;
  };
  children: ReactNode;
  rendersDesktopSplitContent: boolean;
  shouldRenderDesktopPaneFallback: boolean;
  renderWorkspaceScreenHeader: () => ReactNode;
  newTabLauncher: NewTabLauncher;
  focusedPaneIdOrUndefined?: string;
  isRouteFocused: boolean;
  desktopTabRowItems: WorkspaceDesktopTabRowItem[];
  normalizedServerId: string;
  normalizedWorkspaceId: string;
  setHoveredCloseTabKey: Dispatch<SetStateAction<string | null>>;
  navigateToTabId: (tabId: string) => void;
  handleCloseTabById: (tabId: string) => Promise<void>;
  handleCopyResumeCommand: (agentId: string) => Promise<void> | void;
  handleCopyAgentId: (agentId: string) => Promise<void> | void;
  handleCopyTerminalId: (terminalId: string) => Promise<void> | void;
  handleCopyFilePath: (path: string) => Promise<void> | void;
  handleReloadAgent: (agentId: string) => Promise<void> | void;
  handleRenameTab: (tab: WorkspaceTabDescriptor) => void;
  handleCloseTabsToLeft: (tabId: string) => Promise<void> | void;
  handleCloseTabsToRight: (tabId: string) => Promise<void> | void;
  handleCloseOtherTabs: (tabId: string) => Promise<void> | void;
  handleCreateNewTab: (input?: { paneId?: string }) => void;
  handleReorderTabsInFocusedPane: (nextTabs: WorkspaceTabDescriptor[]) => void;
  desktopFocusModeEnabled: boolean;
  toggleFocusMode: () => void;
}

export const WorkspaceCenterColumn = memo(function WorkspaceCenterColumn({
  isMobile,
  compactHeaderLayout,
  children,
  rendersDesktopSplitContent,
  shouldRenderDesktopPaneFallback,
  renderWorkspaceScreenHeader,
  newTabLauncher,
  focusedPaneIdOrUndefined,
  isRouteFocused,
  desktopTabRowItems,
  normalizedServerId,
  normalizedWorkspaceId,
  setHoveredCloseTabKey,
  navigateToTabId,
  handleCloseTabById,
  handleCopyResumeCommand,
  handleCopyAgentId,
  handleCopyTerminalId,
  handleCopyFilePath,
  handleReloadAgent,
  handleRenameTab,
  handleCloseTabsToLeft,
  handleCloseTabsToRight,
  handleCloseOtherTabs,
  handleCreateNewTab,
  handleReorderTabsInFocusedPane,
  desktopFocusModeEnabled,
  toggleFocusMode,
}: WorkspaceCenterColumnProps) {
  const handleOpenPreview = useCallback(() => {
    newTabLauncher.launch({ kind: "browser" }, { kind: "open" });
  }, [newTabLauncher]);

  if (isMobile) {
    return (
      <View style={styles.centerColumn}>
        <FloatingHeaderLayout
          header={compactHeaderLayout.header}
          floating={compactHeaderLayout.floating}
        >
          {(topOverlayInset) => (
            <StreamTopOverlayProvider value={topOverlayInset}>
              <View style={styles.content}>{children}</View>
            </StreamTopOverlayProvider>
          )}
        </FloatingHeaderLayout>
        <SheetTabs onCloseTab={handleCloseTabById} />
      </View>
    );
  }

  return (
    <View style={styles.centerColumn}>
      {rendersDesktopSplitContent ? null : renderWorkspaceScreenHeader()}

      {shouldRenderDesktopPaneFallback ? (
        <NewTabLauncherProvider value={newTabLauncher}>
          <WorkspaceDesktopTabsRow
            paneId={focusedPaneIdOrUndefined}
            isFocused={isRouteFocused}
            tabs={desktopTabRowItems}
            normalizedServerId={normalizedServerId}
            normalizedWorkspaceId={normalizedWorkspaceId}
            setHoveredCloseTabKey={setHoveredCloseTabKey}
            onNavigateTab={navigateToTabId}
            onCloseTab={handleCloseTabById}
            onCopyResumeCommand={handleCopyResumeCommand}
            onCopyAgentId={handleCopyAgentId}
            onCopyTerminalId={handleCopyTerminalId}
            onCopyFilePath={handleCopyFilePath}
            onReloadAgent={handleReloadAgent}
            onRenameTab={handleRenameTab}
            onCloseTabsToLeft={handleCloseTabsToLeft}
            onCloseTabsToRight={handleCloseTabsToRight}
            onCloseOtherTabs={handleCloseOtherTabs}
            onCreateNewTab={handleCreateNewTab}
            onReorderTabs={handleReorderTabsInFocusedPane}
            onOpenPreview={handleOpenPreview}
            focusModeEnabled={desktopFocusModeEnabled}
            onExitFocusMode={toggleFocusMode}
          />
        </NewTabLauncherProvider>
      ) : null}

      <View style={styles.centerContent}>
        <View style={styles.content}>{children}</View>
      </View>
    </View>
  );
});

const styles = StyleSheet.create({
  centerColumn: {
    flex: 1,
    minWidth: 0,
    height: "100%",
  },
  centerContent: {
    flex: 1,
    minHeight: 0,
    minWidth: 0,
  },
  content: {
    flex: 1,
    minHeight: 0,
    minWidth: 0,
  },
});
