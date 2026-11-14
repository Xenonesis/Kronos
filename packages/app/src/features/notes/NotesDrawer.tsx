import React, { memo, useState, useCallback, useEffect, useMemo, useRef } from "react";
import {
  View,
  Text,
  Modal,
  ScrollView,
  Pressable,
  type NativeSyntheticEvent,
  type TextInputSelectionChangeEventData,
} from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import * as Clipboard from "expo-clipboard";
import {
  StickyNote,
  Plus,
  Search,
  Pin,
  PinOff,
  Trash2,
  Copy,
  ArrowUpRight,
  X,
  Tag,
  Check,
} from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { PressHighlight } from "@/components/ui/press-highlight";
import { useToast } from "@/contexts/toast-context";
import { useNotesStore } from "./use-notes";
import type { NoteRecord } from "./types";

const ThemedStickyNote = withUnistyles(StickyNote, (theme) => ({
  color: theme.colors.accent,
}));
const ThemedMutedNote = withUnistyles(StickyNote, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedSearchIcon = withUnistyles(Search, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedPinIcon = withUnistyles(Pin, (theme) => ({
  color: theme.colors.accent,
}));
const ThemedMutedPin = withUnistyles(Pin, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedTrash = withUnistyles(Trash2, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

export interface NotesDrawerProps {
  onInsertToChat?: (content: string) => void;
}

export const NotesDrawer = memo(function NotesDrawer({ onInsertToChat }: NotesDrawerProps) {
  const toast = useToast();
  const {
    notes,
    selectedNoteId,
    isDrawerOpen,
    isLoading,
    searchQuery,
    pinnedOnly,
    closeDrawer,
    selectNote,
    setSearchQuery,
    setPinnedOnly,
    loadNotes,
    saveNote,
    deleteNote,
    togglePin,
    formatNoteAsContext,
  } = useNotesStore();

  const [activeTitle, setActiveTitle] = useState("");
  const [activeContent, setActiveContent] = useState("");
  const [activeTags, setActiveTags] = useState<string[]>([]);
  const [newTagInput, setNewTagInput] = useState("");
  const [copiedNoteId, setCopiedNoteId] = useState<string | null>(null);

  // Load notes on mount
  useEffect(() => {
    void loadNotes();
  }, [loadNotes]);

  // Global keyboard shortcuts (Ctrl+Shift+N / Cmd+Shift+N) and custom event listeners
  useEffect(() => {
    if (typeof window === "undefined") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+Shift+N or Cmd+Shift+N
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === "n" || e.key === "N")) {
        e.preventDefault();
        useNotesStore.getState().toggleDrawer();
        return;
      }
      // Esc when drawer is open
      if (e.key === "Escape" && isDrawerOpen) {
        e.preventDefault();
        closeDrawer();
        return;
      }
    };

    const handleOpenEvent = (e: Event) => {
      const custom = e as CustomEvent<{ noteId?: string }>;
      useNotesStore.getState().openDrawer(custom.detail?.noteId);
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("kronos:open-notes", handleOpenEvent);
    window.addEventListener("kronos://open-notes", handleOpenEvent);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("kronos:open-notes", handleOpenEvent);
      window.removeEventListener("kronos://open-notes", handleOpenEvent);
    };
  }, [closeDrawer, isDrawerOpen]);

  const selectedNote = useMemo(
    () => notes.find((n) => n.id === selectedNoteId) ?? notes[0] ?? null,
    [notes, selectedNoteId],
  );

  // Sync editor buffer when selected note changes
  useEffect(() => {
    if (selectedNote) {
      setActiveTitle(selectedNote.title);
      setActiveContent(selectedNote.content);
      setActiveTags(selectedNote.tags);
    } else {
      setActiveTitle("");
      setActiveContent("");
      setActiveTags([]);
    }
  }, [selectedNote]);

  // Handle auto-saving changes
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const handleContentChange = useCallback(
    (newContent: string) => {
      setActiveContent(newContent);
      if (!selectedNote) return;
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = setTimeout(() => {
        void saveNote({
          id: selectedNote.id,
          title: activeTitle,
          content: newContent,
          tags: activeTags,
          pinned: selectedNote.pinned,
        });
      }, 400);
    },
    [activeTags, activeTitle, saveNote, selectedNote],
  );

  const handleTitleChange = useCallback(
    (newTitle: string) => {
      setActiveTitle(newTitle);
      if (!selectedNote) return;
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = setTimeout(() => {
        void saveNote({
          id: selectedNote.id,
          title: newTitle,
          content: activeContent,
          tags: activeTags,
          pinned: selectedNote.pinned,
        });
      }, 400);
    },
    [activeContent, activeTags, saveNote, selectedNote],
  );

  const handleCreateNew = useCallback(async () => {
    const newNote = await saveNote({
      title: "Untitled Note",
      content: "",
      tags: [],
      pinned: false,
    });
    selectNote(newNote.id);
  }, [saveNote, selectNote]);

  const handleDelete = useCallback(async () => {
    if (!selectedNote) return;
    await deleteNote(selectedNote.id);
    toast.show("Note deleted");
  }, [deleteNote, selectedNote, toast]);

  const handleTogglePin = useCallback(async () => {
    if (!selectedNote) return;
    await togglePin(selectedNote.id);
  }, [selectedNote, togglePin]);

  const handleAddTag = useCallback(() => {
    const trimmed = newTagInput.trim().toLowerCase();
    if (!trimmed || activeTags.includes(trimmed) || !selectedNote) return;
    const nextTags = [...activeTags, trimmed];
    setActiveTags(nextTags);
    setNewTagInput("");
    void saveNote({
      id: selectedNote.id,
      title: activeTitle,
      content: activeContent,
      tags: nextTags,
      pinned: selectedNote.pinned,
    });
  }, [activeContent, activeTags, activeTitle, newTagInput, saveNote, selectedNote]);

  const handleRemoveTag = useCallback(
    (tagToRemove: string) => {
      if (!selectedNote) return;
      const nextTags = activeTags.filter((t) => t !== tagToRemove);
      setActiveTags(nextTags);
      void saveNote({
        id: selectedNote.id,
        title: activeTitle,
        content: activeContent,
        tags: nextTags,
        pinned: selectedNote.pinned,
      });
    },
    [activeContent, activeTags, activeTitle, saveNote, selectedNote],
  );

  const handleCopyAsContext = useCallback(async () => {
    if (!selectedNote) return;
    const formatted = formatNoteAsContext({
      ...selectedNote,
      title: activeTitle,
      content: activeContent,
      tags: activeTags,
    });
    await Clipboard.setStringAsync(formatted);
    setCopiedNoteId(selectedNote.id);
    setTimeout(() => setCopiedNoteId(null), 1800);
    toast.show(`Copied "${selectedNote.title}" as context`);
  }, [activeContent, activeTags, activeTitle, formatNoteAsContext, selectedNote, toast]);

  const handleInsertIntoChat = useCallback(() => {
    if (!selectedNote) return;
    const formatted = formatNoteAsContext({
      ...selectedNote,
      title: activeTitle,
      content: activeContent,
      tags: activeTags,
    });
    if (onInsertToChat) {
      onInsertToChat(formatted);
    }
    // Also dispatch global event so any active composer can catch it
    if (typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent("kronos:insert-composer", {
          detail: { text: formatted },
        }),
      );
    }
    closeDrawer();
    toast.show(`Inserted note into chat prompt`);
  }, [
    activeContent,
    activeTags,
    activeTitle,
    closeDrawer,
    formatNoteAsContext,
    onInsertToChat,
    selectedNote,
    toast,
  ]);

  // Filter notes by search & pin filter
  const filteredNotes = useMemo(() => {
    return notes.filter((n) => {
      if (pinnedOnly && !n.pinned) return false;
      if (searchQuery.trim().length > 0) {
        const query = searchQuery.toLowerCase();
        const matchesTitle = n.title.toLowerCase().includes(query);
        const matchesContent = n.content.toLowerCase().includes(query);
        const matchesTags = n.tags.some((t) => t.toLowerCase().includes(query));
        return matchesTitle || matchesContent || matchesTags;
      }
      return true;
    });
  }, [notes, pinnedOnly, searchQuery]);

  if (!isDrawerOpen) return null;

  return (
    <Modal visible={isDrawerOpen} transparent animationType="fade" onRequestClose={closeDrawer}>
      <View style={styles.backdrop} testID="notes-drawer-modal">
        <Pressable style={styles.backdropPressable} onPress={closeDrawer} />
        <View style={styles.drawerCard}>
          {/* Drawer Top Header */}
          <View style={styles.topHeader}>
            <View style={styles.titleRow}>
              <ThemedStickyNote size={18} />
              <Text style={styles.topTitle}>Notes & Scratchpad</Text>
              <View style={styles.badge}>
                <Text style={styles.badgeText}>{notes.length}</Text>
              </View>
            </View>
            <View style={styles.headerRight}>
              <Button size="sm" variant="outline" onPress={handleCreateNew}>
                <Plus size={14} color="#ffffff" />
                <Text style={styles.newBtnText}>New Note</Text>
              </Button>
              <ToolbarButton label="Close" onPress={closeDrawer}>
                <ThemedCloseIcon size={18} />
              </ToolbarButton>
            </View>
          </View>

          {/* Drawer Body - Split Left / Right */}
          <View style={styles.bodySplit}>
            {/* Left Column: Note List & Search */}
            <View style={styles.listColumn}>
              <View style={styles.searchRow}>
                <ThemedSearchIcon size={14} />
                <AdaptiveTextInput
                  initialValue={searchQuery}
                  placeholder="Search notes or tags..."
                  onChangeText={setSearchQuery}
                  style={styles.searchInput}
                />
              </View>

              <View style={styles.filterRow}>
                <PressHighlight
                  style={[styles.filterChip, !pinnedOnly && styles.filterChipActive]}
                  onPress={() => setPinnedOnly(false)}
                >
                  <Text style={[styles.filterChipText, !pinnedOnly && styles.filterChipTextActive]}>
                    All ({notes.length})
                  </Text>
                </PressHighlight>
                <PressHighlight
                  style={[styles.filterChip, pinnedOnly && styles.filterChipActive]}
                  onPress={() => setPinnedOnly(true)}
                >
                  <Text style={[styles.filterChipText, pinnedOnly && styles.filterChipTextActive]}>
                    📌 Pinned ({notes.filter((n) => n.pinned).length})
                  </Text>
                </PressHighlight>
              </View>

              <ScrollView style={styles.notesListScroll} contentContainerStyle={styles.notesListContent}>
                {filteredNotes.length === 0 ? (
                  <View style={styles.emptyContainer}>
                    <Text style={styles.emptyText}>No notes found</Text>
                  </View>
                ) : (
                  filteredNotes.map((note) => {
                    const isSelected = selectedNote?.id === note.id;
                    return (
                      <PressHighlight
                        key={note.id}
                        onPress={() => selectNote(note.id)}
                        style={[styles.noteItem, isSelected && styles.noteItemActive]}
                      >
                        <View style={styles.noteItemHeader}>
                          <Text
                            style={[
                              styles.noteItemTitle,
                              isSelected && styles.noteItemTitleActive,
                            ]}
                            numberOfLines={1}
                          >
                            {note.title || "Untitled Note"}
                          </Text>
                          {note.pinned ? <ThemedPinIcon size={12} /> : null}
                        </View>
                        <Text style={styles.noteItemSnippet} numberOfLines={2}>
                          {note.content.replace(/^#+\s+/gm, "").trim() || "(Empty note)"}
                        </Text>
                        {note.tags.length > 0 ? (
                          <View style={styles.noteItemTags}>
                            {note.tags.slice(0, 3).map((tag) => (
                              <View key={tag} style={styles.miniTag}>
                                <Text style={styles.miniTagText}>#{tag}</Text>
                              </View>
                            ))}
                          </View>
                        ) : null}
                      </PressHighlight>
                    );
                  })
                )}
              </ScrollView>
            </View>

            {/* Right Column: Note Editor & Actions */}
            <View style={styles.editorColumn}>
              {selectedNote ? (
                <>
                  <View style={styles.editorHeader}>
                    <AdaptiveTextInput
                      initialValue={activeTitle}
                      placeholder="Note Title..."
                      onChangeText={handleTitleChange}
                      style={styles.titleInput}
                    />
                    <View style={styles.editorToolbar}>
                      <PressHighlight
                        style={styles.iconButton}
                        onPress={handleTogglePin}
                        accessibilityLabel="Pin note"
                      >
                        {selectedNote.pinned ? (
                          <ThemedPinIcon size={16} />
                        ) : (
                          <ThemedMutedPin size={16} />
                        )}
                      </PressHighlight>
                      <PressHighlight
                        style={styles.iconButton}
                        onPress={handleDelete}
                        accessibilityLabel="Delete note"
                      >
                        <ThemedTrash size={16} />
                      </PressHighlight>
                    </View>
                  </View>

                  {/* Tags editing row */}
                  <View style={styles.tagsContainer}>
                    <Tag size={12} color="#888888" />
                    {activeTags.map((tag) => (
                      <View key={tag} style={styles.tagPill}>
                        <Text style={styles.tagPillText}>#{tag}</Text>
                        <Pressable onPress={() => handleRemoveTag(tag)}>
                          <X size={10} color="#aaaaaa" />
                        </Pressable>
                      </View>
                    ))}
                    <AdaptiveTextInput
                      initialValue={newTagInput}
                      placeholder="+ tag"
                      onChangeText={setNewTagInput}
                      onSubmitEditing={handleAddTag}
                      style={styles.addTagInput}
                    />
                  </View>

                  {/* Main Markdown Content Area */}
                  <AdaptiveTextInput
                    initialValue={activeContent}
                    placeholder="Write markdown notes, code snippets, or task lists..."
                    onChangeText={handleContentChange}
                    multiline
                    style={styles.contentEditor}
                  />

                  {/* Footer Actions */}
                  <View style={styles.editorFooter}>
                    <View style={styles.footerLeft}>
                      <Text style={styles.updatedAtText}>
                        Updated{" "}
                        {new Date(selectedNote.updatedAtMs).toLocaleTimeString([], {
                          hour: "2-digit",
                          minute: "2-digit",
                        })}
                      </Text>
                    </View>
                    <View style={styles.footerButtons}>
                      <Button size="sm" variant="outline" onPress={handleCopyAsContext}>
                        {copiedNoteId === selectedNote.id ? (
                          <Check size={14} color="#10b981" />
                        ) : (
                          <Copy size={14} />
                        )}
                        <Text style={styles.btnText}>
                          {copiedNoteId === selectedNote.id ? "Copied!" : "Copy as Context"}
                        </Text>
                      </Button>
                      <Button size="sm" onPress={handleInsertIntoChat}>
                        <ArrowUpRight size={14} color="#ffffff" />
                        <Text style={styles.primaryBtnText}>Insert into Chat</Text>
                      </Button>
                    </View>
                  </View>
                </>
              ) : (
                <View style={styles.noNoteSelected}>
                  <ThemedMutedNote size={36} />
                  <Text style={styles.noNoteText}>Select or create a note to start editing</Text>
                  <Button size="sm" variant="outline" onPress={handleCreateNew}>
                    <Plus size={14} />
                    <Text style={styles.btnText}>Create Note</Text>
                  </Button>
                </View>
              )}
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
    backgroundColor: "rgba(0,0,0,0.65)",
    flexDirection: "row",
    justifyContent: "flex-end",
  },
  backdropPressable: {
    flex: 1,
  },
  drawerCard: {
    width: 760,
    maxWidth: "92%",
    height: "100%",
    backgroundColor: theme.colors.surface0,
    borderLeftWidth: theme.borderWidth[1],
    borderLeftColor: theme.colors.border,
    display: "flex",
    flexDirection: "column",
    shadowColor: "#000000",
    shadowOffset: { width: -4, height: 0 },
    shadowOpacity: 0.35,
    shadowRadius: 16,
    elevation: 20,
  },
  topHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: theme.colors.border,
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  topTitle: {
    fontSize: 15,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  badge: {
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.full,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 1,
  },
  badgeText: {
    fontSize: 11,
    fontWeight: "500",
    color: theme.colors.foregroundMuted,
  },
  headerRight: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  newBtnText: {
    fontSize: 12,
    fontWeight: "500",
    color: "#ffffff",
  },
  bodySplit: {
    flex: 1,
    flexDirection: "row",
    minHeight: 0,
  },
  listColumn: {
    width: 250,
    borderRightWidth: theme.borderWidth[1],
    borderRightColor: theme.colors.border,
    backgroundColor: theme.colors.surface1,
    display: "flex",
    flexDirection: "column",
  },
  searchRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: theme.colors.border,
  },
  searchInput: {
    flex: 1,
    fontSize: 12,
    color: theme.colors.foreground,
    padding: 0,
  },
  filterRow: {
    flexDirection: "row",
    gap: theme.spacing[1],
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: theme.colors.border,
  },
  filterChip: {
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 3,
    borderRadius: theme.borderRadius.md,
    backgroundColor: "transparent",
  },
  filterChipActive: {
    backgroundColor: theme.colors.surface2,
  },
  filterChipText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  filterChipTextActive: {
    color: theme.colors.foreground,
    fontWeight: "600",
  },
  notesListScroll: {
    flex: 1,
  },
  notesListContent: {
    padding: theme.spacing[2],
    gap: theme.spacing[1],
  },
  emptyContainer: {
    padding: theme.spacing[6],
    alignItems: "center",
  },
  emptyText: {
    fontSize: 12,
    color: theme.colors.foregroundMuted,
  },
  noteItem: {
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.md,
    backgroundColor: "transparent",
    gap: 4,
  },
  noteItemActive: {
    backgroundColor: theme.colors.surface2,
  },
  noteItemHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  noteItemTitle: {
    flex: 1,
    fontSize: 13,
    fontWeight: "500",
    color: theme.colors.foreground,
  },
  noteItemTitleActive: {
    color: theme.colors.accent,
    fontWeight: "600",
  },
  noteItemSnippet: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    lineHeight: 15,
  },
  noteItemTags: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 4,
    marginTop: 2,
  },
  miniTag: {
    backgroundColor: theme.colors.surface0,
    borderRadius: 4,
    paddingHorizontal: 4,
    paddingVertical: 1,
  },
  miniTagText: {
    fontSize: 10,
    color: theme.colors.foregroundMuted,
  },
  editorColumn: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    backgroundColor: theme.colors.surface0,
    padding: theme.spacing[4],
  },
  editorHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: theme.spacing[2],
  },
  titleInput: {
    flex: 1,
    fontSize: 18,
    fontWeight: "600",
    color: theme.colors.foreground,
    padding: 0,
  },
  editorToolbar: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  iconButton: {
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.sm,
  },
  tagsContainer: {
    flexDirection: "row",
    alignItems: "center",
    flexWrap: "wrap",
    gap: 6,
    paddingBottom: theme.spacing[2],
    borderBottomWidth: theme.borderWidth[1],
    borderBottomColor: theme.colors.border,
    marginBottom: theme.spacing[3],
  },
  tagPill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.sm,
    paddingHorizontal: 6,
    paddingVertical: 2,
  },
  tagPillText: {
    fontSize: 11,
    color: theme.colors.foreground,
  },
  addTagInput: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    padding: 0,
    minWidth: 50,
  },
  contentEditor: {
    flex: 1,
    fontSize: 13,
    lineHeight: 20,
    color: theme.colors.foreground,
    textAlignVertical: "top",
    padding: 0,
  },
  editorFooter: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingTop: theme.spacing[3],
    borderTopWidth: theme.borderWidth[1],
    borderTopColor: theme.colors.border,
    marginTop: theme.spacing[2],
  },
  footerLeft: {
    flexDirection: "row",
    alignItems: "center",
  },
  updatedAtText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  footerButtons: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  btnText: {
    fontSize: 12,
    color: theme.colors.foreground,
  },
  primaryBtnText: {
    fontSize: 12,
    fontWeight: "500",
    color: "#ffffff",
  },
  noNoteSelected: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: theme.spacing[3],
  },
  noNoteText: {
    fontSize: 13,
    color: theme.colors.foregroundMuted,
  },
}));
