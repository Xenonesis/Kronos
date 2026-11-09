import React, {
  Fragment,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import { Pressable, Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react-native";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { TrailingActionScrim } from "@/components/ui/trailing-action-scrim";
import { useFilePanelOpenWithMenu } from "@/file-panels";
import { buttonControlHeight } from "@/components/ui/control-geometry";
import { useIsCompactFormFactor } from "@/constants/layout";
import { isNative, isWeb } from "@/constants/platform";
import type { DraggableListDragHandleProps } from "@/components/draggable-list.types";
import {
  WorkspaceTabPresentationResolver,
  WorkspaceTabIcon,
  type WorkspaceTabPresentation,
} from "@/screens/workspace/workspace-tab-presentation";
import { buildDeterministicWorkspaceTabId } from "@/workspace-tabs/identity";
import {
  buildWorkspaceDesktopTabActions,
  type WorkspaceDesktopTabActions,
  type WorkspaceTabMenuLabels,
} from "@/screens/workspace/workspace-tab-menu";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import type { SurfaceBackdrop } from "@/styles/surface-backdrop";
import {
  resolveWorkspaceTabChipBackdrop,
  type WorkspaceTabRailSurface,
} from "@/screens/workspace/workspace-tab-chip-backdrop";
import type { Theme } from "@/styles/theme";
import {
  DROPDOWN_WIDTH,
  TAB_CHIP_HORIZONTAL_PADDING,
  TAB_CHIP_GAP,
  TAB_ICON_WIDTH,
  TAB_DROP_INDICATOR_WIDTH,
  TAB_MODIFIED_DOT_SIZE,
} from "./tab-geometry";
import {
  AgentTabTooltipBody,
  normalizeAgentTooltipTitle,
  formatAgentTooltipTitle,
} from "./agent-tab-tooltip";
import { TabContextMenuItem } from "./tab-context-menu";
import type { ResolvedWorkspaceDesktopTabRowItem } from "./tab-chip-types";

const ThemedLoadingSpinner = withUnistyles(LoadingSpinner);
const ThemedX = withUnistyles(X);

const foregroundColorMapping = (theme: Theme) => ({
  color: theme.colors.foreground,
});
const mutedColorMapping = (theme: Theme) => ({
  color: theme.colors.foregroundMuted,
});

export function getFallbackTabLabel(
  tab: WorkspaceTabDescriptor,
  labels: {
    newTab: string;
    newAgent: string;
    setup: string;
    terminal: string;
    agent: string;
    changes: string;
    files: string;
    pullRequest: string;
  },
): string {
  if (tab.target.kind === "new_tab") {
    return labels.newTab;
  }
  if (tab.target.kind === "draft") {
    return labels.newAgent;
  }
  if (tab.target.kind === "setup") {
    return labels.setup;
  }
  if (tab.target.kind === "terminal") {
    return labels.terminal;
  }
  if (tab.target.kind === "file") {
    return tab.target.path.split("/").findLast(Boolean) ?? tab.target.path;
  }
  if (tab.target.kind === "working_diff" || tab.target.kind === "changes_tree") {
    return labels.changes;
  }
  if (tab.target.kind === "files") {
    return labels.files;
  }
  if (tab.target.kind === "pull_request") {
    return labels.pullRequest;
  }
  return labels.agent;
}

export function useMiddleClickClose(onClose: () => void) {
  const ref = useRef<View>(null);

  useEffect(() => {
    if (isNative) return;
    const node = ref.current as unknown as HTMLElement | null;
    if (!node) return;

    function handleAuxClick(event: MouseEvent) {
      if (event.button === 1) {
        event.preventDefault();
        onClose();
      }
    }

    node.addEventListener("auxclick", handleAuxClick);
    return () => node.removeEventListener("auxclick", handleAuxClick);
  }, [onClose]);

  return ref;
}

export function TabHandleContent({
  presentation,
  isHighlighted,
  showLabel,
  backdrop,
  tabLabelSkeletonStyle,
  tabLabelStyle,
  modifiedTestId,
}: {
  presentation: WorkspaceTabPresentation;
  isHighlighted: boolean;
  showLabel: boolean;
  backdrop: SurfaceBackdrop;
  tabLabelSkeletonStyle: React.ComponentProps<typeof View>["style"];
  tabLabelStyle: React.ComponentProps<typeof Text>["style"];
  modifiedTestId: string;
}) {
  const { t } = useTranslation();
  const tabHandleDataSet = useMemo(
    () => ({ statusBucket: presentation.statusBucket ?? "none" }),
    [presentation.statusBucket],
  );

  return (
    <View style={styles.tabHandle} dataSet={tabHandleDataSet}>
      <View style={styles.tabIcon}>
        <WorkspaceTabIcon presentation={presentation} active={isHighlighted} backdrop={backdrop} />
      </View>
      {showLabel && presentation.titleState === "loading" ? (
        <View style={tabLabelSkeletonStyle} />
      ) : null}
      {showLabel && presentation.titleState !== "loading" ? (
        <Text style={tabLabelStyle} selectable={false} numberOfLines={1} ellipsizeMode="tail">
          {presentation.label}
        </Text>
      ) : null}
      {/* The dot is a laid-out sibling of the label, not an overlay, so a truncated label ends
          before it instead of running underneath it. */}
      {presentation.modified ? (
        <View
          style={styles.tabModifiedDot}
          accessibilityLabel={t("workspace.tabs.modified")}
          testID={modifiedTestId}
        />
      ) : null}
    </View>
  );
}

export function TabChip({
  serverId,
  workspaceId,
  tab,
  isActive,
  isDragging,
  isFocused,
  resolvedTabWidth,
  showLabel,
  showCloseButton,
  rail,
  isCloseHovered,
  isClosingTab,
  presentation,
  tooltipLabel,
  accessibilityLabel,
  resolvedTab,
  setHoveredCloseTabKey,
  onNavigateTab,
  onCloseTab,
  dragHandleProps,
}: {
  serverId: string;
  workspaceId: string;
  tab: WorkspaceTabDescriptor;
  isActive: boolean;
  isDragging: boolean;
  isFocused: boolean;
  resolvedTabWidth: number;
  showLabel: boolean;
  showCloseButton: boolean;
  rail: WorkspaceTabRailSurface;
  isCloseHovered: boolean;
  isClosingTab: boolean;
  presentation: WorkspaceTabPresentation;
  tooltipLabel: string;
  accessibilityLabel: string;
  resolvedTab: WorkspaceDesktopTabActions;
  setHoveredCloseTabKey: Dispatch<SetStateAction<string | null>>;
  onNavigateTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => Promise<void> | void;
  dragHandleProps: DraggableListDragHandleProps | undefined;
}) {
  const { closeButtonTestId, contextMenuTestId, menuEntries } = resolvedTab;
  const { t } = useTranslation();
  const openWith = useFilePanelOpenWithMenu(
    { serverId, workspaceId, tabId: tab.tabId },
    contextMenuTestId,
  );
  const middleClickRef = useMiddleClickClose(
    useCallback(() => void onCloseTab(tab.tabId), [onCloseTab, tab.tabId]),
  );
  const isCompact = useIsCompactFormFactor();
  const [hovered, setHovered] = useState(false);
  const isActiveFocused = isActive && isFocused;
  const isHovered = hovered || isCloseHovered;
  const isHighlighted = isActiveFocused || isHovered;
  const isFilled = isActive || isHovered;
  const chipBackdrop: SurfaceBackdrop = resolveWorkspaceTabChipBackdrop({
    rail,
    isActiveFocused,
    isFilled,
  });
  const showCloseControl = showCloseButton && (isHovered || isNative || isCompact || isClosingTab);
  const closeButtonDragBlockers = isWeb
    ? ({
        onPointerDown: (event: { stopPropagation?: () => void }) => {
          event.stopPropagation?.();
        },
        onMouseDown: (event: { stopPropagation?: () => void }) => {
          event.stopPropagation?.();
        },
      } as const)
    : undefined;

  const tabChipStyle = useCallback(
    () => [
      styles.tab,
      isFilled && styles.tabFill(chipBackdrop),
      isWeb && isDragging && ({ cursor: "grabbing" } as object),
      {
        minWidth: resolvedTabWidth,
        width: resolvedTabWidth,
        maxWidth: resolvedTabWidth,
      },
    ],
    [chipBackdrop, isDragging, isFilled, resolvedTabWidth],
  );

  const handleTabPointerEnter = useCallback(() => {
    setHovered(true);
  }, []);

  const handleTabPointerLeave = useCallback(() => {
    setHovered(false);
  }, []);

  const handleNavigateTab = useCallback(() => {
    onNavigateTab(tab.tabId);
  }, [onNavigateTab, tab.tabId]);

  const handleCloseButtonPressIn = useCallback((event: { stopPropagation?: () => void }) => {
    event.stopPropagation?.();
  }, []);

  const handleCloseButtonHoverIn = useCallback(() => {
    setHoveredCloseTabKey(tab.key);
  }, [setHoveredCloseTabKey, tab.key]);

  const handleCloseButtonHoverOut = useCallback(() => {
    setHoveredCloseTabKey((current) => (current === tab.key ? null : current));
  }, [setHoveredCloseTabKey, tab.key]);

  const handleCloseButtonPress = useCallback(
    (event: { stopPropagation?: () => void }) => {
      event.stopPropagation?.();
      void onCloseTab(tab.tabId);
    },
    [onCloseTab, tab.tabId],
  );

  const tabAccessibilityState = useMemo(() => ({ selected: isActive }), [isActive]);
  const testIdentity =
    tab.target.kind === "new_tab" ? tab.tabId : buildDeterministicWorkspaceTabId(tab.target);
  const tabLabelSkeletonStyle = styles.tabLabelSkeleton;
  const tabLabelStyle = useMemo(
    () => [styles.tabLabel, isHighlighted && styles.tabLabelActive],
    [isHighlighted],
  );

  return (
    <View
      ref={middleClickRef}
      style={styles.tabHoverFrame}
      onPointerEnter={handleTabPointerEnter}
      onPointerLeave={handleTabPointerLeave}
    >
      <ContextMenu key={tab.key}>
        <Tooltip delayDuration={400} enabledOnDesktop enabledOnMobile={false}>
          <TooltipTrigger asChild triggerRefProp="triggerRef">
            <ContextMenuTrigger
              {...(dragHandleProps?.attributes as object | undefined)}
              {...(dragHandleProps?.listeners as object | undefined)}
              testID={`workspace-tab-${testIdentity}`}
              triggerRef={dragHandleProps?.setActivatorNodeRef as unknown as undefined}
              style={tabChipStyle}
              onPress={handleNavigateTab}
              accessibilityRole="button"
              accessibilityLabel={accessibilityLabel}
              accessibilityState={tabAccessibilityState}
              aria-selected={isActive}
            >
              <TabHandleContent
                presentation={presentation}
                isHighlighted={isHighlighted}
                showLabel={showLabel}
                backdrop={chipBackdrop}
                tabLabelSkeletonStyle={tabLabelSkeletonStyle}
                tabLabelStyle={tabLabelStyle}
                modifiedTestId={`workspace-tab-modified-${testIdentity}`}
              />
            </ContextMenuTrigger>
          </TooltipTrigger>
          <TooltipContent
            side="bottom"
            align="center"
            offset={8}
            maxWidth={720}
            testID={`workspace-tab-tooltip-${testIdentity}`}
          >
            {tab.target.kind === "agent" ? (
              <AgentTabTooltipBody
                serverId={serverId}
                agentId={tab.target.agentId}
                title={tooltipLabel}
              />
            ) : (
              <Text style={styles.newTabTooltipText}>{tooltipLabel}</Text>
            )}
          </TooltipContent>
        </Tooltip>

        {showCloseButton ? (
          <View
            pointerEvents={showCloseControl ? "box-none" : "none"}
            style={[
              styles.tabTrailingOverlay,
              showCloseControl ? styles.tabTrailingOverlayShown : styles.tabTrailingOverlayHidden,
            ]}
          >
            <TrailingActionScrim backdrop={chipBackdrop} />
            <Pressable
              {...(closeButtonDragBlockers as object | undefined)}
              testID={closeButtonTestId}
              accessibilityRole="button"
              accessibilityLabel={t("workspace.tabs.menu.close")}
              disabled={isClosingTab}
              onPressIn={handleCloseButtonPressIn}
              onHoverIn={handleCloseButtonHoverIn}
              onHoverOut={handleCloseButtonHoverOut}
              onPress={handleCloseButtonPress}
              style={styles.tabCloseButton}
            >
              {({ hovered: closeHovered, pressed }) => {
                const highlighted = closeHovered || pressed;
                if (isClosingTab) {
                  return (
                    <ThemedLoadingSpinner
                      size={12}
                      uniProps={highlighted ? foregroundColorMapping : mutedColorMapping}
                    />
                  );
                }
                return (
                  <ThemedX
                    size={12}
                    uniProps={highlighted ? foregroundColorMapping : mutedColorMapping}
                  />
                );
              }}
            </Pressable>
          </View>
        ) : null}

        <ContextMenuContent
          align="start"
          width={DROPDOWN_WIDTH}
          pages={openWith.pages}
          testID={contextMenuTestId}
        >
          {menuEntries.map((entry) => {
            if (entry.kind === "separator") return <ContextMenuSeparator key={entry.key} />;
            if (entry.kind === "file-panel-open-with") {
              return <Fragment key={entry.key}>{openWith.trigger}</Fragment>;
            }
            return <TabContextMenuItem key={entry.key} entry={entry} />;
          })}
        </ContextMenuContent>
      </ContextMenu>
    </View>
  );
}

export function ResolvedDesktopTabChip({
  serverId,
  workspaceId,
  item,
  isFocused,
  isDragging,
  index,
  tabCount,
  onCopyResumeCommand,
  onCopyAgentId,
  onCopyTerminalId,
  onCopyFilePath,
  onReloadAgent,
  onRenameTab,
  onCloseTabsToLeft,
  onCloseTabsToRight,
  onCloseOtherTabs,
  resolvedTabWidth,
  showLabel,
  showCloseButton,
  rail,
  setHoveredCloseTabKey,
  onNavigateTab,
  onCloseTab,
  labels,
  dragHandleProps,
  showDropIndicatorBefore,
  showDropIndicatorAfter,
}: {
  serverId: string;
  workspaceId: string;
  item: ResolvedWorkspaceDesktopTabRowItem;
  isFocused: boolean;
  isDragging: boolean;
  index: number;
  tabCount: number;
  onCopyResumeCommand: (agentId: string) => Promise<void> | void;
  onCopyAgentId: (agentId: string) => Promise<void> | void;
  onCopyTerminalId: (terminalId: string) => Promise<void> | void;
  onCopyFilePath: (path: string) => Promise<void> | void;
  onReloadAgent: (agentId: string) => Promise<void> | void;
  onRenameTab: (tab: WorkspaceTabDescriptor) => void;
  onCloseTabsToLeft: (tabId: string) => Promise<void> | void;
  onCloseTabsToRight: (tabId: string) => Promise<void> | void;
  onCloseOtherTabs: (tabId: string) => Promise<void> | void;
  resolvedTabWidth: number;
  showLabel: boolean;
  showCloseButton: boolean;
  rail: WorkspaceTabRailSurface;
  setHoveredCloseTabKey: Dispatch<SetStateAction<string | null>>;
  onNavigateTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => Promise<void> | void;
  labels: WorkspaceTabMenuLabels;
  dragHandleProps: DraggableListDragHandleProps | undefined;
  showDropIndicatorBefore: boolean;
  showDropIndicatorAfter: boolean;
}) {
  const { t } = useTranslation();
  const presentation = item.presentation;
  const resolvedTab = useMemo(
    () =>
      buildWorkspaceDesktopTabActions({
        tab: item.tab,
        index,
        tabCount,
        onCopyResumeCommand,
        onCopyAgentId,
        onCopyTerminalId,
        onCopyFilePath,
        onReloadAgent,
        onRenameTab,
        onCloseTab,
        onCloseTabsToLeft,
        onCloseTabsToRight,
        onCloseOtherTabs,
        labels,
      }),
    [
      index,
      item.tab,
      onCloseOtherTabs,
      onCloseTab,
      onCloseTabsToLeft,
      onCloseTabsToRight,
      onCopyAgentId,
      onCopyTerminalId,
      onCopyFilePath,
      onCopyResumeCommand,
      labels,
      onReloadAgent,
      onRenameTab,
      tabCount,
    ],
  );

  const rawTooltipLabel =
    presentation.titleState === "loading" ? t("common.states.loading") : presentation.tooltip;
  const accessibilityLabel =
    item.tab.target.kind === "agent"
      ? normalizeAgentTooltipTitle(rawTooltipLabel)
      : rawTooltipLabel;
  const tooltipLabel =
    item.tab.target.kind === "agent"
      ? formatAgentTooltipTitle(accessibilityLabel)
      : rawTooltipLabel;

  return (
    <View style={styles.tabSlot}>
      {showDropIndicatorBefore ? (
        <View style={[styles.tabDropIndicator, styles.tabDropIndicatorBefore]} />
      ) : null}
      <TabChip
        serverId={serverId}
        workspaceId={workspaceId}
        tab={item.tab}
        isActive={item.isActive}
        isDragging={isDragging}
        isFocused={isFocused}
        resolvedTabWidth={resolvedTabWidth}
        showLabel={showLabel}
        showCloseButton={showCloseButton}
        rail={rail}
        isCloseHovered={item.isCloseHovered}
        isClosingTab={item.isClosingTab}
        presentation={presentation}
        tooltipLabel={tooltipLabel}
        accessibilityLabel={accessibilityLabel}
        resolvedTab={resolvedTab}
        setHoveredCloseTabKey={setHoveredCloseTabKey}
        onNavigateTab={onNavigateTab}
        onCloseTab={onCloseTab}
        dragHandleProps={dragHandleProps}
      />
      {showDropIndicatorAfter ? (
        <View style={[styles.tabDropIndicator, styles.tabDropIndicatorAfter]} />
      ) : null}
    </View>
  );
}

export function WorkspaceDesktopTabPresentationSlot({
  tab,
  serverId,
  workspaceId,
  onResolve,
}: {
  tab: WorkspaceTabDescriptor;
  serverId: string;
  workspaceId: string;
  onResolve: (tabKey: string, presentation: WorkspaceTabPresentation) => void;
}) {
  return (
    <WorkspaceTabPresentationResolver tab={tab} serverId={serverId} workspaceId={workspaceId}>
      {(presentation) => (
        <WorkspaceDesktopTabPresentationCommit
          tabKey={tab.key}
          presentation={presentation}
          onResolve={onResolve}
        />
      )}
    </WorkspaceTabPresentationResolver>
  );
}

export function WorkspaceDesktopTabPresentationCommit({
  tabKey,
  presentation,
  onResolve,
}: {
  tabKey: string;
  presentation: WorkspaceTabPresentation;
  onResolve: (tabKey: string, presentation: WorkspaceTabPresentation) => void;
}) {
  useLayoutEffect(() => {
    onResolve(tabKey, presentation);
  }, [onResolve, presentation, tabKey]);
  return null;
}

const styles = StyleSheet.create((theme) => ({
  tab: {
    height: buttonControlHeight.xs,
    paddingHorizontal: TAB_CHIP_HORIZONTAL_PADDING,
    borderRadius: theme.borderRadius.md,
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
    userSelect: "none",
  },
  tabFill: (backdrop: SurfaceBackdrop) => ({
    backgroundColor: theme.colors[backdrop],
  }),
  tabHoverFrame: {
    position: "relative",
  },
  tabSlot: {
    position: "relative",
    overflow: "visible",
    marginHorizontal: TAB_CHIP_GAP / 2,
  },
  tabHandle: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
    flex: 1,
    minWidth: 0,
    userSelect: "none",
  },
  tabIcon: {
    width: TAB_ICON_WIDTH,
    height: TAB_ICON_WIDTH,
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
  },
  tabDropIndicator: {
    position: "absolute",
    top: theme.spacing[0.5],
    bottom: theme.spacing[0.5],
    width: TAB_DROP_INDICATOR_WIDTH,
    borderRadius: theme.borderRadius.full,
    backgroundColor: theme.colors.accent,
    zIndex: 10,
    pointerEvents: "none",
  },
  tabDropIndicatorBefore: {
    left: -TAB_CHIP_GAP / 2 - TAB_DROP_INDICATOR_WIDTH / 2,
  },
  tabDropIndicatorAfter: {
    right: -TAB_CHIP_GAP / 2 - TAB_DROP_INDICATOR_WIDTH / 2,
  },
  tabLabel: {
    flexShrink: 1,
    minWidth: 0,
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.normal,
    userSelect: "none",
  },
  tabLabelSkeleton: {
    width: 96,
    maxWidth: "100%",
    flexShrink: 1,
    minWidth: 0,
    height: 10,
    borderRadius: theme.borderRadius.full,
    backgroundColor: theme.colors.surface3,
    opacity: 0.9,
  },
  tabLabelActive: {
    color: theme.colors.foreground,
  },
  tabTrailingOverlay: {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    width: 48,
    borderTopRightRadius: theme.borderRadius.md,
    borderBottomRightRadius: theme.borderRadius.md,
    alignItems: "center",
    justifyContent: "center",
    zIndex: 2,
    overflow: "hidden",
  },
  tabTrailingOverlayShown: {
    opacity: 1,
  },
  tabTrailingOverlayHidden: {
    opacity: 0,
  },
  tabCloseButton: {
    position: "absolute",
    right: 4,
    width: 18,
    height: 18,
    borderRadius: theme.borderRadius.sm,
    alignItems: "center",
    justifyContent: "center",
    zIndex: 1,
  },
  tabModifiedDot: {
    width: TAB_MODIFIED_DOT_SIZE,
    height: TAB_MODIFIED_DOT_SIZE,
    flexShrink: 0,
    borderRadius: theme.borderRadius.full,
    backgroundColor: theme.colors.foregroundMuted,
  },
  newTabTooltipText: {
    color: theme.colors.foreground,
    fontSize: theme.fontSize.base,
  },
}));
