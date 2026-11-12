import { memo, useCallback, useState, type ReactElement } from "react";
import { Pressable, Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { composerPillStyles } from "@/composer/pill-styles";
import { useShadowLinterStore } from "@/stores/shadow-linter-store";
import { useDraftStore } from "@/stores/draft-store";
import { buildDraftStoreKey } from "@/stores/draft-keys";

export interface ShadowLinterPillProps {
  serverId: string;
  agentId: string;
}

export const ShadowLinterPill = memo(function ShadowLinterPill({
  serverId,
  agentId,
}: ShadowLinterPillProps): ReactElement | null {
  const diagnostic = useShadowLinterStore((state) => state.diagnosticsByAgent[agentId]);
  const isDismissed = useShadowLinterStore((state) => state.dismissedByAgent[agentId]);
  const clearDiagnostic = useShadowLinterStore((state) => state.clearDiagnostic);
  const dismissDiagnostic = useShadowLinterStore((state) => state.dismissDiagnostic);

  const [isHovered, setIsHovered] = useState(false);

  const handleAutoHeal = useCallback(() => {
    if (!diagnostic) return;
    const draftKey = buildDraftStoreKey({ serverId, agentId });
    useDraftStore.getState().replaceDraftText({ draftKey, text: diagnostic.repairPrompt });
    clearDiagnostic(agentId);
  }, [diagnostic, serverId, agentId, clearDiagnostic]);

  const handleDismiss = useCallback(() => {
    dismissDiagnostic(agentId);
  }, [dismissDiagnostic, agentId]);

  if (!diagnostic || isDismissed) {
    return null;
  }

  return (
    <View style={styles.container}>
      <Pressable
        testID="shadow-linter-pill-heal"
        accessibilityRole="button"
        onPress={handleAutoHeal}
        onHoverIn={() => setIsHovered(true)}
        onHoverOut={() => setIsHovered(false)}
        style={[composerPillStyles.body, styles.alertPill, isHovered && styles.alertPillHover]}
      >
        <Text style={styles.alertIcon}>⚠️</Text>
        <Text style={styles.alertText}>
          {diagnostic.system}: {diagnostic.errorCount} {diagnostic.errorCount === 1 ? "error" : "errors"}
          {diagnostic.primaryFile ? ` in ${diagnostic.primaryFile.split("/").pop()}` : ""}
        </Text>
        <View style={styles.healBadge}>
          <Text style={styles.healText}>⚡ Auto-Heal</Text>
        </View>
      </Pressable>

      <Pressable
        testID="shadow-linter-pill-dismiss"
        accessibilityRole="button"
        onPress={handleDismiss}
        style={styles.dismissButton}
      >
        <Text style={styles.dismissText}>✕</Text>
      </Pressable>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  alertPill: {
    borderColor: "#f59e0b",
    backgroundColor: "rgba(245, 158, 11, 0.12)",
  },
  alertPillHover: {
    backgroundColor: "rgba(245, 158, 11, 0.22)",
  },
  alertIcon: {
    fontSize: 12,
  },
  alertText: {
    fontSize: theme.fontSize.sm,
    color: "#f59e0b",
    fontWeight: theme.fontWeight.medium,
  },
  healBadge: {
    backgroundColor: "#f59e0b",
    borderRadius: theme.borderRadius.sm,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 2,
    marginLeft: theme.spacing[1],
  },
  healText: {
    fontSize: 11,
    color: "#000000",
    fontWeight: theme.fontWeight.bold,
  },
  dismissButton: {
    padding: theme.spacing[1],
    borderRadius: theme.borderRadius.full,
    opacity: 0.6,
  },
  dismissText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
}));
