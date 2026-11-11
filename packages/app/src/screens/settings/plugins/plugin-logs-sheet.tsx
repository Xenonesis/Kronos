import { memo, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import type { PluginListItem, PluginLogEntry } from "@getpaseo/protocol/messages";
import { AdaptiveModalSheet, type SheetHeader } from "@/components/adaptive-modal-sheet";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useFetchQuery } from "@/data/query";
import type { useHostRuntimeClient } from "@/runtime/host-runtime";
import { pluginDisplayName } from "@/plugins/plugin-list";

interface PluginLogsSheetProps {
  client: NonNullable<ReturnType<typeof useHostRuntimeClient>>;
  plugin: PluginListItem;
  serverId: string;
  onClose: () => void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const PluginLogsSheet = memo(function PluginLogsSheet({
  client,
  plugin,
  serverId,
  onClose,
}: PluginLogsSheetProps) {
  const pluginId = plugin.id;
  const { t } = useTranslation();
  const logs = useFetchQuery({
    queryKey: ["plugin-logs", serverId, pluginId],
    queryFn: () => client.getPluginLogs(pluginId),
    dataShape: "list",
    staleTimeMs: 0,
  });
  const refresh = useCallback(() => {
    void logs.refetch();
  }, [logs]);
  const header = useMemo<SheetHeader>(
    () => ({
      title: t("settings.plugins.logs.title", { name: pluginDisplayName(plugin) }),
      actions: (
        <Button variant="outline" onPress={refresh} disabled={logs.isFetching}>
          {logs.isFetching
            ? t("settings.plugins.logs.refreshing")
            : t("settings.plugins.logs.refresh")}
        </Button>
      ),
    }),
    [logs.isFetching, plugin, refresh, t],
  );

  let content = <Text style={styles.logsState}>{t("settings.plugins.logs.loading")}</Text>;
  if (logs.isError) {
    content = (
      <Alert
        size="sm"
        variant="error"
        title={t("settings.plugins.logs.errorTitle")}
        description={errorMessage(logs.error)}
      >
        <Button variant="outline" onPress={refresh}>
          {t("settings.plugins.logs.refresh")}
        </Button>
      </Alert>
    );
  } else if (!logs.isLoading) {
    content = logs.data?.length ? (
      <View style={styles.logList}>
        {logs.data.map((entry: PluginLogEntry) => (
          <View key={entry.sequence} style={styles.logEntry}>
            <Text style={entry.stream === "stderr" ? styles.logStderr : styles.logStdout}>
              {entry.timestamp} {entry.stream}
            </Text>
            <Text style={styles.logMessage}>{entry.message}</Text>
          </View>
        ))}
      </View>
    ) : (
      <Text style={styles.logsState}>{t("settings.plugins.logs.empty")}</Text>
    );
  }

  return (
    <AdaptiveModalSheet header={header} visible onClose={onClose} testID="plugin-logs-sheet">
      {content}
    </AdaptiveModalSheet>
  );
});

const styles = StyleSheet.create((theme) => ({
  logsState: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.sm,
    padding: theme.spacing[4],
    textAlign: "center",
  },
  logList: {
    gap: theme.spacing[2],
  },
  logEntry: {
    paddingVertical: theme.spacing[1],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  logStderr: {
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.sm,
    color: theme.colors.statusDanger,
  },
  logStdout: {
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  logMessage: {
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    marginTop: 2,
  },
}));
