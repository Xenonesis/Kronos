import { memo, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import type { PluginListItem } from "@getpaseo/protocol/messages";
import { formatPluginInstallation } from "@getpaseo/protocol/plugin-source-reference";
import { PluginSettingsButton } from "@/plugins/settings";
import { SettingsRow } from "@/components/settings";
import { StatusBadge } from "@/components/ui/status-badge";
import { Switch } from "@/components/ui/switch";
import { pluginDisplayName } from "@/plugins/plugin-list";
import type { PluginRowAction } from "./plugin-types";
import { PluginActionsMenu } from "./plugin-actions-menu";

interface PluginRowProps {
  plugin: PluginListItem;
  serverId: string;
  clientError?: string;
  pending: boolean;
  pendingAction?: PluginRowAction;
  onAction: (action: PluginRowAction, plugin: PluginListItem) => void;
  onOpenLogs: (plugin: PluginListItem) => void;
  supportsLogs: boolean;
}

export const PluginRow = memo(function PluginRow({
  plugin,
  serverId,
  clientError,
  pending,
  pendingAction,
  onAction,
  onOpenLogs,
  supportsLogs,
}: PluginRowProps) {
  const { t } = useTranslation();
  const toggle = useCallback(
    (enabled: boolean) => onAction(enabled ? "enable" : "disable", plugin),
    [onAction, plugin],
  );
  const status = clientError ? "failed" : plugin.status;
  let badgeVariant: "success" | "error" | "muted" = "muted";
  if (status === "running") badgeVariant = "success";
  else if (status === "failed") badgeVariant = "error";
  const statusLabel = t(`settings.plugins.status.${status}`);
  const statusBadge = useMemo(
    () => <StatusBadge label={statusLabel} variant={badgeVariant} />,
    [badgeVariant, statusLabel],
  );
  const name = pluginDisplayName(plugin);
  const shownId = name === plugin.id ? null : plugin.id;
  const hint = useMemo(
    () =>
      shownId || plugin.description || plugin.installation ? (
        <View>
          {shownId ? <Text style={styles.pluginSource}>{shownId}</Text> : null}
          {plugin.description ? (
            <Text style={styles.pluginDescription}>{plugin.description}</Text>
          ) : null}
          {plugin.installation ? (
            <Text style={styles.pluginSource}>{formatPluginInstallation(plugin.installation)}</Text>
          ) : null}
        </View>
      ) : undefined,
    [shownId, plugin.description, plugin.installation],
  );
  const toggleLabel = `${name}: ${t(
    plugin.enabled ? "settings.plugins.actions.disable" : "settings.plugins.actions.enable",
  )}`;

  return (
    <SettingsRow
      label={name}
      labelAccessory={statusBadge}
      hint={hint}
      error={clientError ?? plugin.error}
      testID={`plugin-row-${plugin.id}`}
    >
      <View style={styles.pluginControls} accessibilityLabel={`${name} ${statusLabel}`}>
        <PluginSettingsButton serverId={serverId} pluginId={plugin.id} />
        <Switch
          value={plugin.enabled}
          onValueChange={toggle}
          disabled={pending}
          accessibilityLabel={toggleLabel}
        />
        <PluginActionsMenu
          plugin={plugin}
          pending={pending}
          pendingAction={pendingAction}
          onAction={onAction}
          onOpenLogs={onOpenLogs}
          supportsLogs={supportsLogs}
        />
      </View>
    </SettingsRow>
  );
});

const styles = StyleSheet.create((theme) => ({
  pluginControls: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  pluginDescription: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    lineHeight: 18,
  },
  pluginSource: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    fontFamily: theme.fontFamily.mono,
  },
}));
