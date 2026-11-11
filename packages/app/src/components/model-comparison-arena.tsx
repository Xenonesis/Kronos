import { memo, useState, useCallback } from "react";
import { View, Text, Modal, Pressable, ScrollView } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Scale, X, Check, Zap, DollarSign, Clock, Trophy } from "lucide-react-native";
import { Button } from "@/components/ui/button";

export interface ModelCandidate {
  modelId: string;
  displayName: string;
  provider: string;
  responseText: string;
  tokensPerSecond: number;
  totalTokens: number;
  latencyMs: number;
  costUsd: number;
}

export interface ModelComparisonArenaProps {
  visible: boolean;
  onClose: () => void;
  prompt: string;
  candidateA: ModelCandidate;
  candidateB: ModelCandidate;
  onSelectWinner: (modelId: string, responseText: string) => void;
}

export const ModelComparisonArena = memo(function ModelComparisonArena({
  visible,
  onClose,
  prompt,
  candidateA,
  candidateB,
  onSelectWinner,
}: ModelComparisonArenaProps) {
  const { theme } = useUnistyles();

  const handleSelectA = useCallback(() => {
    onSelectWinner(candidateA.modelId, candidateA.responseText);
    onClose();
  }, [candidateA, onSelectWinner, onClose]);

  const handleSelectB = useCallback(() => {
    onSelectWinner(candidateB.modelId, candidateB.responseText);
    onClose();
  }, [candidateB, onSelectWinner, onClose]);

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <View style={styles.card}>
          {/* Header */}
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <Scale size={18} color={theme.colors.accent} />
              <View>
                <Text style={styles.titleText}>Dual Model Comparison Arena</Text>
                <Text style={styles.subtitleText} numberOfLines={1}>
                  Prompt: "{prompt}"
                </Text>
              </View>
            </View>
            <Pressable onPress={onClose} style={styles.closeBtn}>
              <X size={18} color={theme.colors.foregroundMuted} />
            </Pressable>
          </View>

          {/* Dual Columns */}
          <View style={styles.arenaBody}>
            {/* Candidate A */}
            <View style={styles.candidateColumn}>
              <View style={styles.columnHeader}>
                <View style={styles.modelTag}>
                  <Text style={styles.modelTagLabel}>MODEL A</Text>
                  <Text style={styles.modelName}>{candidateA.displayName}</Text>
                </View>

                <View style={styles.metricsRow}>
                  <View style={styles.metricItem}>
                    <Clock size={11} color={theme.colors.foregroundMuted} />
                    <Text style={styles.metricVal}>{candidateA.latencyMs}ms</Text>
                  </View>
                  <View style={styles.metricItem}>
                    <Zap size={11} color="#eab308" />
                    <Text style={styles.metricVal}>{candidateA.tokensPerSecond} t/s</Text>
                  </View>
                  <View style={styles.metricItem}>
                    <DollarSign size={11} color={theme.colors.success ?? "#16a34a"} />
                    <Text style={styles.metricVal}>${candidateA.costUsd.toFixed(4)}</Text>
                  </View>
                </View>
              </View>

              <ScrollView style={styles.responseScroll}>
                <Text style={styles.responseText}>{candidateA.responseText}</Text>
              </ScrollView>

              <View style={styles.columnFooter}>
                <Button size="sm" variant="default" onPress={handleSelectA}>
                  <Check size={14} color="#ffffff" />
                  <Text style={styles.acceptBtnText}>Select Model A</Text>
                </Button>
              </View>
            </View>

            {/* Divider */}
            <View style={styles.columnDivider} />

            {/* Candidate B */}
            <View style={styles.candidateColumn}>
              <View style={styles.columnHeader}>
                <View style={styles.modelTag}>
                  <Text style={styles.modelTagLabel}>MODEL B</Text>
                  <Text style={styles.modelName}>{candidateB.displayName}</Text>
                </View>

                <View style={styles.metricsRow}>
                  <View style={styles.metricItem}>
                    <Clock size={11} color={theme.colors.foregroundMuted} />
                    <Text style={styles.metricVal}>{candidateB.latencyMs}ms</Text>
                  </View>
                  <View style={styles.metricItem}>
                    <Zap size={11} color="#eab308" />
                    <Text style={styles.metricVal}>{candidateB.tokensPerSecond} t/s</Text>
                  </View>
                  <View style={styles.metricItem}>
                    <DollarSign size={11} color={theme.colors.success ?? "#16a34a"} />
                    <Text style={styles.metricVal}>${candidateB.costUsd.toFixed(4)}</Text>
                  </View>
                </View>
              </View>

              <ScrollView style={styles.responseScroll}>
                <Text style={styles.responseText}>{candidateB.responseText}</Text>
              </ScrollView>

              <View style={styles.columnFooter}>
                <Button size="sm" variant="default" onPress={handleSelectB}>
                  <Check size={14} color="#ffffff" />
                  <Text style={styles.acceptBtnText}>Select Model B</Text>
                </Button>
              </View>
            </View>
          </View>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.7)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  card: {
    width: "95%",
    maxWidth: 1200,
    height: "85%",
    maxHeight: 800,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
    flexDirection: "column",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    backgroundColor: theme.colors.surface0,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    flex: 1,
  },
  titleText: {
    fontSize: theme.fontSize.base,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  subtitleText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
    maxWidth: 600,
  },
  closeBtn: {
    padding: theme.spacing[1],
  },
  arenaBody: {
    flex: 1,
    flexDirection: "row",
    overflow: "hidden",
  },
  candidateColumn: {
    flex: 1,
    flexDirection: "column",
    backgroundColor: theme.colors.surface1,
  },
  columnDivider: {
    width: 1,
    backgroundColor: theme.colors.border,
  },
  columnHeader: {
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    backgroundColor: theme.colors.surface0,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    gap: theme.spacing[2],
  },
  modelTag: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  modelTagLabel: {
    fontSize: 10,
    fontWeight: "700",
    backgroundColor: theme.colors.surface2,
    color: theme.colors.foregroundMuted,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  modelName: {
    fontSize: theme.fontSize.sm,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  metricsRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[3],
  },
  metricItem: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  metricVal: {
    fontSize: theme.fontSize.code,
    fontFamily: theme.fontFamily.mono,
    color: theme.colors.foregroundMuted,
  },
  responseScroll: {
    flex: 1,
    padding: theme.spacing[4],
  },
  responseText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    lineHeight: 20,
    fontFamily: theme.fontFamily.mono,
  },
  columnFooter: {
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
    alignItems: "center",
  },
  acceptBtnText: {
    color: "#ffffff",
    fontWeight: "600",
  },
}));
