import type { ComponentProps } from "react";
import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { WorkspaceActions } from "@/git/workspace-actions";
import { PluginHeaderButtons } from "@/plugins";
import { WorkspaceOpenInEditorButton } from "@/workspace/open-in-editor/button";
import { WorkspaceScriptsButton } from "@/screens/workspace/scripts/workspace-scripts-button";
import {
  EXPLORER_TOGGLE_KEYS,
  WorkspaceHeaderExplorerToggle,
} from "@/screens/workspace/workspace-explorer-toggle";

type ScriptsButtonProps = ComponentProps<typeof WorkspaceScriptsButton>;
type ExplorerToggleProps = ComponentProps<typeof WorkspaceHeaderExplorerToggle>;
type EditorButtonProps = ComponentProps<typeof WorkspaceOpenInEditorButton>;

export interface WorkspaceHeaderActionsProps
  extends
    Pick<
      ScriptsButtonProps,
      | "serverId"
      | "workspaceId"
      | "liveTerminalIds"
      | "onScriptTerminalStarted"
      | "onViewTerminal"
      | "onOpenUrlInBrowserTab"
    >,
    Pick<EditorButtonProps, "projectId" | "activeFile">,
    Pick<
      ExplorerToggleProps,
      "owner" | "onPress" | "label" | "tooltipLabel" | "accessibilityState"
    > {
  scripts: ScriptsButtonProps["scripts"] | null;
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
  return (
    <View style={styles.headerRight}>
      <PluginHeaderButtons serverId={serverId} workspaceId={workspaceId} />
      {scripts && scripts.length > 0 ? (
        <WorkspaceScriptsButton
          serverId={serverId}
          workspaceId={workspaceId}
          scripts={scripts}
          liveTerminalIds={liveTerminalIds}
          onScriptTerminalStarted={onScriptTerminalStarted}
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
