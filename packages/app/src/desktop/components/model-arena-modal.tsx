import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Columns2, Sparkles, Check, X, Clock } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { useToast } from "@/contexts/toast-context";
import { invokeTauri } from "@/utils/tauri-invoke";

const ThemedArenaIcon = withUnistyles(Columns2, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedClockIcon = withUnistyles(Clock, (theme) => ({ color: theme.colors.foregroundMuted }));

interface ArenaCandidate {
  id: string;
  provider: string;
  label: string;
  modelId: string;
  latencyMs: number;
  tokensPerSecond: number;
  totalTokens: number;
  estimatedCostUsd: number;
  response: string;
  error?: string | null;
}

interface AvailableAiModel {
  id: string;
  provider: string;
  model: string;
  displayName: string;
}

interface ArenaComparisonResult {
  id: string;
  provider: string;
  model: string;
  displayName: string;
  response: string;
  latencyMs: number;
  tokensPerSecond: number;
  totalTokens: number;
  costUsd: number;
  error?: string | null;
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
            {cand.latencyMs}ms • {cand.tokensPerSecond.toFixed(1)} t/s • $
            {cand.estimatedCostUsd.toFixed(4)}
          </Text>
        </View>
      </View>

      <ScrollView style={styles.responseScroll}>
        <Text style={styles.responseText}>
          {cand.error
            ? `Provider error (${cand.provider}): ${cand.error}`
            : cand.response ||
              "Enter a prompt above and click Benchmark Pair to run both models in parallel."}
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
  cwd?: string;
  onClose: () => void;
  onPickWinner: (prompt: string, winningModelId: string) => void;
}

const FALLBACK_CANDIDATES: ArenaCandidate[] = [
  {
    id: "cand-0",
    provider: "claude",
    label: "Claude CLI",
    modelId: "claude",
    latencyMs: 0,
    tokensPerSecond: 0,
    totalTokens: 0,
    estimatedCostUsd: 0,
    response: "",
  },
  {
    id: "cand-1",
    provider: "codex",
    label: "Codex CLI",
    modelId: "codex",
    latencyMs: 0,
    tokensPerSecond: 0,
    totalTokens: 0,
    estimatedCostUsd: 0,
    response: "",
  },
];

export const ModelArenaModal = memo(function ModelArenaModal({
  visible,
  cwd,
  onClose,
  onPickWinner,
}: ModelArenaModalProps) {
  const toast = useToast();
  const [prompt, setPrompt] = useState("");
  const [candidates, setCandidates] = useState<ArenaCandidate[]>(FALLBACK_CANDIDATES);
  const [isComparing, setIsComparing] = useState(false);

  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    async function loadModels() {
      try {
        const models = await invokeTauri<AvailableAiModel[]>("list_available_ai_models");
        if (cancelled || !models || models.length === 0) return;
        const picked = models.slice(0, 2);
        if (picked.length === 1) picked.push(models[0]!);
        const mapped: ArenaCandidate[] = [];
        for (let idx = 0; idx < picked.length; idx++) {
          const m = picked[idx]!;
          mapped.push({
            id: `cand-${idx}`,
            provider: m.provider,
            label: m.displayName || `${m.provider}: ${m.model}`,
            modelId: m.model || m.id,
            latencyMs: 0,
            tokensPerSecond: 0,
            totalTokens: 0,
            estimatedCostUsd: 0,
            response: "",
          });
        }
        setCandidates(mapped);
      } catch {
        // Keep default CLI candidates if probe fails
      }
    }
    void loadModels();
    return () => {
      cancelled = true;
    };
  }, [visible]);

  const handlePrepareComparison = useCallback(async () => {
    const clean = prompt.trim();
    if (!clean || isComparing) return;
    setIsComparing(true);
    try {
      const results = await invokeTauri<ArenaComparisonResult[]>("run_model_arena_comparison", {
        prompt: clean,
        cwd,
        candidates: candidates.map((c) => ({
          provider: c.provider,
          model: c.modelId,
          displayName: c.label,
        })),
      });
      if (results && results.length > 0) {
        setCandidates(
          results.map((r, idx) => ({
            id: r.id || `cand-${idx}`,
            provider: r.provider,
            label: r.displayName || r.model,
            modelId: r.model,
            latencyMs: r.latencyMs,
            tokensPerSecond: r.tokensPerSecond,
            totalTokens: r.totalTokens,
            estimatedCostUsd: r.costUsd,
            response: r.response,
            error: r.error,
          })),
        );
      }
    } catch (err) {
      toast.show(`Arena benchmark failed: ${String(err)}`, { variant: "error" });
    } finally {
      setIsComparing(false);
    }
  }, [candidates, cwd, isComparing, prompt, toast]);

  const handleComparePress = useCallback(() => {
    void handlePrepareComparison();
  }, [handlePrepareComparison]);

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
              onPress={handleComparePress}
              disabled={isComparing || !prompt.trim()}
            >
              <Sparkles size={13} color="#ffffff" />
              <Text style={styles.btnText}>{isComparing ? "Running..." : "Benchmark Pair"}</Text>
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
