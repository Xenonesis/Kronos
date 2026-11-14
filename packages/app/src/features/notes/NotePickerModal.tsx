import React, { memo, useState, useCallback, useMemo } from "react";
import { View, Text, Modal, ScrollView, Pressable } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { StickyNote, Search, Pin, Plus, ExternalLink, X } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { PressHighlight } from "@/components/ui/press-highlight";
import { Button } from "@/components/ui/button";
import { useNotesStore } from "./use-notes";
import type { NoteRecord } from "./types";

const ThemedStickyNote = withUnistyles(StickyNote, (theme) => ({
  color: theme.colors.accent,
}));
const ThemedSearch = withUnistyles(Search, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedClose = withUnistyles(X, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedPin = withUnistyles(Pin, (theme) => ({
  color: theme.colors.accent,
}));

export interface NotePickerModalProps {
  visible: boolean;
  onClose: () => void;
  onSelectNote: (note: NoteRecord) => void;
  onOpenFullDrawer?: () => void;
}

export const NotePickerModal = memo(function NotePickerModal({
  visible,
  onClose,
  onSelectNote,
  onOpenFullDrawer,
}: NotePickerModalProps) {
  const { notes, formatNoteAsContext, openDrawer } = useNotesStore();
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    if (!query.trim()) return notes;
    const q = query.toLowerCase();
    return notes.filter(
      (n) =>
        n.title.toLowerCase().includes(q) ||
        n.content.toLowerCase().includes(q) ||
        n.tags.some((t) => t.toLowerCase().includes(q)),
    );
  }, [notes, query]);

  const handlePick = useCallback(
    (note: NoteRecord) => {
      onSelectNote(note);
      onClose();
    },
    [onClose, onSelectNote],
  );

  const handleOpenDrawer = useCallback(() => {
    onClose();
    if (onOpenFullDrawer) {
      onOpenFullDrawer();
    } else {
      openDrawer();
    }
  }, [onClose, onOpenFullDrawer, openDrawer]);

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <Pressable style={styles.dismissOverlay} onPress={onClose} />
        <View style={styles.card} testID="note-picker-modal">
          <View style={styles.header}>
            <View style={styles.headerTitleRow}>
              <ThemedStickyNote size={16} />
              <Text style={styles.headerTitle}>Attach Note to Prompt</Text>
            </View>
            <View style={styles.headerActions}>
              <PressHighlight style={styles.drawerBtn} onPress={handleOpenDrawer}>
                <ExternalLink size={12} color="#888888" />
                <Text style={styles.drawerBtnText}>Open Notes</Text>
              </PressHighlight>
              <PressHighlight style={styles.closeBtn} onPress={onClose}>
                <ThemedClose size={16} />
              </PressHighlight>
            </View>
          </View>

          <View style={styles.searchRow}>
            <ThemedSearch size={14} />
            <AdaptiveTextInput
              initialValue={query}
              placeholder="Search notes..."
              onChangeText={setQuery}
              style={styles.searchInput}
            />
          </View>

          <ScrollView style={styles.list} contentContainerStyle={styles.listContent}>
            {filtered.length === 0 ? (
              <View style={styles.empty}>
                <Text style={styles.emptyText}>No notes match query</Text>
              </View>
            ) : (
              filtered.map((note) => (
                <PressHighlight
                  key={note.id}
                  style={styles.item}
                  onPress={() => handlePick(note)}
                >
                  <View style={styles.itemHeader}>
                    <Text style={styles.itemTitle} numberOfLines={1}>
                      {note.title}
                    </Text>
                    {note.pinned ? <ThemedPin size={12} /> : null}
                  </View>
                  <Text style={styles.itemSnippet} numberOfLines={2}>
                    {note.content.replace(/^#+\s+/gm, "").trim() || "(Empty note)"}
                  </Text>
                  {note.tags.length > 0 ? (
                    <View style={styles.tagsRow}>
                      {note.tags.map((tag) => (
                        <Text key={tag} style={styles.tagText}>
                          #{tag}
                        </Text>
                      ))}
                    </View>
                  ) : null}
                </PressHighlight>
              ))
            )}
          </ScrollView>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0,0,0,0.55)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  dismissOverlay: {
    ...StyleSheet.absoluteFillObject,
  },
  card: {
    width: 480,
    maxWidth: "92%",
    maxHeight: 520,
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.lg,
    borderWidth: theme.borderWidth[1],
    borderColor: theme.colors.border,
    shadowColor: "#000000",
    shadowOffset: { width: 0, height: 8 },
    shadowOpacity: 0.4,
    shadowRadius: 16,
    elevation: 12,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: theme.colors.border,
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  headerTitle: {
    fontSize: 14,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  headerActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  drawerBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: theme.colors.surface1,
  },
  drawerBtnText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  closeBtn: {
    padding: 4,
  },
  searchRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface1,
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: theme.colors.border,
  },
  searchInput: {
    flex: 1,
    fontSize: 12,
    color: theme.colors.foreground,
    padding: 0,
  },
  list: {
    maxHeight: 380,
  },
  listContent: {
    padding: theme.spacing[2],
    gap: 4,
  },
  empty: {
    padding: theme.spacing[6],
    alignItems: "center",
  },
  emptyText: {
    fontSize: 12,
    color: theme.colors.foregroundMuted,
  },
  item: {
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.md,
    gap: 4,
  },
  itemHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  itemTitle: {
    fontSize: 13,
    fontWeight: "500",
    color: theme.colors.foreground,
    flex: 1,
  },
  itemSnippet: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    lineHeight: 15,
  },
  tagsRow: {
    flexDirection: "row",
    gap: 6,
    marginTop: 2,
  },
  tagText: {
    fontSize: 10,
    color: theme.colors.accent,
  },
}));
