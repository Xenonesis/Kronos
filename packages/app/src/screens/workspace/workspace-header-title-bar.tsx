import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { ScreenTitle } from "@/components/headers/screen-title";
import { WorkspaceHeaderProjectRow } from "@/screens/workspace/workspace-header-project-row";
import {
  WorkspaceHeaderMenuDesktop,
  type WorkspaceHeaderWorkspaceActions,
} from "@/screens/workspace/workspace-header-menu";

export interface WorkspaceHeaderTitleBarProps extends WorkspaceHeaderWorkspaceActions {
  isLoading: boolean;
  title: string;
  subtitle: string;
  isSubtitleDistinct: boolean;
  normalizedServerId: string;
}

export function WorkspaceHeaderTitleBar({
  isLoading,
  title,
  subtitle,
  isSubtitleDistinct,
  currentBranchName,
  normalizedServerId,
  showWorkspaceSetup,
  importAgentDisabled,
  copyPathDisabled,
  onOpenImportSheet,
  onCopyWorkspacePath,
  onCopyBranchName,
  onOpenSetupTab,
}: WorkspaceHeaderTitleBarProps) {
  return (
    <View style={styles.headerTitleContainer}>
      {isLoading ? (
        <View style={styles.headerTitleTextGroup}>
          <View style={styles.headerTitleSkeleton} />
        </View>
      ) : (
        <View style={styles.headerTitleTextGroup}>
          <ScreenTitle testID="workspace-header-title">{title}</ScreenTitle>
          <WorkspaceHeaderProjectRow
            subtitle={subtitle}
            isSubtitleDistinct={isSubtitleDistinct}
            serverId={normalizedServerId}
          />
        </View>
      )}
      <View style={styles.compactHeaderMenuCluster}>
        <WorkspaceHeaderMenuDesktop
          currentBranchName={currentBranchName}
          showWorkspaceSetup={showWorkspaceSetup}
          importAgentDisabled={importAgentDisabled}
          copyPathDisabled={copyPathDisabled}
          onOpenImportSheet={onOpenImportSheet}
          onCopyWorkspacePath={onCopyWorkspacePath}
          onCopyBranchName={onCopyBranchName}
          onOpenSetupTab={onOpenSetupTab}
        />
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  headerTitleContainer: {
    flex: 1,
    flexShrink: 1,
    minWidth: 0,
    flexDirection: "row",
    alignItems: "center",
    gap: {
      xs: theme.spacing[1],
      md: theme.spacing[2],
    },
    overflow: "hidden",
  },
  headerTitleTextGroup: {
    minWidth: 0,
    overflow: "hidden",
    flexShrink: 1,
    flexGrow: {
      xs: 1,
      md: 0,
    },
    flexDirection: {
      xs: "column",
      md: "row",
    },
    alignItems: {
      xs: "stretch",
      md: "center",
    },
    justifyContent: "flex-start",
    gap: {
      xs: 0,
      md: theme.spacing[2],
    },
  },
  headerTitleSkeleton: {
    width: 220,
    maxWidth: "100%",
    height: 22,
    borderRadius: theme.borderRadius.full,
    backgroundColor: theme.colors.surface3,
    opacity: 0.25,
  },
  compactHeaderMenuCluster: {
    flexDirection: "row",
    alignItems: "center",
    gap: {
      xs: 0,
      md: theme.spacing[2],
    },
  },
}));
