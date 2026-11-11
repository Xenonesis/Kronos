import React, { memo, useState } from "react";
import { View, Text, Pressable } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Zap, DollarSign, TrendingDown, ChevronDown, ChevronUp } from "lucide-react-native";

export interface PromptCachingMetrics {
  cacheHitRate: number; // 0 - 100%
  cacheReadTokens: number;
  cacheWriteTokens: number;
  totalTokens: number;
  estimatedCostUsd: number;
  dollarsSavedUsd: number;
}

export interface PromptCachingAnalyticsHudProps {
  metrics?: PromptCachingMetrics;
  visible?: boolean;
}

const DEFAULT_METRICS: PromptCachingMetrics = {
  cacheHitRate: 88.5,
  cacheReadTokens: 42_800,
  cacheWriteTokens: 5_200,
  totalTokens: 58_400,
  estimatedCostUsd: 0.052,
  dollarsSavedUsd: 1.48,
};

export const PromptCachingAnalyticsHud = memo(function PromptCachingAnalyticsHud({
  metrics = DEFAULT_METRICS,
  visible = true,
}: PromptCachingAnalyticsHudProps) {
  const { theme } = useUnistyles();
  const [expanded, setExpanded] = useState(false);

  if (!visible) return null;

  return (
    <View style={styles.container} data-testid="prompt-caching-analytics-hud">
      <Pressable onPress={() => setExpanded(!expanded)} style={styles.statusBar}>
        <View style={styles.leftGroup}>
          <Zap size={12} color="#10b981" />
          <Text style={styles.metricText}>
            Cache Hit: <Text style={styles.highlightText}>{metrics.cacheHitRate.toFixed(1)}%</Text>
          </Text>
        </View>

        <View style={styles.rightGroup}>
          <View style={styles.savingsPill}>
            <TrendingDown size={10} color="#10b981" />
            <Text style={styles.savingsText}>+${metrics.dollarsSavedUsd.toFixed(2)} saved</Text>
          </View>
          {expanded ? (
            <ChevronUp size={12} color={theme.colors.foregroundMuted} />
          ) : (
            <ChevronDown size={12} color={theme.colors.foregroundMuted} />
          )}
        </View>
      </Pressable>

      {expanded ? (
        <View style={styles.expandedCard}>
          <View style={styles.row}>
            <Text style={styles.label}>Cache Read Tokens:</Text>
            <Text style={styles.value}>{metrics.cacheReadTokens.toLocaleString()}</Text>
          </View>
          <View style={styles.row}>
            <Text style={styles.label}>Cache Write Tokens:</Text>
            <Text style={styles.value}>{metrics.cacheWriteTokens.toLocaleString()}</Text>
          </View>
          <View style={styles.row}>
            <Text style={styles.label}>Total Turn Tokens:</Text>
            <Text style={styles.value}>{metrics.totalTokens.toLocaleString()}</Text>
          </View>
          <View style={[styles.row, styles.totalRow]}>
            <Text style={styles.totalLabel}>Session Cost:</Text>
            <Text style={styles.totalValue}>${metrics.estimatedCostUsd.toFixed(4)}</Text>
          </View>
        </View>
      ) : null}
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    marginVertical: 4,
    borderRadius: 6,
    overflow: "hidden",
  },
  statusBar: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    backgroundColor: theme.colors.surface1,
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: 6,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  leftGroup: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  metricText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  highlightText: {
    color: "#10b981",
    fontWeight: "700",
  },
  rightGroup: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  savingsPill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 3,
    backgroundColor: "rgba(16, 185, 129, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  savingsText: {
    fontSize: 10,
    fontWeight: "600",
    color: "#10b981",
  },
  expandedCard: {
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderTopWidth: 0,
    borderColor: theme.colors.border,
    padding: 8,
    borderBottomLeftRadius: 6,
    borderBottomRightRadius: 6,
  },
  row: {
    flexDirection: "row",
    justifyContent: "space-between",
    paddingVertical: 2,
  },
  label: {
    fontSize: 10,
    color: theme.colors.foregroundMuted,
  },
  value: {
    fontSize: 10,
    fontFamily: "monospace",
    color: theme.colors.foreground,
  },
  totalRow: {
    marginTop: 4,
    paddingTop: 4,
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  totalLabel: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  totalValue: {
    fontSize: 11,
    fontFamily: "monospace",
    fontWeight: "700",
    color: theme.colors.foreground,
  },
}));
