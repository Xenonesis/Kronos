import React, { useMemo } from "react";
import { View, type LayoutChangeEvent } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { useTranslation } from "react-i18next";
import { Columns2, Rows2, Ellipsis, Maximize, Minimize, Plus, X } from "lucide-react-native";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem } from "@/components/ui/dropdown-menu";
import { ToolbarButton, ToolbarControls } from "@/components/ui/pane-content-toolbar";
import { buttonControlHeight } from "@/components/ui/control-geometry";
import { Shortcut } from "@/components/ui/shortcut";
import { useShortcutKeys } from "@/hooks/use-shortcut-keys";
import type { ShortcutKey } from "@/utils/format-shortcut";
import type { PaneHost } from "@/panels/panel-manifest";
import type { WorkspaceTabLaunchPurpose } from "@/workspace-tabs/launcher";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import { WorkspaceNewTabMenuContent } from "@/screens/workspace/workspace-new-tab-menu";
import type { Theme } from "@/styles/theme";
import {
  PANE_SPLIT_ACTIONS_HORIZONTAL_PADDING,
  PANE_SPLIT_ACTIONS_OUTER_MARGIN,
} from "./tab-geometry";

const ThemedPlus = withUnistyles(Plus);
const ThemedColumns2 = withUnistyles(Columns2);
const ThemedRows2 = withUnistyles(Rows2);
const ThemedEllipsis = withUnistyles(Ellipsis);
const ThemedMaximize = withUnistyles(Maximize);
const ThemedMinimize = withUnistyles(Minimize);
const ThemedX = withUnistyles(X);

const extraMutedColorMapping = (theme: Theme) => ({
  color: theme.colors.foregroundExtraMuted,
});
const mutedColorMapping = (theme: Theme) => ({
  color: theme.colors.foregroundMuted,
});

type PanePanelKinds = readonly WorkspaceTabDescriptor["kind"][];

export interface WorkspaceNewTabButtonProps {
  panePanelKinds: PanePanelKinds;
  host: PaneHost;
  launchPurpose: WorkspaceTabLaunchPurpose;
  serverId: string;
  paneId?: string;
  shortcutKeys: ShortcutKey[][] | null;
  placement: "inline" | "toolbar";
}

export function WorkspaceNewTabButton({
  panePanelKinds,
  host,
  launchPurpose,
  serverId,
  paneId,
  shortcutKeys,
  placement,
}: WorkspaceNewTabButtonProps) {
  const { t } = useTranslation();
  const tooltipText = t("workspace.tabs.actions.newTab");
  const menu = (
    <DropdownMenu>
      <ToolbarButton
        kind="menu"
        label={tooltipText}
        shortcut={shortcutKeys}
        testID="workspace-new-tab-button"
        style={placement === "inline" ? styles.inlineNewTabButton : undefined}
      >
        <ThemedPlus size={14} uniProps={extraMutedColorMapping} />
      </ToolbarButton>
      <WorkspaceNewTabMenuContent
        serverId={serverId}
        purpose={launchPurpose}
        host={host}
        panePanelKinds={panePanelKinds}
        paneId={paneId}
      />
    </DropdownMenu>
  );

  return placement === "inline" ? <View style={styles.inlineAddButton}>{menu}</View> : menu;
}

export function WorkspacePaneToolbarActions({
  panePanelKinds,
  host,
  launchPurpose,
  showNewTabButton,
  showSplitActions,
  showMaximizeAction,
  paneMaximized,
  serverId,
  paneId,
  newTabShortcutKeys,
  onSplitRight,
  onSplitDown,
  onTogglePaneMaximized,
}: {
  panePanelKinds: PanePanelKinds;
  host: PaneHost;
  launchPurpose: WorkspaceTabLaunchPurpose;
  showNewTabButton: boolean;
  showSplitActions: boolean;
  showMaximizeAction: boolean;
  paneMaximized: boolean;
  serverId: string;
  paneId?: string;
  newTabShortcutKeys: ShortcutKey[][] | null;
  onSplitRight?: () => void;
  onSplitDown?: () => void;
  onTogglePaneMaximized?: () => void;
}) {
  const { t } = useTranslation();
  const splitRightKeys = useShortcutKeys("workspace-pane-split-right");
  const splitDownKeys = useShortcutKeys("workspace-pane-split-down");
  const splitActionsVisible = showSplitActions && Boolean(onSplitRight && onSplitDown);
  const splitRightLeading = useMemo(
    () => <ThemedColumns2 size={14} uniProps={extraMutedColorMapping} />,
    [],
  );
  const splitDownLeading = useMemo(
    () => <ThemedRows2 size={14} uniProps={extraMutedColorMapping} />,
    [],
  );
  const splitRightTrailing = useMemo(
    () => (splitRightKeys ? <Shortcut chord={splitRightKeys} /> : null),
    [splitRightKeys],
  );
  const splitDownTrailing = useMemo(
    () => (splitDownKeys ? <Shortcut chord={splitDownKeys} /> : null),
    [splitDownKeys],
  );
  const maximizeActionVisible = showMaximizeAction && Boolean(onTogglePaneMaximized);
  if (!showNewTabButton && !splitActionsVisible && !maximizeActionVisible) return null;

  return (
    <ToolbarControls style={styles.paneSplitActions}>
      {showNewTabButton ? (
        <WorkspaceNewTabButton
          panePanelKinds={panePanelKinds}
          host={host}
          launchPurpose={launchPurpose}
          placement="toolbar"
          serverId={serverId}
          paneId={paneId}
          shortcutKeys={newTabShortcutKeys}
        />
      ) : null}
      {maximizeActionVisible && onTogglePaneMaximized ? (
        <ToolbarButton
          label={t(
            paneMaximized
              ? "workspace.tabs.actions.restorePane"
              : "workspace.tabs.actions.maximizePane",
          )}
          selected={paneMaximized}
          testID={paneMaximized ? "workspace-restore-pane" : "workspace-maximize-pane"}
          onPress={onTogglePaneMaximized}
        >
          {paneMaximized ? (
            <ThemedMinimize size={14} uniProps={extraMutedColorMapping} />
          ) : (
            <ThemedMaximize size={14} uniProps={extraMutedColorMapping} />
          )}
        </ToolbarButton>
      ) : null}
      {splitActionsVisible && onSplitRight && onSplitDown ? (
        <DropdownMenu>
          <ToolbarButton
            kind="menu"
            label={t("workspace.git.actions.moreActions")}
            testID="workspace-split-pane-menu"
          >
            <ThemedEllipsis size={14} uniProps={extraMutedColorMapping} />
          </ToolbarButton>
          <DropdownMenuContent
            side="bottom"
            align="end"
            offset={4}
            width={220}
            testID="workspace-split-pane-menu-content"
          >
            <DropdownMenuItem
              leading={splitRightLeading}
              trailing={splitRightTrailing}
              testID="workspace-split-pane-right"
              onSelect={onSplitRight}
            >
              {t("workspace.tabs.actions.splitRight")}
            </DropdownMenuItem>
            <DropdownMenuItem
              leading={splitDownLeading}
              trailing={splitDownTrailing}
              testID="workspace-split-pane-down"
              onSelect={onSplitDown}
            >
              {t("workspace.tabs.actions.splitDown")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ) : null}
    </ToolbarControls>
  );
}

export function WorkspaceExitFocusModeButton({
  visible,
  onPress,
  onLayout,
}: {
  visible: boolean;
  onPress: () => void;
  onLayout: (event: LayoutChangeEvent) => void;
}) {
  const { t } = useTranslation();
  const focusModeKeys = useShortcutKeys("toggle-focus");
  if (!visible) {
    return null;
  }

  return (
    <View style={styles.exitFocusModeSlot} onLayout={onLayout}>
      <ToolbarButton
        label={t("workspace.tabs.actions.exitFocusMode")}
        shortcut={focusModeKeys}
        testID="workspace-exit-focus-mode"
        onPress={onPress}
      >
        <ThemedX size={14} uniProps={mutedColorMapping} />
      </ToolbarButton>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  exitFocusModeSlot: {
    alignSelf: "stretch",
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: theme.spacing[0.5],
    borderRightWidth: 1,
    borderRightColor: theme.colors.border,
  },
  inlineAddButton: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: theme.spacing[1],
  },
  inlineNewTabButton: {
    width: buttonControlHeight.xs,
    height: buttonControlHeight.xs,
  },
  paneSplitActions: {
    paddingHorizontal: PANE_SPLIT_ACTIONS_HORIZONTAL_PADDING,
    marginRight: PANE_SPLIT_ACTIONS_OUTER_MARGIN,
  },
}));
