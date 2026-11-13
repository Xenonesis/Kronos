import React, { memo } from "react";
import { Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Coins, AlertTriangle } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { useCostBudgetStore, calculateCostUsd } from "@/stores/cost-budget-store";
import { useSessionStore } from "@/stores/session-store";

const ThemedCoinsIcon = withUnistyles(Coins, (theme) => ({
  color: theme.colors.accent,
}));
const ThemedAlertTriangleIcon = withUnistyles(AlertTriangle, (theme) => ({
  color: theme.colors.destructive ?? "#ef4444",
}));

function formatTokenCount(totalTokens: number): string {
  if (totalTokens >= 1_000_000) {
    return `${(totalTokens / 1_000_000).toFixed(1)}M`;
  }
  if (totalTokens >= 1_000) {
    return `${(totalTokens / 1_000).toFixed(1)}k`;
  }
  return `${totalTokens}`;
}

function resolveEffectiveCostUsd(params: {
  entryCostUsd: number | undefined;
  liveTotalCostUsd: number | undefined;
  liveModel: string | null;
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
}): number {
  if (params.entryCostUsd && params.entryCostUsd > 0) {
    return params.entryCostUsd;
  }
  if (typeof params.liveTotalCostUsd === "number" && params.liveTotalCostUsd > 0) {
    return params.liveTotalCostUsd;
  }
  return calculateCostUsd({
    modelId: params.liveModel,
    inputTokens: params.inputTokens,
    outputTokens: params.outputTokens,
    cacheReadTokens: params.cachedInputTokens,
  });
}

export interface CostBudgetPillProps {
  agentId: string;
  serverId?: string;
  onPress?: () => void;
}

export const CostBudgetPill = memo(function CostBudgetPill({
  agentId,
  serverId,
  onPress,
}: CostBudgetPillProps) {
  const entry = useCostBudgetStore((state) => state.usageByAgent[agentId]);
  const budgetLimitUsd = useCostBudgetStore((state) => state.budgetLimitUsd);
  const openCostModal = useCostBudgetStore((state) => state.openCostModal);
  const handlePillPress = React.useCallback(() => {
    if (onPress) {
      onPress();
    } else {
      openCostModal(agentId);
    }
  }, [agentId, onPress, openCostModal]);
  const liveAgentUsage = useSessionStore((state) => {
    if (!serverId) return null;
    return state.sessions[serverId]?.agents.get(agentId)?.lastUsage ?? null;
  });
  const liveModel = useSessionStore((state) => {
    if (!serverId) return null;
    return state.sessions[serverId]?.agents.get(agentId)?.model ?? null;
  });

  const inputTokens = entry?.inputTokens ?? liveAgentUsage?.inputTokens ?? 0;
  const outputTokens = entry?.outputTokens ?? liveAgentUsage?.outputTokens ?? 0;

  if (inputTokens === 0 && outputTokens === 0) {
    return null;
  }

  const costUsd = resolveEffectiveCostUsd({
    entryCostUsd: entry?.costUsd,
    liveTotalCostUsd: liveAgentUsage?.totalCostUsd,
    liveModel,
    inputTokens,
    outputTokens,
    cachedInputTokens: liveAgentUsage?.cachedInputTokens ?? 0,
  });

  const totalTokens = inputTokens + outputTokens;
  const isOverBudget = costUsd >= budgetLimitUsd;
  const formattedTokens = formatTokenCount(totalTokens);

  return (
    <PressHighlight
      onPress={handlePillPress}
      style={[styles.pill, isOverBudget && styles.pillOverBudget]}
      accessibilityLabel="Open token and USD cost budget HUD"
      testID="cost-budget-pill"
    >
      {isOverBudget ? <ThemedAlertTriangleIcon size={12} /> : <ThemedCoinsIcon size={12} />}
      <Text style={[styles.text, isOverBudget && styles.textOverBudget]}>
        {formattedTokens} tok • ${costUsd.toFixed(4)}
      </Text>
    </PressHighlight>
  );
});

const styles = StyleSheet.create((theme) => ({
  pill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 5,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 3,
    borderRadius: theme.borderRadius.full,
    backgroundColor: theme.colors.surface2,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  pillOverBudget: {
    borderColor: theme.colors.destructive ?? "#ef4444",
    backgroundColor: "rgba(239, 68, 68, 0.12)",
  },
  text: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
  },
  textOverBudget: {
    color: theme.colors.destructive ?? "#ef4444",
  },
}));
