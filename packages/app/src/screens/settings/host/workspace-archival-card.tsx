import { useCallback } from "react";
import { Alert, Text, View } from "react-native";
import { Switch } from "@/components/ui/switch";
import { useDaemonConfig } from "@/hooks/use-daemon-config";
import { useHostRuntimeIsConnected } from "@/runtime/host-runtime";
import { settingsStyles } from "@/styles/settings";

interface AutoArchiveMergedWorkspacesCardProps {
  serverId: string;
}

export function AutoArchiveMergedWorkspacesCard({
  serverId,
}: AutoArchiveMergedWorkspacesCardProps) {
  const isConnected = useHostRuntimeIsConnected(serverId);
  const { config, patchConfig } = useDaemonConfig(serverId);

  const handleValueChange = useCallback(
    (next: boolean) => {
      void patchConfig({ autoArchiveAfterMerge: next }).catch((error) => {
        console.error("[HostPage] Failed to update auto-archive after merge", error);
        Alert.alert(
          "Unable to update workspaces",
          error instanceof Error ? error.message : String(error),
        );
      });
    },
    [patchConfig],
  );

  if (!isConnected) return null;

  return (
    <View style={settingsStyles.card} testID="host-page-auto-archive-merged-workspaces-card">
      <View style={settingsStyles.row}>
        <View style={settingsStyles.rowContent}>
          <Text style={settingsStyles.rowTitle}>Archive merged PR workspaces</Text>
          <Text style={settingsStyles.rowHint}>
            Automatically archive clean Paseo workspaces after their pull request is merged
          </Text>
        </View>
        <Switch
          value={config?.autoArchiveAfterMerge === true}
          onValueChange={handleValueChange}
          accessibilityLabel="Archive merged PR workspaces"
          testID="host-page-auto-archive-merged-workspaces-switch"
        />
      </View>
    </View>
  );
}
