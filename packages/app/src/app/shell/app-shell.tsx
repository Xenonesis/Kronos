import { type ReactNode, useCallback, useMemo } from "react";
import { useWindowDimensions, View } from "react-native";
import { GestureDetector } from "react-native-gesture-handler";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { usePathname } from "expo-router";
import { StyleSheet } from "react-native-unistyles";

import { useActiveWorkspaceSelection } from "@/stores/navigation-active-workspace-store";
import { useIsExplorerSidebarOpen } from "@/workspace-tabs/explorer-sidebar";
import { ComposerEditingSessionsProvider } from "@/composer/editing-session";
import { SheetHostScope } from "@/components/ui/isolated-bottom-sheet-modal";
import { CommandCenter } from "@/command-center/command-center";
import { CommandCenterRootActions } from "@/command-center/root-registration";
import { CommandCenterProvider } from "@/command-center/provider";
import { CommandCenterWorkspaceActions } from "@/command-center/workspace-registration";
import { PluginCommandCenterActions } from "@/plugins/command-center/registration";
import { AddProjectFlowHost } from "@/components/add-project-flow-host";
import { WorktreeSetupCalloutSource } from "@/components/worktree-setup-callout-source";
import { DownloadToast } from "@/components/download-toast";
import { QuittingOverlay } from "@/components/quitting-overlay";
import { KeyboardShortcutsDialog } from "@/components/keyboard-shortcuts-dialog";
import { ChangelogHost } from "@/changelog";
import { AppDiagnosticHost } from "@/components/app-diagnostic-host";
import { AppearanceStyleBoundary } from "@/components/appearance-style-boundary";
import { LeftSidebar } from "@/components/left-sidebar";
import { WindowSidebarMenuToggle } from "@/components/headers/sidebar-menu-toggle";
import { DesktopWindowControls } from "@/components/desktop/window-controls";
import { SidebarModelProvider } from "@/components/sidebar/sidebar-model";
import { WorkspacePinShortcutHandler } from "@/components/workspace-pin-shortcut-handler";
import { WorkspaceRenameHost } from "@/components/workspace-rename-host";
import { WorkspaceExplorerOpenGesture } from "@/mobile-panels/explorer-open-gesture";
import { ProviderSettingsHost } from "@/components/provider-settings-host";
import { WorkspaceSetupDialog } from "@/components/workspace-setup-dialog";
import { WorkspaceShortcutTargetsSubscriber } from "@/components/workspace-shortcut-targets-subscriber";
import { FloatingPanelPortalHost } from "@/components/ui/floating-panel-portal";
import { HostChooserModal } from "@/hosts/host-chooser";
import { HostConfirmationSheet } from "@/hosts/host-confirmation-sheet";
import { HEADER_INNER_HEIGHT, useIsCompactFormFactor } from "@/constants/layout";
import {
  canDesktopAppSidebarShare,
  resolveDesktopAppChromeLayout,
  resolveDesktopAppContentMinimum,
  resolveDesktopSidebarVisibility,
} from "@/components/desktop-sidebar-layout";
import { isWeb } from "@/constants/platform";
import { useDesktopNativeBridgeCoordinator } from "@/desktop/use-desktop-native-bridge-coordinator";
import { useSettingsRouteEntry } from "@/navigation/settings-presentation";
import { LegacyAgentSkillsMigration } from "@/agent-skills/legacy-migration";
import { RosettaCalloutSource } from "@/desktop/updates/rosetta-callout-source";
import { UpdateCalloutSource } from "@/desktop/updates/update-callout-source";
import { GlobalCommandPaletteHost } from "@/desktop/command-palette/command-palette-host";
import { useActiveWorktreeNewAction } from "@/hooks/use-active-worktree-new-action";
import { useGlobalNewWorkspaceAction } from "@/hooks/use-global-new-workspace-action";
import { useLatchedBoolean } from "@/hooks/use-latched-boolean";
import { useKeyboardShortcuts } from "@/hooks/use-keyboard-shortcuts";
import { useCompactWebViewportZoomLock } from "@/hooks/use-compact-web-viewport-zoom-lock";
import { useAppSettings } from "@/hooks/use-settings";
import { useOpenAgentListGesture } from "@/mobile-panels/gestures";
import { useIsMobilePanelActive } from "@/mobile-panels/provider";
import { useKeyboardActionDispatcher } from "@/keyboard/keyboard-action-dispatcher-context";
import { useHosts } from "@/runtime/host-runtime";
import { usePanelStore } from "@/stores/panel-store";
import { getNextThemePreference } from "@/styles/theme";
import { WorkspaceLayoutModePublisher } from "@/workspace-tabs/layout-mode-publisher";
import {
  useHasWindowChromeObstruction,
  WindowChromeRegion,
  WindowChromeSafeArea,
} from "@/utils/desktop-window";
import {
  parseHostWorkspaceRouteFromPathname,
  parseServerIdFromPathname,
} from "@/utils/host-routes";
import { useStoreReady } from "../bootstrap/host-runtime-bootstrap";

const rowStyle = { flex: 1, flexDirection: "row" } as const;
const flexStyle = { flex: 1 } as const;
const MOBILE_WEB_GESTURE_TOUCH_ACTION = isWeb ? "auto" : "pan-y";
const WINDOW_SIDEBAR_TOGGLE_HORIZONTAL_PADDING = 12;

interface AppContainerProps {
  children: ReactNode;
  chromeEnabled?: boolean;
}

export function AppContainer({
  children,
  chromeEnabled: chromeEnabledOverride,
}: AppContainerProps) {
  const keyboardActionDispatcher = useKeyboardActionDispatcher();
  const daemons = useHosts();
  const { settings, updateSettings } = useAppSettings();
  const toggleMobileAgentList = usePanelStore((state) => state.toggleMobileAgentList);
  const toggleDesktopAgentList = usePanelStore((state) => state.toggleDesktopAgentList);
  const exitFocusMode = usePanelStore((state) => state.exitFocusMode);
  const isFocusModeEnabled = usePanelStore((state) => state.desktop.focusModeEnabled);
  const isDesktopAgentListOpen = usePanelStore((state) => state.desktop.agentListOpen);
  const sidebarWidth = usePanelStore((state) => state.sidebarWidth);
  const { width: windowWidth } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const viewportWidth = windowWidth - insets.left - insets.right;

  const cycleTheme = useCallback(() => {
    void updateSettings({ theme: getNextThemePreference(settings.theme) });
  }, [settings.theme, updateSettings]);

  const isCompactLayout = useIsCompactFormFactor();
  useCompactWebViewportZoomLock(isCompactLayout);
  const pathname = usePathname();
  const isWorkspaceRoute = parseHostWorkspaceRouteFromPathname(pathname) !== null;
  const isWorkspaceFocusModeEnabled = isWorkspaceRoute && isFocusModeEnabled;
  const chromeEnabled = chromeEnabledOverride ?? daemons.length > 0;
  const hasMountedDesktopSidebar = useLatchedBoolean(chromeEnabled);
  const toggleAgentList = isCompactLayout ? toggleMobileAgentList : toggleDesktopAgentList;
  const toggleDesktopSidebars = useCallback(() => {
    if (
      keyboardActionDispatcher.dispatch({
        id: "sidebar.toggle.both",
        scope: "sidebar",
      })
    ) {
      return;
    }
    toggleAgentList();
  }, [keyboardActionDispatcher, toggleAgentList]);

  const keyboardShortcutsEnabled = chromeEnabled || pathname.startsWith("/settings");

  useKeyboardShortcuts({
    enabled: keyboardShortcutsEnabled,
    isMobile: isCompactLayout,
    isWorkspaceFocusModeEnabled,
    toggleAgentList,
    toggleBothSidebars: toggleDesktopSidebars,
    exitFocusMode,
    cycleTheme,
  });

  useActiveWorktreeNewAction();
  useGlobalNewWorkspaceAction();
  useDesktopNativeBridgeCoordinator();

  const activeWorkspace = useActiveWorkspaceSelection();
  const explorerVisible = useIsExplorerSidebarOpen({
    isCompact: isCompactLayout,
    workspaceKey: activeWorkspace
      ? `${activeWorkspace.serverId}:${activeWorkspace.workspaceId}`
      : null,
  });
  const appContentMinimumWidth = resolveDesktopAppContentMinimum({
    explorerVisible: isWorkspaceRoute && explorerVisible,
    isSettingsRoute: pathname.includes("/settings"),
  });
  const desktopSidebarMounted = hasMountedDesktopSidebar && !isWorkspaceFocusModeEnabled;
  const desktopSidebarVisible = resolveDesktopSidebarVisibility({
    chromeEnabled,
    isCompactLayout,
    isMounted: desktopSidebarMounted,
    isOpen: isDesktopAgentListOpen,
    canShare: canDesktopAppSidebarShare({
      contentMinimumWidth: appContentMinimumWidth,
      requestedSidebarWidth: sidebarWidth,
      viewportWidth,
    }),
  });
  const hasTopLeftWindowControls = useHasWindowChromeObstruction("top-left");
  const appChromeLayout = resolveDesktopAppChromeLayout({
    desktopSidebarRendered: desktopSidebarVisible,
    hasTopLeftWindowControls,
    sidebarControlsEnabled: chromeEnabled && !isWorkspaceFocusModeEnabled,
  });
  const sidebarChrome = (
    <SidebarChrome
      mounted={isCompactLayout ? chromeEnabled : desktopSidebarMounted}
      visible={isCompactLayout ? chromeEnabled : desktopSidebarVisible}
      keyboardShortcutsEnabled={keyboardShortcutsEnabled}
    />
  );
  let themedSidebarChrome = sidebarChrome;
  if (isWeb) {
    themedSidebarChrome = <AppearanceStyleBoundary>{sidebarChrome}</AppearanceStyleBoundary>;
  }
  const workspaceChromeStyle = useMemo(
    () => [
      rowStyle,
      !isCompactLayout && {
        paddingLeft: insets.left,
        paddingRight: insets.right,
      },
    ],
    [insets.left, insets.right, isCompactLayout],
  );
  const workspaceChrome = (
    <View style={workspaceChromeStyle}>
      <WorkspaceLayoutModePublisher />
      {!isCompactLayout ? (
        <WindowChromeRegion corners={appChromeLayout.sidebarCorners}>
          {themedSidebarChrome}
        </WindowChromeRegion>
      ) : null}
      <WorkspaceExplorerOpenGesture enabled={chromeEnabled && isCompactLayout}>
        <WindowChromeRegion
          corners={isCompactLayout && chromeEnabled ? "both" : appChromeLayout.contentCorners}
        >
          <View style={flexStyle}>{children}</View>
        </WindowChromeRegion>
      </WorkspaceExplorerOpenGesture>
    </View>
  );

  const surface = (
    <View style={layoutStyles.surfaceFill}>
      {workspaceChrome}
      <AppearanceStyleBoundary>
        {!isCompactLayout && appChromeLayout.sidebarToggleOwner === "window" ? (
          <WindowChromeRegion corners="top-left">
            <WindowChromeSafeArea
              placement="inline"
              horizontalPadding={WINDOW_SIDEBAR_TOGGLE_HORIZONTAL_PADDING}
              pointerEvents="box-none"
              style={layoutStyles.windowSidebarToggle}
            >
              <WindowSidebarMenuToggle />
            </WindowChromeSafeArea>
          </WindowChromeRegion>
        ) : null}
        <DesktopWindowControls />
        <FloatingPanelPortalHost />
      </AppearanceStyleBoundary>
      {isCompactLayout ? themedSidebarChrome : null}
      <AppearanceStyleBoundary>
        <DownloadToast />
        <RosettaCalloutSource />
        <UpdateCalloutSource />
        <LegacyAgentSkillsMigration />
        <WorktreeSetupCalloutSource />
        <CommandCenterRootActions />
        <CommandCenterWorkspaceActions />
        <PluginCommandCenterActions />
        <WorkspacePinShortcutHandler />
        <WorkspaceRenameHost />
        <CommandCenter />
        <GlobalCommandPaletteHost />
        <AddProjectFlowHost />
        <HostChooserModal />
        <HostConfirmationSheet />
        <ProviderSettingsHost />
        <WorkspaceSetupDialog />
        <KeyboardShortcutsDialog />
        <AppDiagnosticHost />
        <ChangelogHost />
        <QuittingOverlay />
      </AppearanceStyleBoundary>
    </View>
  );

  return (
    <AppContainerLifetime compactGesturesEnabled={chromeEnabled && isCompactLayout}>
      <SheetHostScope enabled={isCompactLayout}>{surface}</SheetHostScope>
    </AppContainerLifetime>
  );
}

function AppContainerLifetime({
  children,
  compactGesturesEnabled,
}: {
  children: ReactNode;
  compactGesturesEnabled: boolean;
}) {
  return (
    <ComposerEditingSessionsProvider>
      <CommandCenterProvider>
        <MobileGestureWrapper chromeEnabled={compactGesturesEnabled}>
          {children}
        </MobileGestureWrapper>
      </CommandCenterProvider>
    </ComposerEditingSessionsProvider>
  );
}

export function SidebarChrome({
  mounted,
  visible,
  keyboardShortcutsEnabled,
}: {
  mounted: boolean;
  visible: boolean;
  keyboardShortcutsEnabled: boolean;
}) {
  const isCompactLayout = useIsCompactFormFactor();
  const isMobileActive = useIsMobilePanelActive("agent-list");
  const isDesktopOpen = usePanelStore((state) => state.desktop.agentListOpen);
  const active = visible && (isCompactLayout ? isMobileActive : isDesktopOpen);
  return (
    <SidebarModelProvider active={active}>
      {mounted ? <LeftSidebar active={active} /> : null}
      <WorkspaceShortcutTargetsSubscriber enabled={keyboardShortcutsEnabled} />
    </SidebarModelProvider>
  );
}

export function MobileGestureWrapper({
  children,
  chromeEnabled,
}: {
  children: ReactNode;
  chromeEnabled: boolean;
}) {
  const openGesture = useOpenAgentListGesture(chromeEnabled);
  const isCompactLayout = useIsCompactFormFactor();

  return (
    <GestureDetector gesture={openGesture} touchAction={MOBILE_WEB_GESTURE_TOUCH_ACTION}>
      <View
        collapsable={false}
        style={layoutStyles.surfaceFill}
        testID={isCompactLayout ? "compact-shell" : "wide-shell"}
      >
        {children}
      </View>
    </GestureDetector>
  );
}

export function AppWithSidebar({ children }: { children: ReactNode }) {
  useSettingsRouteEntry();
  const pathname = usePathname();
  const hosts = useHosts();
  const storeReady = useStoreReady();
  const routeServerId = useMemo(() => parseServerIdFromPathname(pathname), [pathname]);
  const routeHasKnownHost =
    routeServerId !== null && hosts.some((host) => host.serverId === routeServerId);
  const shouldShowAppChrome =
    storeReady &&
    (pathname === "/open-project" ||
      pathname === "/new" ||
      pathname === "/sessions" ||
      pathname === "/schedules" ||
      routeHasKnownHost);

  return <AppContainer chromeEnabled={shouldShowAppChrome}>{children}</AppContainer>;
}

export const layoutStyles = StyleSheet.create((theme) => ({
  surfaceFill: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
  },
  windowSidebarToggle: {
    position: "absolute",
    top: 1,
    left: 0,
    zIndex: 20,
    height: HEADER_INNER_HEIGHT,
    flexDirection: "row",
    alignItems: "center",
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: "transparent",
  },
}));
