import { useHostFeature } from "@/runtime/host-features";
import { AssistantFileLinkResolverProvider } from "@/assistant-file-links";
import { useFileExplorerActions } from "@/hooks/use-file-explorer-actions";
import { revealWorkspaceTab } from "@/workspace-tabs/navigation";
import type { LayoutChangeEvent } from "react-native";
import { WorkspaceTabDragScope } from "@/components/workspace-tab-drag-scope";
import {
  WorkspaceComposerEditingSessionsProvider,
  useComposerTabFullscreen,
} from "@/composer/editing-session";
import { SheetHostScope } from "@/components/ui/isolated-bottom-sheet-modal";
import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useStoreWithEqualityFn } from "zustand/traditional";
import { useIsFocused } from "expo-router/react-navigation";
import { BackHandler, View } from "react-native";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { StyleSheet } from "react-native-unistyles";
import invariant from "tiny-invariant";
import { SidebarMenuToggle } from "@/components/headers/sidebar-menu-toggle";
import { ScreenHeader } from "@/components/headers/screen-header";
import { WorkspaceHeaderTitleBar } from "@/screens/workspace/workspace-header-title-bar";
import { WorkspaceHeaderActions } from "@/screens/workspace/workspace-header-actions";
import { WorkspaceCompactHeader } from "@/screens/workspace/workspace-compact-header";
import { WorkspaceFocusedPaneContent } from "@/screens/workspace/workspace-focused-pane-content";
import {
  FloatingPanelPortalHost,
  FloatingPanelPortalHostNameProvider,
} from "@/components/ui/floating-panel-portal";
import { ImportSessionSheet } from "@/components/import-session-sheet";
import { useNavigateToImportedAgent } from "@/hooks/use-import-session";
import { useToast } from "@/contexts/toast-context";
import { usePanelStore } from "@/stores/panel-store";
import {
  openExplorerSidebarView,
  toggleExplorerSidebar,
  useIsExplorerSidebarOpen,
} from "@/workspace-tabs/explorer-sidebar";
import {
  openPreferredWorkspacePreview,
  openPreferredWorkspaceTarget,
  openWorkspaceTargetBeside,
} from "@/workspace-tabs/open-beside";
import { traceInstant } from "@/performance/native-trace";
import { useSessionStore } from "@/stores/session-store";
import {
  collectAllTabs,
  DEFAULT_PANE_ID,
  FOCUSED_PANE_PLACEMENT,
  selectExplorerSidebarPaneId,
  selectPaneFocusSuspended,
  type WorkspaceTabPlacement,
  useWorkspaceLayoutStore,
  usePendingWorkspaceOpen,
  useWorkspaceLayoutStoreHydrated,
} from "@/stores/workspace-layout-store";
import {
  buildWorkspaceTabPersistenceKey,
  type WorkspaceTab,
  type WorkspaceTabTarget,
} from "@/workspace-tabs/model";
import { useSettings } from "@/hooks/use-settings";
import { useCreateFlowStore } from "@/stores/create-flow-store";
import { normalizeWorkspaceTabTarget } from "@/workspace-tabs/identity";
import { useVisibleAgentIds } from "./visible-agent-ids";
import {
  getHostRuntimeStore,
  useHostRuntimeClient,
  useHostRuntimeIsConnected,
} from "@/runtime/host-runtime";
import { prefetchProvidersSnapshot } from "@/hooks/use-providers-snapshot";
import {
  shouldSeedWorkspaceSetupTab,
  shouldShowWorkspaceSetup,
  useWorkspaceSetupStore,
} from "@/stores/workspace-setup-store";
import { useWorkspace } from "@/stores/session-store-hooks";
import { useArchiveAgent } from "@/hooks/use-archive-agent";
import { useStableEvent } from "@/hooks/use-stable-event";
import { removeResidentBrowserWebview } from "@/desktop/browser/resident-webviews";
import { useBrowserStore } from "@/desktop/browser/store";
import { getDesktopHost } from "@/desktop/host";
import { generateDraftId } from "@/stores/draft-keys";
import { resolveWorkspaceRouteId } from "@/utils/workspace-identity";
import { useOpenAgentTabLabels } from "@/subagents/use-open-agent-tab-labels";
import {
  useWorkspaceTabRename,
  WorkspaceTabRenameModal,
} from "@/screens/workspace/use-workspace-tab-rename";
import type { WorkspaceDesktopTabRowItem } from "@/screens/workspace/workspace-desktop-tabs-row";
import { renderWorkspaceRouteGate } from "@/screens/workspace/workspace-route-state-views";
import { useWorkspaceScriptActions } from "./scripts/use-workspace-script-actions";
import { useWorkspaceTabActions } from "./hooks/use-workspace-tab-actions";
import { useWorkspaceTabLaunchers } from "./hooks/use-workspace-tab-launchers";
import { useWorkspacePaneLayout } from "./hooks/use-workspace-pane-layout";
import { useWorkspaceMarkdownOpener } from "./hooks/use-workspace-markdown-opener";
import { WorkspaceDesktopSplitView } from "./views/workspace-desktop-split-view";
import { WorkspaceCenterColumn } from "./views/workspace-center-column";
import { WorkspaceExplorerView } from "./views/workspace-explorer-view";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import {
  resolveWorkspaceExplorerToggleOwner,
  EXPLORER_TOGGLE_KEYS,
  WorkspaceExplorerSidebarToggle,
} from "@/screens/workspace/workspace-explorer-toggle";
import { useHasWindowChromeObstruction } from "@/utils/desktop-window";
import { useWorkspaceRecovery } from "@/workspace-recovery/use-workspace-recovery";
import {
  buildWorkspaceTabSnapshot,
  createWorkspaceAgentVisibilitySelector,
  workspaceAgentVisibilityEqual,
} from "@/workspace-tabs/agent-visibility";
import { deriveWorkspacePaneState } from "@/screens/workspace/workspace-pane-state";
import { buildWorkspacePaneContentModel } from "@/screens/workspace/workspace-pane-content";
import { useMountedTabSet } from "@/screens/workspace/use-mounted-tab-set";
import { ensurePanelsRegistered } from "@/panels/register-panels";
import { SheetPaneHostProvider, type SheetPaneHost } from "@/screens/workspace/sheet-pane";
import type { PaneHost } from "@/panels/panel-manifest";
import { WorkspaceFocusProvider } from "@/workspace/focus";
import { DiffDocumentWorkspaceCacheProvider } from "@/git/diff-document/workspace-cache";
import { NewTabLauncherProvider, type NewTabLauncher } from "@/workspace-tabs/launcher";
import type { BulkCloseConfirmationLabels } from "@/screens/workspace/workspace-bulk-close";
import { useModifiedPanelTabIds } from "@/panels/panel-instance-attributes";
import { supportsDesktopPaneSplits, useIsCompactFormFactor } from "@/constants/layout";
import { isNative } from "@/constants/platform";
import { useWorkspaceTerminals } from "@/screens/workspace/terminals/use-workspace-terminals";
import {
  createWorkspaceFileTabTarget,
  normalizeWorkspaceFileLocation,
  type WorkspaceFileLocation,
  type WorkspaceFileOpenRequest,
} from "@/workspace/file-open";
import { RenderProfile } from "@/utils/render-profiler";
import { useWorkspaceCheckoutStatus } from "@/screens/workspace/use-workspace-checkout-status";
import { useWorkspacePullRequestDiscovery } from "@/panels/pull-request";

import {
  NO_SHEETS,
  WORKSPACE_FLOATING_PANEL_PORTAL_HOST_PREFIX,
  getWorkspaceSetupStatus,
  getWorkspaceScripts,
  getWorkspaceFileLocationFields,
  buildWorkspaceFileLocation,
  trimNonEmpty,
  decodeSegment,
  useSyncWorkspaceActiveBrowser,
  useStableTabDescriptorMap,
  useCloseTabs,
  paneLocalPlacement,
  useWorkspaceTerminalTabActions,
  useLastMainPane,
} from "./workspace-screen-helpers";
import {
  useWorkspaceRouteActions,
  useResolvedWorkspaceRouteState,
  shouldInspectWorkspaceRecovery,
  renderWorkspaceScreenGateShell,
} from "./workspace-screen-modals";
import {
  buildWorkspaceHeaderCheckoutState,
  deriveWorkspaceHeaderFields,
  WorkspaceDocumentTitleEffectSlot,
  resolveCompactHeaderLayout,
  shouldShowWorkspaceScreenHeader,
} from "./workspace-screen-header";
import { useWorkspaceKeyboardActions } from "./use-workspace-keyboard-actions";

const EMPTY_UI_TABS: WorkspaceTab[] = [];

interface WorkspaceScreenProps {
  serverId: string;
  workspaceId: string;
  isRouteFocused?: boolean;
}

type WorkspaceScreenContentProps = WorkspaceScreenProps & {
  isRouteFocused: boolean;
};

export const WorkspaceScreen = memo(function WorkspaceScreen({
  serverId,
  workspaceId,
  isRouteFocused,
}: WorkspaceScreenProps) {
  ensurePanelsRegistered();
  const navigationFocused = useIsFocused();
  useEffect(() => {
    traceInstant("paseo.workspace.mount", { serverId, workspaceId });
    return () => {
      traceInstant("paseo.workspace.unmount", { serverId, workspaceId });
    };
  }, [serverId, workspaceId]);
  return (
    <WorkspaceScreenContent
      serverId={serverId}
      workspaceId={workspaceId}
      isRouteFocused={isRouteFocused ?? navigationFocused}
    />
  );
});

function WorkspaceContentProviders({
  children,
  workspaceKey,
  sheetPaneHost,
}: {
  children: ReactNode;
  workspaceKey: string | null;
  sheetPaneHost: SheetPaneHost | null;
}) {
  return (
    <WorkspaceFocusProvider workspaceKey={workspaceKey}>
      <DiffDocumentWorkspaceCacheProvider>
        <SheetPaneHostProvider value={sheetPaneHost}>{children}</SheetPaneHostProvider>
      </DiffDocumentWorkspaceCacheProvider>
    </WorkspaceFocusProvider>
  );
}

function WorkspacePanelSurface({
  launcher,
  children,
  tabIds,
  workspaceKey,
  onLayout,
}: {
  onLayout: (event: LayoutChangeEvent) => void;
  workspaceKey: string;
  launcher: NewTabLauncher;
  children: ReactNode;
  tabIds: readonly string[];
}) {
  const isCompact = useIsCompactFormFactor();
  return (
    <WorkspaceComposerEditingSessionsProvider workspaceKey={workspaceKey} tabIds={tabIds}>
      <NewTabLauncherProvider value={launcher}>
        <SheetHostScope enabled={!isCompact}>
          <View style={styles.threePaneRow} onLayout={onLayout}>
            {children}
          </View>
        </SheetHostScope>
      </NewTabLauncherProvider>
    </WorkspaceComposerEditingSessionsProvider>
  );
}

function WorkspaceScreenContent({
  serverId,
  workspaceId,
  isRouteFocused,
}: WorkspaceScreenContentProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const isMobile = useIsCompactFormFactor();
  const canRenderDesktopPaneSplits = supportsDesktopPaneSplits();
  const layoutMode = useWorkspaceLayoutStore((state) => state.layoutMode);
  const hasMacTrafficLights = useHasWindowChromeObstruction("top-left");
  const explorerToggleOwner = resolveWorkspaceExplorerToggleOwner({
    isMobile,
    hasMacTrafficLights,
  });
  const isFocusModeEnabled = usePanelStore((state) => state.desktop.focusModeEnabled);
  const toggleFocusMode = usePanelStore((state) => state.toggleFocusMode);

  const normalizedServerId = useMemo(() => trimNonEmpty(decodeSegment(serverId)) ?? "", [serverId]);

  const normalizedWorkspaceId = useMemo(
    () => resolveWorkspaceRouteId({ routeWorkspaceId: workspaceId }) ?? "",
    [workspaceId],
  );
  const workspaceDescriptor = useWorkspace(normalizedServerId, normalizedWorkspaceId);
  useEffect(() => {
    if (!normalizedServerId || !normalizedWorkspaceId || workspaceDescriptor) return;
    void getHostRuntimeStore()
      .prepareWorkspaceRoute(normalizedServerId, normalizedWorkspaceId)
      .catch(() => undefined);
  }, [normalizedServerId, normalizedWorkspaceId, workspaceDescriptor]);

  useEffect(() => {
    if (!isRouteFocused || !normalizedServerId || !normalizedWorkspaceId) return;
    return getHostRuntimeStore().acquireScriptStatusDemand(
      normalizedServerId,
      normalizedWorkspaceId,
    );
  }, [isRouteFocused, normalizedServerId, normalizedWorkspaceId]);

  const workspaceScripts = getWorkspaceScripts(workspaceDescriptor);
  const { handleRetryHost, handleManageHost, handleDismissMissingWorkspace } =
    useWorkspaceRouteActions(normalizedServerId);

  const client = useHostRuntimeClient(normalizedServerId);
  const isConnected = useHostRuntimeIsConnected(normalizedServerId);
  const supportsProvidersSnapshot = useSessionStore(
    (state) => state.sessions[normalizedServerId]?.serverInfo?.features?.providersSnapshot === true,
  );
  const workspaceDirectory = workspaceDescriptor?.workspaceDirectory || null;
  const isMissingWorkspaceDirectory = Boolean(workspaceDescriptor) && !workspaceDirectory;
  const [isImportSheetVisible, setIsImportSheetVisible] = useState(false);
  const canOpenImportSheet = [client, isConnected, workspaceDirectory].every(Boolean);
  const openImportSheet = useCallback(() => {
    setIsImportSheetVisible(true);
  }, []);
  const closeImportSheet = useCallback(() => {
    setIsImportSheetVisible(false);
  }, []);

  useEffect(() => {
    if (
      !isRouteFocused ||
      !isConnected ||
      !client ||
      !workspaceDirectory ||
      !supportsProvidersSnapshot
    ) {
      return;
    }
    prefetchProvidersSnapshot(normalizedServerId, client, {
      cwd: workspaceDirectory,
    });
  }, [
    client,
    isConnected,
    isRouteFocused,
    normalizedServerId,
    supportsProvidersSnapshot,
    workspaceDirectory,
  ]);

  const persistenceKey = useMemo(
    () =>
      buildWorkspaceTabPersistenceKey({
        serverId: normalizedServerId,
        workspaceId: normalizedWorkspaceId,
      }),
    [normalizedServerId, normalizedWorkspaceId],
  );
  const openTab = useWorkspaceLayoutStore((state) => state.openTab);
  const replaceWorkspaceTabTarget = useWorkspaceLayoutStore((state) => state.replaceTab);
  const openWorkspaceTabFocused = useCallback(
    (workspaceKey: string, target: WorkspaceTabTarget, placement?: WorkspaceTabPlacement) =>
      openTab({ workspaceKey, target, intent: "reveal", placement }),
    [openTab],
  );
  const createWorkspaceTab = useCallback(
    (
      workspaceKey: string,
      target: WorkspaceTabTarget,
      placement?: WorkspaceTabPlacement,
      stateValue?: unknown,
    ) =>
      openTab({
        workspaceKey,
        target,
        intent: "new",
        placement,
        state: stateValue as never,
      }),
    [openTab],
  );
  const revealWorkspaceChildTab = useCallback(
    (
      workspaceKey: string,
      target: WorkspaceTabTarget,
      parentTabId: string,
      placement?: WorkspaceTabPlacement,
    ) =>
      openTab({
        workspaceKey,
        target,
        intent: "reveal",
        parentTabId,
        placement,
      }),
    [openTab],
  );
  const panelRevisions = useWorkspaceLayoutStore((state) =>
    persistenceKey ? state.panelRevisionByWorkspace[persistenceKey] : undefined,
  );
  const focusWorkspacePane = useWorkspaceLayoutStore((state) => state.focusPane);
  const hasHydratedWorkspaces = useSessionStore(
    (state) => state.sessions[normalizedServerId]?.hasHydratedWorkspaces ?? false,
  );
  const pendingOpenTarget = usePendingWorkspaceOpen({
    onOpened: revealWorkspaceTab,
    workspaceKey: persistenceKey,
    hasHydratedWorkspaces,
    workspaceExists: workspaceDescriptor !== null,
  });
  const workspaceRecovery = useWorkspaceRecovery({
    serverId: normalizedServerId,
    workspaceId: normalizedWorkspaceId,
    enabled: shouldInspectWorkspaceRecovery(
      hasHydratedWorkspaces,
      workspaceDescriptor,
      pendingOpenTarget,
    ),
  });

  const selectWorkspaceAgentVisibility = useMemo(
    () =>
      createWorkspaceAgentVisibilitySelector({
        serverId: normalizedServerId,
        workspaceId: normalizedWorkspaceId,
      }),
    [normalizedServerId, normalizedWorkspaceId],
  );
  const workspaceAgentVisibility = useStoreWithEqualityFn(
    useSessionStore,
    selectWorkspaceAgentVisibility,
    workspaceAgentVisibilityEqual,
  );

  const {
    handleTerminalCreated,
    handleScriptTerminalSelected,
    handleWorkspacePathUnavailable,
    handleTerminalCreateQueued,
    handleTerminalCreateFailed,
  } = useWorkspaceTerminalTabActions({
    persistenceKey,
    openWorkspaceTabFocused,
    replaceWorkspaceTabTarget,
    labels: {
      workspacePathUnavailable: t("workspace.header.toasts.workspacePathUnavailable"),
      terminalQueued: t("workspace.header.toasts.terminalQueued"),
    },
    toast,
  });
  const queryClient = useQueryClient();
  const {
    createMutation: createTerminalMutation,
    createTerminal,
    handleScriptTerminalStarted,
    handleViewScriptTerminal,
    invalidateTerminals,
    killMutation: killTerminalMutation,
    knownTerminalIds,
    liveTerminalIds,
    pendingCreateInput: pendingTerminalCreateInput,
    query: terminalsQuery,
    queryKey: terminalsQueryKey,
    removeTerminalFromCache,
    standaloneTerminalIds,
  } = useWorkspaceTerminals({
    client,
    isConnected,
    isRouteFocused,
    normalizedServerId,
    normalizedWorkspaceId,
    workspaceDirectory,
    workspaceScripts,
    hasHydratedWorkspaces,
    isMissingWorkspaceDirectory,
    onTerminalCreated: handleTerminalCreated,
    onScriptTerminalSelected: handleScriptTerminalSelected,
    onWorkspacePathUnavailable: handleWorkspacePathUnavailable,
    onTerminalCreateQueued: handleTerminalCreateQueued,
    onTerminalCreateFailed: handleTerminalCreateFailed,
  });
  const { archiveAgent } = useArchiveAgent();

  const { checkoutQuery, isCheckoutStatusLoading } = useWorkspaceCheckoutStatus({
    client,
    isConnected,
    isRouteFocused,
    normalizedServerId,
    normalizedWorkspaceId,
    workspaceDirectory,
  });
  const hasHydratedAgents = useSessionStore(
    (state) => state.sessions[normalizedServerId]?.hasHydratedAgents ?? false,
  );
  const workspaceRouteState = useResolvedWorkspaceRouteState({
    serverId: normalizedServerId,
    workspace: workspaceDescriptor,
    hasHydratedWorkspaces,
    recovery: workspaceRecovery.state,
  });
  const workspaceHeaderCheckoutState = buildWorkspaceHeaderCheckoutState({
    isCheckoutStatusLoading,
    isError: checkoutQuery.isError,
    data: checkoutQuery.data,
  });
  const {
    isWorkspaceHeaderLoading,
    workspaceHeaderTitle,
    workspaceHeaderSubtitle,
    isWorkspaceHeaderSubtitleDistinct,
    isGitCheckout,
    currentBranchName,
  } = deriveWorkspaceHeaderFields({
    workspace: workspaceDescriptor,
    checkoutState: workspaceHeaderCheckoutState,
  });
  const showMobileAgent = usePanelStore((state) => state.showMobileAgent);

  const isExplorerSidebarShowing = useIsExplorerSidebarOpen({
    isCompact: isMobile,
    workspaceKey: persistenceKey,
  });
  const hasPullRequest = useWorkspacePullRequestDiscovery({
    workspaceKey: persistenceKey,
    serverId: normalizedServerId,
    cwd: workspaceDirectory,
    workspaceActive: isRouteFocused,
    isGit: isGitCheckout,
    layoutMode,
    explorerOpen: isExplorerSidebarShowing,
  });

  const explorerSidebarToggleAccessibilityState = useMemo(
    () => ({ expanded: isExplorerSidebarShowing }),
    [isExplorerSidebarShowing],
  );

  useEffect(() => {
    if (!isRouteFocused || isNative || !isMobile || !isExplorerSidebarShowing) {
      return;
    }

    const handler = BackHandler.addEventListener("hardwareBackPress", () => {
      showMobileAgent();
      return true;
    });

    return () => handler.remove();
  }, [isExplorerSidebarShowing, isMobile, isRouteFocused, showMobileAgent]);

  const workspaceLayout = useWorkspaceLayoutStore((state) =>
    persistenceKey ? (state.layoutByWorkspace[persistenceKey] ?? null) : null,
  );
  const unfocusedPaneId = useWorkspaceLayoutStore((state) =>
    persistenceKey ? state.focusRestorationByWorkspace[persistenceKey]?.restorePaneId : undefined,
  );
  const explorerSidebarPaneId = useWorkspaceLayoutStore((state) =>
    persistenceKey ? selectExplorerSidebarPaneId(state, persistenceKey) : null,
  );
  const paneFocusSuspended = useWorkspaceLayoutStore((state) =>
    persistenceKey ? selectPaneFocusSuspended(state, persistenceKey) : false,
  );
  const lastMainPaneRef = useLastMainPane({
    workspaceKey: persistenceKey,
    layout: workspaceLayout,
    explorerSidebarPaneId,
  });
  const lastMainPaneId = lastMainPaneRef.current.paneId;
  const hasHydratedWorkspaceLayoutStore = useWorkspaceLayoutStoreHydrated();
  const workspaceSetupStatus = getWorkspaceSetupStatus(workspaceDescriptor);
  const claimFailedSetupSurface = useWorkspaceSetupStore((state) => state.claimFailedSetupSurface);
  const supportsSetupObservation = useHostFeature(normalizedServerId, "workspaceSetupObservation");
  const showWorkspaceSetup = shouldShowWorkspaceSetup(
    workspaceSetupStatus,
    supportsSetupObservation,
  );
  const uiTabs = useMemo(
    () => (workspaceLayout ? collectAllTabs(workspaceLayout.root) : EMPTY_UI_TABS),
    [workspaceLayout],
  );
  useOpenAgentTabLabels({
    client,
    serverId: normalizedServerId,
    tabs: uiTabs,
    enabled: hasHydratedWorkspaceLayoutStore,
  });
  useSyncWorkspaceActiveBrowser({
    workspaceLayout,
    isRouteFocused,
    workspaceId: normalizedWorkspaceId,
  });
  const openWorkspaceTabInBackground = useCallback(
    (workspaceKey: string, target: WorkspaceTabTarget, placement?: WorkspaceTabPlacement) =>
      openTab({ workspaceKey, target, intent: "background", placement }),
    [openTab],
  );
  const openInSidePane = useSettings((settings) => settings.openInSidePane);
  const pullRequestOpenLocation = useSettings((settings) => settings.pullRequestOpenLocation);
  const selectWorkspaceTabInPane = useWorkspaceLayoutStore((state) => state.selectTabInPane);
  const closeWorkspaceTab = useWorkspaceLayoutStore((state) => state.closeTab);
  const sheets = useWorkspaceLayoutStore((state) =>
    persistenceKey ? (state.sheetsByWorkspace[persistenceKey] ?? NO_SHEETS) : NO_SHEETS,
  );
  const unpinWorkspaceAgent = useWorkspaceLayoutStore((state) => state.unpinAgent);
  const hideWorkspaceAgent = useWorkspaceLayoutStore((state) => state.hideAgent);
  const setWorkspaceTabState = useWorkspaceLayoutStore((state) => state.setTabState);
  const reconcileWorkspaceTabs = useWorkspaceLayoutStore((state) => state.reconcileTabs);
  const splitWorkspacePane = useWorkspaceLayoutStore((state) => state.splitPane);
  const splitWorkspacePaneEmpty = useWorkspaceLayoutStore((state) => state.splitPaneEmpty);
  const moveWorkspaceTabToPane = useWorkspaceLayoutStore((state) => state.moveTabToPane);
  const closeWorkspacePane = useWorkspaceLayoutStore((state) => state.closePane);
  const handleToggleExplorerSidebar = useCallback(() => {
    toggleExplorerSidebar({
      isCompact: isMobile,
      workspaceKey: persistenceKey,
    });
  }, [isMobile, persistenceKey]);
  const paneFocusSuppressedRef = useRef(false);
  const resizeWorkspaceSplit = useWorkspaceLayoutStore((state) => state.resizeSplit);
  const reorderWorkspaceTabsInPane = useWorkspaceLayoutStore((state) => state.reorderTabsInPane);
  const pendingByDraftId = useCreateFlowStore((state) => state.pendingByDraftId);
  const { closingTabIds, closeTab } = useCloseTabs();

  const closeWorkspaceTabWithCleanup = useCallback(
    function closeWorkspaceTabWithCleanup(input: {
      tabId: string;
      target?: WorkspaceTabTarget | null;
    }) {
      const normalizedTabId = trimNonEmpty(input.tabId);
      if (!normalizedTabId || !persistenceKey) {
        return;
      }

      if (input.target?.kind === "agent") {
        unpinWorkspaceAgent(persistenceKey, input.target.agentId);
        hideWorkspaceAgent(persistenceKey, input.target.agentId);
      }
      if (input.target?.kind === "browser") {
        const { browserId } = input.target;
        useBrowserStore.getState().removeBrowser(browserId);
        removeResidentBrowserWebview(browserId);
        void getDesktopHost()?.browser?.unregisterWorkspaceBrowser?.(browserId);
      }
      closeWorkspaceTab(persistenceKey, normalizedTabId);
    },
    [closeWorkspaceTab, hideWorkspaceAgent, persistenceKey, unpinWorkspaceAgent],
  );

  const focusedPaneTabState = useMemo(
    () =>
      deriveWorkspacePaneState({
        layout: workspaceLayout,
        tabs: uiTabs,
        paneId: workspaceLayout?.focusedPaneId ?? unfocusedPaneId,
      }),
    [uiTabs, workspaceLayout, unfocusedPaneId],
  );
  const viewedTimelineSync = useSessionStore(
    (state) => state.sessions[normalizedServerId]?.viewedTimelineSync ?? null,
  );
  const syncFocusedPaneOnly = useMemo(
    () => isMobile || isFocusModeEnabled || !supportsDesktopPaneSplits(),
    [isFocusModeEnabled, isMobile],
  );
  const visibleAgentIds = useVisibleAgentIds({
    layout: workspaceLayout,
    tabs: uiTabs,
    routeFocused: isRouteFocused,
    focusedPaneOnly: syncFocusedPaneOnly,
    explorerPaneId: explorerSidebarPaneId,
    explorerVisible: isExplorerSidebarShowing,
  });
  useEffect(() => {
    for (const agentId of visibleAgentIds) {
      void getHostRuntimeStore()
        .prepareAgentTimeline(normalizedServerId, agentId)
        .catch(() => undefined);
    }
  }, [normalizedServerId, visibleAgentIds]);
  useLayoutEffect(() => {
    if (!persistenceKey || !viewedTimelineSync) {
      return;
    }
    viewedTimelineSync.replaceVisibleAgentIds(persistenceKey, visibleAgentIds);
  }, [persistenceKey, viewedTimelineSync, visibleAgentIds]);
  useEffect(() => {
    if (!persistenceKey || !viewedTimelineSync) {
      return;
    }
    return () => viewedTimelineSync.replaceVisibleAgentIds(persistenceKey, []);
  }, [persistenceKey, viewedTimelineSync]);
  const setFocusedAgentId = useSessionStore((state) => state.setFocusedAgentId);
  const setFocusedTerminalId = useSessionStore((state) => state.setFocusedTerminalId);
  const focusedPaneAgentId = useMemo(() => {
    const target = focusedPaneTabState.activeTab?.descriptor.target;
    if (target?.kind !== "agent") {
      return null;
    }
    return target.agentId;
  }, [focusedPaneTabState.activeTab]);
  const focusedPaneTerminalId = useMemo(() => {
    const target = focusedPaneTabState.activeTab?.descriptor.target;
    if (target?.kind !== "terminal") {
      return null;
    }
    return target.terminalId;
  }, [focusedPaneTabState.activeTab]);

  useEffect(() => {
    if (!isRouteFocused) {
      return;
    }
    setFocusedAgentId(normalizedServerId, focusedPaneAgentId);
    setFocusedTerminalId(normalizedServerId, focusedPaneTerminalId);
  }, [
    focusedPaneAgentId,
    focusedPaneTerminalId,
    isRouteFocused,
    normalizedServerId,
    setFocusedAgentId,
    setFocusedTerminalId,
  ]);

  useEffect(() => {
    if (!isRouteFocused) {
      return;
    }
    return () => {
      setFocusedAgentId(normalizedServerId, null);
      setFocusedTerminalId(normalizedServerId, null);
    };
  }, [isRouteFocused, normalizedServerId, setFocusedAgentId, setFocusedTerminalId]);

  const openWorkspaceDraftTab = useCallback(
    function openWorkspaceDraftTab(input?: {
      ambient?: boolean;
      draftId?: string;
      focus?: boolean;
      paneId?: string | null;
    }) {
      if (!persistenceKey) {
        return null;
      }

      const target = normalizeWorkspaceTabTarget({
        kind: "draft",
        draftId: trimNonEmpty(input?.draftId) ?? generateDraftId(),
      });
      invariant(target?.kind === "draft", "Draft tab target must be valid");
      if (input?.ambient) {
        return openWorkspaceTabFocused(persistenceKey, target);
      }
      const placement = paneLocalPlacement(input?.paneId);
      if (input?.focus === false) {
        return openWorkspaceTabInBackground(persistenceKey, target, placement);
      }
      return openWorkspaceTabFocused(persistenceKey, target, placement);
    },
    [openWorkspaceTabFocused, openWorkspaceTabInBackground, persistenceKey],
  );

  useLayoutEffect(() => {
    if (!isRouteFocused) {
      return;
    }
    if (!normalizedServerId || !normalizedWorkspaceId || !persistenceKey) {
      return;
    }
    if (!hasHydratedWorkspaceLayoutStore) {
      return;
    }

    const hasActivePendingDraftCreateInWorkspace = uiTabs.some((tab) => {
      if (tab.target.kind !== "draft") {
        return false;
      }
      const pending = pendingByDraftId[tab.target.draftId];
      return pending?.serverId === normalizedServerId && pending.lifecycle === "active";
    });

    reconcileWorkspaceTabs(
      persistenceKey,
      buildWorkspaceTabSnapshot({
        agentVisibility: workspaceAgentVisibility,
        agentsHydrated: hasHydratedAgents,
        terminalsHydrated: terminalsQuery.isSuccess,
        knownTerminalIds,
        standaloneTerminalIds,
        hasActivePendingTerminalCreate:
          createTerminalMutation.isPending || pendingTerminalCreateInput !== null,
        hasActivePendingDraftCreate: hasActivePendingDraftCreateInWorkspace,
      }),
    );
  }, [
    hasHydratedAgents,
    hasHydratedWorkspaceLayoutStore,
    pendingTerminalCreateInput,
    createTerminalMutation.isPending,
    isRouteFocused,
    normalizedServerId,
    normalizedWorkspaceId,
    pendingByDraftId,
    persistenceKey,
    reconcileWorkspaceTabs,
    knownTerminalIds,
    standaloneTerminalIds,
    terminalsQuery.isSuccess,
    uiTabs,
    workspaceAgentVisibility,
  ]);

  const activeTabId = focusedPaneTabState.activeTabId;
  const activeTab = focusedPaneTabState.activeTab;

  const tabs = useMemo<WorkspaceTabDescriptor[]>(
    () => focusedPaneTabState.tabs.map((tab) => tab.descriptor),
    [focusedPaneTabState.tabs],
  );
  const hasSetupTab = useMemo(
    () =>
      uiTabs.some(
        (tab) => tab.target.kind === "setup" && tab.target.workspaceId === normalizedWorkspaceId,
      ),
    [normalizedWorkspaceId, uiTabs],
  );
  const navigateToTabId = useCallback(
    function navigateToTabId(tabId: string) {
      if (!tabId || !persistenceKey) {
        return;
      }
      revealWorkspaceTab(persistenceKey, tabId);
    },
    [persistenceKey],
  );
  const selectTabInPane = useCallback(
    (paneId: string, tabId: string) => {
      if (persistenceKey) {
        selectWorkspaceTabInPane(persistenceKey, paneId, tabId);
      }
    },
    [persistenceKey, selectWorkspaceTabInPane],
  );

  const navigateToImportedAgent = useNavigateToImportedAgent(normalizedServerId);
  const handleImportedAgent = useCallback(
    (agentId: string) => {
      if (!persistenceKey) {
        return;
      }
      const tabId = openWorkspaceTabFocused(
        persistenceKey,
        { kind: "agent", agentId },
        FOCUSED_PANE_PLACEMENT,
      );
      if (tabId) {
        navigateToTabId(tabId);
      }
    },
    [navigateToTabId, openWorkspaceTabFocused, persistenceKey],
  );

  useEffect(() => {
    if (!isRouteFocused || !persistenceKey || !shouldSeedWorkspaceSetupTab(workspaceSetupStatus)) {
      return;
    }

    const target = normalizeWorkspaceTabTarget({
      kind: "setup",
      workspaceId: normalizedWorkspaceId,
    });
    if (!target) {
      return;
    }
    if (
      !claimFailedSetupSurface({
        serverId: normalizedServerId,
        workspaceId: normalizedWorkspaceId,
        status: workspaceSetupStatus,
      })
    ) {
      return;
    }
    if (hasSetupTab) {
      return;
    }

    openWorkspaceTabInBackground(persistenceKey, target, {
      mode: "prefer",
      paneId: DEFAULT_PANE_ID,
    });
  }, [
    claimFailedSetupSurface,
    hasSetupTab,
    isRouteFocused,
    normalizedWorkspaceId,
    normalizedServerId,
    openWorkspaceTabInBackground,
    persistenceKey,
    workspaceSetupStatus,
  ]);

  const handleOpenFileFromChat = useCallback(
    (location: WorkspaceFileLocation, parentTabId?: string | null) => {
      const normalizedLocation = normalizeWorkspaceFileLocation(location);
      if (!normalizedLocation || !persistenceKey) {
        return;
      }
      const target = createWorkspaceFileTabTarget(normalizedLocation);
      const tabId = parentTabId
        ? revealWorkspaceChildTab(persistenceKey, target, parentTabId, FOCUSED_PANE_PLACEMENT)
        : openWorkspaceTabFocused(persistenceKey, target, FOCUSED_PANE_PLACEMENT);
      if (tabId) {
        navigateToTabId(tabId);
      }
    },
    [navigateToTabId, openWorkspaceTabFocused, revealWorkspaceChildTab, persistenceKey],
  );

  const handleOpenPreferredAssistantFile = useCallback(
    (input: { location: WorkspaceFileLocation; parentTabId?: string | null }) => {
      const location = normalizeWorkspaceFileLocation(input.location);
      if (!location || !persistenceKey) {
        return;
      }

      openPreferredWorkspaceTarget({
        workspaceKey: persistenceKey,
        target: createWorkspaceFileTabTarget(location),
        source: "chatFiles",
        preferences: openInSidePane,
        parentTabId: input.parentTabId,
      });
    },
    [openInSidePane, persistenceKey],
  );

  const handleOpenWorkspaceFileFromPane = useStableEvent(function handleOpenWorkspaceFileFromPane({
    request,
    paneId,
    parentTabId,
    focusPaneBeforeOpen,
  }: {
    request: WorkspaceFileOpenRequest;
    paneId?: string | null;
    parentTabId: string;
    focusPaneBeforeOpen?: boolean;
  }) {
    if (focusPaneBeforeOpen && paneId && persistenceKey) {
      focusWorkspacePane(persistenceKey, paneId);
    }
    if (request.disposition === "side") {
      const location = normalizeWorkspaceFileLocation(request.location);
      if (!location || !persistenceKey) return;
      openWorkspaceTargetBeside({
        workspaceKey: persistenceKey,
        target: createWorkspaceFileTabTarget(location),
        parentTabId,
      });
      return;
    }
    if (request.disposition === "preferred") {
      handleOpenPreferredAssistantFile({
        location: request.location,
        parentTabId,
      });
      return;
    }
    handleOpenFileFromChat(request.location, parentTabId);
  });

  const [hoveredCloseTabKey, setHoveredCloseTabKey] = useState<string | null>(null);
  const { handleRenameTab, renamingTab, handleRenameModalSubmit, handleRenameModalClose } =
    useWorkspaceTabRename({
      client,
      normalizedServerId,
      queryClient,
      terminalsData: terminalsQuery.data,
      terminalsQueryKey,
    });

  const allTabDescriptorsById = useMemo(() => {
    const map = new Map<string, WorkspaceTabDescriptor>();
    for (const tab of uiTabs) {
      map.set(tab.tabId, {
        key: tab.tabId,
        tabId: tab.tabId,
        kind: tab.target.kind,
        target: tab.target,
      });
    }
    return map;
  }, [uiTabs]);

  const bulkCloseConfirmationLabels = useMemo<BulkCloseConfirmationLabels>(
    () => ({
      newTab: t("workspace.tabs.actions.newTab"),
      all: ({ agents, terminals: terminalCount, tabs: tabCount }) =>
        t("workspace.tabs.confirmations.bulk.all", {
          agents,
          terminals: terminalCount,
          tabs: tabCount,
        }),
      agentsAndTerminals: ({ agents, terminals: terminalCount }) =>
        t("workspace.tabs.confirmations.bulk.agentsAndTerminals", {
          agents,
          terminals: terminalCount,
        }),
      terminalsAndTabs: ({ terminals: terminalCount, tabs: tabCount }) =>
        t("workspace.tabs.confirmations.bulk.terminalsAndTabs", {
          terminals: terminalCount,
          tabs: tabCount,
        }),
      agentsAndTabs: ({ agents, tabs: tabCount }) =>
        t("workspace.tabs.confirmations.bulk.agentsAndTabs", {
          agents,
          tabs: tabCount,
        }),
      terminals: ({ terminals: terminalCount }) =>
        t("workspace.tabs.confirmations.bulk.terminals", {
          terminals: terminalCount,
        }),
      tabs: ({ tabs: tabCount }) => t("workspace.tabs.confirmations.bulk.tabs", { tabs: tabCount }),
      agents: ({ agents }) => t("workspace.tabs.confirmations.bulk.agents", { agents }),
    }),
    [t],
  );
  const explorerSidebarToggleLabel = isExplorerSidebarShowing
    ? t("workspace.tabs.explorerSidebar.close")
    : t("workspace.tabs.explorerSidebar.open");

  const activeTabKey = useMemo(() => activeTabId ?? "", [activeTabId]);
  const {
    handleCreateDraftTab,
    handleCreateTerminal,
    handleCreateTerminalWithProfile,
    handleCreateBrowserTab,
    handleCreateNewTab,
    launchWorkspaceTab,
    handleOpenUrlInBrowserTab,
    handleSelectSwitcherTab,
    handleCreateEmptySplit,
  } = useWorkspaceTabLaunchers({
    persistenceKey,
    workspaceLayout,
    openWorkspaceTabFocused,
    createWorkspaceTab,
    replaceWorkspaceTabTarget,
    splitWorkspacePaneEmpty,
    createTerminal,
    openWorkspaceDraftTab,
    navigateToTabId,
    paneLocalPlacement,
    focusedPanePlacement: FOCUSED_PANE_PLACEMENT,
  });

  const killTerminalAsync = killTerminalMutation.mutateAsync;

  const {
    handleCloseTabById,
    handleCopyAgentId,
    handleCopyTerminalId,
    handleCopyFilePath,
    handleCopyResumeCommand,
    handleReloadAgent,
    handleCopyWorkspacePath,
    handleCopyBranchName,
    handleOpenSetupTab,
    handleCloseTabsToLeft,
    handleCloseTabsToRight,
    handleCloseOtherTabs,
    handleClosePane,
    handleCloseTabsToLeftInPane,
    handleCloseTabsToRightInPane,
    handleCloseOtherTabsInPane,
  } = useWorkspaceTabActions({
    t,
    toast,
    normalizedServerId,
    normalizedWorkspaceId,
    persistenceKey,
    client,
    isConnected,
    workspaceDirectory,
    currentBranchName,
    closeTab,
    closeWorkspaceTabWithCleanup,
    closeWorkspaceTab,
    closeWorkspacePane,
    removeTerminalFromCache,
    killTerminalAsync,
    invalidateTerminals,
    setHoveredCloseTabKey,
    archiveAgent,
    allTabDescriptorsById,
    sheets,
    tabs,
    workspaceLayout,
    explorerSidebarPaneId,
    bulkCloseConfirmationLabels,
    openWorkspaceTabFocused,
    focusedPanePlacement: FOCUSED_PANE_PLACEMENT,
  });

  const workspaceActionsEnabled = Boolean(
    isRouteFocused && normalizedServerId && normalizedWorkspaceId,
  );

  useWorkspaceKeyboardActions({
    workspaceActionsEnabled,
    normalizedServerId,
    normalizedWorkspaceId,
    persistenceKey,
    isMobile,
    pullRequestOpenLocation,
    openWorkspaceTabFocused,
    focusedPaneTabState,
    activeTab,
    activeTabId,
    handleRenameTab,
    handleReloadAgent,
    handleCopyResumeCommand,
    handleCopyAgentId,
    handleCopyTerminalId,
    handleCopyFilePath,
    handleCloseTabsToLeft,
    handleCloseTabsToRight,
    handleCloseOtherTabs,
    handleCreateDraftTab,
    handleCreateTerminal,
    handleCreateBrowserTab,
    handleCreateNewTab,
    handleCloseTabById,
    navigateToTabId,
    tabs,
    isGitCheckout,
    handleToggleExplorerSidebar,
    toggleFocusMode,
    workspaceLayout,
    handleCreateEmptySplit,
    focusWorkspacePane,
    moveWorkspaceTabToPane,
    paneFocusSuppressedRef,
    handleClosePane,
    showWorkspaceSetup,
    handleOpenSetupTab,
  });

  const activeTabDescriptor = useMemo(() => activeTab?.descriptor ?? null, [activeTab]);
  const activeFileFields = getWorkspaceFileLocationFields(activeTabDescriptor);
  const activeFilePath = activeFileFields.path;
  const activeFileLineStart = activeFileFields.lineStart;
  const activeFileLineEnd = activeFileFields.lineEnd;
  const activeFileLocation = useMemo<WorkspaceFileLocation | null>(
    () =>
      buildWorkspaceFileLocation({
        path: activeFilePath,
        lineStart: activeFileLineStart,
        lineEnd: activeFileLineEnd,
      }),
    [activeFileLineEnd, activeFileLineStart, activeFilePath],
  );
  const shouldRenderDesktopPaneFallback = useMemo(
    () => !isMobile && !canRenderDesktopPaneSplits,
    [isMobile, canRenderDesktopPaneSplits],
  );

  useEffect(() => {
    if (!isRouteFocused || isNative || typeof document === "undefined" || activeTabDescriptor) {
      return;
    }
    document.title = "Workspace";
  }, [activeTabDescriptor, isRouteFocused]);

  const buildPaneContentModel = useCallback(
    (input: {
      tab: WorkspaceTabDescriptor;
      paneId?: string | null;
      focusPaneBeforeOpen?: boolean;
      host?: PaneHost;
      onCloseCurrentTab?: () => void;
    }) =>
      buildWorkspacePaneContentModel({
        tab: input.tab,
        normalizedServerId,
        normalizedWorkspaceId,
        host:
          input.host ??
          (input.paneId !== null && input.paneId === explorerSidebarPaneId ? "explorer" : "main"),
        panelRevision: panelRevisions?.[input.tab.tabId] ?? 0,
        onOpenTab: (target) => {
          if (!persistenceKey) {
            return;
          }
          const tabId = revealWorkspaceChildTab(
            persistenceKey,
            target,
            input.tab.tabId,
            paneLocalPlacement(input.focusPaneBeforeOpen ? input.paneId : null),
          );
          if (tabId) {
            navigateToTabId(tabId);
          }
        },
        onOpenPreferredTarget: (target, source) => {
          if (!persistenceKey) return;
          openPreferredWorkspacePreview({
            workspaceKey: persistenceKey,
            serverId: normalizedServerId,
            workspaceId: normalizedWorkspaceId,
            explorerSidebarPaneId,
            lastMainPaneId,
            target,
            source,
            preferences: openInSidePane,
          });
        },
        onOpenTargetToSide:
          canRenderDesktopPaneSplits &&
          input.paneId !== null &&
          input.paneId === explorerSidebarPaneId
            ? (target) => {
                if (!persistenceKey) return;
                openWorkspaceTargetBeside({
                  workspaceKey: persistenceKey,
                  target,
                  parentTabId: input.tab.tabId,
                });
              }
            : undefined,
        onCloseCurrentTab:
          input.onCloseCurrentTab ??
          (() => {
            void handleCloseTabById(input.tab.tabId);
          }),
        onRetargetCurrentTab: (target) => {
          if (!persistenceKey) {
            return;
          }
          replaceWorkspaceTabTarget(persistenceKey, input.tab.tabId, target);
        },
        onSetCurrentTabState: (state) => {
          if (persistenceKey) {
            setWorkspaceTabState(persistenceKey, input.tab.tabId, state);
          }
        },
        onOpenWorkspaceFile: (request: WorkspaceFileOpenRequest) => {
          handleOpenWorkspaceFileFromPane({
            request,
            paneId: input.paneId,
            parentTabId: input.tab.tabId,
            focusPaneBeforeOpen: input.focusPaneBeforeOpen,
          });
        },
        onOpenImportSheet: openImportSheet,
      }),
    [
      handleCloseTabById,
      panelRevisions,
      handleOpenWorkspaceFileFromPane,
      navigateToTabId,
      normalizedServerId,
      normalizedWorkspaceId,
      canRenderDesktopPaneSplits,
      openImportSheet,
      openInSidePane,
      revealWorkspaceChildTab,
      persistenceKey,
      replaceWorkspaceTabTarget,
      setWorkspaceTabState,
      explorerSidebarPaneId,
      lastMainPaneId,
    ],
  );
  const focusedPaneId = useMemo(
    () => focusedPaneTabState.pane?.id ?? null,
    [focusedPaneTabState.pane],
  );
  const focusedPaneTabIds = useMemo(() => tabs.map((tab) => tab.tabId), [tabs]);
  const modifiedFocusedPaneTabIds = useModifiedPanelTabIds({
    serverId: normalizedServerId,
    workspaceId: normalizedWorkspaceId,
    tabIds: focusedPaneTabIds,
  });
  const focusedPaneTabDescriptorMap = useStableTabDescriptorMap(tabs);
  const { mountedTabIds: mountedFocusedPaneTabIdsSet } = useMountedTabSet({
    activeTabId,
    allTabIds: focusedPaneTabIds,
    retainedTabIds: modifiedFocusedPaneTabIds,
    cap: 3,
  });
  const mountedFocusedPaneTabIds = useMemo(
    () => focusedPaneTabIds.filter((tabId) => mountedFocusedPaneTabIdsSet.has(tabId)),
    [focusedPaneTabIds, mountedFocusedPaneTabIdsSet],
  );
  const buildMobilePaneContentModel = useCallback(
    function buildMobilePaneContentModel(input: {
      paneId: string | null;
      tab: WorkspaceTabDescriptor;
    }) {
      return buildPaneContentModel({
        tab: input.tab,
        paneId: input.paneId,
        focusPaneBeforeOpen: false,
      });
    },
    [buildPaneContentModel],
  );
  const buildSheetPaneContentModel = useCallback(
    function buildSheetPaneContentModel(tab: WorkspaceTabDescriptor, closeCurrentTab: () => void) {
      return buildPaneContentModel({
        tab,
        paneId: null,
        focusPaneBeforeOpen: false,
        host: "sheet",
        onCloseCurrentTab: closeCurrentTab,
      });
    },
    [buildPaneContentModel],
  );
  const sheetPaneHost = useMemo<SheetPaneHost | null>(
    () =>
      persistenceKey
        ? {
            workspaceKey: persistenceKey,
            serverId: normalizedServerId,
            workspaceId: normalizedWorkspaceId,
            isWorkspaceFocused: isRouteFocused,
            buildContent: buildSheetPaneContentModel,
          }
        : null,
    [
      buildSheetPaneContentModel,
      isRouteFocused,
      normalizedServerId,
      normalizedWorkspaceId,
      persistenceKey,
    ],
  );
  const {
    handleFocusPane,
    handleSplitPane,
    handleMoveTabToPane,
    handleResizePaneSplit,
    handleReorderTabsInPane,
    handleReorderTabsInFocusedPane,
  } = useWorkspacePaneLayout({
    persistenceKey: persistenceKey ?? "",
    focusedPaneId,
    paneFocusSuppressedRef,
    focusWorkspacePane,
    splitWorkspacePane,
    moveWorkspaceTabToPane,
    resizeWorkspaceSplit,
    reorderWorkspaceTabsInPane,
  });

  const content = (
    <WorkspaceFocusedPaneContent
      isMissingWorkspaceDirectory={isMissingWorkspaceDirectory}
      activeTabDescriptor={activeTabDescriptor}
      hasHydratedAgents={hasHydratedAgents}
      hasLoadedTerminals={terminalsQuery.isSuccess}
      mountedFocusedPaneTabIds={mountedFocusedPaneTabIds}
      focusedPaneTabDescriptorMap={focusedPaneTabDescriptorMap}
      isRouteFocused={isRouteFocused}
      focusedPaneId={focusedPaneId}
      paneFocusSuspended={paneFocusSuspended}
      onFocusPane={handleFocusPane}
      buildMobilePaneContentModel={buildMobilePaneContentModel}
    />
  );

  const buildDesktopPaneContentModel = useCallback(
    function buildDesktopPaneContentModel(input: { paneId: string; tab: WorkspaceTabDescriptor }) {
      return buildPaneContentModel({
        tab: input.tab,
        paneId: input.paneId,
        focusPaneBeforeOpen: true,
      });
    },
    [buildPaneContentModel],
  );

  const desktopTabRowItems = useMemo<WorkspaceDesktopTabRowItem[]>(
    () =>
      tabs.map((tab) => ({
        tab,
        isActive: tab.tabId === activeTabDescriptor?.tabId,
        isCloseHovered: hoveredCloseTabKey === tab.key,
        isClosingTab: closingTabIds.has(tab.tabId),
      })),
    [activeTabDescriptor?.tabId, closingTabIds, hoveredCloseTabKey, tabs],
  );

  const containerStyle = [styles.container, styles.containerWorkspaceBackground];

  const workspaceScreenGate = renderWorkspaceScreenGateShell({
    gate: useMemo(
      () =>
        renderWorkspaceRouteGate({
          state: workspaceRouteState,
          actions: {
            onRetryHost: handleRetryHost,
            onManageHost: handleManageHost,
            onDismissMissingWorkspace: handleDismissMissingWorkspace,
            onRecoverWorkspace: workspaceRecovery.restore,
            onRetryRecoveryInspection: workspaceRecovery.retryInspection,
          },
        }),
      [
        handleDismissMissingWorkspace,
        handleManageHost,
        handleRetryHost,
        workspaceRecovery.restore,
        workspaceRecovery.retryInspection,
        workspaceRouteState,
      ],
    ),
    workspaceKey: persistenceKey,
  });

  const headerRight = useMemo(
    () => (
      <WorkspaceHeaderActions
        serverId={normalizedServerId}
        workspaceId={normalizedWorkspaceId}
        scripts={workspaceDescriptor?.scripts ?? null}
        liveTerminalIds={liveTerminalIds}
        onScriptTerminalStarted={handleScriptTerminalStarted}
        onViewTerminal={handleViewScriptTerminal}
        onOpenUrlInBrowserTab={handleOpenUrlInBrowserTab}
        workspaceDirectory={workspaceDirectory}
        projectId={workspaceDescriptor?.projectId ?? null}
        activeFile={activeFileLocation}
        owner={explorerToggleOwner}
        onPress={handleToggleExplorerSidebar}
        label={explorerSidebarToggleLabel}
        tooltipLabel={t("workspace.tabs.explorerSidebar.toggle")}
        accessibilityState={explorerSidebarToggleAccessibilityState}
      />
    ),
    [
      workspaceDescriptor,
      normalizedServerId,
      normalizedWorkspaceId,
      workspaceDirectory,
      activeFileLocation,
      liveTerminalIds,
      handleScriptTerminalStarted,
      handleViewScriptTerminal,
      handleOpenUrlInBrowserTab,
      handleToggleExplorerSidebar,
      explorerSidebarToggleLabel,
      explorerSidebarToggleAccessibilityState,
      explorerToggleOwner,
      t,
    ],
  );

  const showScreenHeader = useMemo(
    () => shouldShowWorkspaceScreenHeader({ isFocusModeEnabled, isMobile }),
    [isFocusModeEnabled, isMobile],
  );
  const renderExplorerSidebarHeaderAction = useCallback(
    () => (
      <WorkspaceExplorerSidebarToggle
        owner={explorerToggleOwner}
        onPress={handleToggleExplorerSidebar}
        label={explorerSidebarToggleLabel}
        tooltipLabel={t("workspace.tabs.explorerSidebar.toggle")}
        tooltipKeys={EXPLORER_TOGGLE_KEYS}
        accessibilityState={explorerSidebarToggleAccessibilityState}
      />
    ),
    [
      explorerSidebarToggleAccessibilityState,
      explorerSidebarToggleLabel,
      explorerToggleOwner,
      handleToggleExplorerSidebar,
      t,
    ],
  );
  const createTerminalDisabled = useMemo(
    () => createTerminalMutation.isPending || pendingTerminalCreateInput !== null,
    [createTerminalMutation.isPending, pendingTerminalCreateInput],
  );
  const showCreateBrowserTab = true;
  const newTabLauncher = useMemo<NewTabLauncher>(
    () => ({
      showChanges: isGitCheckout,
      showPullRequest: hasPullRequest,
      showBrowser: showCreateBrowserTab,
      terminalDisabled: createTerminalDisabled,
      launch: launchWorkspaceTab,
    }),
    [
      createTerminalDisabled,
      hasPullRequest,
      isGitCheckout,
      launchWorkspaceTab,
      showCreateBrowserTab,
    ],
  );
  const focusedPaneIdOrUndefined = useMemo(() => focusedPaneId ?? undefined, [focusedPaneId]);
  const desktopFocusModeEnabled = useMemo(
    () => isFocusModeEnabled && !isMobile,
    [isFocusModeEnabled, isMobile],
  );
  const workspaceFloatingPanelPortalHostName = useMemo(
    () =>
      `${WORKSPACE_FLOATING_PANEL_PORTAL_HOST_PREFIX}:${normalizedServerId}:${normalizedWorkspaceId}`,
    [normalizedServerId, normalizedWorkspaceId],
  );
  const renderWorkspaceScreenHeader = useCallback(
    () =>
      showScreenHeader ? (
        <ScreenHeader
          left={
            <>
              <SidebarMenuToggle />
              <WorkspaceHeaderTitleBar
                isLoading={isWorkspaceHeaderLoading}
                title={workspaceHeaderTitle}
                subtitle={workspaceHeaderSubtitle}
                isSubtitleDistinct={isWorkspaceHeaderSubtitleDistinct}
                currentBranchName={currentBranchName}
                normalizedServerId={normalizedServerId}
                showWorkspaceSetup={showWorkspaceSetup}
                importAgentDisabled={!canOpenImportSheet}
                copyPathDisabled={!workspaceDirectory}
                onOpenImportSheet={openImportSheet}
                onCopyWorkspacePath={handleCopyWorkspacePath}
                onCopyBranchName={handleCopyBranchName}
                onOpenSetupTab={handleOpenSetupTab}
              />
            </>
          }
          right={headerRight}
        />
      ) : null,
    [
      canOpenImportSheet,
      currentBranchName,
      handleCopyBranchName,
      handleCopyWorkspacePath,
      handleOpenSetupTab,
      headerRight,
      isWorkspaceHeaderLoading,
      normalizedServerId,
      openImportSheet,
      showScreenHeader,
      showWorkspaceSetup,
      workspaceDirectory,
      workspaceHeaderSubtitle,
      workspaceHeaderTitle,
      isWorkspaceHeaderSubtitleDistinct,
    ],
  );
  const scriptActions = useWorkspaceScriptActions({
    client,
    serverId: normalizedServerId,
    workspaceId: normalizedWorkspaceId,
    scripts: workspaceScripts,
    onScriptTerminalStarted: handleScriptTerminalStarted,
  });
  const compactHeaderScripts = useMemo(
    () => ({
      actions: scriptActions,
      scripts: workspaceScripts,
      liveTerminalIds,
      onViewTerminal: handleViewScriptTerminal,
      onOpenUrlInBrowserTab: handleOpenUrlInBrowserTab,
    }),
    [
      scriptActions,
      workspaceScripts,
      liveTerminalIds,
      handleViewScriptTerminal,
      handleOpenUrlInBrowserTab,
    ],
  );
  const compactWorkspaceHeader = useMemo(
    () =>
      isMobile ? (
        <WorkspaceCompactHeader
          isLoading={isWorkspaceHeaderLoading}
          title={workspaceHeaderTitle}
          subtitle={workspaceHeaderSubtitle}
          isSubtitleDistinct={isWorkspaceHeaderSubtitleDistinct}
          normalizedServerId={normalizedServerId}
          normalizedWorkspaceId={normalizedWorkspaceId}
          onToggleExplorer={handleToggleExplorerSidebar}
          explorerToggleLabel={explorerSidebarToggleLabel}
          explorerToggleAccessibilityState={explorerSidebarToggleAccessibilityState}
          tabs={tabs}
          activeTabKey={activeTabKey}
          onSelectSwitcherTab={handleSelectSwitcherTab}
          createTerminalDisabled={createTerminalDisabled}
          onCreateDraftTab={handleCreateDraftTab}
          onCreateTerminal={handleCreateTerminal}
          onCopyResumeCommand={handleCopyResumeCommand}
          onCopyAgentId={handleCopyAgentId}
          onCopyTerminalId={handleCopyTerminalId}
          onCopyFilePath={handleCopyFilePath}
          onReloadAgent={handleReloadAgent}
          onRenameTab={handleRenameTab}
          onCloseTab={handleCloseTabById}
          onCloseTabsAbove={handleCloseTabsToLeft}
          onCloseTabsBelow={handleCloseTabsToRight}
          onCloseOtherTabs={handleCloseOtherTabs}
          scripts={compactHeaderScripts}
          currentBranchName={currentBranchName}
          showWorkspaceSetup={showWorkspaceSetup}
          showCreateBrowserTab={showCreateBrowserTab}
          importAgentDisabled={!canOpenImportSheet}
          copyPathDisabled={!workspaceDirectory}
          onCreateTerminalWithProfile={handleCreateTerminalWithProfile}
          onCreateBrowser={handleCreateBrowserTab}
          onOpenImportSheet={openImportSheet}
          onCopyWorkspacePath={handleCopyWorkspacePath}
          onCopyBranchName={handleCopyBranchName}
          onOpenSetupTab={handleOpenSetupTab}
        />
      ) : null,
    [
      activeTabKey,
      canOpenImportSheet,
      compactHeaderScripts,
      createTerminalDisabled,
      currentBranchName,
      explorerSidebarToggleAccessibilityState,
      explorerSidebarToggleLabel,
      handleCloseOtherTabs,
      handleCloseTabById,
      handleCloseTabsToLeft,
      handleCloseTabsToRight,
      handleCopyAgentId,
      handleCopyBranchName,
      handleCopyFilePath,
      handleCopyResumeCommand,
      handleCopyTerminalId,
      handleCopyWorkspacePath,
      handleCreateBrowserTab,
      handleCreateDraftTab,
      handleCreateTerminal,
      handleCreateTerminalWithProfile,
      handleOpenSetupTab,
      handleReloadAgent,
      handleRenameTab,
      handleSelectSwitcherTab,
      handleToggleExplorerSidebar,
      isMobile,
      isWorkspaceHeaderLoading,
      isWorkspaceHeaderSubtitleDistinct,
      normalizedServerId,
      normalizedWorkspaceId,
      openImportSheet,
      showCreateBrowserTab,
      showWorkspaceSetup,
      tabs,
      workspaceDirectory,
      workspaceHeaderSubtitle,
      workspaceHeaderTitle,
    ],
  );
  const renderCompactWorkspaceHeader = useCallback(
    () => compactWorkspaceHeader,
    [compactWorkspaceHeader],
  );
  const desktopSplitContent = useMemo(() => {
    if (!canRenderDesktopPaneSplits || !workspaceLayout || !persistenceKey) {
      return null;
    }
    return (
      <WorkspaceDesktopSplitView
        layout={workspaceLayout}
        renderMainHeader={renderWorkspaceScreenHeader}
        renderCompactHeader={renderCompactWorkspaceHeader}
        focusModeEnabled={desktopFocusModeEnabled}
        onExitFocusMode={toggleFocusMode}
        workspaceKey={persistenceKey}
        normalizedServerId={normalizedServerId}
        normalizedWorkspaceId={normalizedWorkspaceId}
        isWorkspaceFocused={isRouteFocused}
        uiTabs={uiTabs}
        hoveredCloseTabKey={hoveredCloseTabKey}
        setHoveredCloseTabKey={setHoveredCloseTabKey}
        closingTabIds={closingTabIds}
        onNavigateTab={navigateToTabId}
        onCloseTab={handleCloseTabById}
        onCopyResumeCommand={handleCopyResumeCommand}
        onCopyAgentId={handleCopyAgentId}
        onCopyTerminalId={handleCopyTerminalId}
        onCopyFilePath={handleCopyFilePath}
        onReloadAgent={handleReloadAgent}
        onRenameTab={handleRenameTab}
        onCloseTabsToLeft={handleCloseTabsToLeftInPane}
        onCloseTabsToRight={handleCloseTabsToRightInPane}
        onCloseOtherTabs={handleCloseOtherTabsInPane}
        onCreateNewTab={handleCreateNewTab}
        buildPaneContentModel={buildDesktopPaneContentModel}
        onFocusPane={handleFocusPane}
        onSplitPane={handleSplitPane}
        onSplitPaneEmpty={handleCreateEmptySplit}
        onMoveTabToPane={handleMoveTabToPane}
        onSelectTabInPane={selectTabInPane}
        onResizeSplit={handleResizePaneSplit}
        onReorderTabsInPane={handleReorderTabsInPane}
      />
    );
  }, [
    canRenderDesktopPaneSplits,
    workspaceLayout,
    renderWorkspaceScreenHeader,
    renderCompactWorkspaceHeader,
    persistenceKey,
    desktopFocusModeEnabled,
    toggleFocusMode,
    normalizedServerId,
    normalizedWorkspaceId,
    isRouteFocused,
    uiTabs,
    hoveredCloseTabKey,
    setHoveredCloseTabKey,
    closingTabIds,
    navigateToTabId,
    handleCloseTabById,
    handleCopyResumeCommand,
    handleCopyAgentId,
    handleCopyTerminalId,
    handleCopyFilePath,
    handleReloadAgent,
    handleRenameTab,
    handleCloseTabsToLeftInPane,
    handleCloseTabsToRightInPane,
    handleCloseOtherTabsInPane,
    handleCreateNewTab,
    buildDesktopPaneContentModel,
    handleFocusPane,
    handleSplitPane,
    handleCreateEmptySplit,
    handleMoveTabToPane,
    selectTabInPane,
    handleResizePaneSplit,
    handleReorderTabsInPane,
  ]);
  const desktopContent = desktopSplitContent ?? content;
  const rendersDesktopSplitContent = !isMobile && desktopSplitContent !== null;

  const workspacePanelTabIds = useMemo(
    () => [...uiTabs.map((tab) => tab.tabId), ...sheets.map((sheet) => sheet.tab.tabId)],
    [uiTabs, sheets],
  );
  const workspacePanelKey = `${normalizedServerId}:${normalizedWorkspaceId}`;
  const compactHeaderLayout = resolveCompactHeaderLayout({
    header: compactWorkspaceHeader,
    activeTab: activeTabDescriptor,
    composerFullscreen: useComposerTabFullscreen(workspacePanelKey, activeTabId),
  });

  const workspaceCenterColumn = (
    <WorkspaceCenterColumn
      isMobile={isMobile}
      compactHeaderLayout={compactHeaderLayout}
      rendersDesktopSplitContent={rendersDesktopSplitContent}
      shouldRenderDesktopPaneFallback={shouldRenderDesktopPaneFallback}
      renderWorkspaceScreenHeader={renderWorkspaceScreenHeader}
      newTabLauncher={newTabLauncher}
      focusedPaneIdOrUndefined={focusedPaneIdOrUndefined}
      isRouteFocused={isRouteFocused}
      desktopTabRowItems={desktopTabRowItems}
      normalizedServerId={normalizedServerId}
      normalizedWorkspaceId={normalizedWorkspaceId}
      setHoveredCloseTabKey={setHoveredCloseTabKey}
      navigateToTabId={navigateToTabId}
      handleCloseTabById={handleCloseTabById}
      handleCopyResumeCommand={handleCopyResumeCommand}
      handleCopyAgentId={handleCopyAgentId}
      handleCopyTerminalId={handleCopyTerminalId}
      handleCopyFilePath={handleCopyFilePath}
      handleReloadAgent={handleReloadAgent}
      handleRenameTab={handleRenameTab}
      handleCloseTabsToLeft={handleCloseTabsToLeft}
      handleCloseTabsToRight={handleCloseTabsToRight}
      handleCloseOtherTabs={handleCloseOtherTabs}
      handleCreateNewTab={handleCreateNewTab}
      handleReorderTabsInFocusedPane={handleReorderTabsInFocusedPane}
      desktopFocusModeEnabled={desktopFocusModeEnabled}
      toggleFocusMode={toggleFocusMode}
    >
      {isMobile ? content : desktopContent}
    </WorkspaceCenterColumn>
  );

  const [workspaceViewportWidth, setWorkspaceViewportWidth] = useState(0);
  const measureWorkspaceWidth = useCallback(
    (event: LayoutChangeEvent) => setWorkspaceViewportWidth(event.nativeEvent.layout.width),
    [],
  );
  const sharedExplorer = (
    <WorkspaceExplorerView
      workspaceLayout={workspaceLayout}
      explorerSidebarPaneId={explorerSidebarPaneId}
      persistenceKey={persistenceKey}
      isMobile={isMobile}
      uiTabs={uiTabs}
      normalizedServerId={normalizedServerId}
      normalizedWorkspaceId={normalizedWorkspaceId}
      isRouteFocused={isRouteFocused}
      paneFocusSuspended={paneFocusSuspended}
      closingTabIds={closingTabIds}
      hoveredCloseTabKey={hoveredCloseTabKey}
      setHoveredCloseTabKey={setHoveredCloseTabKey}
      selectTabInPane={selectTabInPane}
      handleCloseTabById={handleCloseTabById}
      handleCreateNewTab={handleCreateNewTab}
      handleCopyResumeCommand={handleCopyResumeCommand}
      handleCopyAgentId={handleCopyAgentId}
      handleCopyTerminalId={handleCopyTerminalId}
      handleCopyFilePath={handleCopyFilePath}
      handleReloadAgent={handleReloadAgent}
      handleRenameTab={handleRenameTab}
      handleCloseTabsToLeftInPane={handleCloseTabsToLeftInPane}
      handleCloseTabsToRightInPane={handleCloseTabsToRightInPane}
      handleCloseOtherTabsInPane={handleCloseOtherTabsInPane}
      toggleFocusMode={toggleFocusMode}
      handleReorderTabsInPane={handleReorderTabsInPane}
      buildDesktopPaneContentModel={buildDesktopPaneContentModel}
      renderExplorerSidebarHeaderAction={renderExplorerSidebarHeaderAction}
      workspaceViewportWidth={workspaceViewportWidth}
      isExplorerSidebarShowing={isExplorerSidebarShowing}
      desktopFocusModeEnabled={desktopFocusModeEnabled}
    />
  );

  const { revealDirectory } = useFileExplorerActions({
    serverId: normalizedServerId,
    workspaceId: normalizedWorkspaceId,
    workspaceRoot: workspaceDirectory,
  });
  const { openMarkdownPath } = useWorkspaceMarkdownOpener({
    isRouteFocused,
    activeTabId,
    workspaceDirectory,
    client,
    normalizedServerId,
    persistenceKey: persistenceKey ?? "",
    isMobile,
    handleOpenPreferredAssistantFile,
    revealDirectory,
    openExplorerSidebarView,
  });

  const renderedWorkspaceScreen = (
    <RenderProfile id="WorkspaceScreenContent">
      <View style={containerStyle}>
        <WorkspaceDocumentTitleEffectSlot
          tab={activeTabDescriptor}
          serverId={normalizedServerId}
          workspaceId={normalizedWorkspaceId}
          isRouteFocused={isRouteFocused}
        />
        <WorkspacePanelSurface
          onLayout={measureWorkspaceWidth}
          launcher={newTabLauncher}
          workspaceKey={workspacePanelKey}
          tabIds={workspacePanelTabIds}
        >
          <FloatingPanelPortalHostNameProvider hostName={workspaceFloatingPanelPortalHostName}>
            <WorkspaceTabDragScope
              layout={workspaceLayout}
              explorerSidebarPaneId={explorerSidebarPaneId}
              allowCrossPane={layoutMode === "split-panes"}
              normalizedServerId={normalizedServerId}
              normalizedWorkspaceId={normalizedWorkspaceId}
              uiTabs={uiTabs}
              onMoveTabToPane={handleMoveTabToPane}
              onReorderTabsInPane={handleReorderTabsInPane}
              onSplitPane={handleSplitPane}
            >
              {workspaceCenterColumn}
              {sharedExplorer}
            </WorkspaceTabDragScope>
          </FloatingPanelPortalHostNameProvider>
          <FloatingPanelPortalHost name={workspaceFloatingPanelPortalHostName} />
        </WorkspacePanelSurface>
        <ImportSessionSheet
          visible={isRouteFocused && isImportSheetVisible}
          client={client}
          serverId={normalizedServerId}
          cwd={workspaceDirectory}
          workspaceId={normalizedWorkspaceId}
          onClose={closeImportSheet}
          onImportedAgent={handleImportedAgent}
          onImported={navigateToImportedAgent}
        />
        <WorkspaceTabRenameModal
          renamingTab={isRouteFocused ? renamingTab : null}
          onSubmit={handleRenameModalSubmit}
          onClose={handleRenameModalClose}
        />
      </View>
    </RenderProfile>
  );

  if (workspaceScreenGate) {
    return workspaceScreenGate;
  }
  return (
    <WorkspaceContentProviders
      key={persistenceKey}
      workspaceKey={persistenceKey}
      sheetPaneHost={sheetPaneHost}
    >
      <AssistantFileLinkResolverProvider
        client={client}
        serverId={normalizedServerId}
        workspaceRoot={workspaceDirectory ?? undefined}
        onOpenWorkspaceFile={openMarkdownPath}
        toast={toast}
      >
        {renderedWorkspaceScreen}
      </AssistantFileLinkResolverProvider>
    </WorkspaceContentProviders>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
  },
  containerWorkspaceBackground: {
    backgroundColor: theme.colors.surfaceWorkspace,
  },
  threePaneRow: {
    zIndex: 0,
    flex: 1,
    minHeight: 0,
    flexDirection: "row",
    alignItems: "stretch",
  },
  centerColumn: {
    flex: 1,
    minHeight: 0,
  },
  centerContent: {
    flex: 1,
    minHeight: 0,
  },
  content: {
    flex: 1,
    minHeight: 0,
    backgroundColor: theme.colors.surface0,
    position: "relative",
  },
}));
