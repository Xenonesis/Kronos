import React from "react";
import { StyleSheet, Text, View } from "react-native";

export function calculateTps(tokens: number, elapsedMs: number): number {
  if (elapsedMs <= 0 || tokens <= 0) return 0;
  return Number(((tokens / elapsedMs) * 1000).toFixed(1));
}

export function formatTokenMetric(tokens: number): string {
  if (tokens >= 1_000_000) {
    return `${(tokens / 1_000_000).toFixed(1)}m`;
  }
  if (tokens >= 10_000) {
    return `${(tokens / 1_000).toFixed(1)}k`;
  }
  if (tokens >= 1_000) {
    return `${(tokens / 1_000).toFixed(1)}k`;
  }
  return String(tokens);
}

export interface TelemetryHudProps {
  tokens?: number | null;
  contextTokens?: number | null;
  tps?: number;
  ttftMs?: number | null;
  costUsd?: number | null;
  isStreaming?: boolean;
}

export function TelemetryHud({
  tokens,
  contextTokens,
  tps = 0,
  ttftMs,
  costUsd,
  isStreaming,
}: TelemetryHudProps) {
  const displayTokens =
    tokens !== undefined && tokens !== null && tokens > 0
      ? tokens
      : contextTokens !== undefined && contextTokens !== null && contextTokens > 0
        ? contextTokens
        : null;

  const showTokens = displayTokens !== null;
  const showTps = isStreaming && tps > 0;
  const showTtft = ttftMs !== undefined && ttftMs !== null && ttftMs > 0;
  const showCost = costUsd !== undefined && costUsd !== null && costUsd > 0;

  if (!showTokens && !showTps && !showTtft && !showCost) {
    return null;
  }

  return (
    <View style={styles.container} testID="telemetry-hud">
      {showTokens && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>TOK</Text>
          <Text style={styles.badgeValue}>{formatTokenMetric(displayTokens)}</Text>
        </View>
      )}
      {showTps && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>SPD</Text>
          <Text style={styles.badgeValue}>{tps} TPS</Text>
        </View>
      )}
      {showTtft && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>LAT</Text>
          <Text style={styles.badgeValue}>{ttftMs}ms TTFT</Text>
        </View>
      )}
      {showCost && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>COST</Text>
          <Text style={styles.badgeValue}>${costUsd.toFixed(4)}</Text>
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: 7,
    paddingVertical: 3,
    backgroundColor: "#0D0D0D",
    borderWidth: 1,
    borderColor: "#262626",
    borderRadius: 3,
    flexShrink: 0,
    minHeight: 22,
  },
  badge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  badgeLabel: {
    fontSize: 10,
    fontFamily: "monospace",
    color: "#4AF626",
    fontWeight: "700",
  },
  badgeValue: {
    fontSize: 10,
    fontFamily: "monospace",
    color: "#EAEAEA",
    fontWeight: "600",
  },
});
