import React, { memo, useCallback, useState } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { DatabaseZap, TrendingDown, ChevronDown, ChevronUp } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";

const ThemedDatabaseZapIcon = withUnistyles(DatabaseZap, () => ({ color: "#10b981" }));
const ThemedTrendingDownIcon = withUnistyles(TrendingDown, () => ({ color: "#10b981" }));
const ThemedChevronDownIcon = withUnistyles(ChevronDown, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedChevronUpIcon = withUnistyles(ChevronUp, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

export interface PromptCachingMetrics {
  inputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  outputTokens: number;
  estimatedSavingsUsd: number;
  netCostUsd: number;
}

export interface PromptCachingAnalyticsHudProps {
  metrics?: PromptCachingMetrics | null;
}

export const PromptCachingAnalyticsHud = memo(function PromptCachingAnalyticsHud({
  metrics,
}: PromptCachingAnalyticsHudProps) {
  const [expanded, setExpanded] = useState(true);

  const toggleExpanded = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);

  if (
    !metrics ||
    (metrics.inputTokens <= 0 &&
      metrics.cacheReadTokens <= 0 &&
      metrics.cacheWriteTokens <= 0 &&
      metrics.outputTokens <= 0)
  ) {
    return null;
  }

  const totalInput = metrics.inputTokens + metrics.cacheReadTokens;
  const hitRate = totalInput > 0 ? Math.round((metrics.cacheReadTokens / totalInput) * 100) : 0;

  return (
    <View style={styles.container} data-testid="prompt-caching-analytics-hud">
      <PressHighlight onPress={toggleExpanded} style={styles.headerPill}>
        <ThemedDatabaseZapIcon size={12} />
        <Text style={styles.pillText}>Cache Hit: {hitRate}%</Text>
        <View style={styles.savingsBadge}>
          <ThemedTrendingDownIcon size={10} />
          <Text style={styles.savingsText}>Saved ${metrics.estimatedSavingsUsd.toFixed(4)}</Text>
        </View>
        {expanded ? <ThemedChevronUpIcon size={12} /> : <ThemedChevronDownIcon size={12} />}
      </PressHighlight>

      {expanded ? (
        <View style={styles.expandedCard}>
          <View style={styles.row}>
            <Text style={styles.label}>Prompt Cache Read Tokens</Text>
            <Text style={styles.value}>{metrics.cacheReadTokens.toLocaleString()}</Text>
          </View>
          <View style={styles.row}>
            <Text style={styles.label}>Uncached Input Tokens</Text>
            <Text style={styles.value}>{metrics.inputTokens.toLocaleString()}</Text>
          </View>
          <View style={styles.row}>
            <Text style={styles.label}>Cache Write Tokens</Text>
            <Text style={styles.value}>{metrics.cacheWriteTokens.toLocaleString()}</Text>
          </View>
          <View style={styles.row}>
            <Text style={styles.label}>Output Tokens</Text>
            <Text style={styles.value}>{metrics.outputTokens.toLocaleString()}</Text>
          </View>
          <View style={[styles.row, styles.totalRow]}>
            <Text style={styles.totalLabel}>Net Session Cost</Text>
            <Text style={styles.totalValue}>${metrics.netCostUsd.toFixed(4)}</Text>
          </View>
        </View>
      ) : null}
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    marginVertical: 8,
  },
  headerPill: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 6,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    paddingHorizontal: 10,
    paddingVertical: 6,
    borderRadius: 6,
  },
  pillText: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  savingsBadge: {
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
