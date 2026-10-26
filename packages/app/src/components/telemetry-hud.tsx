import React from "react";
import { StyleSheet, Text, View } from "react-native";

export function calculateTps(tokens: number, elapsedMs: number): number {
  if (elapsedMs <= 0 || tokens <= 0) return 0;
  return Number(((tokens / elapsedMs) * 1000).toFixed(1));
}

export interface TelemetryHudProps {
  tps?: number;
  ttftMs?: number | null;
  costUsd?: number | null;
  isStreaming?: boolean;
}

export function TelemetryHud({ tps = 0, ttftMs, costUsd, isStreaming }: TelemetryHudProps) {
  if (!isStreaming && tps === 0 && !ttftMs && (!costUsd || costUsd <= 0)) return null;

  return (
    <View style={styles.container} testID="telemetry-hud">
      {isStreaming && tps > 0 && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>SPD</Text>
          <Text style={styles.badgeValue}>{tps} TPS</Text>
        </View>
      )}
      {ttftMs !== undefined && ttftMs !== null && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>LAT</Text>
          <Text style={styles.badgeValue}>{ttftMs}ms TTFT</Text>
        </View>
      )}
      {costUsd !== undefined && costUsd !== null && costUsd > 0 && (
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
    paddingHorizontal: 6,
    paddingVertical: 2,
    backgroundColor: "#0A0A0A",
    borderWidth: 1,
    borderColor: "#222222",
    borderRadius: 0,
  },
  badge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  badgeLabel: {
    fontSize: 10,
    fontFamily: "monospace",
    color: "#FF2A2A",
    fontWeight: "700",
  },
  badgeValue: {
    fontSize: 10,
    fontFamily: "monospace",
    color: "#EAEAEA",
  },
});
