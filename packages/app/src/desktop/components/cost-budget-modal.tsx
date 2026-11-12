import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Coins, X, RotateCcw, AlertTriangle, ShieldCheck } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { useCostBudgetStore } from "@/stores/cost-budget-store";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";

const ThemedCoinsIcon = withUnistyles(Coins, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedResetIcon = withUnistyles(RotateCcw, (theme) => ({ color: theme.colors.foreground }));

export interface CostBudgetModalProps {
  visible: boolean;
  agentId: string;
  onClose: () => void;
}

const BudgetThresholdControls = memo(function BudgetThresholdControls({
  budgetDraft,
  resetKey,
  onChangeBudget,
  onSaveBudget,
  onResetAgent,
}: {
  budgetDraft: string;
  resetKey: number;
  onChangeBudget: (text: string) => void;
  onSaveBudget: () => void;
  onResetAgent: () => void;
}) {
  return (
    <View style={styles.budgetRow}>
      <Text style={styles.label}>Session Alert Threshold ($ USD)</Text>
      <View style={styles.budgetInputRow}>
        <AdaptiveTextInput
          initialValue={budgetDraft}
          resetKey={resetKey}
          onChangeText={onChangeBudget}
          keyboardType="numeric"
          style={styles.input}
        />
        <Button size="sm" onPress={onSaveBudget}>
          <Text style={styles.btnText}>Update Cap</Text>
        </Button>
        <Button size="sm" variant="outline" onPress={onResetAgent}>
          <ThemedResetIcon size={13} />
          <Text style={styles.outlineBtnText}>Reset Agent</Text>
        </Button>
      </View>
    </View>
  );
});

export const CostBudgetModal = memo(function CostBudgetModal({
  visible,
  agentId,
  onClose,
}: CostBudgetModalProps) {
  const usage = useCostBudgetStore((s) => s.usageByAgent[agentId]);
  const usageByAgent = useCostBudgetStore((s) => s.usageByAgent);
  const budgetLimitUsd = useCostBudgetStore((s) => s.budgetLimitUsd);
  const setBudgetLimitUsd = useCostBudgetStore((s) => s.setBudgetLimitUsd);
  const resetAgentUsage = useCostBudgetStore((s) => s.resetAgentUsage);

  const [budgetDraft, setBudgetDraft] = useState(String(budgetLimitUsd));
  const [resetKey, setResetKey] = useState(0);

  useEffect(() => {
    if (visible) {
      setBudgetDraft(String(budgetLimitUsd));
      setResetKey((k) => k + 1);
    }
  }, [visible, budgetLimitUsd]);

  const handleSaveBudget = useCallback(() => {
    const parsed = Number.parseFloat(budgetDraft);
    if (Number.isFinite(parsed) && parsed > 0) {
      setBudgetLimitUsd(parsed);
    }
  }, [budgetDraft, setBudgetLimitUsd]);

  const handleResetAgent = useCallback(() => {
    resetAgentUsage(agentId);
  }, [agentId, resetAgentUsage]);

  if (!visible) return null;

  const totalSessionCost = Object.values(usageByAgent).reduce((sum, e) => sum + e.costUsd, 0);
  const agentCost = usage?.costUsd ?? 0;
  const pct = Math.min(100, (totalSessionCost / Math.max(0.01, budgetLimitUsd)) * 100);
  const isOver = totalSessionCost >= budgetLimitUsd;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop} testID="cost-budget-modal">
        <View style={styles.card}>
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <ThemedCoinsIcon size={18} />
              <Text style={styles.title}>Token Usage & USD Cost Budget HUD</Text>
            </View>
            <ToolbarButton label="Close" onPress={onClose}>
              <ThemedCloseIcon size={18} />
            </ToolbarButton>
          </View>

          <ScrollView style={styles.body}>
            <View style={styles.statGrid}>
              <View style={styles.statBox}>
                <Text style={styles.statLabel}>Current Agent Cost</Text>
                <Text style={styles.statValue}>${agentCost.toFixed(4)}</Text>
                <Text style={styles.statSub}>
                  {(usage?.inputTokens ?? 0).toLocaleString()} in /{" "}
                  {(usage?.outputTokens ?? 0).toLocaleString()} out
                </Text>
              </View>
              <View style={styles.statBox}>
                <Text style={styles.statLabel}>Workspace Session Total</Text>
                <Text style={[styles.statValue, isOver && styles.overText]}>
                  ${totalSessionCost.toFixed(4)}
                </Text>
                <Text style={styles.statSub}>
                  {pct.toFixed(1)}% of ${budgetLimitUsd.toFixed(2)} cap
                </Text>
              </View>
            </View>

            <View style={styles.progressTrack}>
              <View
                style={[
                  styles.progressFill,
                  { width: `${pct}%` },
                  isOver && styles.progressFillOver,
                ]}
              />
            </View>

            <View style={[styles.banner, isOver ? styles.bannerWarn : styles.bannerOk]}>
              {isOver ? (
                <AlertTriangle size={15} color="#ef4444" />
              ) : (
                <ShieldCheck size={15} color="#22c55e" />
              )}
              <Text style={styles.bannerText}>
                {isOver
                  ? "Session budget cap reached! Switch to a local Ollama model or raise the cap."
                  : "Usage is within configured session USD budget."}
              </Text>
            </View>

            <BudgetThresholdControls
              budgetDraft={budgetDraft}
              resetKey={resetKey}
              onChangeBudget={setBudgetDraft}
              onSaveBudget={handleSaveBudget}
              onResetAgent={handleResetAgent}
            />
          </ScrollView>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0,0,0,0.65)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  card: {
    width: "100%",
    maxWidth: 560,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    padding: theme.spacing[4],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  titleRow: { flexDirection: "row", alignItems: "center", gap: 8 },
  title: { fontSize: theme.fontSize.base, fontWeight: "700", color: theme.colors.foreground },
  closeBtn: { padding: 4 },
  body: { padding: theme.spacing[4] },
  statGrid: { flexDirection: "row", gap: 12, marginBottom: 14 },
  statBox: {
    flex: 1,
    padding: 12,
    borderRadius: theme.borderRadius.md,
    backgroundColor: theme.colors.surface2,
    borderWidth: 1,
    borderColor: theme.colors.border,
    gap: 4,
  },
  statLabel: { fontSize: 11, color: theme.colors.foregroundMuted, fontWeight: "600" },
  statValue: { fontSize: 20, fontWeight: "700", color: theme.colors.foreground },
  overText: { color: "#ef4444" },
  statSub: { fontSize: 11, color: theme.colors.foregroundMuted },
  progressTrack: {
    height: 8,
    borderRadius: 99,
    backgroundColor: theme.colors.surface2,
    overflow: "hidden",
    marginBottom: 14,
  },
  progressFill: { height: "100%", backgroundColor: theme.colors.accent },
  progressFillOver: { backgroundColor: "#ef4444" },
  banner: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    padding: 10,
    borderRadius: theme.borderRadius.md,
    marginBottom: 16,
  },
  bannerOk: { backgroundColor: "rgba(34, 197, 94, 0.1)" },
  bannerWarn: { backgroundColor: "rgba(239, 68, 68, 0.12)" },
  bannerText: { flex: 1, fontSize: 12, color: theme.colors.foreground },
  budgetRow: { gap: 8 },
  label: { fontSize: 12, fontWeight: "600", color: theme.colors.foreground },
  budgetInputRow: { flexDirection: "row", alignItems: "center", gap: 8 },
  input: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.borderRadius.sm,
    paddingHorizontal: 10,
    paddingVertical: 6,
    color: theme.colors.foreground,
    fontSize: 13,
  },
  btnText: { color: "#ffffff", fontWeight: "600", fontSize: 12 },
  outlineBtnText: {
    color: theme.colors.foreground,
    fontWeight: "600",
    fontSize: 12,
    marginLeft: 4,
  },
}));
