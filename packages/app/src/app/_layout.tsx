import "@/styles/unistyles";
import { BottomSheetModalProvider } from "@gorhom/bottom-sheet";
import { PortalProvider } from "@gorhom/portal";
import { LucideProvider } from "lucide-react-native";
import * as Linking from "expo-linking";
import { Stack, useNavigationContainerRef, useRouter } from "expo-router";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { AppState, View } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { KeyboardProvider } from "react-native-keyboard-controller";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { AppearanceProvider } from "@/appearance/provider";
import { useHostChooser } from "@/hosts/host-chooser";
import { isNative } from "@/constants/platform";
import { HorizontalScrollProvider } from "@/contexts/horizontal-scroll-context";
import { SidebarCalloutProvider } from "@/contexts/sidebar-callout-context";
import { ToastProvider } from "@/contexts/toast-context";
import { VoiceProvider } from "@/contexts/voice-context";
import { registerWorkspaceRouteNavigationRef } from "@/navigation/workspace-route-navigation";
import { ThemedStack } from "@/navigation/themed-stack";
import { AgentNavigationListener } from "@/desktop/agent-navigation";
import { listenToDesktopEvent } from "@/desktop/electron/events";
import { updateDesktopWindowChrome } from "@/desktop/electron/window";
import { getDesktopHost } from "@/desktop/host";
import { useFaviconStatus } from "@/hooks/use-favicon-status";
import { KeyboardShiftProvider } from "@/keyboard/shift";
import { useOpenProject } from "@/hooks/use-open-project";
import { useAppSettings } from "@/hooks/use-settings";
import { MobilePanelsProvider } from "@/mobile-panels/provider";
import { KeyboardActionDispatcherProvider } from "@/keyboard/keyboard-action-dispatcher-context";
import { polyfillCrypto } from "@/polyfills/crypto";
import { getHostRuntimeStore, useHostRegistryLoaded } from "@/runtime/host-runtime";
import { flushDraftPersistStorage } from "@/stores/draft-store";
import { ICON_STROKE_WIDTH } from "@/styles/theme";
import { installWebScrollbarStyles } from "@/styles/install-web-scrollbar-styles";
import { WindowChromeProvider } from "@/utils/desktop-window";
import { buildOpenProjectRoute } from "@/utils/host-routes";
import { NotificationOpenRouter } from "@/notification-open/react";
import { ensureOsNotificationPermission } from "@/utils/os-notifications";

import {
  HostRuntimeBootstrapProvider,
  HostSessionManager,
  useStoreReady,
  useHostRuntimeBootstrapState,
  useEarliestOnlineHostServerId,
  type HostRuntimeBootstrapState,
} from "./bootstrap/host-runtime-bootstrap";
import { AppWithSidebar } from "./shell/app-shell";

export {
  useHostRuntimeBootstrapState,
  useEarliestOnlineHostServerId,
  useStoreReady,
  type HostRuntimeBootstrapState,
};

polyfillCrypto();

const flexStyle = { flex: 1 } as const;

interface WindowWithIdleCallback {
  requestIdleCallback?: (callback: () => void) => number;
  cancelIdleCallback?: (handle: number) => void;
}

function ProvidersWrapper({ children }: { children: ReactNode }) {
  return (
    <AppearanceProvider>
      <VoiceProvider>
        <DesktopWindowControlsSync />
        <OfferLinkListener />
        <HostSessionManager />
        <FaviconStatusSync />
        {children}
      </VoiceProvider>
    </AppearanceProvider>
  );
}

function DesktopWindowControlsSync() {
  const { isLoading } = useAppSettings();
  const { theme } = useUnistyles();
  const surface0 = theme.colors.surface0;

  useEffect(() => {
    if (isLoading || isNative) return;
    void updateDesktopWindowChrome({
      backgroundColor: surface0,
      trafficLightOffsetY: -4,
    }).catch((error) => {
      console.warn("[DesktopWindow] Failed to update window controls overlay", error);
    });
  }, [isLoading, surface0]);

  return null;
}

function OfferLinkListener() {
  const router = useRouter();

  useEffect(() => {
    let cancelled = false;
    const handleUrl = async (url: string | null) => {
      if (!url) return;
      if (!url.includes("#offer=") && !url.includes("#connect=") && !url.startsWith("relay://"))
        return;
      try {
        const result = await getHostRuntimeStore().importConnectionLink(url, "openProject");
        if (!cancelled && result.status === "connected") router.replace(buildOpenProjectRoute());
      } catch (error) {
        console.warn("[OfferLinkListener] Pairing link failed", error);
      }
    };

    void Linking.getInitialURL()
      .then((url) => handleUrl(url))
      .catch(() => undefined);

    const subscription = Linking.addEventListener("url", (event) => {
      void handleUrl(event.url);
    });

    return () => {
      cancelled = true;
      subscription.remove();
    };
  }, [router]);

  return null;
}

interface OpenProjectEventPayload {
  path?: unknown;
}

interface PendingOpenProjectRequest {
  id: number;
  serverId: string;
  path: string;
}

let nextOpenProjectRequestId = 1;

function OpenProjectListener() {
  const chooseHost = useHostChooser();
  const hostRegistryLoaded = useHostRegistryLoaded();
  const [request, setRequest] = useState<PendingOpenProjectRequest | null>(null);
  const [pendingPath, setPendingPath] = useState<string | null>(null);
  const openProject = useOpenProject(request?.serverId ?? null);

  const openPathOnChosenHost = useCallback(
    (path: string) => {
      const nextPath = path.trim();
      if (!nextPath) {
        return;
      }

      if (!hostRegistryLoaded) {
        setPendingPath(nextPath);
        return;
      }

      chooseHost({
        title: "Choose host",
        onChooseHost: (serverId) => {
          setRequest({
            id: nextOpenProjectRequestId++,
            serverId,
            path: nextPath,
          });
        },
      });
    },
    [chooseHost, hostRegistryLoaded],
  );

  useEffect(() => {
    if (!hostRegistryLoaded || !pendingPath) {
      return;
    }
    const nextPath = pendingPath;
    setPendingPath(null);
    openPathOnChosenHost(nextPath);
  }, [hostRegistryLoaded, openPathOnChosenHost, pendingPath]);

  useEffect(() => {
    if (!request) {
      return;
    }
    let cancelled = false;
    void openProject(request.path).then((result) => {
      if (cancelled) {
        return null;
      }

      if (!result.ok) {
        setRequest((current) => (current?.id === request.id ? null : current));
        return null;
      }

      setRequest((current) => (current?.id === request.id ? null : current));
      return null;
    });
    return () => {
      cancelled = true;
    };
  }, [openProject, request]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void getDesktopHost()
      ?.getPendingOpenProject?.()
      ?.then((pending) => {
        if (!disposed && pending) {
          openPathOnChosenHost(pending);
        }
        return;
      })
      .catch(() => undefined);

    void listenToDesktopEvent<OpenProjectEventPayload>("open-project", (payload) => {
      if (disposed) {
        return;
      }
      const nextPath = typeof payload?.path === "string" ? payload.path.trim() : "";
      openPathOnChosenHost(nextPath);
    })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
        return;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [openPathOnChosenHost]);

  return null;
}

function FaviconStatusSync() {
  useFaviconStatus();
  return null;
}

const ROOT_STACK_SCREEN_OPTIONS = {
  headerShown: false,
  animation: "none" as const,
};
const ROOT_STACK_NESTED_NAVIGATOR_SCREENS = ["h/[serverId]"] as const;

function RootStack() {
  const storeReady = useStoreReady();
  return (
    <ThemedStack
      screenOptions={ROOT_STACK_SCREEN_OPTIONS}
      nestedNavigatorScreens={ROOT_STACK_NESTED_NAVIGATOR_SCREENS}
    >
      <Stack.Screen name="index" />
      <Stack.Protected guard={storeReady}>
        <Stack.Screen name="welcome" />
        <Stack.Screen name="settings/index" />
        <Stack.Screen name="settings/[section]" />
        <Stack.Screen name="new" />
        <Stack.Screen name="open-project" />
        <Stack.Screen name="sessions" />
        <Stack.Screen name="schedules" />
        <Stack.Screen name="pair-scan" />
      </Stack.Protected>
      <Stack.Screen name="h/[serverId]" />
      <Stack.Screen name="settings/hosts/[serverId]/index" />
      <Stack.Screen name="settings/hosts/[serverId]/[hostSection]" />
      <Stack.Screen name="settings/hosts/[serverId]/plugins/[pluginId]/[screenId]" />
      <Stack.Screen name="settings/hosts/[serverId]/projects/index" />
      <Stack.Screen name="settings/hosts/[serverId]/projects/[projectId]" />
    </ThemedStack>
  );
}

function WorkspaceRouteNavigationBridge() {
  const navigationRef = useNavigationContainerRef();

  useEffect(() => {
    return registerWorkspaceRouteNavigationRef(navigationRef);
  }, [navigationRef]);

  return null;
}

function AppShell() {
  useEffect(() => {
    if (getDesktopHost()) void ensureOsNotificationPermission();
  }, []);
  return (
    <MobilePanelsProvider>
      <HorizontalScrollProvider>
        <OpenProjectListener />
        <AgentNavigationListener />
        <AppWithSidebar>
          <WorkspaceRouteNavigationBridge />
          <RootStack />
        </AppWithSidebar>
      </HorizontalScrollProvider>
    </MobilePanelsProvider>
  );
}

function NotificationOpenBridge() {
  const { startupBlocker } = useHostRuntimeBootstrapState();
  return <NotificationOpenRouter startupBlocked={startupBlocker.kind !== "none"} />;
}

function RuntimeProviders({ children }: { children: ReactNode }) {
  return (
    <HostRuntimeBootstrapProvider>
      <NotificationOpenBridge />
      <SidebarCalloutProvider>
        <ProvidersWrapper>{children}</ProvidersWrapper>
      </SidebarCalloutProvider>
    </HostRuntimeBootstrapProvider>
  );
}

function RootProviders({ children }: { children: ReactNode }) {
  return (
    <KeyboardActionDispatcherProvider>
      <WindowChromeProvider>
        <KeyboardProvider>
          <KeyboardShiftProvider>
            <ToastProvider>
              <PortalProvider>
                <BottomSheetModalProvider>{children}</BottomSheetModalProvider>
              </PortalProvider>
            </ToastProvider>
          </KeyboardShiftProvider>
        </KeyboardProvider>
      </WindowChromeProvider>
    </KeyboardActionDispatcherProvider>
  );
}

function RootAppTree() {
  return (
    <GestureHandlerRootView style={flexStyle}>
      <View style={layoutStyles.surfaceFill}>
        <LucideProvider strokeWidth={ICON_STROKE_WIDTH}>
          <RootProviders>
            <RuntimeProviders>
              <AppShell />
            </RuntimeProviders>
          </RootProviders>
        </LucideProvider>
      </View>
    </GestureHandlerRootView>
  );
}

export default function RootLayout() {
  useEffect(() => installWebScrollbarStyles(), []);
  useEffect(() => {
    void getDesktopHost()
      ?.invoke?.("mark_first_paint")
      ?.catch?.(() => {});
  }, []);
  useEffect(() => {
    if (typeof window === "undefined") return;
    const win = window as unknown as WindowWithIdleCallback;
    const schedulePrefetch =
      typeof win.requestIdleCallback === "function"
        ? (cb: () => void) => win.requestIdleCallback!(cb)
        : (cb: () => void) => setTimeout(cb, 1200);

    const handle = schedulePrefetch(() => {
      void import("../terminal/runtime/terminal-emulator-runtime").catch(() => {});
      void import("../file-panels/source/editor/view.web").catch(() => {});
    });

    return () => {
      if (typeof win.cancelIdleCallback === "function" && typeof handle === "number") {
        win.cancelIdleCallback(handle);
      }
    };
  }, []);
  useEffect(() => {
    const subscription = AppState.addEventListener("change", (nextState) => {
      if (nextState !== "active") {
        void flushDraftPersistStorage();
      }
    });
    return () => subscription.remove();
  }, []);

  return <RootAppTree />;
}

const layoutStyles = StyleSheet.create((theme) => ({
  surfaceFill: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
  },
}));
