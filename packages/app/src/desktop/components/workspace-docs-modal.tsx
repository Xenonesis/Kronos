import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { BookOpen, StickyNote, Terminal, Save, Plus, X, ArrowUpRight } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { PressHighlight } from "@/components/ui/press-highlight";
import { invokeTauri } from "@/utils/tauri-invoke";
import { useToast } from "@/contexts/toast-context";
import {
  interpolateWorkflowVariables,
  parseCustomWorkflowFile,
} from "@/client-slash-commands/custom-workflow-loader";
import { useCascadeFlowStore } from "@/stores/cascade-flow-store";

type DocKind = "command" | "notepad" | "playbook";

const ThemedBookOpenIcon = withUnistyles(BookOpen, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedSaveIcon = withUnistyles(Save, (theme) => ({ color: theme.colors.foreground }));
const ThemedMutedStickyIcon = withUnistyles(StickyNote, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedMutedBookIcon = withUnistyles(BookOpen, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedMutedTerminalIcon = withUnistyles(Terminal, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

const DOC_TABS: ReadonlyArray<{ id: DocKind; label: string }> = [
  { id: "notepad", label: "Context Notepads" },
  { id: "playbook", label: "Repo Playbooks" },
  { id: "command", label: "Custom Workflows" },
];

const DocsTabButton = memo(function DocsTabButton({
  id,
  label,
  active,
  onSelect,
}: {
  id: DocKind;
  label: string;
  active: boolean;
  onSelect: (tab: DocKind) => void;
}) {
  const handlePress = useCallback(() => {
    onSelect(id);
  }, [id, onSelect]);

  const renderIcon = () => {
    if (id === "notepad") {
      return active ? (
        <StickyNote size={13} color="#ffffff" />
      ) : (
        <ThemedMutedStickyIcon size={13} />
      );
    }
    if (id === "playbook") {
      return active ? <BookOpen size={13} color="#ffffff" /> : <ThemedMutedBookIcon size={13} />;
    }
    return active ? <Terminal size={13} color="#ffffff" /> : <ThemedMutedTerminalIcon size={13} />;
  };

  return (
    <PressHighlight onPress={handlePress} style={[styles.tab, active && styles.tabActive]}>
      {renderIcon()}
      <Text style={[styles.tabLabel, active && styles.tabLabelActive]}>{label}</Text>
    </PressHighlight>
  );
});

const DocSidebarRow = memo(function DocSidebarRow({
  doc,
  active,
  onSelect,
}: {
  doc: KronosWorkspaceDoc;
  active: boolean;
  onSelect: (doc: KronosWorkspaceDoc) => void;
}) {
  const handlePress = useCallback(() => {
    onSelect(doc);
  }, [doc, onSelect]);

  return (
    <PressHighlight onPress={handlePress} style={[styles.docItem, active && styles.docItemActive]}>
      <Text style={[styles.docName, active && styles.docNameActive]}>{doc.name}.md</Text>
      <Text style={styles.docPath} numberOfLines={1}>
        {doc.relativePath}
      </Text>
    </PressHighlight>
  );
});

const DocsSidebar = memo(function DocsSidebar({
  activeTab,
  newName,
  newNameResetKey,
  filteredDocs,
  selectedPath,
  onChangeNewName,
  onCreateNew,
  onSelectDoc,
}: {
  activeTab: DocKind;
  newName: string;
  newNameResetKey: number;
  filteredDocs: KronosWorkspaceDoc[];
  selectedPath: string;
  onChangeNewName: (text: string) => void;
  onCreateNew: () => void;
  onSelectDoc: (doc: KronosWorkspaceDoc) => void;
}) {
  return (
    <View style={styles.sidebar}>
      {activeTab !== "playbook" && (
        <View style={styles.newRow}>
          <AdaptiveTextInput
            initialValue={newName}
            resetKey={newNameResetKey}
            onChangeText={onChangeNewName}
            placeholder="new-doc-name"
            style={styles.newInput}
          />
          <PressHighlight onPress={onCreateNew} style={styles.addBtn}>
            <Plus size={14} color="#ffffff" />
          </PressHighlight>
        </View>
      )}
      <ScrollView style={styles.docList}>
        {filteredDocs.map((doc) => (
          <DocSidebarRow
            key={doc.relativePath}
            doc={doc}
            active={doc.relativePath === selectedPath}
            onSelect={onSelectDoc}
          />
        ))}
      </ScrollView>
    </View>
  );
});

const DocsEditorPane = memo(function DocsEditorPane({
  selectedPath,
  editorText,
  editorResetKey,
  onChangeText,
  onSave,
  onInject,
}: {
  selectedPath: string;
  editorText: string;
  editorResetKey: number;
  onChangeText: (text: string) => void;
  onSave: () => void;
  onInject: () => void;
}) {
  return (
    <View style={styles.editorPane}>
      <Text style={styles.pathHeader}>{selectedPath}</Text>
      <AdaptiveTextInput
        initialValue={editorText}
        resetKey={editorResetKey}
        onChangeText={onChangeText}
        multiline
        style={styles.editorInput}
      />
      <View style={styles.footer}>
        <Button size="sm" variant="outline" onPress={onSave}>
          <ThemedSaveIcon size={13} />
          <Text style={styles.outlineBtnText}>Save to .kronos</Text>
        </Button>
        <Button size="sm" onPress={onInject}>
          <ArrowUpRight size={13} color="#ffffff" />
          <Text style={styles.primaryBtnText}>Insert into Composer</Text>
        </Button>
      </View>
    </View>
  );
});

interface KronosWorkspaceDoc {
  kind: DocKind;
  name: string;
  relativePath: string;
  content: string;
}

export interface WorkspaceDocsModalProps {
  visible: boolean;
  initialTab?: DocKind;
  cwd: string;
  onClose: () => void;
  onInsertPrompt: (text: string) => void;
}

export const WorkspaceDocsModal = memo(function WorkspaceDocsModal({
  visible,
  initialTab = "notepad",
  cwd,
  onClose,
  onInsertPrompt,
}: WorkspaceDocsModalProps) {
  const toast = useToast();
  const [activeTab, setActiveTab] = useState<DocKind>(initialTab);
  const [docs, setDocs] = useState<KronosWorkspaceDoc[]>([]);
  const [selectedPath, setSelectedPath] = useState<string>(".kronos/playbooks.md");
  const [editorText, setEditorText] = useState("");
  const [editorResetKey, setEditorResetKey] = useState(0);
  const [newName, setNewName] = useState("");
  const [newNameResetKey, setNewNameResetKey] = useState(0);

  const loadDocs = useCallback(async () => {
    try {
      const scanned =
        (await invokeTauri<KronosWorkspaceDoc[]>("scan_kronos_workspace_docs", { cwd })) ?? [];
      setDocs(scanned);
      const match =
        scanned.find((d) => d.kind === activeTab) ??
        (activeTab === "playbook"
          ? {
              kind: "playbook" as const,
              name: "playbooks",
              relativePath: ".kronos/playbooks.md",
              content:
                "# Repository Playbooks\n\n- Build & Typecheck: `npm run typecheck`\n- Rust Unit Tests: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`\n",
            }
          : undefined);
      if (match) {
        setSelectedPath(match.relativePath);
        setEditorText(match.content);
      } else {
        setSelectedPath(
          activeTab === "command"
            ? ".kronos/commands/custom-review.md"
            : ".kronos/notepads/architecture.md",
        );
        setEditorText(
          activeTab === "command"
            ? "---\nname: custom-review\ndescription: Custom workflow\n---\nReview $ACTIVE_FILE:\n$SELECTION"
            : "# Context Notepad\n\nAdd persistent architectural notes to reuse via @notepad.",
        );
      }
      setEditorResetKey((k) => k + 1);
    } catch (err) {
      toast.show(`Failed to scan .kronos docs: ${String(err)}`, { variant: "error" });
    }
  }, [activeTab, cwd, toast]);

  useEffect(() => {
    if (visible) {
      setActiveTab(initialTab);
    }
  }, [visible, initialTab]);

  useEffect(() => {
    if (visible) {
      void loadDocs();
    }
  }, [visible, activeTab, loadDocs]);

  const handleSave = useCallback(async () => {
    try {
      const saved = await invokeTauri<string>("save_kronos_workspace_doc", {
        cwd,
        relativePath: selectedPath,
        content: editorText,
      });
      toast.show(`Saved ${saved ?? selectedPath}`, { variant: "success" });
      await loadDocs();
    } catch (err) {
      toast.show(`Save failed: ${String(err)}`, { variant: "error" });
    }
  }, [cwd, editorText, loadDocs, selectedPath, toast]);

  const handleSavePress = useCallback(() => {
    void handleSave();
  }, [handleSave]);

  const handleCreateNew = useCallback(() => {
    const clean = newName.trim().replace(/[^a-zA-Z0-9_-]/g, "-");
    if (!clean) return;
    const path =
      activeTab === "command" ? `.kronos/commands/${clean}.md` : `.kronos/notepads/${clean}.md`;
    setSelectedPath(path);
    setNewName("");
    setNewNameResetKey((k) => k + 1);
  }, [activeTab, newName]);

  const handleSelectDoc = useCallback((doc: KronosWorkspaceDoc) => {
    setSelectedPath(doc.relativePath);
    setEditorText(doc.content);
    setEditorResetKey((k) => k + 1);
  }, []);

  const handleInjectToComposer = useCallback(() => {
    const activeFile = useCascadeFlowStore.getState().activeFile ?? "";
    if (activeTab === "command") {
      const parsed = parseCustomWorkflowFile(selectedPath, editorText);
      const interpolated = interpolateWorkflowVariables(parsed.template, {
        activeFile,
      });
      onInsertPrompt(interpolated);
    } else {
      onInsertPrompt(
        `<kronos-${activeTab} path="${selectedPath}">\n${editorText}\n</kronos-${activeTab}>`,
      );
    }
    onClose();
  }, [activeTab, editorText, onClose, onInsertPrompt, selectedPath]);

  if (!visible) return null;

  const filteredDocs = docs.filter((d) => d.kind === activeTab);

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop} testID="workspace-docs-modal">
        <View style={styles.card}>
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <ThemedBookOpenIcon size={18} />
              <Text style={styles.title}>Workspace Knowledge, Playbooks & Custom Commands</Text>
            </View>
            <ToolbarButton label="Close" onPress={onClose}>
              <ThemedCloseIcon size={18} />
            </ToolbarButton>
          </View>

          <View style={styles.tabBar}>
            {DOC_TABS.map(({ id, label }) => (
              <DocsTabButton
                key={id}
                id={id}
                label={label}
                active={activeTab === id}
                onSelect={setActiveTab}
              />
            ))}
          </View>

          <View style={styles.body}>
            <DocsSidebar
              activeTab={activeTab}
              newName={newName}
              newNameResetKey={newNameResetKey}
              filteredDocs={filteredDocs}
              selectedPath={selectedPath}
              onChangeNewName={setNewName}
              onCreateNew={handleCreateNew}
              onSelectDoc={handleSelectDoc}
            />

            <DocsEditorPane
              selectedPath={selectedPath}
              editorText={editorText}
              editorResetKey={editorResetKey}
              onChangeText={setEditorText}
              onSave={handleSavePress}
              onInject={handleInjectToComposer}
            />
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
    maxWidth: 860,
    height: "82%",
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
  tabBar: {
    flexDirection: "row",
    gap: 8,
    paddingHorizontal: theme.spacing[4],
    paddingVertical: 10,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
  },
  tab: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: 12,
    paddingVertical: 6,
    borderRadius: 6,
    backgroundColor: theme.colors.surface2,
  },
  tabActive: { backgroundColor: theme.colors.accent },
  tabLabel: { fontSize: 12, fontWeight: "600", color: theme.colors.foregroundMuted },
  tabLabelActive: { color: "#ffffff" },
  body: { flex: 1, flexDirection: "row" },
  sidebar: {
    width: 240,
    borderRightWidth: 1,
    borderRightColor: theme.colors.border,
    padding: 10,
  },
  newRow: { flexDirection: "row", gap: 6, marginBottom: 10 },
  newInput: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 6,
    paddingHorizontal: 8,
    paddingVertical: 4,
    color: theme.colors.foreground,
    fontSize: 12,
  },
  addBtn: {
    backgroundColor: theme.colors.accent,
    borderRadius: 6,
    width: 28,
    alignItems: "center",
    justifyContent: "center",
  },
  docList: { flex: 1 },
  docItem: { padding: 8, borderRadius: 6, marginBottom: 4 },
  docItemActive: { backgroundColor: theme.colors.surface2 },
  docName: { fontSize: 12, fontWeight: "700", color: theme.colors.foreground },
  docNameActive: { color: theme.colors.accent },
  docPath: { fontSize: 10, color: theme.colors.foregroundMuted, fontFamily: "monospace" },
  editorPane: { flex: 1, padding: theme.spacing[4], gap: 10 },
  pathHeader: { fontSize: 12, fontFamily: "monospace", color: theme.colors.foregroundMuted },
  editorInput: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 8,
    padding: 12,
    color: theme.colors.foreground,
    fontFamily: "monospace",
    fontSize: 12,
    textAlignVertical: "top",
  },
  footer: { flexDirection: "row", justifyContent: "flex-end", gap: 10 },
  outlineBtnText: {
    color: theme.colors.foreground,
    fontSize: 12,
    fontWeight: "600",
    marginLeft: 4,
  },
  primaryBtnText: { color: "#ffffff", fontSize: 12, fontWeight: "600", marginLeft: 4 },
}));
