import { ChevronRight, Globe, Monitor } from "lucide-react-native";
import type { TFunction } from "i18next";
import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, Text, View } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { AgentProfilesSection } from "@/agent-profiles";
import { AgentSkillsSection } from "@/agent-skills";
import { Alert as InlineAlert } from "@/components/ui/alert";
import { StatusBadge, type StatusBadgeVariant } from "@/components/ui/status-badge";
import { LocalDaemonSection } from "@/desktop/components/desktop-updates-section";
import { PairDeviceModal } from "@/desktop/components/pair-device-modal";
import { useIsLocalDaemon } from "@/hooks/use-is-local-daemon";
import {
  useHostRuntimeIsConnected,
  useHostRuntimeSnapshot,
  useHosts,
} from "@/runtime/host-runtime";
import { ProvidersSection } from "@/screens/settings/providers-section";
import { HostUsageSection } from "@/usage";
import { HostAppearanceSection } from "@/screens/settings/host-appearance-section";
import { SettingsSection } from "@/components/settings/headings/settings-section";
import { useSessionStore } from "@/stores/session-store";
import { settingsStyles } from "@/styles/settings";
import type { HostConnection, HostProfile } from "@/types/host-connection";
import { formatConnectionStatus, getConnectionStatusTone } from "@/utils/daemons";
import type { Theme } from "@/styles/theme";
import { BrowserToolsOptInCard } from "./browser-tools-card";
import { RestartRecoveryCard } from "./restart-recovery-card";
import { IdleRuntimeCleanupCard } from "./idle-runtime-cleanup-card";
import { HostConnectionsSection } from "./host/connections-section";
import { UpdateDaemonCard } from "./host/daemon-update-card";
import { RemoveHostSection } from "./host/remove-host-section";
import { InjectPaseoToolsCard, AppendSystemPromptCard } from "./host/agent-orchestration-cards";
import { AutoArchiveMergedWorkspacesCard } from "./host/workspace-archival-card";
import {
  TerminalProfilesSection,
  EnableTerminalAgentHooksCard,
} from "./host/terminal-profiles-section";

function formatActiveConnectionBadge(
  activeConnection: { type: HostConnection["type"]; display: string } | null,
  theme: Theme,
  t: TFunction,
): { icon: React.ReactNode; text: string } | null {
  if (!activeConnection) return null;
  if (activeConnection.type === "relay") {
    return {
      icon: <Globe size={theme.iconSize.sm} color={theme.colors.foregroundMuted} />,
      text: t("settings.host.badges.relay"),
    };
  }
  if (activeConnection.type === "directSocket" || activeConnection.type === "directPipe") {
    return {
      icon: <Monitor size={theme.iconSize.sm} color={theme.colors.foregroundMuted} />,
      text: t("settings.host.badges.local"),
    };
  }
  if (activeConnection.type === "remoteSsh") {
    return {
      icon: <Globe size={theme.iconSize.sm} color={theme.colors.foregroundMuted} />,
      text: t("settings.host.badges.remoteSsh"),
    };
  }
  return {
    icon: <Monitor size={theme.iconSize.sm} color={theme.colors.foregroundMuted} />,
    text: activeConnection.display,
  };
}

function formatDaemonVersionBadge(version: string | null): string | null {
  const trimmed = version?.trim();
  if (!trimmed) return null;
  return trimmed.startsWith("v") ? trimmed : `v${trimmed}`;
}

function useHostProfile(serverId: string): HostProfile | null {
  const daemons = useHosts();
  return daemons.find((entry) => entry.serverId === serverId) ?? null;
}

function HostNotFound() {
  const { t } = useTranslation();
  return (
    <View>
      <View style={[settingsStyles.card, styles.emptyCard]}>
        <Text style={styles.emptyText}>{t("settings.host.notFound")}</Text>
      </View>
    </View>
  );
}

function HostStatusBadges({ serverId }: { serverId: string }) {
  const { t } = useTranslation();
  const { theme } = useUnistyles();
  const snapshot = useHostRuntimeSnapshot(serverId);
  const daemonVersion = useSessionStore(
    (state) => state.sessions[serverId]?.serverInfo?.version ?? null,
  );

  const connectionStatus = snapshot?.connectionStatus ?? "connecting";
  const activeConnection = snapshot?.activeConnection ?? null;
  const statusLabel = formatConnectionStatus(connectionStatus);
  const statusTone = getConnectionStatusTone(connectionStatus);
  let statusVariant: StatusBadgeVariant = "muted";
  let statusDotColor = theme.colors.foregroundMuted;
  if (statusTone === "success") {
    statusVariant = "success";
    statusDotColor = theme.colors.statusDotSuccess;
  } else if (statusTone === "warning") {
    statusVariant = "warning";
    statusDotColor = theme.colors.statusDotWarning;
  } else if (statusTone === "error") {
    statusVariant = "error";
    statusDotColor = theme.colors.statusDotDanger;
  }
  const connectionBadge = formatActiveConnectionBadge(activeConnection, theme, t);
  const versionBadgeText = formatDaemonVersionBadge(daemonVersion);
  const statusDotStyle = useMemo(
    () => [styles.statusDot, { backgroundColor: statusDotColor }],
    [statusDotColor],
  );
  const statusLeading = useMemo(() => <View style={statusDotStyle} />, [statusDotStyle]);

  return (
    <View style={styles.identityBadges} testID="host-page-identity">
      <StatusBadge label={statusLabel} variant={statusVariant} leading={statusLeading} />
      {connectionBadge ? (
        <View style={styles.badgePill}>
          {connectionBadge.icon}
          <Text style={styles.badgeText} numberOfLines={1}>
            {connectionBadge.text}
          </Text>
        </View>
      ) : null}
      {versionBadgeText ? (
        <View style={styles.badgePill}>
          <Text style={styles.badgeText} numberOfLines={1}>
            {versionBadgeText}
          </Text>
        </View>
      ) : null}
    </View>
  );
}

function HostConnectionError({ serverId }: { serverId: string }) {
  const { t } = useTranslation();
  const snapshot = useHostRuntimeSnapshot(serverId);
  const lastError = snapshot?.lastError ?? null;
  const connectionError =
    typeof lastError === "string" && lastError.trim().length > 0 ? lastError.trim() : null;
  if (!connectionError) return null;
  return (
    <View style={styles.connectionError}>
      <InlineAlert
        size="sm"
        variant="error"
        title={connectionError}
        description={snapshot?.authFailureReason ? t("settings.host.password.guidance") : undefined}
        testID="host-connection-error"
      />
    </View>
  );
}

export function HostConnectionsPage({ serverId }: { serverId: string }) {
  const host = useHostProfile(serverId);

  if (!host) {
    return <HostNotFound />;
  }

  return (
    <View>
      <HostConnectionError serverId={serverId} />
      <HostConnectionsSection host={host} />
    </View>
  );
}

export function HostPairDevicePage({ serverId }: { serverId: string }) {
  const { t } = useTranslation();
  const host = useHostProfile(serverId);

  if (!host) {
    return <HostNotFound />;
  }

  return (
    <SettingsSection title={t("settings.host.pairDevices.title")}>
      <PairDeviceRow serverId={serverId} />
    </SettingsSection>
  );
}

export function HostAgentsPage({ serverId }: { serverId: string }) {
  const { t } = useTranslation();
  const host = useHostProfile(serverId);
  const isConnected = useHostRuntimeIsConnected(serverId);

  if (!host) {
    return <HostNotFound />;
  }

  return (
    <View>
      {isConnected ? (
        <SettingsSection title={t("settings.hostSections.agents")}>
          <InjectPaseoToolsCard serverId={serverId} />
          <BrowserToolsOptInCard serverId={serverId} />
          <AppendSystemPromptCard serverId={serverId} />
          <RestartRecoveryCard serverId={serverId} />
          <IdleRuntimeCleanupCard serverId={serverId} />
        </SettingsSection>
      ) : (
        <View style={[settingsStyles.card, styles.emptyCard]}>
          <Text style={styles.emptyText}>{t("settings.host.agents.unavailable")}</Text>
        </View>
      )}
      <AgentSkillsSection serverId={serverId} />
      <AgentProfilesSection serverId={serverId} />
    </View>
  );
}

export function HostWorkspacesPage({ serverId }: { serverId: string }) {
  const { t } = useTranslation();
  const host = useHostProfile(serverId);
  const isConnected = useHostRuntimeIsConnected(serverId);

  if (!host) {
    return <HostNotFound />;
  }

  return (
    <View>
      {isConnected ? (
        <SettingsSection title={t("settings.hostSections.workspaces")}>
          <AutoArchiveMergedWorkspacesCard serverId={serverId} />
        </SettingsSection>
      ) : (
        <View style={[settingsStyles.card, styles.emptyCard]}>
          <Text style={styles.emptyText}>{t("settings.host.workspaces.unavailable")}</Text>
        </View>
      )}
    </View>
  );
}

export function HostProvidersPage({ serverId }: { serverId: string }) {
  const host = useHostProfile(serverId);

  if (!host) {
    return <HostNotFound />;
  }

  return (
    <View>
      <ProvidersSection serverId={serverId} />
    </View>
  );
}

export function HostUsagePage({ serverId }: { serverId: string }) {
  const host = useHostProfile(serverId);

  if (!host) {
    return <HostNotFound />;
  }

  return (
    <View>
      <HostUsageSection serverId={serverId} />
    </View>
  );
}

export function HostSettingsPage({
  serverId,
  onHostRemoved,
}: {
  serverId: string;
  onHostRemoved?: () => void;
}) {
  const host = useHostProfile(serverId);
  const isLocalDaemon = useIsLocalDaemon(serverId);

  if (!host) {
    return <HostNotFound />;
  }

  return (
    <View>
      <HostStatusBadges serverId={serverId} />
      <HostConnectionError serverId={serverId} />

      <HostAppearanceSection host={host} />

      {isLocalDaemon ? <LocalDaemonSection /> : null}

      {!isLocalDaemon ? <UpdateDaemonCard key={host.serverId} host={host} /> : null}

      <RemoveHostSection host={host} isLocalDaemon={isLocalDaemon} onRemoved={onHostRemoved} />
    </View>
  );
}

function PairDeviceRow({ serverId }: { serverId: string }) {
  const { t } = useTranslation();
  const { theme } = useUnistyles();
  const [isModalOpen, setIsModalOpen] = useState(false);

  const handleOpen = useCallback(() => setIsModalOpen(true), []);
  const handleClose = useCallback(() => setIsModalOpen(false), []);

  return (
    <View style={settingsStyles.card}>
      <Pressable
        style={settingsStyles.row}
        onPress={handleOpen}
        accessibilityRole="button"
        testID="host-page-pair-device-row"
      >
        <View style={settingsStyles.rowContent}>
          <Text style={settingsStyles.rowTitle}>{t("settings.host.pairDevices.rowTitle")}</Text>
          <Text style={settingsStyles.rowHint}>{t("settings.host.pairDevices.rowHint")}</Text>
        </View>
        <ChevronRight size={theme.iconSize.sm} color={theme.colors.foregroundMuted} />
      </Pressable>

      <PairDeviceModal
        serverId={serverId}
        visible={isModalOpen}
        onClose={handleClose}
        testID="host-page-pair-device-card"
      />
    </View>
  );
}

export function HostTerminalsPage({ serverId }: { serverId: string }) {
  const host = useHostProfile(serverId);

  if (!host) {
    return <HostNotFound />;
  }

  return (
    <View>
      <SettingsSection title="Terminal agents">
        <EnableTerminalAgentHooksCard serverId={serverId} />
      </SettingsSection>
      <TerminalProfilesSection serverId={serverId} />
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  identityBadges: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
    flexWrap: "wrap",
    marginBottom: theme.spacing[6],
  },
  statusDot: {
    width: 6,
    height: 6,
    borderRadius: theme.borderRadius.full,
  },
  badgePill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.full,
    borderWidth: 1,
    borderColor: theme.colors.border,
    backgroundColor: theme.colors.surface3,
    maxWidth: 200,
  },
  badgeText: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.normal,
    color: theme.colors.foregroundMuted,
    flexShrink: 1,
  },
  connectionError: {
    marginBottom: theme.spacing[6],
  },
  emptyCard: {
    padding: theme.spacing[4],
    alignItems: "center",
  },
  emptyText: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
}));
