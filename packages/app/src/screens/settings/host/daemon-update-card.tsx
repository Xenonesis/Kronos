import { ArrowUpToLine } from "lucide-react-native";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
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
import { isVersionMismatch } from "@/desktop/updates/desktop-updates";
import { resolveAppVersion } from "@/utils/app-version";
import { updateDaemonFromSettings } from "../daemon-lifecycle";

interface UpdateDaemonCardProps {
  host: HostProfile;
}

const ThemedUpdate = withUnistyles(ArrowUpToLine);
const lifecycleIconProps = (theme: Theme) => ({
  size: theme.iconSize.sm,
  color: theme.colors.foreground,
});
const updateIcon = <ThemedUpdate uniProps={lifecycleIconProps} />;

type DaemonUpdateState =
  | { status: "idle" }
  | { status: "complete"; workerVersion: string }
  | { status: "updating"; phase: string }
  | { status: "failed"; title: string; message: string };

export function UpdateDaemonCard({ host }: UpdateDaemonCardProps) {
  const { t } = useTranslation();
  const daemonClient = useHostRuntimeClient(host.serverId);
  const isConnected = useHostRuntimeIsConnected(host.serverId);
  const runtime = getHostRuntimeStore();
  const [updateState, setUpdateState] = useState<DaemonUpdateState>({ status: "idle" });
  const isMountedRef = useRef(true);
  const unsubscribeRef = useRef<(() => void) | null>(null);

  const daemonVersion = useSessionStore(
    (state) => state.sessions[host.serverId]?.serverInfo?.version ?? null,
  );
  const supportsSelfUpdate = useSessionStore(
    (state) => state.sessions[host.serverId]?.serverInfo?.features?.daemonSelfUpdate === true,
  );
  const desktopManaged = useSessionStore(
    (state) => state.sessions[host.serverId]?.serverInfo?.desktopManaged === true,
  );

  const appVersion = resolveAppVersion();
  const hasVersionMismatch = isVersionMismatch(appVersion, daemonVersion);

  useEffect(() => {
    return () => {
      isMountedRef.current = false;
      unsubscribeRef.current?.();
    };
  }, []);

  const isHostConnected = useCallback(
    () => isHostRuntimeConnected(runtime.getSnapshot(host.serverId)),
    [host.serverId, runtime],
  );
  const handleUpdate = useCallback(() => {
    if (!daemonClient) {
      setUpdateState({
        status: "failed",
        title: t("settings.host.daemon.update.unavailableTitle"),
        message: t("settings.host.daemon.update.unavailableMessage"),
      });
      return;
    }
    if (!isHostConnected()) {
      setUpdateState({
        status: "failed",
        title: t("settings.host.daemon.update.offlineTitle"),
        message: t("settings.host.daemon.update.offlineMessage"),
      });
      return;
    }

    void confirmDialog({
      title: t("settings.host.daemon.update.confirmTitle", { name: host.label }),
      message: t("settings.host.daemon.update.confirmMessage"),
      confirmLabel: t("settings.host.daemon.update.confirm"),
      cancelLabel: t("common.actions.cancel"),
      destructive: false,
    })
      .then((confirmed) => {
        if (!confirmed || !isMountedRef.current) return;
        setUpdateState({
          status: "updating",
          phase: t("settings.host.daemon.update.phaseStarting"),
        });
        const requestId = `settings_daemon_update_${host.serverId}`;

        const unsubscribe = daemonClient.on("daemon.update.progress", (message) => {
          if (message.payload.requestId !== requestId) return;
          if (!isMountedRef.current) return;
          const { phase } = message.payload;
          if (phase === "starting")
            setUpdateState({
              status: "updating",
              phase: t("settings.host.daemon.update.phaseStarting"),
            });
          else if (phase === "downloading")
            setUpdateState({
              status: "updating",
              phase: t("settings.host.daemon.update.phaseDownloading"),
            });
          else if (phase === "installing")
            setUpdateState({
              status: "updating",
              phase: t("settings.host.daemon.update.phaseInstalling"),
            });
          else if (phase === "complete")
            setUpdateState({
              status: "updating",
              phase: t("settings.host.daemon.update.phaseComplete"),
            });
        });
        unsubscribeRef.current = unsubscribe;

        void updateDaemonFromSettings(host.serverId, {
          updateDaemon: () => daemonClient.updateDaemon(requestId),
          getStatus: async () => ({
            ...(await daemonClient.getDaemonStatus()),
            serverId: daemonClient.getLastServerInfoMessage()?.serverId ?? "",
            version: daemonClient.getLastServerInfoMessage()?.version ?? null,
          }),
        })
          .then(({ workerVersion }) => {
            unsubscribeRef.current = null;
            unsubscribe();
            if (isMountedRef.current) setUpdateState({ status: "complete", workerVersion });
            return undefined;
          })
          .catch((error) => {
            unsubscribeRef.current = null;
            unsubscribe();
            console.error(`[HostPage] Failed to update daemon ${host.label}`, error);
            if (!isMountedRef.current) return;
            setUpdateState({
              status: "failed",
              title: t("settings.host.daemon.update.requestFailedTitle"),
              message: t("settings.host.daemon.update.requestFailedMessage", {
                error: error instanceof Error ? error.message : "Unknown error",
              }),
            });
          });
        return;
      })
      .catch((error) => {
        console.error(`[HostPage] Failed to open update confirmation for ${host.label}`, error);
        if (!isMountedRef.current) return;
        setUpdateState({
          status: "failed",
          title: t("settings.host.daemon.update.requestFailedTitle"),
          message: t("settings.host.daemon.update.dialogFailedMessage"),
        });
      });
  }, [daemonClient, host.label, host.serverId, isHostConnected, t]);

  const shouldShowUpdate = hasVersionMismatch && (supportsSelfUpdate || desktopManaged);
  if (!shouldShowUpdate && updateState.status !== "complete") {
    return null;
  }

  const isUpdating = updateState.status === "updating";
  const buttonLabel = isUpdating ? updateState.phase : t("settings.host.daemon.update.confirm");

  return (
    <View style={settingsStyles.card} testID="host-page-update-card">
      <View style={settingsStyles.row}>
        <View style={settingsStyles.rowContent}>
          <Text style={settingsStyles.rowTitle}>{t("settings.host.daemon.update.title")}</Text>
          <Text style={settingsStyles.rowHint}>
            {desktopManaged
              ? t("settings.host.daemon.update.desktopManagedHint")
              : t("settings.host.daemon.update.hint")}
          </Text>
        </View>
        <Button
          variant="outline"
          size="sm"
          leftIcon={updateIcon}
          onPress={handleUpdate}
          disabled={desktopManaged || isUpdating || !daemonClient || !isConnected}
          testID="host-page-update-button"
        >
          {buttonLabel}
        </Button>
      </View>
      {updateState.status === "complete" ? (
        <InlineAlert
          size="sm"
          variant="success"
          title={t("desktop.daemon.lifecycle.workerUpdated", {
            version: updateState.workerVersion,
          })}
          description={t("desktop.daemon.lifecycle.supervisorRefresh")}
        />
      ) : null}
      {updateState.status === "failed" ? (
        <View style={styles.cardAlert}>
          <InlineAlert
            size="sm"
            variant="error"
            title={updateState.title}
            description={updateState.message}
            testID="host-page-update-error"
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
