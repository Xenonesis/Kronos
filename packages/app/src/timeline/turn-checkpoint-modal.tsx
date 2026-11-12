import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { History, Camera, RotateCcw, X, Loader2 } from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { TurnCheckpointScrubber, type TurnCheckpoint } from "./turn-checkpoint-scrubber";
import { invokeTauri } from "@/utils/tauri-invoke";
import { useToast } from "@/contexts/toast-context";

const ThemedHistoryIcon = withUnistyles(History, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedLoaderIcon = withUnistyles(Loader2, (theme) => ({ color: theme.colors.accent }));

interface CheckpointRowProps {
  cp: TurnCheckpoint;
  onRollback: (turnId: string) => void;
}

const CheckpointRow = memo(function CheckpointRow({ cp, onRollback }: CheckpointRowProps) {
  const handlePress = useCallback(() => {
    onRollback(cp.id);
  }, [cp.id, onRollback]);

  return (
    <View style={styles.row}>
      <View style={styles.rowMeta}>
        <Text style={styles.rowId}>#{cp.id}</Text>
        <Text style={styles.rowSummary}>{cp.userPromptSummary}</Text>
        {cp.timestamp ? <Text style={styles.rowTime}>{cp.timestamp}</Text> : null}
      </View>
      <Button size="sm" variant="destructive" onPress={handlePress}>
        <RotateCcw size={13} color="#ffffff" />
        <Text style={styles.primaryBtnText}>Rewind</Text>
      </Button>
    </View>
  );
});

interface BackendCheckpointInfo {
  turnId: string;
  refName: string;
  timestamp: string;
  summary: string;
}

export interface TurnCheckpointModalProps {
  visible: boolean;
  cwd: string;
  onClose: () => void;
}

export const TurnCheckpointModal = memo(function TurnCheckpointModal({
  visible,
  cwd,
  onClose,
}: TurnCheckpointModalProps) {
  const toast = useToast();
  const [checkpoints, setCheckpoints] = useState<TurnCheckpoint[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const loadCheckpoints = useCallback(async () => {
    setIsLoading(true);
    try {
      const items = await invokeTauri<BackendCheckpointInfo[]>("list_turn_checkpoints", {
        repoPath: cwd,
      });
      if (items && items.length > 0) {
        setCheckpoints(
          items.map((item, idx) => ({
            turnIndex: items.length - idx,
            id: item.turnId,
            timestamp: item.timestamp,
            userPromptSummary: item.summary || item.turnId,
            filesChangedCount: 0,
            additions: 0,
            deletions: 0,
          })),
        );
      } else {
        setCheckpoints([]);
      }
    } catch (err) {
      toast.show(`Failed to load checkpoints: ${String(err)}`, { variant: "error" });
    } finally {
      setIsLoading(false);
    }
  }, [cwd, toast]);

  useEffect(() => {
    if (visible) {
      void loadCheckpoints();
    }
  }, [visible, loadCheckpoints]);

  const handleCaptureSnapshot = useCallback(async () => {
    setIsLoading(true);
    try {
      const turnId = `turn-${Date.now()}`;
      await invokeTauri<string>("create_turn_checkpoint", { repoPath: cwd, turnId });
      toast.show(`Captured checkpoint ${turnId}`, { variant: "success" });
      await loadCheckpoints();
    } catch (err) {
      toast.show(`Failed to create checkpoint: ${String(err)}`, { variant: "error" });
      setIsLoading(false);
    }
  }, [cwd, loadCheckpoints, toast]);

  const handleRollbackByTurnId = useCallback(
    async (turnId: string) => {
      setIsLoading(true);
      try {
        await invokeTauri<void>("restore_turn_checkpoint", { repoPath: cwd, turnId });
        toast.show(`Rolled back workspace to checkpoint ${turnId}`, { variant: "success" });
        onClose();
      } catch (err) {
        toast.show(`Rollback failed: ${String(err)}`, { variant: "error" });
      } finally {
        setIsLoading(false);
      }
    },
    [cwd, onClose, toast],
  );

  const handleScrubberRollback = useCallback(
    (turnIndex: number) => {
      const match = checkpoints.find((c) => c.turnIndex === turnIndex);
      if (match) {
        void handleRollbackByTurnId(match.id);
      }
    },
    [checkpoints, handleRollbackByTurnId],
  );

  if (!visible) return null;

  const renderCheckpointBody = () => {
    if (isLoading) {
      return (
        <View style={styles.center}>
          <ThemedLoaderIcon size={20} />
          <Text style={styles.mutedText}>Inspecting git shadow checkpoints...</Text>
        </View>
      );
    }
    if (checkpoints.length === 0) {
      return (
        <View style={styles.center}>
          <Text style={styles.mutedText}>
            No shadow checkpoints captured yet in this repository. Click &ldquo;Capture
            Checkpoint&rdquo; to snapshot working tree state before risky refactors.
          </Text>
        </View>
      );
    }
    return checkpoints.map((cp) => (
      <CheckpointRow key={cp.id} cp={cp} onRollback={handleRollbackByTurnId} />
    ));
  };

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop} testID="turn-checkpoint-modal">
        <View style={styles.card}>
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <ThemedHistoryIcon size={18} />
              <Text style={styles.title}>Timeline Checkpoints & Instant Rollback</Text>
            </View>
            <View style={styles.headerActions}>
              <Button size="sm" onPress={handleCaptureSnapshot} disabled={isLoading}>
                <Camera size={13} color="#ffffff" />
                <Text style={styles.primaryBtnText}>Capture Checkpoint</Text>
              </Button>
              <ToolbarButton label="Close" onPress={onClose}>
                <ThemedCloseIcon size={18} />
              </ToolbarButton>
            </View>
          </View>

          <TurnCheckpointScrubber
            checkpoints={checkpoints}
            currentTurnIndex={checkpoints[0]?.turnIndex ?? 1}
            onRollbackToTurn={handleScrubberRollback}
          />

          <ScrollView style={styles.list}>{renderCheckpointBody()}</ScrollView>
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
    maxWidth: 680,
    maxHeight: "82%",
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
  headerActions: { flexDirection: "row", alignItems: "center", gap: 10 },
  closeBtn: { padding: 4 },
  primaryBtnText: { color: "#ffffff", fontWeight: "600", fontSize: 12, marginLeft: 4 },
  list: { padding: theme.spacing[4] },
  center: { padding: 24, alignItems: "center", gap: 8 },
  mutedText: { color: theme.colors.foregroundMuted, fontSize: 13, textAlign: "center" },
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    padding: 12,
    borderRadius: theme.borderRadius.md,
    backgroundColor: theme.colors.surface2,
    borderWidth: 1,
    borderColor: theme.colors.border,
    marginBottom: 8,
  },
  rowMeta: { flex: 1, marginRight: 12, gap: 2 },
  rowId: { fontSize: 12, fontWeight: "700", color: theme.colors.accent },
  rowSummary: { fontSize: 13, color: theme.colors.foreground },
  rowTime: { fontSize: 11, color: theme.colors.foregroundMuted },
}));
