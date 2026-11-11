import React, { memo, useState } from "react";
import { View, Text, Pressable, ScrollView } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { GitFork, Network, Layers, ArrowRight, Sparkles, X, Box } from "lucide-react-native";
import { useSessionStore } from "@/stores/session-store";
import { useDraftStore } from "@/stores/draft-store";
import { useToast } from "@/contexts/toast-context";

export interface DependencyNode {
  id: string;
  name: string;
  path: string;
  type: "entry" | "module" | "utility" | "store";
  dependencies: string[];
  callers: string[];
  exports: string[];
}

export interface VisualDependencyGraphModalProps {
  visible: boolean;
  onClose: () => void;
  nodes?: DependencyNode[];
}

const DEFAULT_GRAPH_NODES: DependencyNode[] = [
  {
    id: "app-entry",
    name: "App Root",
    path: "packages/app/src/app/index.tsx",
    type: "entry",
    dependencies: ["session-store", "editor-view", "terminal-pane"],
    callers: [],
    exports: ["PaseoDesktopApp", "rootRoute"],
  },
  {
    id: "session-store",
    name: "Session Store",
    path: "packages/app/src/stores/session-store.ts",
    type: "store",
    dependencies: ["protocol-client"],
    callers: ["app-entry", "agent-panel", "composer"],
    exports: ["useSessionStore", "AgentSession", "FocusedAgent"],
  },
  {
    id: "editor-view",
    name: "CodeMirror Editor",
    path: "packages/app/src/file-panels/source/editor/view.web.tsx",
    type: "module",
    dependencies: ["ghost-text-provider", "inline-codelens"],
    callers: ["app-entry", "file-pane"],
    exports: ["FileEditorView", "editorBaseExtensions"],
  },
  {
    id: "ghost-text-provider",
    name: "Ghost Text Engine",
    path: "packages/app/src/editor/ghost-text-provider.ts",
    type: "utility",
    dependencies: [],
    callers: ["editor-view"],
    exports: ["generatePredictiveGhostText", "GhostTextMatch"],
  },
  {
    id: "protocol-client",
    name: "Desktop Rust IPC",
    path: "packages/desktop-tauri/src-tauri/src/agent/runner.rs",
    type: "module",
    dependencies: [],
    callers: ["session-store"],
    exports: ["DesktopAgentRunner", "execute_agent_turn"],
  },
];

export const VisualDependencyGraphModal = memo(function VisualDependencyGraphModal({
  visible,
  onClose,
  nodes = DEFAULT_GRAPH_NODES,
}: VisualDependencyGraphModalProps) {
  const { theme } = useUnistyles();
  const toast = useToast();
  const [selectedNodeId, setSelectedNodeId] = useState<string>(nodes[0]?.id ?? "app-entry");

  if (!visible) return null;

  const selectedNode = nodes.find((n) => n.id === selectedNodeId) || nodes[0];

  const handleAddGraphToContext = () => {
    const sessions = useSessionStore.getState().sessions;
    const serverIds = Object.keys(sessions);
    const serverId = serverIds[0];
    const session = serverId ? sessions[serverId] : undefined;
    const activeAgent = session?.agents ? Array.from(session.agents.values())[0] : undefined;

    if (!serverId || !activeAgent) {
      toast.show("No active agent available to add context", { variant: "error" });
      return;
    }

    const draftKey = `agent:${serverId}:${activeAgent.id}`;
    const contextPrompt =
      `### 🕸️ Visual Architecture Context: \`${selectedNode.name}\`\n` +
      `File: \`${selectedNode.path}\`\n` +
      `Exports: ${selectedNode.exports.join(", ")}\n` +
      `Dependencies: ${selectedNode.dependencies.join(", ") || "None"}\n` +
      `Callers: ${selectedNode.callers.join(", ") || "None"}\n\n` +
      `Please keep this architectural sub-graph and module interfaces in mind during refactoring.`;

    useDraftStore.getState().replaceDraftText({ draftKey, text: contextPrompt });
    toast.show("Architecture sub-graph added to composer context", { variant: "success" });
    onClose();
  };

  return (
    <View style={styles.overlay}>
      <View style={styles.modal}>
        {/* Header */}
        <View style={styles.header}>
          <View style={styles.headerTitleRow}>
            <Network size={20} color={theme.colors.accent} />
            <Text style={styles.headerTitle}>Module Dependency & Architecture Graph</Text>
          </View>
          <Pressable onPress={onClose} style={styles.closeButton}>
            <X size={18} color={theme.colors.foregroundMuted} />
          </Pressable>
        </View>

        <View style={styles.contentBody}>
          {/* Nodes Map List */}
          <ScrollView style={styles.nodesColumn}>
            <Text style={styles.columnTitle}>Workspace Modules ({nodes.length})</Text>
            {nodes.map((node) => {
              const isSelected = node.id === selectedNode.id;
              return (
                <Pressable
                  key={node.id}
                  onPress={() => setSelectedNodeId(node.id)}
                  style={[styles.nodeCard, isSelected && styles.nodeCardSelected]}
                >
                  <View style={styles.nodeHeader}>
                    <Box
                      size={14}
                      color={isSelected ? theme.colors.accent : theme.colors.foregroundMuted}
                    />
                    <Text style={[styles.nodeName, isSelected && styles.nodeNameSelected]}>
                      {node.name}
                    </Text>
                  </View>
                  <Text style={styles.nodePath} numberOfLines={1}>
                    {node.path}
                  </Text>
                </Pressable>
              );
            })}
          </ScrollView>

          {/* Selected Node Details & Connections */}
          <View style={styles.detailsColumn}>
            <View style={styles.detailsHeader}>
              <Text style={styles.detailsTitle}>{selectedNode.name}</Text>
              <Text style={styles.detailsPath}>{selectedNode.path}</Text>
            </View>

            <View style={styles.connectionsBlock}>
              <Text style={styles.sectionLabel}>
                Outbound Dependencies ({selectedNode.dependencies.length})
              </Text>
              <View style={styles.badgeWrap}>
                {selectedNode.dependencies.length > 0 ? (
                  selectedNode.dependencies.map((dep, idx) => (
                    <View key={idx} style={styles.depBadge}>
                      <ArrowRight size={10} color={theme.colors.accent} />
                      <Text style={styles.depText}>{dep}</Text>
                    </View>
                  ))
                ) : (
                  <Text style={styles.emptyText}>No outbound dependencies</Text>
                )}
              </View>

              <Text style={[styles.sectionLabel, { marginTop: 12 }]}>
                Inbound Callers ({selectedNode.callers.length})
              </Text>
              <View style={styles.badgeWrap}>
                {selectedNode.callers.length > 0 ? (
                  selectedNode.callers.map((cal, idx) => (
                    <View key={idx} style={styles.calBadge}>
                      <Text style={styles.calText}>{cal}</Text>
                    </View>
                  ))
                ) : (
                  <Text style={styles.emptyText}>Root module / No inbound callers</Text>
                )}
              </View>

              <Text style={[styles.sectionLabel, { marginTop: 12 }]}>
                Exported Symbols ({selectedNode.exports.length})
              </Text>
              <View style={styles.badgeWrap}>
                {selectedNode.exports.map((exp, idx) => (
                  <View key={idx} style={styles.expBadge}>
                    <Text style={styles.expText}>{exp}</Text>
                  </View>
                ))}
              </View>
            </View>

            <View style={styles.detailsFooter}>
              <Pressable onPress={handleAddGraphToContext} style={styles.addContextButton}>
                <Sparkles size={14} color="#ffffff" />
                <Text style={styles.addContextButtonText}>Add Sub-Graph to Context</Text>
              </Pressable>
            </View>
          </View>
        </View>
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  overlay: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: "rgba(0,0,0,0.7)",
    justifyContent: "center",
    alignItems: "center",
    zIndex: 1200,
  },
  modal: {
    width: "90%",
    maxWidth: 720,
    maxHeight: "85%",
    backgroundColor: theme.colors.surface0,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: 20,
    boxShadow: "0 10px 30px rgba(0,0,0,0.5)",
  },
  header: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: 16,
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  headerTitle: {
    fontSize: 16,
    fontWeight: "700",
    color: theme.colors.foreground,
  },
  closeButton: {
    padding: 6,
  },
  contentBody: {
    flexDirection: "row",
    gap: 16,
    maxHeight: 460,
  },
  nodesColumn: {
    width: "40%",
  },
  columnTitle: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
    marginBottom: 8,
  },
  nodeCard: {
    backgroundColor: theme.colors.surface1,
    borderRadius: 6,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: 8,
    marginBottom: 6,
  },
  nodeCardSelected: {
    borderColor: theme.colors.accent,
    backgroundColor: "rgba(99, 102, 241, 0.1)",
  },
  nodeHeader: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    marginBottom: 2,
  },
  nodeName: {
    fontSize: 12,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  nodeNameSelected: {
    color: theme.colors.accent,
  },
  nodePath: {
    fontSize: 10,
    fontFamily: "monospace",
    color: theme.colors.foregroundMuted,
  },
  detailsColumn: {
    flex: 1,
    backgroundColor: theme.colors.surface1,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: 14,
    justifyContent: "space-between",
  },
  detailsHeader: {
    marginBottom: 12,
  },
  detailsTitle: {
    fontSize: 15,
    fontWeight: "700",
    color: theme.colors.foreground,
  },
  detailsPath: {
    fontSize: 11,
    fontFamily: "monospace",
    color: theme.colors.foregroundMuted,
    marginTop: 2,
  },
  connectionsBlock: {
    flex: 1,
  },
  sectionLabel: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
    marginBottom: 6,
  },
  badgeWrap: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 6,
  },
  depBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: "rgba(99, 102, 241, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 3,
    borderRadius: 4,
  },
  depText: {
    fontSize: 11,
    color: theme.colors.accent,
    fontFamily: "monospace",
  },
  calBadge: {
    backgroundColor: "rgba(16, 185, 129, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 3,
    borderRadius: 4,
  },
  calText: {
    fontSize: 11,
    color: "#10b981",
    fontFamily: "monospace",
  },
  expBadge: {
    backgroundColor: "rgba(0,0,0,0.2)",
    paddingHorizontal: 6,
    paddingVertical: 3,
    borderRadius: 4,
  },
  expText: {
    fontSize: 11,
    color: theme.colors.foreground,
    fontFamily: "monospace",
  },
  emptyText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    fontStyle: "italic",
  },
  detailsFooter: {
    flexDirection: "row",
    justifyContent: "flex-end",
    paddingTop: 12,
  },
  addContextButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    backgroundColor: theme.colors.accent,
    paddingHorizontal: 14,
    paddingVertical: 8,
    borderRadius: 6,
  },
  addContextButtonText: {
    color: "#ffffff",
    fontSize: 12,
    fontWeight: "600",
  },
}));
