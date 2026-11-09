import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
} from "react";
import { AppState } from "react-native";
import { SessionProvider } from "@/contexts/session-context";
import { legacyFavoriteProfileMigration } from "@/agent-profiles/migration";
import { PluginCatalogSync } from "@/plugins";
import { useSessionStore } from "@/stores/session-store";
import {
  getHostRuntimeStore,
  hasConfiguredLocalDaemonOverride,
  useHostRuntimeClient,
  useHostRuntimeIsConnected,
  useHosts,
} from "@/runtime/host-runtime";
import { getDaemonStartService } from "@/runtime/daemon-start-service";
import { shouldUseDesktopDaemon } from "@/desktop/daemon/desktop-daemon";
import { loadDesktopSettings } from "@/desktop/settings/desktop-settings";
import {
  resolveStartupBlocker,
  resolveStartupNavigationReady,
  shouldRunStartupGiveUpTimer,
  startHostRuntimeBootstrap,
  bindHostRuntimeAppState,
  type StartupBlocker,
} from "@/navigation/host-runtime-bootstrap";
import type { HostProfile } from "@/types/host-connection";

export interface HostRuntimeBootstrapState {
  splashError: string | null;
  retry: () => void;
  hasGivenUpWaitingForHost: boolean;
  storeReady: boolean;
  startupBlocker: StartupBlocker;
}

const HostRuntimeBootstrapContext = createContext<HostRuntimeBootstrapState>({
  splashError: null,
  retry: () => {},
  hasGivenUpWaitingForHost: false,
  storeReady: false,
  startupBlocker: { kind: "none" },
});

function ManagedDaemonSession({ daemon }: { daemon: HostProfile }) {
  const client = useHostRuntimeClient(daemon.serverId);

  if (!client) {
    return null;
  }

  return (
    <SessionProvider key={daemon.serverId} serverId={daemon.serverId} client={client}>
      <LegacyFavoriteProfileMigrationBootstrap serverId={daemon.serverId} client={client} />
      <PluginCatalogSync serverId={daemon.serverId} client={client} />
    </SessionProvider>
  );
}

function LegacyFavoriteProfileMigrationBootstrap({
  serverId,
  client,
}: {
  serverId: string;
  client: NonNullable<ReturnType<typeof useHostRuntimeClient>>;
}) {
  const serverInfo = useSessionStore((state) => state.sessions[serverId]?.serverInfo ?? null);
  const isConnected = useHostRuntimeIsConnected(serverId);

  useEffect(() => {
    if (!serverInfo || !isConnected) {
      return;
    }
    void legacyFavoriteProfileMigration.migrateHost(serverId, client).catch((error) => {
      console.warn("[AgentProfiles] Failed to migrate legacy favourites", error);
    });
  }, [client, isConnected, serverId, serverInfo]);

  return null;
}

export function HostSessionManager() {
  const hosts = useHosts();

  if (hosts.length === 0) {
    return null;
  }

  return (
    <>
      {hosts.map((daemon) => (
        <ManagedDaemonSession key={daemon.serverId} daemon={daemon} />
      ))}
    </>
  );
}

export function useEarliestOnlineHostServerId(): string | null {
  const store = getHostRuntimeStore();
  const subscribe = useCallback(
    (listener: () => void) => {
      const unsubscribeAll = store.subscribeAll(listener);
      const unsubscribeHostList = store.subscribeHostList(listener);
      return () => {
        unsubscribeAll();
        unsubscribeHostList();
      };
    },
    [store],
  );
  return useSyncExternalStore(
    subscribe,
    () => store.getEarliestOnlineHostServerId(),
    () => store.getEarliestOnlineHostServerId(),
  );
}

function useDaemonStartLastError(): string | null {
  const service = getDaemonStartService({ store: getHostRuntimeStore() });
  return useSyncExternalStore(
    (listener) => service.subscribe(listener),
    () => service.getLastError(),
    () => service.getLastError(),
  );
}

function useDaemonStartIsRunning(): boolean {
  const service = getDaemonStartService({ store: getHostRuntimeStore() });
  return useSyncExternalStore(
    (listener) => service.subscribe(listener),
    () => service.isRunning(),
    () => service.isRunning(),
  );
}

const STARTUP_GIVE_UP_TIMEOUT_MS = 5_000;

async function shouldStartBuiltInDaemon(): Promise<boolean> {
  if (!shouldUseDesktopDaemon()) {
    return false;
  }
  if (hasConfiguredLocalDaemonOverride()) {
    return false;
  }
  const settings = await loadDesktopSettings();
  return settings.daemon.manageBuiltInDaemon;
}

export function HostRuntimeBootstrapProvider({ children }: { children: ReactNode }) {
  useEffect(() => {
    const store = getHostRuntimeStore();
    return bindHostRuntimeAppState(store, AppState);
  }, []);

  useEffect(() => {
    const store = getHostRuntimeStore();
    const daemonStartService = getDaemonStartService({ store });
    startHostRuntimeBootstrap({
      store,
      daemonStartService,
      shouldStartDaemon: shouldStartBuiltInDaemon,
    });
  }, []);

  const anyOnlineHostServerId = useEarliestOnlineHostServerId();
  const daemonStartError = useDaemonStartLastError();
  const daemonStartIsRunning = useDaemonStartIsRunning();
  const [hasGivenUpWaitingForHost, setHasGivenUpWaitingForHost] = useState(false);
  const isDesktopRuntime = shouldUseDesktopDaemon();
  const startupBlocker = useMemo(
    () =>
      resolveStartupBlocker({
        isDesktopRuntime,
        anyOnlineHostServerId,
        daemonStartIsRunning,
        daemonStartError,
      }),
    [anyOnlineHostServerId, daemonStartError, daemonStartIsRunning, isDesktopRuntime],
  );
  const shouldRunGiveUpTimer = shouldRunStartupGiveUpTimer({
    startupBlocker,
    anyOnlineHostServerId,
    hasGivenUpWaitingForHost,
  });

  useEffect(() => {
    if (!shouldRunGiveUpTimer) {
      return;
    }
    const handle = setTimeout(() => {
      setHasGivenUpWaitingForHost(true);
    }, STARTUP_GIVE_UP_TIMEOUT_MS);
    return () => {
      clearTimeout(handle);
    };
  }, [shouldRunGiveUpTimer]);

  const retry = useCallback(() => {
    const daemonStartService = getDaemonStartService({
      store: getHostRuntimeStore(),
    });
    void daemonStartService.startIfEnabled({
      shouldStart: shouldStartBuiltInDaemon,
    });
  }, []);

  const splashError =
    startupBlocker.kind === "managed-daemon-error" ? startupBlocker.message : null;
  const storeReady = resolveStartupNavigationReady({ startupBlocker });

  const state = useMemo<HostRuntimeBootstrapState>(
    () => ({
      splashError,
      retry,
      hasGivenUpWaitingForHost,
      storeReady,
      startupBlocker,
    }),
    [splashError, retry, hasGivenUpWaitingForHost, storeReady, startupBlocker],
  );

  return (
    <HostRuntimeBootstrapContext.Provider value={state}>
      {children}
    </HostRuntimeBootstrapContext.Provider>
  );
}

export function useStoreReady(): boolean {
  return useContext(HostRuntimeBootstrapContext).storeReady;
}

export function useHostRuntimeBootstrapState(): HostRuntimeBootstrapState {
  return useContext(HostRuntimeBootstrapContext);
}
