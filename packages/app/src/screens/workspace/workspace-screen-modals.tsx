import React, { useCallback, useMemo, type ReactElement, type ReactNode } from "react";
import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { useRouter, type Href } from "expo-router";
import { MenuHeader } from "@/components/headers/menu-header";
import { WorkspaceFocusProvider } from "@/workspace/focus";
import { getHostRuntimeStore, useHostRuntimeSnapshot, useHosts } from "@/runtime/host-runtime";
import { buildHostRootRoute, buildSettingsHostRoute } from "@/utils/host-routes";
import {
  resolveWorkspaceRouteState,
  type WorkspaceRouteState,
} from "@/screens/workspace/workspace-route-state";
import type { WorkspaceRecoveryModel } from "@/workspace-recovery/model";
import type { WorkspaceDescriptor } from "@/stores/session-store";
import type { WorkspaceTabTarget } from "@/workspace-tabs/model";

function getHostDisplayName(host: { label?: string | null } | null, fallback: string): string {
  const trimmed = host?.label?.trim();
  return trimmed ? trimmed : fallback;
}

export function useWorkspaceRouteActions(normalizedServerId: string): {
  handleRetryHost: () => void;
  handleManageHost: () => void;
  handleDismissMissingWorkspace: () => void;
} {
  const router = useRouter();
  const handleRetryHost = useCallback(() => {
    if (!normalizedServerId) {
      return;
    }
    void getHostRuntimeStore().runProbeCycleNow(normalizedServerId);
  }, [normalizedServerId]);

  const handleManageHost = useCallback(() => {
    if (!normalizedServerId) {
      return;
    }
    router.push(buildSettingsHostRoute(normalizedServerId) as Href);
  }, [normalizedServerId, router]);

  const handleDismissMissingWorkspace = useCallback(() => {
    if (router.canGoBack()) {
      router.back();
      return;
    }
    if (normalizedServerId) {
      router.replace(buildHostRootRoute(normalizedServerId) as Href);
      return;
    }
    router.replace("/" as Href);
  }, [normalizedServerId, router]);

  return {
    handleRetryHost,
    handleManageHost,
    handleDismissMissingWorkspace,
  };
}

export function useResolvedWorkspaceRouteState(input: {
  serverId: string;
  workspace: WorkspaceDescriptor | null;
  hasHydratedWorkspaces: boolean;
  recovery: WorkspaceRecoveryModel;
}): WorkspaceRouteState {
  const hosts = useHosts();
  const host = useMemo(
    () => hosts.find((entry) => entry.serverId === input.serverId) ?? null,
    [hosts, input.serverId],
  );
  const hostSnapshot = useHostRuntimeSnapshot(input.serverId);
  const hostName = useMemo(() => getHostDisplayName(host, input.serverId), [host, input.serverId]);
  return useMemo(
    () =>
      resolveWorkspaceRouteState({
        hostName,
        connectionStatus: hostSnapshot?.connectionStatus ?? "connecting",
        lastError: hostSnapshot?.lastError ?? null,
        workspace: input.workspace,
        hasHydratedWorkspaces: input.hasHydratedWorkspaces,
        recovery: input.recovery,
      }),
    [
      hostName,
      hostSnapshot?.connectionStatus,
      hostSnapshot?.lastError,
      input.workspace,
      input.hasHydratedWorkspaces,
      input.recovery,
    ],
  );
}

export function shouldInspectWorkspaceRecovery(
  hasHydratedWorkspaces: boolean,
  workspace: WorkspaceDescriptor | null,
  pendingOpenTarget: WorkspaceTabTarget | null,
): boolean {
  return pendingOpenTarget?.kind === "agent" && hasHydratedWorkspaces && workspace === null;
}

export function WorkspaceScreenGateFrame({ children }: { children: ReactNode }) {
  return (
    <>
      <MenuHeader />
      <View style={styles.centerContent}>{children}</View>
    </>
  );
}

export function renderWorkspaceScreenGateShell(input: {
  gate: ReactNode;
  workspaceKey: string | null;
}): ReactElement | null {
  if (!input.gate) {
    return null;
  }

  return (
    <WorkspaceFocusProvider workspaceKey={input.workspaceKey}>
      <View style={styles.container}>
        <View style={styles.threePaneRow}>
          <View style={styles.centerColumn}>
            <WorkspaceScreenGateFrame>{input.gate}</WorkspaceScreenGateFrame>
          </View>
        </View>
      </View>
    </WorkspaceFocusProvider>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
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
}));
