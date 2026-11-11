import type { ComponentProps } from "react";
import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { WorkspaceActions } from "@/git/workspace-actions";
import { PluginHeaderButtons } from "@/plugins";
import { WorkspaceOpenInEditorButton } from "@/workspace/open-in-editor/button";
import { WorkspaceScriptsButton } from "@/screens/workspace/scripts/workspace-scripts-button";
import { useWorkspaceScriptActions } from "@/screens/workspace/scripts/use-workspace-script-actions";
import { useHostRuntimeClient } from "@/runtime/host-runtime";
import {
  EXPLORER_TOGGLE_KEYS,
  WorkspaceHeaderExplorerToggle,
} from "@/screens/workspace/workspace-explorer-toggle";

type ExplorerToggleProps = ComponentProps<typeof WorkspaceHeaderExplorerToggle>;
type EditorButtonProps = ComponentProps<typeof WorkspaceOpenInEditorButton>;

export interface WorkspaceHeaderActionsProps
  extends
    Pick<EditorButtonProps, "projectId" | "activeFile">,
    Pick<
      ExplorerToggleProps,
      "owner" | "onPress" | "label" | "tooltipLabel" | "accessibilityState"
    > {
  serverId: string;
  workspaceId: string;
  scripts: ComponentProps<typeof WorkspaceScriptsButton>["scripts"] | null;
  liveTerminalIds?: readonly string[];
  onScriptTerminalStarted?: (terminalId: string) => void;
  onViewTerminal?: (terminalId: string) => void;
  onOpenUrlInBrowserTab?: (url: string) => void;
  workspaceDirectory: string | null;
}

export function WorkspaceHeaderActions({
  serverId,
  workspaceId,
  scripts,
  liveTerminalIds,
  onScriptTerminalStarted,
  onViewTerminal,
  onOpenUrlInBrowserTab,
  workspaceDirectory,
  projectId,
  activeFile,
  owner,
  onPress,
  label,
  tooltipLabel,
  accessibilityState,
}: WorkspaceHeaderActionsProps) {
  const client = useHostRuntimeClient(serverId);
  const scriptActions = useWorkspaceScriptActions({
    client,
    serverId,
    workspaceId,
    scripts: scripts ?? [],
    onScriptTerminalStarted,
  });

  return (
    <View style={styles.headerRight}>
      <PluginHeaderButtons serverId={serverId} workspaceId={workspaceId} />
      {scripts && scripts.length > 0 ? (
        <WorkspaceScriptsButton
          actions={scriptActions}
          scripts={scripts}
          liveTerminalIds={liveTerminalIds}
          onViewTerminal={onViewTerminal}
          onOpenUrlInBrowserTab={onOpenUrlInBrowserTab}
          hideLabels
        />
      ) : null}
      {workspaceDirectory ? (
        <WorkspaceOpenInEditorButton
          serverId={serverId}
          cwd={workspaceDirectory}
          projectId={projectId}
          activeFile={activeFile}
          hideLabels
        />
      ) : null}
      {workspaceDirectory ? (
        <>
          <WorkspaceActions serverId={serverId} cwd={workspaceDirectory} />
          <WorkspaceHeaderExplorerToggle
            owner={owner}
            onPress={onPress}
            label={label}
            tooltipLabel={tooltipLabel}
            tooltipKeys={EXPLORER_TOGGLE_KEYS}
            style={styles.compactHeaderActionButton}
            accessibilityState={accessibilityState}
          />
        </>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  headerRight: {
    flexDirection: "row",
    alignItems: "center",
    gap: {
      xs: theme.spacing[1],
      md: theme.spacing[2],
    },
  },
  compactHeaderActionButton: {
    marginRight: {
      xs: 0,
      md: -theme.spacing[2],
    },
  },
}));
