import { memo, useState, useCallback, useEffect } from "react";
import { View, Text, ScrollView, Modal } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Brain, Plus, Trash2, X, BookMarked, Tag } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { invokeTauri } from "@/utils/tauri-invoke";
import { useToast } from "@/contexts/toast-context";

const ThemedBrainIcon = withUnistyles(Brain, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedTagIcon = withUnistyles(Tag, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedBookIcon = withUnistyles(BookMarked, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedTrashIcon = withUnistyles(Trash2, () => ({ color: "#dc2626" }));
const ThemedPlusIcon = withUnistyles(Plus, () => ({ color: "#ffffff" }));

export interface MemoryItem {
  id: string;
  category: string;
  content: string;
  createdAt: string;
}

export interface MemoriesManagementDialogProps {
  visible: boolean;
  cwd: string;
  onClose: () => void;
}

const MemoryRow = memo(function MemoryRow({
  item,
  onDelete,
}: {
  item: MemoryItem;
  onDelete: (id: string) => void;
}) {
  const handleDelete = useCallback(() => {
    onDelete(item.id);
  }, [item.id, onDelete]);

  return (
    <View style={styles.memoryRow}>
      <View style={styles.memoryInfo}>
        <View style={styles.badge}>
          <Text style={styles.badgeText}>{item.category}</Text>
        </View>
        <Text style={styles.memoryText}>{item.content}</Text>
      </View>
      <ToolbarButton label="Delete memory" onPress={handleDelete} style={styles.deleteBtn}>
        <ThemedTrashIcon size={14} />
      </ToolbarButton>
    </View>
  );
});

function MemoriesListBody({
  isLoading,
  memories,
  onDelete,
}: {
  isLoading: boolean;
  memories: MemoryItem[];
  onDelete: (id: string) => void;
}) {
  if (isLoading) {
    return (
      <View style={styles.emptyState}>
        <Text style={styles.emptyStateText}>Loading .kronos/memories.md...</Text>
      </View>
    );
  }
  if (memories.length === 0) {
    return (
      <View style={styles.emptyState}>
        <ThemedBookIcon size={28} />
        <Text style={styles.emptyStateText}>No memories recorded yet.</Text>
        <Text style={styles.emptyStateSub}>
          Preferences added here are saved to .kronos/memories.md and injected into agent runs.
        </Text>
      </View>
    );
  }
  return (
    <>
      {memories.map((m) => (
        <MemoryRow key={m.id} item={m} onDelete={onDelete} />
      ))}
    </>
  );
}

export const MemoriesManagementDialog = memo(function MemoriesManagementDialog({
  visible,
  cwd,
  onClose,
}: MemoriesManagementDialogProps) {
  const toast = useToast();
  const [memories, setMemories] = useState<MemoryItem[]>([]);
  const [newCategory, setNewCategory] = useState("Code Style");
  const [newContent, setNewContent] = useState("");
  const [inputResetKey, setInputResetKey] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    if (!visible || !cwd) return;
    let cancelled = false;
    async function load() {
      setIsLoading(true);
      try {
        const loaded = await invokeTauri<MemoryItem[]>("load_workspace_memories", { cwd });
        if (!cancelled) {
          setMemories(Array.isArray(loaded) ? loaded : []);
        }
      } catch (err) {
        if (!cancelled) {
          toast.show(`Failed to load .kronos/memories.md: ${String(err)}`, { variant: "error" });
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [visible, cwd, toast]);

  const persistMemories = useCallback(
    async (updated: MemoryItem[]) => {
      try {
        await invokeTauri<void>("save_workspace_memories", { cwd, entries: updated });
      } catch (err) {
        toast.show(`Failed to persist .kronos/memories.md: ${String(err)}`, { variant: "error" });
      }
    },
    [cwd, toast],
  );

  const handleAddMemory = useCallback(() => {
    if (!newContent.trim()) return;
    const item: MemoryItem = {
      id: `mem_${Date.now()}`,
      category: newCategory.trim() || "General",
      content: newContent.trim(),
      createdAt: new Date().toISOString(),
    };
    const updated = [...memories, item];
    setMemories(updated);
    setNewContent("");
    setInputResetKey((k) => k + 1);
    void persistMemories(updated);
  }, [newCategory, newContent, memories, persistMemories]);

  const handleDeleteMemory = useCallback(
    (id: string) => {
      const updated = memories.filter((m) => m.id !== id);
      setMemories(updated);
      void persistMemories(updated);
    },
    [memories, persistMemories],
  );

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <View style={styles.card}>
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <ThemedBrainIcon size={18} />
              <View>
                <Text style={styles.titleText}>Project Memories (.kronos/memories.md)</Text>
                <Text style={styles.subtitleText}>
                  {memories.length} persistent rules active for agents
                </Text>
              </View>
            </View>
            <ToolbarButton label="Close dialog" onPress={onClose} style={styles.closeBtn}>
              <ThemedCloseIcon size={18} />
            </ToolbarButton>
          </View>

          <View style={styles.addRow}>
            <View style={styles.categoryInputContainer}>
              <ThemedTagIcon size={12} />
              <AdaptiveTextInput
                initialValue={newCategory}
                onChangeText={setNewCategory}
                placeholder="Category..."
                style={styles.categoryInput}
              />
            </View>
            <AdaptiveTextInput
              initialValue={newContent}
              resetKey={inputResetKey}
              onChangeText={setNewContent}
              placeholder="Add coding rule, preferred library, or architectural constraint..."
              style={styles.contentInput}
              onSubmitEditing={handleAddMemory}
            />
            <Button size="sm" variant="default" onPress={handleAddMemory}>
              <ThemedPlusIcon size={14} />
              <Text style={styles.addBtnText}>Add</Text>
            </Button>
          </View>

          <ScrollView style={styles.list}>
            <MemoriesListBody
              isLoading={isLoading}
              memories={memories}
              onDelete={handleDeleteMemory}
            />
          </ScrollView>

          <View style={styles.footer}>
            <Text style={styles.footerHint}>
              Persisted in .kronos/memories.md and injected into every agent prompt.
            </Text>
            <Button size="sm" variant="outline" onPress={onClose}>
              Done
            </Button>
          </View>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.65)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  card: {
    width: "90%",
    maxWidth: 680,
    maxHeight: 600,
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
  },
  closeBtn: {
    padding: theme.spacing[1],
  },
  addRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[2],
    gap: theme.spacing[2],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
  },
  categoryInputContainer: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
    width: 120,
  },
  categoryInput: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foreground,
    padding: 0,
    flex: 1,
  },
  contentInput: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
  },
  addBtnText: {
    color: "#ffffff",
    fontWeight: "600",
  },
  list: {
    maxHeight: 360,
    padding: theme.spacing[3],
  },
  memoryRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingVertical: theme.spacing[2],
    paddingHorizontal: theme.spacing[3],
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: theme.colors.border,
    marginBottom: theme.spacing[2],
  },
  memoryInfo: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  badge: {
    backgroundColor: "rgba(59, 130, 246, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  badgeText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.accent,
    fontWeight: "600",
  },
  memoryText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    flex: 1,
  },
  deleteBtn: {
    padding: theme.spacing[1],
  },
  emptyState: {
    alignItems: "center",
    justifyContent: "center",
    paddingVertical: theme.spacing[6],
    gap: theme.spacing[1],
  },
  emptyStateText: {
    fontSize: theme.fontSize.sm,
    fontWeight: "500",
    color: theme.colors.foreground,
  },
  emptyStateSub: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
    textAlign: "center",
  },
  footer: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  footerHint: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
    flex: 1,
  },
}));
