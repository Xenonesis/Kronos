import { memo, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet } from "react-native-unistyles";
import { MoreHorizontal, Trash2 } from "lucide-react-native";
import type { PluginListItem } from "@getpaseo/protocol/messages";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { pluginDisplayName } from "@/plugins/plugin-list";
import type { PluginRowAction } from "./plugin-types";

interface PluginActionsMenuProps {
  plugin: PluginListItem;
  pending: boolean;
  pendingAction?: PluginRowAction;
  onAction: (action: PluginRowAction, plugin: PluginListItem) => void;
  onOpenLogs: (plugin: PluginListItem) => void;
  supportsLogs: boolean;
}

export const PluginActionsMenu = memo(function PluginActionsMenu({
  plugin,
  pending,
  pendingAction,
  onAction,
  onOpenLogs,
  supportsLogs,
}: PluginActionsMenuProps) {
  const { t } = useTranslation();
  const reload = useCallback(() => onAction("reload", plugin), [onAction, plugin]);
  const remove = useCallback(() => onAction("remove", plugin), [onAction, plugin]);
  const openLogs = useCallback(() => onOpenLogs(plugin), [onOpenLogs, plugin]);
  const removeIcon = useMemo(() => <Trash2 size={16} color={styles.dangerIcon.color} />, []);
  const menuLabel = t("settings.plugins.actions.menu", { name: pluginDisplayName(plugin) });

  return (
    <DropdownMenu compactMode="sheet">
      <DropdownMenuTrigger
        accessibilityRole="button"
        accessibilityLabel={menuLabel}
        disabled={pending}
        hitSlop={8}
        style={styles.menuButton}
      >
        <MoreHorizontal size={18} color={styles.menuIcon.color} />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" width={220} sheetTitle={menuLabel}>
        {supportsLogs ? (
          <DropdownMenuItem onSelect={openLogs} disabled={pending}>
            {t("settings.plugins.logs.action")}
          </DropdownMenuItem>
        ) : null}
        <DropdownMenuItem
          onSelect={reload}
          disabled={pending || !plugin.enabled}
          status={pendingAction === "reload" ? "pending" : "idle"}
          pendingLabel={t("settings.plugins.actions.reloading")}
        >
          {t("settings.plugins.actions.reload")}
        </DropdownMenuItem>
        <DropdownMenuItem
          destructive
          leading={removeIcon}
          onSelect={remove}
          disabled={pending}
          status={pendingAction === "remove" ? "pending" : "idle"}
          pendingLabel={t("settings.plugins.actions.removing")}
        >
          {t("settings.plugins.actions.remove")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
});

const styles = StyleSheet.create((theme) => ({
  menuButton: {
    padding: theme.spacing[1],
  },
  menuIcon: {
    color: theme.colors.foregroundMuted,
  },
  dangerIcon: {
    color: theme.colors.statusDanger,
  },
}));
