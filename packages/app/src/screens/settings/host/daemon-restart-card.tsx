import { RotateCw } from "lucide-react-native";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Alert, Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Alert as InlineAlert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  getHostRuntimeStore,
  isHostRuntimeConnected,
  useHostRuntimeClient,
  useHostRuntimeIsConnected,
} from "@/runtime/host-runtime";
import { useSessionStore } from "@/stores/session-store";
import { settingsStyles } from "@/styles/settings";
import type { Theme } from "@/styles/theme";
import type { HostProfile } from "@/types/host-connection";
import { confirmDialog } from "@/utils/confirm-dialog";
import { restartDaemonFromSettings } from "../daemon-lifecycle";

interface RestartDaemonCardProps {
  host: HostProfile;
}

const ThemedRestart = withUnistyles(RotateCw);
const lifecycleIconProps = (theme: Theme) => ({
  size: theme.iconSize.sm,
  color: theme.colors.foreground,
});
const restartIcon = <ThemedRestart uniProps={lifecycleIconProps} />;

export function RestartDaemonCard({ host }: RestartDaemonCardProps) {
  const { t } = useTranslation();
  const daemonClient = useHostRuntimeClient(host.serverId);
  const isConnected = useHostRuntimeIsConnected(host.serverId);
  const runtime = getHostRuntimeStore();
  const [isRestarting, setIsRestarting] = useState(false);
  // Rendered in the card, not an Alert.alert: react-native-web's Alert is a no-op, and a failed
  // close-terminals restart must name the processes left running until the user retries.
  const [restartError, setRestartError] = useState<string | null>(null);
  const isMountedRef = useRef(true);
  // COMPAT(terminalsSurviveRestart): added in v0.12.0, remove gate after 2027-04-21.
  const terminalsSurviveRestart = useSessionStore(
    (state) =>
      state.sessions[host.serverId]?.serverInfo?.features?.terminalsSurviveRestart === true,
  );

  useEffect(() => {
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const isHostConnected = useCallback(
    () => isHostRuntimeConnected(runtime.getSnapshot(host.serverId)),
    [host.serverId, runtime],
  );

  const waitForDaemonRestart = useCallback(async (request: Promise<void>) => {
    try {
      await request;
    } catch (error) {
      console.error("[HostPage] Daemon restart failed", error);
      if (isMountedRef.current) {
        setRestartError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (isMountedRef.current) setIsRestarting(false);
    }
  }, []);

  const startRestart = useCallback(
    (killTerminals: boolean) => {
      if (!daemonClient) {
        Alert.alert(
          t("settings.host.daemon.restart.unavailableTitle"),
          t("settings.host.daemon.restart.unavailableMessage"),
        );
        return;
      }
      if (!isHostConnected()) {
        Alert.alert(
          t("settings.host.daemon.restart.offlineTitle"),
          t("settings.host.daemon.restart.offlineMessage"),
        );
        return;
      }

      void confirmDialog({
        title: t("settings.host.daemon.restart.confirmTitle", { name: host.label }),
        message: killTerminals
          ? t("settings.host.daemon.restart.closeTerminalsConfirmMessage")
          : t("settings.host.daemon.restart.confirmMessage"),
        confirmLabel: killTerminals
          ? t("settings.host.daemon.restart.closeTerminals")
          : t("settings.host.daemon.restart.confirm"),
        cancelLabel: t("common.actions.cancel"),
        destructive: true,
      })
        .then((confirmed) => {
          if (!confirmed) return;
          setRestartError(null);
          setIsRestarting(true);
          const restartRequest = restartDaemonFromSettings(
            host.serverId,
            `settings_daemon_restart_${host.serverId}`,
            {
              restartServer: (reason, options) =>
                daemonClient.restartServer(reason, undefined, options),
              getStatus: async () => ({
                ...(await daemonClient.getDaemonStatus()),
                serverId: daemonClient.getLastServerInfoMessage()?.serverId ?? "",
                version: daemonClient.getLastServerInfoMessage()?.version ?? null,
              }),
            },
            { killTerminals },
          );
          void waitForDaemonRestart(restartRequest);
          return;
        })
        .catch((error) => {
          console.error(`[HostPage] Failed to open restart confirmation for ${host.label}`, error);
          if (isMountedRef.current) {
            setRestartError(t("settings.host.daemon.restart.dialogFailedMessage"));
          }
        });
    },
    [daemonClient, host.label, host.serverId, isHostConnected, t, waitForDaemonRestart],
  );

  const handleRestart = useCallback(() => startRestart(false), [startRestart]);
  const handleRestartAndCloseTerminals = useCallback(() => startRestart(true), [startRestart]);

  return (
    <View style={settingsStyles.card} testID="host-page-restart-card">
      <View style={settingsStyles.row}>
        <View style={settingsStyles.rowContent}>
          <Text style={settingsStyles.rowTitle}>{t("settings.host.daemon.restart.title")}</Text>
          <Text style={settingsStyles.rowHint}>{t("settings.host.daemon.restart.hint")}</Text>
        </View>
        <Button
          variant="outline"
          size="sm"
          leftIcon={restartIcon}
          onPress={handleRestart}
          disabled={isRestarting || !daemonClient || !isConnected}
          testID="host-page-restart-button"
        >
          {isRestarting
            ? t("settings.host.daemon.restart.restarting")
            : t("settings.host.daemon.restart.confirm")}
        </Button>
      </View>
      {terminalsSurviveRestart ? (
        <View style={settingsStyles.row}>
          <View style={settingsStyles.rowContent}>
            <Text style={settingsStyles.rowHint}>
              {t("settings.host.daemon.restart.closeTerminalsHint")}
            </Text>
          </View>
          <Button
            variant="outline"
            size="sm"
            onPress={handleRestartAndCloseTerminals}
            disabled={isRestarting || !daemonClient || !isConnected}
            testID="host-page-restart-close-terminals-button"
          >
            {t("settings.host.daemon.restart.closeTerminals")}
          </Button>
        </View>
      ) : null}
      {restartError ? (
        <View style={styles.cardAlert}>
          <InlineAlert
            size="sm"
            variant="error"
            title={t("settings.host.daemon.restart.requestFailedTitle")}
            description={restartError}
            testID="host-page-restart-error"
          />
        </View>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  cardAlert: {
    marginHorizontal: theme.spacing[4],
    marginBottom: theme.spacing[4],
  },
}));
