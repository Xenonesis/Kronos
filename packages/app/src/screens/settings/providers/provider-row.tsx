import { memo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { ChevronRight } from "lucide-react-native";
import { useIsCompactFormFactor } from "@/constants/layout";
import { settingsStyles } from "@/styles/settings";
import { PressHighlight } from "@/components/ui/press-highlight";
import { useProviderIcon } from "@/components/provider-icons";
import { Switch } from "@/components/ui/switch";
import { filterSelectableModels } from "@/provider-selection/model-catalog";
import type { Theme } from "@/styles/theme";
import type { ProviderDefinition, ProviderEntry } from "./provider-types";
import { getProviderStatus, StatusIndicator } from "./provider-status";
import { ProviderActionsMenu } from "./provider-actions-menu";

interface ProviderRowProps {
  serverId: string;
  def: ProviderDefinition;
  entry: ProviderEntry;
  enabled: boolean;
  isToggling: boolean;
  isRemoving: boolean;
  canRemove: boolean;
  isFirst: boolean;
  onPress: (providerId: string) => void;
  onToggleEnabled: (providerId: string, enabled: boolean) => void;
  onRemove: (providerId: string, providerLabel: string) => void;
}

const ThemedChevronRight = withUnistyles(ChevronRight, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

export const ProviderRow = memo(function ProviderRow({
  serverId,
  def,
  entry,
  enabled,
  isToggling,
  isRemoving,
  canRemove,
  isFirst,
  onPress,
  onToggleEnabled,
  onRemove,
}: ProviderRowProps) {
  const { t } = useTranslation();
  const isCompact = useIsCompactFormFactor();
  const ProviderIcon = useProviderIcon(def.id, serverId);
  const providerError =
    enabled &&
    entry.status === "error" &&
    typeof entry.error === "string" &&
    entry.error.trim().length > 0
      ? entry.error.trim()
      : null;
  const modelCount = filterSelectableModels(entry.models ?? null)?.length ?? 0;
  const providerStatus = getProviderStatus(entry.status, enabled, modelCount, t);

  const handlePress = useCallback(() => {
    onPress(def.id);
  }, [def.id, onPress]);

  const handleToggleValueChange = useCallback(
    (value: boolean) => {
      onToggleEnabled(def.id, value);
    },
    [def.id, onToggleEnabled],
  );

  return (
    <PressHighlight
      style={[settingsStyles.row, !isFirst && settingsStyles.rowBorder, styles.row]}
      onPress={handlePress}
      accessibilityRole="button"
      accessibilityLabel={t("settings.providers.providerDetails", { name: def.label })}
    >
      <View style={styles.rowContent}>
        <ThemedChevronRight size={styles.chevronIcon.fontSize} />
        <ProviderIcon size={styles.providerIcon.fontSize} color={styles.providerIcon.color} />
        <View style={styles.textColumn}>
          <View style={styles.titleRow}>
            <Text style={settingsStyles.rowTitle} numberOfLines={1}>
              {def.label}
            </Text>
            {!isCompact ? <Text style={styles.separator}>·</Text> : null}
            <StatusIndicator status={providerStatus} compact={isCompact} />
          </View>
          {providerError && !isCompact ? (
            <Text style={styles.errorText} numberOfLines={3}>
              {providerError}
            </Text>
          ) : null}
        </View>
      </View>
      <View style={styles.trailingControls}>
        {canRemove ? (
          <ProviderActionsMenu
            providerId={def.id}
            providerLabel={def.label}
            isRemoving={isRemoving}
            onRemove={onRemove}
          />
        ) : null}
        <Switch
          value={enabled}
          onValueChange={handleToggleValueChange}
          disabled={isToggling || isRemoving}
          accessibilityLabel={t("settings.providers.enableProvider", { name: def.label })}
        />
      </View>
    </PressHighlight>
  );
});

const styles = StyleSheet.create((theme: Theme) => ({
  row: {
    gap: theme.spacing[3],
    minHeight: 56,
  },
  rowContent: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  chevronIcon: {
    fontSize: theme.iconSize.sm,
  },
  providerIcon: {
    fontSize: theme.iconSize.md,
    color: theme.colors.foregroundMuted,
  },
  foreground: {
    color: theme.colors.foreground,
  },
  foregroundMuted: {
    color: theme.colors.foregroundMuted,
  },
  textColumn: {
    flex: 1,
    minWidth: 0,
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  separator: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
  errorText: {
    color: theme.colors.palette.red[300],
    fontSize: theme.fontSize.sm,
    marginTop: theme.spacing[1],
  },
  trailingControls: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
}));
