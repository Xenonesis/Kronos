import { memo, useState, useCallback, useMemo, useEffect } from "react";
import { View, Text, Pressable, Modal, TextInput, ScrollView } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Check, X, FileCode, CheckCheck, Search, Sparkles, ShieldAlert } from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { DiffViewer } from "@/components/diff-viewer";
import { buildLineDiff, type DiffLine } from "@/utils/tool-call-parsers";
import { playTaskSuccessSound } from "@/audio/audio-suite";
import { isWeb } from "@/constants/platform";

export interface ReviewFileEntry {
  path: string;
  oldText?: string;
  newText?: string;
  diffLines?: DiffLine[];
  additions?: number;
  deletions?: number;
  status?: "modified" | "added" | "deleted";
}

export interface MultiFileDiffReviewProps {
  visible: boolean;
  onClose: () => void;
  files: ReviewFileEntry[];
  onAcceptAll: () => void;
  onRejectAll: () => void;
  onAcceptSelected?: (selectedPaths: string[]) => void;
  title?: string;
}

export const MultiFileDiffReview = memo(function MultiFileDiffReview({
  visible,
  onClose,
  files,
  onAcceptAll,
  onRejectAll,
  onAcceptSelected,
  title = "Agent Review: Multi-File Diff",
}: MultiFileDiffReviewProps) {
  const { theme } = useUnistyles();
  const [selectedFileIndex, setSelectedFileIndex] = useState(0);
  const [searchFilter, setSearchFilter] = useState("");
  const [checkedFiles, setCheckedFiles] = useState<Record<string, boolean>>(() => {
    const initial: Record<string, boolean> = {};
    for (const f of files) {
      initial[f.path] = true;
    }
    return initial;
  });

  // Re-sync checked state when files change
  useEffect(() => {
    const nextChecked: Record<string, boolean> = {};
    for (const f of files) {
      nextChecked[f.path] = true;
    }
    setCheckedFiles(nextChecked);
    setSelectedFileIndex(0);
  }, [files]);

  const filteredFiles = useMemo(() => {
    if (!searchFilter.trim()) {
      return files;
    }
    const q = searchFilter.toLowerCase();
    return files.filter((f) => f.path.toLowerCase().includes(q));
  }, [files, searchFilter]);

  const currentFile = filteredFiles[selectedFileIndex] ?? filteredFiles[0];

  const currentDiffLines = useMemo((): DiffLine[] => {
    if (!currentFile) {
      return [];
    }
    if (currentFile.diffLines && currentFile.diffLines.length > 0) {
      return currentFile.diffLines;
    }
    if (currentFile.oldText !== undefined && currentFile.newText !== undefined) {
      return buildLineDiff(currentFile.oldText, currentFile.newText);
    }
    return [];
  }, [currentFile]);

  const toggleCheck = useCallback((filePath: string) => {
    setCheckedFiles((prev) => ({
      ...prev,
      [filePath]: !prev[filePath],
    }));
  }, []);

  const toggleAllChecked = useCallback(() => {
    setCheckedFiles((prev) => {
      const allChecked = files.every((f) => prev[f.path]);
      const nextChecked: Record<string, boolean> = {};
      for (const f of files) {
        nextChecked[f.path] = !allChecked;
      }
      return nextChecked;
    });
  }, [files]);

  const handleAcceptAll = useCallback(() => {
    playTaskSuccessSound();
    onAcceptAll();
    onClose();
  }, [onAcceptAll, onClose]);

  const handleAcceptSelected = useCallback(() => {
    const selected = files.filter((f) => checkedFiles[f.path]).map((f) => f.path);
    if (selected.length === 0) {
      return;
    }
    playTaskSuccessSound();
    if (onAcceptSelected) {
      onAcceptSelected(selected);
    } else {
      onAcceptAll();
    }
    onClose();
  }, [files, checkedFiles, onAcceptSelected, onAcceptAll, onClose]);

  const handleRejectAll = useCallback(() => {
    onRejectAll();
    onClose();
  }, [onRejectAll, onClose]);

  // Keyboard shortcut listener: Ctrl+Enter to Fast Apply, Escape to Close
  useEffect(() => {
    if (!visible || !isWeb) {
      return;
    }
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        handleAcceptAll();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [visible, handleAcceptAll, onClose]);

  const totalAdditions = useMemo(() => {
    return files.reduce((acc: number, f: ReviewFileEntry) => {
      if (f.additions !== undefined) return acc + f.additions;
      const lines: DiffLine[] =
        f.diffLines ??
        (f.oldText !== undefined && f.newText !== undefined
          ? buildLineDiff(f.oldText, f.newText)
          : []);
      return acc + lines.filter((l: DiffLine) => l.type === "add").length;
    }, 0);
  }, [files]);

  const totalDeletions = useMemo(() => {
    return files.reduce((acc: number, f: ReviewFileEntry) => {
      if (f.deletions !== undefined) return acc + f.deletions;
      const lines: DiffLine[] =
        f.diffLines ??
        (f.oldText !== undefined && f.newText !== undefined
          ? buildLineDiff(f.oldText, f.newText)
          : []);
      return acc + lines.filter((l: DiffLine) => l.type === "remove").length;
    }, 0);
  }, [files]);

  if (!visible) {
    return null;
  }

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <View style={styles.modalCard}>
          {/* Header */}
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <Sparkles size={18} color={theme.colors.accent} />
              <Text style={styles.titleText}>{title}</Text>
              <View style={styles.statsBadge}>
                <Text style={styles.additionStat}>+{totalAdditions}</Text>
                <Text style={styles.deletionStat}>-{totalDeletions}</Text>
                <Text style={styles.fileCountStat}>
                  ({files.length} {files.length === 1 ? "file" : "files"})
                </Text>
              </View>
            </View>
            <Pressable
              onPress={onClose}
              style={styles.closeBtn}
              accessibilityLabel="Close diff modal"
            >
              <X size={18} color={theme.colors.foregroundMuted} />
            </Pressable>
          </View>

          {/* Main Dual-Pane Body */}
          <View style={styles.body}>
            {/* Sidebar File List */}
            <View style={styles.sidebar}>
              <View style={styles.searchBox}>
                <Search size={14} color={theme.colors.foregroundMuted} />
                <TextInput
                  value={searchFilter}
                  onChangeText={setSearchFilter}
                  placeholder="Filter changed files..."
                  placeholderTextColor={theme.colors.foregroundMuted}
                  style={styles.searchInput}
                />
              </View>

              <View style={styles.selectionBar}>
                <Pressable onPress={toggleAllChecked} style={styles.selectAllBtn}>
                  <Text style={styles.selectAllText}>Toggle Select All</Text>
                </Pressable>
              </View>

              <ScrollView style={styles.fileList}>
                {filteredFiles.map((f, idx) => {
                  const isSelected = f.path === currentFile?.path;
                  const isChecked = checkedFiles[f.path] !== false;
                  return (
                    <Pressable
                      key={f.path}
                      onPress={() => setSelectedFileIndex(idx)}
                      style={[styles.fileItem, isSelected && styles.fileItemSelected]}
                    >
                      <Pressable
                        onPress={() => toggleCheck(f.path)}
                        style={[styles.checkbox, isChecked && styles.checkboxChecked]}
                        accessibilityLabel={`Select ${f.path}`}
                      >
                        {isChecked && <Check size={12} color="#ffffff" />}
                      </Pressable>
                      <FileCode size={14} color={theme.colors.foregroundMuted} />
                      <Text
                        style={[styles.filePathText, isSelected && styles.filePathSelected]}
                        numberOfLines={1}
                      >
                        {f.path.split(/[/\\]/).pop() ?? f.path}
                      </Text>
                      <View style={styles.fileStatsPill}>
                        {f.additions ? <Text style={styles.fileAdd}>+{f.additions}</Text> : null}
                        {f.deletions ? <Text style={styles.fileDel}>-{f.deletions}</Text> : null}
                      </View>
                    </Pressable>
                  );
                })}
              </ScrollView>
            </View>

            {/* Right Pane: Diff Content */}
            <View style={styles.diffPane}>
              {currentFile ? (
                <>
                  <View style={styles.diffPaneHeader}>
                    <Text style={styles.currentFilePath} numberOfLines={1}>
                      {currentFile.path}
                    </Text>
                    <View style={styles.fastApplyHint}>
                      <Text style={styles.shortcutKey}>Ctrl+Enter</Text>
                      <Text style={styles.shortcutLabel}>Fast Apply All</Text>
                    </View>
                  </View>
                  <View style={styles.diffViewerContainer}>
                    <DiffViewer diffLines={currentDiffLines} fillAvailableHeight />
                  </View>
                </>
              ) : (
                <View style={styles.emptyDiffState}>
                  <ShieldAlert size={32} color={theme.colors.foregroundMuted} />
                  <Text style={styles.emptyDiffText}>No files matching search filter.</Text>
                </View>
              )}
            </View>
          </View>

          {/* Footer Actions */}
          <View style={styles.footer}>
            <View style={styles.footerLeft}>
              <Button size="sm" variant="ghost" onPress={handleRejectAll}>
                <X size={14} color={theme.colors.destructive} />
                <Text style={styles.rejectBtnText}>Reject All</Text>
              </Button>
            </View>
            <View style={styles.footerRight}>
              <Button size="sm" variant="outline" onPress={handleAcceptSelected}>
                <Check size={14} color={theme.colors.foreground} />
                <Text style={styles.actionBtnText}>Accept Selected</Text>
              </Button>
              <Button size="sm" variant="default" onPress={handleAcceptAll}>
                <CheckCheck size={14} color="#ffffff" />
                <Text style={styles.fastApplyBtnText}>Accept All (Fast Apply)</Text>
              </Button>
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
    backgroundColor: "rgba(0, 0, 0, 0.65)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  modalCard: {
    width: "92%",
    maxWidth: 1100,
    height: "85%",
    maxHeight: 780,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 10 },
    shadowOpacity: 0.35,
    shadowRadius: 20,
    overflow: "hidden",
    flexDirection: "column",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  titleText: {
    fontSize: theme.fontSize.base,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  statsBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 2,
    borderRadius: theme.borderRadius.sm,
    marginLeft: theme.spacing[2],
  },
  additionStat: {
    fontSize: theme.fontSize.code,
    fontWeight: "600",
    color: theme.colors.success ?? "#16a34a",
  },
  deletionStat: {
    fontSize: theme.fontSize.code,
    fontWeight: "600",
    color: theme.colors.destructive ?? "#dc2626",
  },
  fileCountStat: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  closeBtn: {
    padding: theme.spacing[1],
    borderRadius: theme.borderRadius.sm,
  },
  body: {
    flex: 1,
    flexDirection: "row",
    overflow: "hidden",
  },
  sidebar: {
    width: 320,
    borderRightWidth: 1,
    borderRightColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
    flexDirection: "column",
  },
  searchBox: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    gap: theme.spacing[2],
  },
  searchInput: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    padding: 0,
  },
  selectionBar: {
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[1],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  selectAllBtn: {
    paddingVertical: 2,
  },
  selectAllText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.accent,
    fontWeight: "500",
  },
  fileList: {
    flex: 1,
  },
  fileItem: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    gap: theme.spacing[2],
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: theme.colors.border,
  },
  fileItemSelected: {
    backgroundColor: theme.colors.surface2,
  },
  checkbox: {
    width: 16,
    height: 16,
    borderRadius: 3,
    borderWidth: 1,
    borderColor: theme.colors.borderAccent,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: theme.colors.surface1,
  },
  checkboxChecked: {
    backgroundColor: theme.colors.accent,
    borderColor: theme.colors.accent,
  },
  filePathText: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
  },
  filePathSelected: {
    fontWeight: "600",
    color: theme.colors.accent,
  },
  fileStatsPill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  fileAdd: {
    fontSize: theme.fontSize.code,
    color: theme.colors.success ?? "#16a34a",
    fontWeight: "600",
  },
  fileDel: {
    fontSize: theme.fontSize.code,
    color: theme.colors.destructive ?? "#dc2626",
    fontWeight: "600",
  },
  diffPane: {
    flex: 1,
    backgroundColor: theme.colors.surface1,
    flexDirection: "column",
  },
  diffPaneHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[2],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
  },
  currentFilePath: {
    fontSize: theme.fontSize.sm,
    fontWeight: "500",
    color: theme.colors.foreground,
    flex: 1,
  },
  fastApplyHint: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  shortcutKey: {
    fontSize: theme.fontSize.code,
    backgroundColor: theme.colors.surface3,
    color: theme.colors.foregroundMuted,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
    fontWeight: "600",
  },
  shortcutLabel: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  diffViewerContainer: {
    flex: 1,
  },
  emptyDiffState: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: theme.spacing[2],
  },
  emptyDiffText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  footer: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
  },
  footerLeft: {
    flexDirection: "row",
    alignItems: "center",
  },
  footerRight: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  rejectBtnText: {
    color: theme.colors.destructive,
    fontWeight: "500",
  },
  actionBtnText: {
    color: theme.colors.foreground,
    fontWeight: "500",
  },
  fastApplyBtnText: {
    color: "#ffffff",
    fontWeight: "600",
  },
}));
