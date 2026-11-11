import { memo, useState, useCallback } from "react";
import { View, Text, Modal, Pressable, TextInput, ScrollView } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { BrainCircuit, X, Plus, Trash2, Tag, BookMarked } from "lucide-react-native";
import { Button } from "@/components/ui/button";

export interface MemoryItem {
  id: string;
  category: string;
  content: string;
}

export interface MemoriesManagementDialogProps {
  visible: boolean;
  onClose: () => void;
  workspacePath: string;
  initialMemories?: MemoryItem[];
  onSave?: (memories: MemoryItem[]) => void;
}

const DEFAULT_MEMORIES: MemoryItem[] = [
  {
    id: "mem-1",
    category: "Architecture",
    content: "All desktop integrations must run native in Rust (zero bundled Node in installer)",
  },
  {
    id: "mem-2",
    category: "Code Style",
    content: "Use numeric theme.spacing[1..6] in Unistyles; never use sm/md/lg spacing names",
  },
  {
    id: "mem-3",
    category: "Conventions",
    content: "Always run npm run typecheck and npx oxlint before finalizing agent changes",
  },
];

export const MemoriesManagementDialog = memo(function MemoriesManagementDialog({
  visible,
  onClose,
  workspacePath,
  initialMemories = DEFAULT_MEMORIES,
  onSave,
}: MemoriesManagementDialogProps) {
  const { theme } = useUnistyles();
  const [memories, setMemories] = useState<MemoryItem[]>(initialMemories);
  const [newCategory, setNewCategory] = useState("Conventions");
  const [newContent, setNewContent] = useState("");

  const handleAddMemory = useCallback(() => {
    if (!newContent.trim()) return;
    const item: MemoryItem = {
      id: `mem-${Date.now()}`,
      category: newCategory.trim() || "General",
      content: newContent.trim(),
    };
    const next = [...memories, item];
    setMemories(next);
    setNewContent("");
    onSave?.(next);
  }, [newContent, newCategory, memories, onSave]);

  const handleDeleteMemory = useCallback(
    (id: string) => {
      const next = memories.filter((m) => m.id !== id);
      setMemories(next);
      onSave?.(next);
    },
    [memories, onSave],
  );

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <View style={styles.card}>
          {/* Header */}
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <BrainCircuit size={18} color={theme.colors.accent} />
              <View>
                <Text style={styles.titleText}>Project Memories (.kronos/memories.md)</Text>
                <Text style={styles.subtitleText} numberOfLines={1}>
                  {workspacePath}
                </Text>
              </View>
            </View>
            <Pressable onPress={onClose} style={styles.closeBtn}>
              <X size={18} color={theme.colors.foregroundMuted} />
            </Pressable>
          </View>

          {/* Add New Memory Row */}
          <View style={styles.addRow}>
            <View style={styles.categoryInputContainer}>
              <Tag size={13} color={theme.colors.foregroundMuted} />
              <TextInput
                value={newCategory}
                onChangeText={setNewCategory}
                placeholder="Category"
                placeholderTextColor={theme.colors.foregroundMuted}
                style={styles.categoryInput}
              />
            </View>
            <TextInput
              value={newContent}
              onChangeText={setNewContent}
              placeholder="e.g. Always use pnpm, never import React from react..."
              placeholderTextColor={theme.colors.foregroundMuted}
              style={styles.contentInput}
            />
            <Button size="sm" variant="default" onPress={handleAddMemory}>
              <Plus size={14} color="#ffffff" />
              <Text style={styles.addBtnText}>Add</Text>
            </Button>
          </View>

          {/* Memory List */}
          <ScrollView style={styles.list}>
            {memories.length === 0 ? (
              <View style={styles.emptyState}>
                <BookMarked size={28} color={theme.colors.foregroundMuted} />
                <Text style={styles.emptyStateText}>No memories recorded yet.</Text>
                <Text style={styles.emptyStateSub}>
                  Preferences added here are remembered across all agent chats.
                </Text>
              </View>
            ) : (
              memories.map((m) => (
                <View key={m.id} style={styles.memoryRow}>
                  <View style={styles.memoryInfo}>
                    <View style={styles.badge}>
                      <Text style={styles.badgeText}>{m.category}</Text>
                    </View>
                    <Text style={styles.memoryText}>{m.content}</Text>
                  </View>
                  <Pressable
                    onPress={() => handleDeleteMemory(m.id)}
                    style={styles.deleteBtn}
                    accessibilityLabel="Delete memory"
                  >
                    <Trash2 size={14} color="#dc2626" />
                  </Pressable>
                </View>
              ))
            )}
          </ScrollView>

          {/* Footer */}
          <View style={styles.footer}>
            <Text style={styles.footerHint}>
              Auto-injected into Kronos agent instructions under # Learned Memories.
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
