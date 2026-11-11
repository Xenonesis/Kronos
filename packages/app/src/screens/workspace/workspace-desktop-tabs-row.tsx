import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import { View, type LayoutChangeEvent, type ScrollView } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import Animated from "react-native-reanimated";
import { useTranslation } from "react-i18next";
import { SortableInlineList } from "@/components/sortable-inline-list";
import type { DraggableRenderItemInfo } from "@/components/draggable-list.types";
import { useShortcutKeys } from "@/hooks/use-shortcut-keys";
import { WORKSPACE_SECONDARY_HEADER_HEIGHT, useIsCompactFormFactor } from "@/constants/layout";
import { useWorkspaceTabLayout } from "@/screens/workspace/use-workspace-tab-layout";
import type { WorkspaceTabPresentation } from "@/screens/workspace/workspace-tab-presentation";
import type { WorkspaceTabMenuLabels } from "@/screens/workspace/workspace-tab-menu";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import type { PaneHost } from "@/panels/panel-manifest";
import type { WorkspaceTabLaunchPurpose } from "@/workspace-tabs/launcher";
import type { WorkspaceTabRailSurface } from "@/screens/workspace/workspace-tab-chip-backdrop";
import { RenderProfile } from "@/utils/render-profiler";
import { useKeyboardActionHandler } from "@/hooks/use-keyboard-action-handler";
import { buildWorkspaceKeyboardHandlerId } from "@/keyboard/handler-id";
import type { KeyboardActionDefinition } from "@/keyboard/keyboard-action-dispatcher";
import {
  HorizontalScrollBoundaryShades,
  useHorizontalScrollBoundary,
} from "@/components/ui/horizontal-scroll-boundary";

import {
  DEFAULT_INLINE_ADD_BUTTON_RESERVED_WIDTH,
  PANE_SPLIT_ACTIONS_RESERVED_WIDTH,
  PANE_MAXIMIZE_ACTION_RESERVED_WIDTH,
  TAB_CHIP_HORIZONTAL_PADDING,
  TAB_CHIP_GAP,
  TAB_ROW_PADDING_HORIZONTAL,
  TAB_ICON_WIDTH,
  TAB_CONTENT_GAP,
  TAB_MIN_WIDTH,
  TAB_MAX_WIDTH,
  TAB_CLOSE_BUTTON_RESERVED_WIDTH,
  updateMeasuredWidth,
} from "./desktop-tabs/tab-geometry";
import {
  type WorkspaceTabLabelMeasurement,
  type WorkspaceTabTrackSnapshot,
  TabLabelMeasurement,
  completeWorkspaceTabLabelWidths,
  sameWidths,
  workspaceTabLabelSignature,
} from "./desktop-tabs/tab-measurements";
import type {
  WorkspaceDesktopTabRowItem,
  ResolvedWorkspaceDesktopTabRowItem,
} from "./desktop-tabs/tab-chip-types";
import {
  ResolvedDesktopTabChip,
  WorkspaceDesktopTabPresentationSlot,
  getFallbackTabLabel,
} from "./desktop-tabs/tab-chip";
import {
  WorkspaceNewTabButton,
  WorkspacePaneToolbarActions,
  WorkspaceExitFocusModeButton,
} from "./desktop-tabs/pane-toolbar-actions";

export type { WorkspaceDesktopTabRowItem, ResolvedWorkspaceDesktopTabRowItem };

const TabMeasurementCache = createContext<Map<string, WorkspaceTabLabelMeasurement> | null>(null);

export function WorkspaceTabMeasurementScope({ children }: { children: React.ReactNode }) {
  const cache = useMemo(() => new Map<string, WorkspaceTabLabelMeasurement>(), []);
  return <TabMeasurementCache.Provider value={cache}>{children}</TabMeasurementCache.Provider>;
}

export function useWorkspaceNewTabKeyboardAction({
  handlerName = "workspace-new-tab",
  ownsKeyboardShortcuts,
  normalizedServerId,
  normalizedWorkspaceId,
  paneId,
  createNewTab,
}: {
  handlerName?: string;
  ownsKeyboardShortcuts: boolean;
  normalizedServerId: string;
  normalizedWorkspaceId: string;
  paneId?: string;
  createNewTab: () => void;
}) {
  const handleNewTabKeyboardAction = useCallback(
    (action: KeyboardActionDefinition): boolean => {
      if (!ownsKeyboardShortcuts) return false;
      if (action.id === "workspace.tab.menu.open") {
        createNewTab();
        return true;
      }
      return false;
    },
    [createNewTab, ownsKeyboardShortcuts],
  );

  useKeyboardActionHandler({
    handlerId: buildWorkspaceKeyboardHandlerId({
      name: handlerName,
      serverId: normalizedServerId,
      workspaceId: normalizedWorkspaceId,
      paneId,
    }),
    actions: ["workspace.tab.menu.open"],
    enabled: ownsKeyboardShortcuts,
    priority: 200,
    handle: handleNewTabKeyboardAction,
  });
}

function tabKeyExtractor(tab: WorkspaceDesktopTabRowItem) {
  return `${tab.tab.key}:${tab.tab.kind}`;
}

export interface WorkspaceDesktopTabsRowProps {
  host?: PaneHost;
  launchPurpose?: WorkspaceTabLaunchPurpose;
  paneId?: string;
  isFocused?: boolean;
  ownsKeyboardShortcuts?: boolean;
  tabs: WorkspaceDesktopTabRowItem[];
  normalizedServerId: string;
  normalizedWorkspaceId: string;
  setHoveredCloseTabKey: Dispatch<SetStateAction<string | null>>;
  onNavigateTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => Promise<void> | void;
  onCopyResumeCommand: (agentId: string) => Promise<void> | void;
  onCopyAgentId: (agentId: string) => Promise<void> | void;
  onCopyTerminalId: (terminalId: string) => Promise<void> | void;
  onCopyFilePath: (path: string) => Promise<void> | void;
  onReloadAgent: (agentId: string) => Promise<void> | void;
  onRenameTab: (tab: WorkspaceTabDescriptor) => void;
  onCloseTabsToLeft: (tabId: string) => Promise<void> | void;
  onCloseTabsToRight: (tabId: string) => Promise<void> | void;
  onCloseOtherTabs: (tabId: string) => Promise<void> | void;
  onCreateNewTab: (input: { paneId?: string }) => void;
  onReorderTabs: (nextTabs: WorkspaceTabDescriptor[]) => void;
  externalDndContext?: boolean;
  activeDragTabId?: string | null;
  tabDropPreviewIndex?: number | null;
  showPaneSplitActions?: boolean;
  showPaneMaximizeAction?: boolean;
  paneMaximized?: boolean;
  onTogglePaneMaximized?: () => void;
  onSplitRight?: () => void;
  onSplitDown?: () => void;
  focusModeEnabled: boolean;
  onExitFocusMode: () => void;
}

interface ResolvedWorkspaceDesktopTabsRowProps extends Omit<WorkspaceDesktopTabsRowProps, "tabs"> {
  tabs: ResolvedWorkspaceDesktopTabRowItem[];
}

const EMPTY_RESOLVED_TAB_ROWS: ResolvedWorkspaceDesktopTabRowItem[] = [];

export function WorkspaceDesktopTabsRow(props: WorkspaceDesktopTabsRowProps) {
  const [presentations, setPresentations] = useState(
    () => new Map<string, WorkspaceTabPresentation>(),
  );
  const handlePresentation = useCallback(
    (tabKey: string, presentation: WorkspaceTabPresentation) => {
      setPresentations((current) => {
        if (current.get(tabKey) === presentation) {
          return current;
        }
        const next = new Map(current);
        next.set(tabKey, presentation);
        return next;
      });
    },
    [],
  );
  const currentTabKeys = useMemo(
    () => new Set(props.tabs.map((item) => item.tab.key)),
    [props.tabs],
  );
  useEffect(() => {
    setPresentations((current) => {
      const removedKeys = [...current.keys()].filter((key) => !currentTabKeys.has(key));
      if (removedKeys.length === 0) {
        return current;
      }
      const next = new Map(current);
      for (const key of removedKeys) {
        next.delete(key);
      }
      return next;
    });
  }, [currentTabKeys]);
  const resolvedTabs = useMemo(
    () =>
      props.tabs.flatMap((item) => {
        const presentation = presentations.get(item.tab.key);
        return presentation ? [{ ...item, presentation }] : [];
      }),
    [presentations, props.tabs],
  );

  return (
    <>
      <ResolvedWorkspaceDesktopTabsRow {...props} tabs={resolvedTabs} />
      {props.tabs.map(({ tab }) => (
        <WorkspaceDesktopTabPresentationSlot
          key={`${tab.key}:${tab.kind}`}
          tab={tab}
          serverId={props.normalizedServerId}
          workspaceId={props.normalizedWorkspaceId}
          onResolve={handlePresentation}
        />
      ))}
    </>
  );
}

function ResolvedWorkspaceDesktopTabsRow({
  host = "main",
  launchPurpose = "primary",
  paneId,
  isFocused = false,
  ownsKeyboardShortcuts = isFocused,
  tabs,
  normalizedServerId,
  normalizedWorkspaceId,
  setHoveredCloseTabKey,
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
  onReorderTabs,
  externalDndContext = false,
  activeDragTabId = null,
  tabDropPreviewIndex = null,
  showPaneSplitActions = false,
  showPaneMaximizeAction = false,
  paneMaximized = false,
  onTogglePaneMaximized,
  onSplitRight,
  onSplitDown,
  focusModeEnabled,
  onExitFocusMode,
}: ResolvedWorkspaceDesktopTabsRowProps) {
  const { t } = useTranslation();
  const isCompact = useIsCompactFormFactor();
  const newTabKeys = useShortcutKeys("workspace-tab-new");
  const [tabsContainerWidth, setTabsContainerWidth] = useState<number>(0);
  const tabsScrollRef = useRef<ScrollView>(null);
  const [exitFocusModeWidth, setExitFocusModeWidth] = useState<number>(0);
  const tabScrollBoundary = useHorizontalScrollBoundary();
  const measurementCache = useContext(TabMeasurementCache);
  const [labelMeasurements, setLabelMeasurements] = useState(() => new Map(measurementCache ?? []));
  useLayoutEffect(() => {
    if (!measurementCache) return;
    measurementCache.clear();
    for (const [key, value] of labelMeasurements) measurementCache.set(key, value);
  }, [measurementCache, labelMeasurements]);
  const [trackSnapshot, setTrackSnapshot] = useState<WorkspaceTabTrackSnapshot | null>(null);

  const handleTabsContainerLayout = useCallback((event: LayoutChangeEvent) => {
    updateMeasuredWidth(setTabsContainerWidth, event);
  }, []);

  const handleExitFocusModeLayout = useCallback((event: LayoutChangeEvent) => {
    updateMeasuredWidth(setExitFocusModeWidth, event);
  }, []);

  const layoutMetrics = useMemo(
    () => ({
      rowHorizontalInset: 0,
      actionsReservedWidth: Math.max(
        0,
        DEFAULT_INLINE_ADD_BUTTON_RESERVED_WIDTH +
          (focusModeEnabled ? exitFocusModeWidth : 0) +
          (showPaneSplitActions ? PANE_SPLIT_ACTIONS_RESERVED_WIDTH : 0) +
          (showPaneMaximizeAction ? PANE_MAXIMIZE_ACTION_RESERVED_WIDTH : 0),
      ),
      rowPaddingHorizontal: TAB_ROW_PADDING_HORIZONTAL,
      tabGap: TAB_CHIP_GAP,
      minTabWidth: TAB_MIN_WIDTH,
      maxTabWidth: TAB_MAX_WIDTH,
      tabIconWidth: TAB_ICON_WIDTH,
      tabContentGap: TAB_CONTENT_GAP,
      tabHorizontalPadding: TAB_CHIP_HORIZONTAL_PADDING,
      closeButtonWidth: TAB_CLOSE_BUTTON_RESERVED_WIDTH,
    }),
    [exitFocusModeWidth, focusModeEnabled, showPaneMaximizeAction, showPaneSplitActions],
  );

  const panePanelKinds = useMemo(() => tabs.map(({ tab }) => tab.kind), [tabs]);

  const fallbackTabLabels = useMemo(
    () => ({
      newTab: t("workspace.tabs.actions.newTab"),
      newAgent: t("workspace.tabs.fallback.newAgent"),
      setup: t("workspace.tabs.fallback.setup"),
      terminal: t("workspace.tabs.fallback.terminal"),
      agent: t("workspace.tabs.fallback.agent"),
      changes: t("panels.diff.changesLabel"),
      files: t("panels.files.label"),
      pullRequest: t("panels.pullRequest.label"),
    }),
    [t],
  );

  const tabMenuLabels = useMemo<WorkspaceTabMenuLabels>(
    () => ({
      copyResumeCommand: t("workspace.tabs.menu.copyResumeCommand"),
      copyAgentId: t("workspace.tabs.menu.copyAgentId"),
      copyTerminalId: t("workspace.tabs.menu.copyTerminalId"),
      copyFilePath: t("workspace.tabs.menu.copyFilePath"),
      rename: t("workspace.tabs.menu.rename"),
      closeAbove: t("workspace.tabs.menu.closeAbove"),
      closeBelow: t("workspace.tabs.menu.closeBelow"),
      closeLeft: t("workspace.tabs.menu.closeLeft"),
      closeRight: t("workspace.tabs.menu.closeRight"),
      closeOthers: t("workspace.tabs.menu.closeOthers"),
      reloadAgent: t("workspace.tabs.menu.reloadAgent"),
      reloadAgentTooltip: t("workspace.tabs.menu.reloadAgentTooltip"),
      close: t("workspace.tabs.menu.close"),
    }),
    [t],
  );

  const tabLabels = useMemo(
    () =>
      tabs.map((tab) => {
        const label =
          tab.presentation.titleState === "loading"
            ? getFallbackTabLabel(tab.tab, fallbackTabLabels)
            : tab.presentation.label;
        return { key: tab.tab.key, label, modified: tab.presentation.modified };
      }),
    [fallbackTabLabels, tabs],
  );

  const tabLabelSignature = useMemo(() => workspaceTabLabelSignature(tabLabels), [tabLabels]);
  const currentTabLabelKeys = useMemo(() => new Set(tabLabels.map(({ key }) => key)), [tabLabels]);

  useEffect(() => {
    setLabelMeasurements((current) => {
      const removedKeys = [...current.keys()].filter((key) => !currentTabLabelKeys.has(key));
      if (removedKeys.length === 0) {
        return current;
      }
      const next = new Map(current);
      for (const key of removedKeys) {
        next.delete(key);
      }
      return next;
    });
  }, [currentTabLabelKeys]);

  const publishMeasuredTrack = useCallback(() => {
    if (tabsContainerWidth <= 0) {
      return;
    }
    const labelWidths = completeWorkspaceTabLabelWidths(tabLabels, labelMeasurements);
    if (!labelWidths) {
      return;
    }

    setTrackSnapshot((current) => {
      if (
        current?.signature === tabLabelSignature &&
        sameWidths(current.labelWidths, labelWidths)
      ) {
        return current;
      }
      return {
        signature: tabLabelSignature,
        tabs,
        labels: tabLabels,
        labelWidths,
      };
    });
  }, [labelMeasurements, tabLabelSignature, tabLabels, tabs, tabsContainerWidth]);

  useLayoutEffect(() => {
    publishMeasuredTrack();
  }, [publishMeasuredTrack]);

  const handleTabLabelLayout = useCallback(
    (key: string, label: string, event: LayoutChangeEvent) => {
      const width = Math.ceil(event.nativeEvent.layout.width);
      if (width <= 0) {
        return;
      }
      setLabelMeasurements((current) => {
        const measurement = current.get(key);
        if (measurement?.label === label && measurement.width === width) {
          return current;
        }
        const next = new Map(current);
        next.set(key, { label, width });
        return next;
      });
    },
    [],
  );

  const displayedTabs = useMemo(() => {
    if (!trackSnapshot) {
      return EMPTY_RESOLVED_TAB_ROWS;
    }
    const currentTabs = new Map(
      tabs.map((tab, index) => [tab.tab.key, { tab, label: tabLabels[index]?.label }]),
    );
    return trackSnapshot.tabs.map((snapshotTab, index) => {
      const current = currentTabs.get(snapshotTab.tab.key);
      return current?.label === trackSnapshot.labels[index]?.label ? current.tab : snapshotTab;
    });
  }, [tabLabels, tabs, trackSnapshot]);

  const { layout } = useWorkspaceTabLayout({
    tabLabelWidths: trackSnapshot?.labelWidths ?? [],
    viewportWidthOverride: tabsContainerWidth > 0 ? tabsContainerWidth : null,
    metrics: layoutMetrics,
    compact: isCompact,
  });

  // A scroll view keeps its offset when its content shrinks under the viewport, so a row that
  // stops scrolling would leave the tabs it had panned past out of view with no way back.
  useEffect(() => {
    if (!layout.requiresHorizontalScrollFallback) {
      tabsScrollRef.current?.scrollTo({ x: 0, animated: false });
    }
  }, [layout.requiresHorizontalScrollFallback]);

  const handleDragEnd = useCallback(
    (nextTabs: ResolvedWorkspaceDesktopTabRowItem[]) => {
      onReorderTabs(nextTabs.map((tab) => tab.tab));
    },
    [onReorderTabs],
  );

  const getTabDragData = useMemo(() => {
    if (!paneId) return undefined;
    return (tab: ResolvedWorkspaceDesktopTabRowItem) => ({
      kind: "workspace-tab" as const,
      paneId,
      tabId: tab.tab.tabId,
    });
  }, [paneId]);

  const createNewTab = useCallback(() => onCreateNewTab({ paneId }), [onCreateNewTab, paneId]);

  useWorkspaceNewTabKeyboardAction({
    ownsKeyboardShortcuts,
    normalizedServerId,
    normalizedWorkspaceId,
    paneId,
    createNewTab,
  });

  const rail: WorkspaceTabRailSurface = host === "explorer" ? "sidebar" : "surface";
  const onSidebar = rail === "sidebar";

  const renderTab = useCallback(
    ({
      item,
      index,
      dragHandleProps,
      isActive,
    }: DraggableRenderItemInfo<ResolvedWorkspaceDesktopTabRowItem>) => {
      const shouldShowCloseButton =
        layout.closeButtonPolicy === "all" && item.presentation.showCloseButton;
      const layoutItem = layout.items[index] ?? null;
      const resolvedTabWidth = layoutItem?.width ?? 150;
      const showLabel = layoutItem?.showLabel ?? true;
      const showDropIndicatorBefore = activeDragTabId !== null && tabDropPreviewIndex === index;
      const showDropIndicatorAfter =
        activeDragTabId !== null &&
        tabDropPreviewIndex === displayedTabs.length &&
        index === displayedTabs.length - 1;

      return (
        <ResolvedDesktopTabChip
          key={`${item.tab.key}:${item.tab.kind}`}
          serverId={normalizedServerId}
          workspaceId={normalizedWorkspaceId}
          item={item}
          isFocused={isFocused}
          isDragging={isActive}
          index={index}
          tabCount={displayedTabs.length}
          onCopyResumeCommand={onCopyResumeCommand}
          onCopyAgentId={onCopyAgentId}
          onCopyTerminalId={onCopyTerminalId}
          onCopyFilePath={onCopyFilePath}
          onReloadAgent={onReloadAgent}
          onRenameTab={onRenameTab}
          onCloseTabsToLeft={onCloseTabsToLeft}
          onCloseTabsToRight={onCloseTabsToRight}
          onCloseOtherTabs={onCloseOtherTabs}
          resolvedTabWidth={resolvedTabWidth}
          showLabel={showLabel}
          showCloseButton={shouldShowCloseButton}
          rail={rail}
          setHoveredCloseTabKey={setHoveredCloseTabKey}
          onNavigateTab={onNavigateTab}
          onCloseTab={onCloseTab}
          labels={tabMenuLabels}
          dragHandleProps={dragHandleProps}
          showDropIndicatorBefore={showDropIndicatorBefore}
          showDropIndicatorAfter={showDropIndicatorAfter}
        />
      );
    },
    [
      activeDragTabId,
      displayedTabs.length,
      isFocused,
      layout.closeButtonPolicy,
      layout.items,
      normalizedServerId,
      normalizedWorkspaceId,
      onCloseOtherTabs,
      onCloseTab,
      onCloseTabsToLeft,
      onCloseTabsToRight,
      onCopyAgentId,
      onCopyTerminalId,
      onCopyFilePath,
      onCopyResumeCommand,
      onNavigateTab,
      onReloadAgent,
      onRenameTab,
      rail,
      setHoveredCloseTabKey,
      tabDropPreviewIndex,
      tabMenuLabels,
    ],
  );

  const tabsScrollStyle = useMemo(
    () => [
      styles.tabsScroll,
      layout.requiresHorizontalScrollFallback
        ? styles.tabsScrollOverflow
        : styles.tabsScrollFitContent,
    ],
    [layout.requiresHorizontalScrollFallback],
  );

  const row = (
    <View
      style={[styles.tabsContainer, onSidebar && styles.tabsContainerSidebar]}
      testID="workspace-tabs-row"
      onLayout={handleTabsContainerLayout}
    >
      <View
        style={styles.tabLabelMeasurements}
        pointerEvents="none"
        accessibilityElementsHidden
        importantForAccessibility="no-hide-descendants"
      >
        {tabLabels.map(({ key, label }) => (
          <TabLabelMeasurement
            key={`${key}:${label}`}
            tabKey={key}
            label={label}
            onMeasure={handleTabLabelLayout}
          />
        ))}
      </View>
      <WorkspaceExitFocusModeButton
        visible={focusModeEnabled}
        onPress={onExitFocusMode}
        onLayout={handleExitFocusModeLayout}
      />
      <View style={styles.tabsScrollContainer}>
        <Animated.ScrollView
          ref={tabsScrollRef}
          horizontal
          scrollEnabled={layout.requiresHorizontalScrollFallback}
          testID="workspace-tabs-scroll"
          style={tabsScrollStyle}
          contentContainerStyle={styles.tabsContent}
          showsHorizontalScrollIndicator={false}
          onLayout={tabScrollBoundary.onLayout}
          onContentSizeChange={tabScrollBoundary.onContentSizeChange}
          onScroll={tabScrollBoundary.onScroll}
          scrollEventThrottle={16}
        >
          <SortableInlineList
            data={displayedTabs}
            keyExtractor={tabKeyExtractor}
            useDragHandle
            disabled={!externalDndContext && displayedTabs.length < 2}
            onDragEnd={handleDragEnd}
            externalDndContext={externalDndContext}
            activeId={activeDragTabId}
            getItemData={getTabDragData}
            renderItem={renderTab}
          />
          {!layout.requiresHorizontalScrollFallback ? (
            <WorkspaceNewTabButton
              panePanelKinds={panePanelKinds}
              host={host}
              launchPurpose={launchPurpose}
              placement="inline"
              serverId={normalizedServerId}
              paneId={paneId}
              shortcutKeys={newTabKeys}
            />
          ) : null}
        </Animated.ScrollView>
        <HorizontalScrollBoundaryShades
          visible={layout.requiresHorizontalScrollFallback}
          backdrop={rail}
          testIDPrefix="workspace-tabs-scroll-shade"
          leftStyle={tabScrollBoundary.leftShadeStyle}
          rightStyle={tabScrollBoundary.rightShadeStyle}
        />
      </View>
      <WorkspacePaneToolbarActions
        panePanelKinds={panePanelKinds}
        host={host}
        launchPurpose={launchPurpose}
        showNewTabButton={layout.requiresHorizontalScrollFallback}
        showSplitActions={showPaneSplitActions}
        showMaximizeAction={showPaneMaximizeAction}
        paneMaximized={paneMaximized}
        serverId={normalizedServerId}
        paneId={paneId}
        newTabShortcutKeys={newTabKeys}
        onSplitRight={onSplitRight}
        onSplitDown={onSplitDown}
        onTogglePaneMaximized={onTogglePaneMaximized}
      />
    </View>
  );

  return <RenderProfile id="WorkspaceDesktopTabsRow">{row}</RenderProfile>;
}

const styles = StyleSheet.create((theme) => ({
  tabsContainer: {
    minWidth: 0,
    height: WORKSPACE_SECONDARY_HEADER_HEIGHT,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
    flexDirection: "row",
    alignItems: "center",
    overflow: "visible",
  },
  tabsContainerSidebar: {
    backgroundColor: theme.colors.surfaceSidebar,
  },
  tabsScroll: {
    minWidth: 0,
  },
  tabsScrollContainer: {
    minWidth: 0,
    flex: 1,
    alignSelf: "stretch",
  },
  tabsScrollFitContent: {
    flex: 1,
  },
  tabsScrollOverflow: {
    flex: 1,
  },
  tabsContent: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: TAB_ROW_PADDING_HORIZONTAL,
  },
  tabLabelMeasurements: {
    position: "absolute",
    top: 0,
    left: 0,
    opacity: 0,
    alignItems: "flex-start",
    pointerEvents: "none",
  },
}));
