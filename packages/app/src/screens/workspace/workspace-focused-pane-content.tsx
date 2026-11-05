import { memo, useCallback, useMemo } from "react";
import { Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { RetainedPanel } from "@/components/retained-panel";
import type { Theme } from "@/styles/theme";
import { RenderProfile } from "@/utils/render-profiler";
import {
  WorkspacePaneContent,
  type WorkspacePaneContentModel,
} from "@/screens/workspace/workspace-pane-content";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";

const ThemedLoadingSpinner = withUnistyles(LoadingSpinner);
const mutedColorMapping = (theme: Theme) => ({ color: theme.colors.foregroundMuted });

interface PaneContentModelInput {
  paneId: string | null;
  tab: WorkspaceTabDescriptor;
}

interface MobileMountedTabSlotProps {
  tabDescriptor: WorkspaceTabDescriptor;
  isVisible: boolean;
  isWorkspaceFocused: boolean;
  isPaneFocused: boolean;
  paneId: string | null;
  onFocusPane: (paneId: string) => void;
  buildPaneContentModel: (input: PaneContentModelInput) => WorkspacePaneContentModel;
}

const MobileMountedTabSlot = memo(function MobileMountedTabSlot({
  tabDescriptor,
  isVisible,
  isWorkspaceFocused,
  isPaneFocused,
  paneId,
  onFocusPane,
  buildPaneContentModel,
}: MobileMountedTabSlotProps) {
  const content = useMemo(
    () =>
      buildPaneContentModel({
        paneId,
        tab: tabDescriptor,
      }),
    [buildPaneContentModel, paneId, tabDescriptor],
  );
  const handleTouch = useCallback(() => {
    if (!isPaneFocused && paneId) onFocusPane(paneId);
    return false;
  }, [isPaneFocused, onFocusPane, paneId]);

  return (
    <RenderProfile id={`MobileMountedTabSlot:${tabDescriptor.kind}:${tabDescriptor.tabId}`}>
      <RetainedPanel active={isVisible} style={styles.mobileMountedTabSlot}>
        <View style={styles.mobileMountedTabSlot} onStartShouldSetResponderCapture={handleTouch}>
          <WorkspacePaneContent
            content={content}
            isWorkspaceFocused={isWorkspaceFocused}
            isPaneFocused={isPaneFocused}
          />
        </View>
      </RetainedPanel>
    </RenderProfile>
  );
});

export interface WorkspaceFocusedPaneContentProps {
  isMissingWorkspaceDirectory: boolean;
  activeTabDescriptor: WorkspaceTabDescriptor | null;
  hasHydratedAgents: boolean;
  hasLoadedTerminals: boolean;
  mountedFocusedPaneTabIds: string[];
  focusedPaneTabDescriptorMap: Map<string, WorkspaceTabDescriptor>;
  isRouteFocused: boolean;
  focusedPaneId: string | null;
  paneFocusSuspended: boolean;
  onFocusPane: MobileMountedTabSlotProps["onFocusPane"];
  buildMobilePaneContentModel: MobileMountedTabSlotProps["buildPaneContentModel"];
}

export function WorkspaceFocusedPaneContent({
  isMissingWorkspaceDirectory,
  activeTabDescriptor,
  hasHydratedAgents,
  hasLoadedTerminals,
  mountedFocusedPaneTabIds,
  focusedPaneTabDescriptorMap,
  isRouteFocused,
  focusedPaneId,
  paneFocusSuspended,
  onFocusPane,
  buildMobilePaneContentModel,
}: WorkspaceFocusedPaneContentProps) {
  if (isMissingWorkspaceDirectory) {
    return (
      <View style={styles.emptyState}>
        <Text style={styles.emptyStateText}>
          Workspace directory is missing. Reload workspace data before opening tabs.
        </Text>
      </View>
    );
  }
  if (!activeTabDescriptor && (!hasHydratedAgents || !hasLoadedTerminals)) {
    return (
      <View style={styles.emptyState}>
        <ThemedLoadingSpinner uniProps={mutedColorMapping} />
      </View>
    );
  }
  if (!activeTabDescriptor) {
    return (
      <View style={styles.emptyState}>
        <Text style={styles.emptyStateText}>
          No tabs are available yet. Use New tab to create an agent or terminal.
        </Text>
      </View>
    );
  }
  return mountedFocusedPaneTabIds.map((tabId) => {
    const tabDescriptor = focusedPaneTabDescriptorMap.get(tabId);
    if (!tabDescriptor) {
      return null;
    }
    return (
      <MobileMountedTabSlot
        key={tabId}
        tabDescriptor={tabDescriptor}
        isVisible={isRouteFocused && tabId === activeTabDescriptor.tabId}
        isWorkspaceFocused={isRouteFocused}
        isPaneFocused={!paneFocusSuspended && tabId === activeTabDescriptor.tabId}
        paneId={focusedPaneId}
        onFocusPane={onFocusPane}
        buildPaneContentModel={buildMobilePaneContentModel}
      />
    );
  });
}

const styles = StyleSheet.create((theme) => ({
  mobileMountedTabSlot: {
    ...StyleSheet.absoluteFill,
  },
  emptyState: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: theme.spacing[3],
    paddingHorizontal: theme.spacing[6],
  },
  emptyStateText: {
    color: theme.colors.foregroundMuted,
    textAlign: "center",
  },
}));
