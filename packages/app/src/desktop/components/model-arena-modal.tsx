import React, { memo, useState, useCallback } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Columns2, Sparkles, Check, X, Clock } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { calculateCostUsd } from "@/stores/cost-budget-store";

const ThemedArenaIcon = withUnistyles(Columns2, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedClockIcon = withUnistyles(Clock, (theme) => ({ color: theme.colors.foregroundMuted }));

interface ArenaCandidate {
  id: string;
  label: string;
  modelId: string;
  latencyMs: number;
  estimatedCostUsd: number;
  response: string;
}

const ArenaCandidatePane = memo(function ArenaCandidatePane({
  cand,
  onDispatch,
}: {
  cand: ArenaCandidate;
  onDispatch: (modelId: string) => void;
}) {
  const handlePress = useCallback(() => {
    onDispatch(cand.modelId);
  }, [cand.modelId, onDispatch]);

  return (
    <View style={styles.pane}>
      <View style={styles.paneHeader}>
        <Text style={styles.paneTitle}>{cand.label}</Text>
        <View style={styles.stats}>
          <ThemedClockIcon size={11} />
          <Text style={styles.statText}>
            {cand.latencyMs}ms • ${cand.estimatedCostUsd.toFixed(4)}
          </Text>
        </View>
      </View>

      <ScrollView style={styles.responseScroll}>
        <Text style={styles.responseText}>
          {cand.response ||
            "Enter a prompt above to compare latency, cost, and dispatch a side-by-side run."}
        </Text>
      </ScrollView>

      <Button size="sm" onPress={handlePress}>
        <Check size={13} color="#ffffff" />
        <Text style={styles.btnText}>Dispatch {cand.label}</Text>
      </Button>
    </View>
  );
});

export interface ModelArenaModalProps {
  visible: boolean;
  onClose: () => void;
  onPickWinner: (prompt: string, winningModelId: string) => void;
}

const DEFAULT_CANDIDATES: ArenaCandidate[] = [
  {
    id: "left",
    label: "Claude Sonnet 4",
    modelId: "claude-sonnet-4",
    latencyMs: 0,
    estimatedCostUsd: 0,
    response: "",
  },
  {
    id: "right",
    label: "GPT-4o / Codex",
    modelId: "codex",
    latencyMs: 0,
    estimatedCostUsd: 0,
    response: "",
  },
];

export const ModelArenaModal = memo(function ModelArenaModal({
  visible,
  onClose,
  onPickWinner,
}: ModelArenaModalProps) {
  const [prompt, setPrompt] = useState("");
  const [candidates, setCandidates] = useState<ArenaCandidate[]>(DEFAULT_CANDIDATES);
  const [isComparing, setIsComparing] = useState(false);

  const handlePrepareComparison = useCallback(() => {
    const clean = prompt.trim();
    if (!clean) return;
    setIsComparing(true);
    const simulated = candidates.map((c, idx) => {
      const estInputTokens = Math.max(400, clean.length * 2);
      const estOutputTokens = 320 + idx * 80;
      return {
        ...c,
        latencyMs: 850 + idx * 340,
        estimatedCostUsd: calculateCostUsd({
          modelId: c.modelId,
          inputTokens: estInputTokens,
          outputTokens: estOutputTokens,
        }),
        response: `Ready to dispatch parallel Arena benchmark for [${c.label}] on prompt:\n"${clean}"\n\nClick "Dispatch & Adopt ${c.label}" below to spawn an isolated agent turn pinned to ${c.modelId}.`,
      };
    });
    setCandidates(simulated);
    setIsComparing(false);
  }, [candidates, prompt]);

  const handleDispatchModel = useCallback(
    (modelId: string) => {
      onPickWinner(prompt.trim() || "Compare implementation strategies", modelId);
      onClose();
    },
    [onClose, onPickWinner, prompt],
  );

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop} testID="model-arena-modal">
        <View style={styles.card}>
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <ThemedArenaIcon size={18} />
              <Text style={styles.title}>Side-by-Side Multi-Model Arena (Zed AI Parity)</Text>
            </View>
            <ToolbarButton label="Close" onPress={onClose}>
              <ThemedCloseIcon size={18} />
            </ToolbarButton>
          </View>

          <View style={styles.promptBar}>
            <AdaptiveTextInput
              initialValue={prompt}
              onChangeText={setPrompt}
              placeholder="Enter coding prompt to benchmark across models..."
              style={styles.promptInput}
            />
            <Button
              size="sm"
              onPress={handlePrepareComparison}
              disabled={isComparing || !prompt.trim()}
            >
              <Sparkles size={13} color="#ffffff" />
              <Text style={styles.btnText}>Benchmark Pair</Text>
            </Button>
          </View>

          <View style={styles.columns}>
            {candidates.map((cand) => (
              <ArenaCandidatePane key={cand.id} cand={cand} onDispatch={handleDispatchModel} />
            ))}
          </View>
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
    maxWidth: 920,
    height: "80%",
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
  promptBar: {
    flexDirection: "row",
    alignItems: "center",
    gap: 10,
    padding: theme.spacing[4],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  promptInput: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 6,
    paddingHorizontal: 10,
    paddingVertical: 8,
    color: theme.colors.foreground,
    fontSize: 13,
  },
  btnText: { color: "#ffffff", fontSize: 12, fontWeight: "600", marginLeft: 4 },
  columns: { flex: 1, flexDirection: "row", gap: 12, padding: theme.spacing[4] },
  pane: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 8,
    padding: 12,
    gap: 10,
  },
  paneHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    paddingBottom: 8,
  },
  paneTitle: { fontSize: 14, fontWeight: "700", color: theme.colors.accent },
  stats: { flexDirection: "row", alignItems: "center", gap: 4 },
  statText: { fontSize: 11, color: theme.colors.foregroundMuted },
  responseScroll: { flex: 1 },
  responseText: { fontSize: 12, color: theme.colors.foreground, lineHeight: 18 },
}));
