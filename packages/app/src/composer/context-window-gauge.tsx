import { memo, useMemo, useCallback } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Gauge, Sparkles } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";

interface ContextWindowGaugeProps {
  totalTokensUsed: number;
  contextLimit?: number;
  onCompactRequest?: () => void;
}

const ThemedGauge = withUnistyles(Gauge);
const ThemedSparkles = withUnistyles(Sparkles);

export const ContextWindowGauge = memo(function ContextWindowGauge({
  totalTokensUsed,
  contextLimit = 128_000,
  onCompactRequest,
}: ContextWindowGaugeProps) {
  const percentage = useMemo(() => {
    if (contextLimit <= 0) return 0;
    return Math.min(100, Math.round((totalTokensUsed / contextLimit) * 100));
  }, [totalTokensUsed, contextLimit]);

  const statusColor = useMemo(() => {
    if (percentage > 85) return "#ef4444"; // Red alert
    if (percentage > 60) return "#f59e0b"; // Amber caution
    return "#10b981"; // Emerald safe
  }, [percentage]);

  const formattedUsed = useMemo(() => {
    if (totalTokensUsed >= 1000) {
      return `${(totalTokensUsed / 1000).toFixed(1)}k`;
    }
    return `${totalTokensUsed}`;
  }, [totalTokensUsed]);

  const formattedLimit = useMemo(() => {
    return `${Math.round(contextLimit / 1000)}k`;
  }, [contextLimit]);

  const handleCompact = useCallback(() => {
    onCompactRequest?.();
  }, [onCompactRequest]);

  if (totalTokensUsed <= 0) return null;

  return (
    <View style={styles.container} testID="context-window-gauge">
      <View style={styles.headerRow}>
        <View style={styles.infoRow}>
          <ThemedGauge size={12} color={statusColor} />
          <Text style={styles.tokenText}>
            Context: {formattedUsed} / {formattedLimit} ({percentage}%)
          </Text>
        </View>
        {percentage > 65 && onCompactRequest ? (
          <PressHighlight
            style={styles.compactButton}
            onPress={handleCompact}
            accessibilityRole="button"
            accessibilityLabel="Compact Context"
          >
            <ThemedSparkles size={11} />
            <Text style={styles.compactButtonText}>Compact</Text>
          </PressHighlight>
        ) : null}
      </View>

      <View style={styles.progressBarTrack}>
        <View
          style={[
            styles.progressBarFill,
            {
              width: `${percentage}%`,
              backgroundColor: statusColor,
            },
          ]}
        />
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    paddingHorizontal: 8,
    paddingVertical: 4,
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.sm,
    gap: 3,
  },
  headerRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  infoRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  tokenText: {
    fontSize: 10,
    fontFamily: theme.fontFamily.mono,
    color: theme.colors.foregroundMuted,
  },
  compactButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 3,
    paddingHorizontal: 5,
    paddingVertical: 1,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: theme.colors.surface2,
  },
  compactButtonText: {
    fontSize: 10,
    fontWeight: "600",
    color: theme.colors.primary,
  },
  progressBarTrack: {
    height: 3,
    backgroundColor: theme.colors.border,
    borderRadius: 2,
    overflow: "hidden",
    width: "100%",
  },
  progressBarFill: {
    height: "100%",
    borderRadius: 2,
  },
}));
