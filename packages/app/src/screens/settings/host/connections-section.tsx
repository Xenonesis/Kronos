import type { TFunction } from "i18next";
import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Alert, Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { AdaptiveModalSheet, type SheetHeader } from "@/components/adaptive-modal-sheet";
import { SettingsSection } from "@/components/settings/headings/settings-section";
import { Button } from "@/components/ui/button";
import { useHostMutations, useHostRuntimeSnapshot } from "@/runtime/host-runtime";
import { settingsStyles } from "@/styles/settings";
import type { Theme } from "@/styles/theme";
import type { HostConnection, HostProfile } from "@/types/host-connection";
import { formatLatency } from "@/utils/latency";

interface HostConnectionsSectionProps {
  host: HostProfile;
}

interface PendingRemoveConnection {
  connectionId: string;
  title: string;
}

interface ConnectionRowProps {
  connection: HostConnection;
  showBorder: boolean;
  latencyMs: number | null | undefined;
  latencyLoading: boolean;
  latencyError: boolean;
  onRemove: (connection: HostConnection) => void;
}

const ThemedButton = withUnistyles(Button);
const destructiveButtonProps = (theme: Theme) => ({
  textStyle: { color: theme.colors.destructive },
});

function formatHostConnectionLabel(connection: HostConnection, t: TFunction): string {
  if (connection.type === "routed") return `Via ${connection.originServerId}`;
  if (connection.type === "relay") {
    return `${t("settings.host.badges.relay")} (${connection.relayEndpoint})`;
  }
  if (connection.type === "directSocket" || connection.type === "directPipe") {
    return `${t("settings.host.badges.local")} (${connection.path})`;
  }
  if (connection.type === "remoteSsh") {
    return `${t("settings.host.badges.remoteSsh")} (${connection.host})`;
  }
  return `TCP (${connection.endpoint})`;
}

export function HostConnectionsSection({ host }: HostConnectionsSectionProps) {
  const { t } = useTranslation();
  const { removeConnection } = useHostMutations();
  const snapshot = useHostRuntimeSnapshot(host.serverId);
  const probeByConnectionId = snapshot?.probeByConnectionId ?? new Map();
  const [pendingRemoveConnection, setPendingRemoveConnection] =
    useState<PendingRemoveConnection | null>(null);
  const [isRemovingConnection, setIsRemovingConnection] = useState(false);
  const removeConnectionHeader = useMemo<SheetHeader>(
    () => ({ title: t("settings.host.connections.removeTitle") }),
    [t],
  );

  const handleRequestRemove = useCallback(
    (connection: HostConnection) => {
      setPendingRemoveConnection({
        connectionId: connection.id,
        title: formatHostConnectionLabel(connection, t),
      });
    },
    [t],
  );

  const handleCloseConfirm = useCallback(() => {
    if (isRemovingConnection) return;
    setPendingRemoveConnection(null);
  }, [isRemovingConnection]);

  const handleCancelConfirm = useCallback(() => {
    setPendingRemoveConnection(null);
  }, []);

  const handleConfirmRemove = useCallback(() => {
    if (!pendingRemoveConnection) return;
    const { connectionId } = pendingRemoveConnection;
    setIsRemovingConnection(true);
    void removeConnection(host.serverId, connectionId)
      .then(() => setPendingRemoveConnection(null))
      .catch((error) => {
        console.error("[HostPage] Failed to remove connection", error);
        Alert.alert(
          t("settings.host.connections.removeErrorTitle"),
          t("settings.host.connections.removeErrorMessage"),
        );
      })
      .finally(() => setIsRemovingConnection(false));
  }, [pendingRemoveConnection, removeConnection, host.serverId, t]);

  return (
    <SettingsSection title={t("settings.host.connections.title")}>
      <View style={settingsStyles.card} testID="host-page-connections-card">
        {host.connections.map((conn, index) => {
          const probe = probeByConnectionId.get(conn.id);
          return (
            <ConnectionRow
              key={conn.id}
              connection={conn}
              showBorder={index > 0}
              latencyMs={probe?.status === "available" ? probe.latencyMs : undefined}
              latencyLoading={!probe || probe.status === "pending"}
              latencyError={probe?.status === "unavailable"}
              onRemove={handleRequestRemove}
            />
          );
        })}
      </View>

      {pendingRemoveConnection ? (
        <AdaptiveModalSheet
          header={removeConnectionHeader}
          visible
          onClose={handleCloseConfirm}
          testID="remove-connection-confirm-modal"
        >
          <Text style={styles.confirmText}>
            {t("settings.host.connections.removeMessage", {
              name: pendingRemoveConnection.title,
            })}
          </Text>
          <View style={styles.confirmActions}>
            <Button
              variant="secondary"
              size="sm"
              style={FLEX_1_STYLE}
              onPress={handleCancelConfirm}
              disabled={isRemovingConnection}
            >
              {t("common.actions.cancel")}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              style={FLEX_1_STYLE}
              onPress={handleConfirmRemove}
              disabled={isRemovingConnection}
              testID="remove-connection-confirm"
            >
              {t("settings.host.connections.removeAction")}
            </Button>
          </View>
        </AdaptiveModalSheet>
      ) : null}
    </SettingsSection>
  );
}

function ConnectionRow({
  connection,
  showBorder,
  latencyMs,
  latencyLoading,
  latencyError,
  onRemove,
}: ConnectionRowProps) {
  const { t } = useTranslation();
  const title = formatHostConnectionLabel(connection, t);

  const latencyText = (() => {
    if (latencyLoading) return "...";
    if (latencyError) return t("settings.host.connections.timeout");
    if (latencyMs != null) return formatLatency(latencyMs);
    return "—";
  })();

  const handlePressRemove = useCallback(() => {
    onRemove(connection);
  }, [onRemove, connection]);

  const rowStyle = useMemo(
    () => [settingsStyles.row, showBorder && settingsStyles.rowBorder],
    [showBorder],
  );

  return (
    <View style={rowStyle}>
      <View style={settingsStyles.rowContent}>
        <Text style={settingsStyles.rowTitle} numberOfLines={1}>
          {title}
        </Text>
      </View>
      <Text style={styles.connectionLatency(latencyError)}>{latencyText}</Text>
      <ThemedButton
        variant="ghost"
        size="sm"
        uniProps={destructiveButtonProps}
        onPress={handlePressRemove}
      >
        {t("settings.host.connections.removeAction")}
      </ThemedButton>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  connectionLatency: (latencyError: boolean) => ({
    fontSize: theme.fontSize.base,
    marginRight: theme.spacing[2],
    color: latencyError ? theme.colors.palette.red[300] : theme.colors.foregroundMuted,
  }),
  confirmText: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
  confirmActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    marginTop: theme.spacing[4],
  },
}));

const FLEX_1_STYLE = { flex: 1 };
