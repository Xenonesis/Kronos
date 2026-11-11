import { memo, useState, useCallback, useMemo } from "react";
import { View, Text, Pressable, ScrollView } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import {
  History,
  RotateCcw,
  GitFork,
  ChevronRight,
  Sparkles,
  Camera,
  CheckCircle2,
} from "lucide-react-native";
import { Button } from "@/components/ui/button";

export interface TurnCheckpoint {
  turnIndex: number;
  id: string;
  timestamp: string;
  userPromptSummary: string;
  filesChangedCount: number;
  additions: number;
  deletions: number;
  commitHash?: string;
}

export interface TurnCheckpointScrubberProps {
  checkpoints: TurnCheckpoint[];
  currentTurnIndex: number;
  onRollbackToTurn: (turnIndex: number) => void;
  onForkFromTurn?: (turnIndex: number) => void;
  onSelectTurnPreview?: (turnIndex: number) => void;
}

export const TurnCheckpointScrubber = memo(function TurnCheckpointScrubber({
  checkpoints,
  currentTurnIndex,
  onRollbackToTurn,
  onForkFromTurn,
  onSelectTurnPreview,
}: TurnCheckpointScrubberProps) {
  const { theme } = useUnistyles();
  const [selectedTurn, setSelectedTurn] = useState<number>(currentTurnIndex);

  const activeCheckpoint = useMemo(() => {
    return (
      checkpoints.find((c) => c.turnIndex === selectedTurn) ?? checkpoints[checkpoints.length - 1]
    );
  }, [checkpoints, selectedTurn]);

  const handleSelect = useCallback(
    (turnIndex: number) => {
      setSelectedTurn(turnIndex);
      onSelectTurnPreview?.(turnIndex);
    },
    [onSelectTurnPreview],
  );

  const handleRollback = useCallback(() => {
    if (activeCheckpoint) {
      onRollbackToTurn(activeCheckpoint.turnIndex);
    }
  }, [activeCheckpoint, onRollbackToTurn]);

  const handleFork = useCallback(() => {
    if (activeCheckpoint && onForkFromTurn) {
      onForkFromTurn(activeCheckpoint.turnIndex);
    }
  }, [activeCheckpoint, onForkFromTurn]);

  if (checkpoints.length <= 1) {
    return null;
  }

  return (
    <View style={styles.container}>
      <View style={styles.header}>
        <View style={styles.headerTitleRow}>
          <History size={15} color={theme.colors.accent} />
          <Text style={styles.headerTitle}>Turn Checkpoint Timeline</Text>
          <Text style={styles.turnCountBadge}>{checkpoints.length} turns</Text>
        </View>

        {activeCheckpoint && activeCheckpoint.turnIndex < currentTurnIndex && (
          <View style={styles.rollbackActions}>
            {onForkFromTurn && (
              <Button size="sm" variant="outline" onPress={handleFork}>
                <GitFork size={13} color={theme.colors.foreground} />
                <Text style={styles.forkBtnText}>Fork Branch</Text>
              </Button>
            )}
            <Button size="sm" variant="destructive" onPress={handleRollback}>
              <RotateCcw size={13} color="#ffffff" />
              <Text style={styles.rollbackBtnText}>
                Rollback to Turn #{activeCheckpoint.turnIndex}
              </Text>
            </Button>
          </View>
        )}
      </View>

      {/* Horizontal Scrubber Strip */}
      <ScrollView horizontal showsHorizontalScrollIndicator={false} style={styles.scrubberScroll}>
        <View style={styles.track}>
          {checkpoints.map((cp, idx) => {
            const isSelected = cp.turnIndex === selectedTurn;
            const isLatest = cp.turnIndex === currentTurnIndex;
            return (
              <Pressable
                key={cp.id}
                onPress={() => handleSelect(cp.turnIndex)}
                style={[
                  styles.nodeCard,
                  isSelected && styles.nodeCardSelected,
                  isLatest && styles.nodeCardLatest,
                ]}
              >
                <View style={styles.nodeHeader}>
                  <View style={styles.turnBadge}>
                    <Camera
                      size={11}
                      color={isSelected ? theme.colors.accent : theme.colors.foregroundMuted}
                    />
                    <Text
                      style={[styles.turnBadgeText, isSelected && styles.turnBadgeTextSelected]}
                    >
                      #{cp.turnIndex}
                    </Text>
                  </View>
                  {isLatest && (
                    <View style={styles.latestPill}>
                      <Text style={styles.latestText}>HEAD</Text>
                    </View>
                  )}
                </View>

                <Text style={styles.promptSummary} numberOfLines={1}>
                  {cp.userPromptSummary || `Turn ${cp.turnIndex}`}
                </Text>

                <View style={styles.statsRow}>
                  <Text style={styles.addText}>+{cp.additions}</Text>
                  <Text style={styles.delText}>-{cp.deletions}</Text>
                  {cp.filesChangedCount > 0 && (
                    <Text style={styles.filesText}>({cp.filesChangedCount}f)</Text>
                  )}
                </View>
              </Pressable>
            );
          })}
        </View>
      </ScrollView>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    backgroundColor: theme.colors.surface0,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    paddingVertical: theme.spacing[2],
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[3],
    paddingBottom: theme.spacing[2],
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  headerTitle: {
    fontSize: theme.fontSize.sm,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  turnCountBadge: {
    fontSize: theme.fontSize.code,
    backgroundColor: theme.colors.surface2,
    color: theme.colors.foregroundMuted,
    paddingHorizontal: 6,
    paddingVertical: 1,
    borderRadius: 4,
  },
  rollbackActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  forkBtnText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foreground,
    fontWeight: "500",
  },
  rollbackBtnText: {
    fontSize: theme.fontSize.code,
    color: "#ffffff",
    fontWeight: "600",
  },
  scrubberScroll: {
    paddingHorizontal: theme.spacing[3],
  },
  track: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  nodeCard: {
    width: 150,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[2],
    gap: 4,
  },
  nodeCardSelected: {
    borderColor: theme.colors.accent,
    backgroundColor: theme.colors.surface2,
  },
  nodeCardLatest: {
    borderStyle: "dashed",
  },
  nodeHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  turnBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  turnBadgeText: {
    fontSize: theme.fontSize.code,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
  },
  turnBadgeTextSelected: {
    color: theme.colors.accent,
  },
  latestPill: {
    backgroundColor: theme.colors.accent,
    paddingHorizontal: 4,
    paddingVertical: 1,
    borderRadius: 3,
  },
  latestText: {
    fontSize: 9,
    color: "#ffffff",
    fontWeight: "700",
  },
  promptSummary: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foreground,
    lineHeight: 16,
  },
  statsRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    marginTop: 2,
  },
  addText: {
    fontSize: 10,
    fontWeight: "600",
    color: theme.colors.success ?? "#16a34a",
  },
  delText: {
    fontSize: 10,
    fontWeight: "600",
    color: theme.colors.destructive ?? "#dc2626",
  },
  filesText: {
    fontSize: 10,
    color: theme.colors.foregroundMuted,
  },
}));
