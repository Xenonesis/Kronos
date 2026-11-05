import { Trash2 } from "lucide-react-native";
import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Alert, Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { AdaptiveModalSheet, type SheetHeader } from "@/components/adaptive-modal-sheet";
import { SettingsSection } from "@/components/settings/headings/settings-section";
import { Button } from "@/components/ui/button";
import { startDesktopDaemon, stopDesktopDaemon } from "@/desktop/daemon/desktop-daemon";
import { useDaemonStatus } from "@/desktop/hooks/use-daemon-status";
import { useDesktopSettings } from "@/desktop/settings/desktop-settings";
import { useHostMutations } from "@/runtime/host-runtime";
import { settingsStyles } from "@/styles/settings";
import type { Theme } from "@/styles/theme";
import type { HostProfile } from "@/types/host-connection";
import { RestartDaemonCard } from "./daemon-restart-card";

interface RemoveHostSectionProps {
  host: HostProfile;
  isLocalDaemon: boolean;
  onRemoved?: () => void;
}

const ThemedTrash2 = withUnistyles(Trash2);
const ThemedButton = withUnistyles(Button);
const destructiveIconProps = (theme: Theme) => ({
  size: theme.iconSize.sm,
  color: theme.colors.destructive,
});
const destructiveButtonProps = (theme: Theme) => ({
  textStyle: { color: theme.colors.destructive },
});
const removeIcon = <ThemedTrash2 uniProps={destructiveIconProps} />;

export function RemoveHostSection({ host, isLocalDaemon, onRemoved }: RemoveHostSectionProps) {
  const { t } = useTranslation();
  const { removeHost } = useHostMutations();
  const { updateSettings } = useDesktopSettings();
  const { data: daemonStatusData, setStatus } = useDaemonStatus();
  const [isConfirming, setIsConfirming] = useState(false);
  const [isRemoving, setIsRemoving] = useState(false);
  const daemonStatus = daemonStatusData?.status ?? null;
  const stopsOwnedDaemon = isLocalDaemon && daemonStatus?.ownedByDesktop === true;
  const removeHostHeader = useMemo<SheetHeader>(
    () => ({
      title: stopsOwnedDaemon
        ? t("settings.host.daemon.remove.localConfirmTitle")
        : t("settings.host.daemon.remove.title"),
    }),
    [stopsOwnedDaemon, t],
  );

  const handleOpenConfirm = useCallback(() => setIsConfirming(true), []);
  const handleCloseConfirm = useCallback(() => {
    if (isRemoving) return;
    setIsConfirming(false);
  }, [isRemoving]);
  const handleCancel = useCallback(() => setIsConfirming(false), []);
  const rollbackLocalhostRemoval = useCallback(
    async (shouldRestartDaemon: boolean) => {
      await updateSettings({ daemon: { manageBuiltInDaemon: true } });
      if (!shouldRestartDaemon) {
        return;
      }
      setStatus(await startDesktopDaemon());
    },
    [setStatus, updateSettings],
  );
  const handleConfirmRemove = useCallback(() => {
    setIsRemoving(true);
    const remove = async () => {
      let didDisableDaemonManagement = false;
      let didStopDaemon = false;
      if (isLocalDaemon) {
        try {
          await updateSettings({ daemon: { manageBuiltInDaemon: false } });
          didDisableDaemonManagement = true;
          if (daemonStatus?.status === "running" && daemonStatus.ownedByDesktop) {
            setStatus(await stopDesktopDaemon("host_remove"));
            didStopDaemon = true;
          }
          await removeHost(host.serverId);
        } catch (error) {
          if (didDisableDaemonManagement) {
            try {
              await rollbackLocalhostRemoval(didStopDaemon);
            } catch (rollbackError) {
              console.error("[HostPage] Failed to roll back localhost removal", rollbackError);
            }
          }
          throw error;
        }
        return;
      }
      await removeHost(host.serverId);
    };
    void remove()
      .then(() => {
        setIsConfirming(false);
        onRemoved?.();
        return;
      })
      .catch((error) => {
        console.error("[HostPage] Failed to remove host", error);
        Alert.alert(
          t("settings.host.daemon.remove.errorTitle"),
          isLocalDaemon
            ? t("settings.host.daemon.remove.localErrorMessage")
            : t("settings.host.daemon.remove.errorMessage"),
        );
      })
      .finally(() => setIsRemoving(false));
  }, [
    daemonStatus,
    host.serverId,
    isLocalDaemon,
    onRemoved,
    removeHost,
    rollbackLocalhostRemoval,
    setStatus,
    t,
    updateSettings,
  ]);

  return (
    <SettingsSection
      title={t("settings.host.daemon.dangerZone")}
      testID="host-page-remove-host-card"
    >
      <RestartDaemonCard host={host} />

      <View style={settingsStyles.card}>
        <View style={settingsStyles.row}>
          <View style={settingsStyles.rowContent}>
            <Text style={settingsStyles.rowTitle}>
              {isLocalDaemon
                ? t("settings.host.daemon.remove.localTitle")
                : t("settings.host.daemon.remove.title")}
            </Text>
            <Text style={settingsStyles.rowHint}>
              {stopsOwnedDaemon
                ? t("settings.host.daemon.remove.localHint")
                : t("settings.host.daemon.remove.hint")}
            </Text>
          </View>
          <ThemedButton
            variant="outline"
            size="sm"
            leftIcon={removeIcon}
            uniProps={destructiveButtonProps}
            onPress={handleOpenConfirm}
            testID="host-page-remove-host-button"
          >
            {t("settings.host.connections.removeAction")}
          </ThemedButton>
        </View>
      </View>

      {isConfirming ? (
        <AdaptiveModalSheet
          header={removeHostHeader}
          visible
          onClose={handleCloseConfirm}
          testID="remove-host-confirm-modal"
        >
          <Text style={styles.confirmText}>
            {stopsOwnedDaemon
              ? t("settings.host.daemon.remove.localConfirmMessage")
              : t("settings.host.daemon.remove.confirmMessage", { name: host.label })}
          </Text>
          <View style={styles.confirmActions}>
            <Button
              variant="secondary"
              size="sm"
              style={FLEX_1_STYLE}
              onPress={handleCancel}
              disabled={isRemoving}
            >
              {t("common.actions.cancel")}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              style={FLEX_1_STYLE}
              onPress={handleConfirmRemove}
              disabled={isRemoving}
              testID="remove-host-confirm"
            >
              {t("settings.host.connections.removeAction")}
            </Button>
          </View>
        </AdaptiveModalSheet>
      ) : null}
    </SettingsSection>
  );
}

const styles = StyleSheet.create((theme) => ({
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
