import React, { memo, useCallback, useEffect, useState } from "react";
import { View, Text, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Network, ArrowRight, Sparkles, X, Box } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { Button } from "@/components/ui/button";
import { useSessionStore } from "@/stores/session-store";
import { useDraftStore } from "@/stores/draft-store";
import { useToast } from "@/contexts/toast-context";
import { invokeTauri } from "@/utils/tauri-invoke";

const ThemedNetworkIcon = withUnistyles(Network, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedBoxActiveIcon = withUnistyles(Box, (theme) => ({ color: theme.colors.accent }));
const ThemedBoxMutedIcon = withUnistyles(Box, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedArrowRightIcon = withUnistyles(ArrowRight, (theme) => ({ color: theme.colors.accent }));
const ThemedSparklesIcon = withUnistyles(Sparkles, () => ({ color: "#ffffff" }));

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
  cwd?: string;
  serverId?: string;
  agentId?: string;
  nodes?: DependencyNode[];
}

const ModuleNodeItem = memo(function ModuleNodeItem({
  node,
  isSelected,
  onSelect,
}: {
  node: DependencyNode;
  isSelected: boolean;
  onSelect: (id: string) => void;
}) {
  const handlePress = useCallback(() => {
    onSelect(node.id);
  }, [node.id, onSelect]);

  return (
    <PressHighlight
      onPress={handlePress}
      style={[styles.nodeCard, isSelected && styles.nodeCardSelected]}
    >
      <View style={styles.nodeHeader}>
        {isSelected ? <ThemedBoxActiveIcon size={14} /> : <ThemedBoxMutedIcon size={14} />}
        <Text style={[styles.nodeName, isSelected && styles.nodeNameSelected]}>{node.name}</Text>
      </View>
      <Text style={styles.nodePath} numberOfLines={1}>
        {node.path}
      </Text>
    </PressHighlight>
  );
});

const NodeDetailsPanel = memo(function NodeDetailsPanel({
  node,
  onAddContext,
}: {
  node: DependencyNode;
  onAddContext: () => void;
}) {
  return (
    <View style={styles.detailsColumn}>
      <View style={styles.detailsHeader}>
        <Text style={styles.detailsTitle}>{node.name}</Text>
        <Text style={styles.detailsPath}>{node.path}</Text>
      </View>

      <View style={styles.connectionsBlock}>
        <Text style={styles.sectionLabel}>Outbound Dependencies ({node.dependencies.length})</Text>
        <View style={styles.badgeWrap}>
          {node.dependencies.length > 0 ? (
            node.dependencies.map((dep) => (
              <View key={dep} style={styles.depBadge}>
                <ThemedArrowRightIcon size={10} />
                <Text style={styles.depText}>{dep}</Text>
              </View>
            ))
          ) : (
            <Text style={styles.emptyText}>No outbound dependencies</Text>
          )}
        </View>

        <Text style={styles.sectionLabelSpaced}>Inbound Callers ({node.callers.length})</Text>
        <View style={styles.badgeWrap}>
          {node.callers.length > 0 ? (
            node.callers.map((cal) => (
              <View key={cal} style={styles.calBadge}>
                <Text style={styles.calText}>{cal}</Text>
              </View>
            ))
          ) : (
            <Text style={styles.emptyText}>Root module / No inbound callers</Text>
          )}
        </View>

        <Text style={styles.sectionLabelSpaced}>Exported Symbols ({node.exports.length})</Text>
        <View style={styles.badgeWrap}>
          {node.exports.length > 0 ? (
            node.exports.map((exp) => (
              <View key={exp} style={styles.expBadge}>
                <Text style={styles.expText}>{exp}</Text>
              </View>
            ))
          ) : (
            <Text style={styles.emptyText}>No exported symbols scanned</Text>
          )}
        </View>
      </View>

      <View style={styles.detailsFooter}>
        <Button size="sm" variant="default" onPress={onAddContext} style={styles.addContextButton}>
          <ThemedSparklesIcon size={14} />
          <Text style={styles.addContextButtonText}>Add Sub-Graph to Context</Text>
        </Button>
      </View>
    </View>
  );
});

export const VisualDependencyGraphModal = memo(function VisualDependencyGraphModal({
  visible,
  onClose,
  cwd,
  serverId: propServerId,
  agentId: propAgentId,
  nodes: propNodes,
}: VisualDependencyGraphModalProps) {
  const toast = useToast();
  const [fetchedNodes, setFetchedNodes] = useState<DependencyNode[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const nodes = propNodes ?? fetchedNodes;
  const [selectedNodeId, setSelectedNodeId] = useState<string>("");

  useEffect(() => {
    if (!visible || propNodes || !cwd) return;
    let cancelled = false;
    async function scan() {
      setIsLoading(true);
      try {
        const res = await invokeTauri<{ detailedNodes?: DependencyNode[] }>(
          "get_dependency_graph",
          { cwd },
        );
        if (cancelled) return;
        const list = Array.isArray(res?.detailedNodes) ? res.detailedNodes : [];
        setFetchedNodes(list);
        if (list[0]) {
          setSelectedNodeId(list[0].id);
        }
      } catch (err) {
        if (!cancelled) {
          toast.show(`Dependency scan failed: ${String(err)}`, { variant: "error" });
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    }
    void scan();
    return () => {
      cancelled = true;
    };
  }, [visible, propNodes, cwd, toast]);

  const selectedNode = nodes.find((n) => n.id === selectedNodeId) ?? nodes[0] ?? null;

  const handleAddGraphToContext = useCallback(() => {
    if (!selectedNode) return;
    const sessions = useSessionStore.getState().sessions;
    const serverId = propServerId ?? Object.keys(sessions)[0];
    const session = serverId ? sessions[serverId] : undefined;
    const activeAgentId =
      propAgentId ?? (session?.agents ? Array.from(session.agents.values())[0]?.id : undefined);

    if (!serverId || !activeAgentId) {
      toast.show("No active agent available to add context", { variant: "error" });
      return;
    }

    const draftKey = `agent:${serverId}:${activeAgentId}`;
    const contextPrompt =
      `### Visual Architecture Context: \`${selectedNode.name}\`\n` +
      `File: \`${selectedNode.path}\`\n` +
      `Exports: ${selectedNode.exports.join(", ") || "None detected"}\n` +
      `Dependencies: ${selectedNode.dependencies.join(", ") || "None"}\n` +
      `Callers: ${selectedNode.callers.join(", ") || "None"}\n\n` +
      `Please keep this architectural sub-graph and module interfaces in mind during refactoring.`;

    useDraftStore.getState().replaceDraftText({ draftKey, text: contextPrompt });
    toast.show("Architecture sub-graph added to composer context", { variant: "success" });
    onClose();
  }, [onClose, propAgentId, propServerId, selectedNode, toast]);

  if (!visible) return null;

  const activeNodeId = selectedNode?.id ?? "";

  return (
    <View style={styles.overlay}>
      <View style={styles.modal}>
        <View style={styles.header}>
          <View style={styles.headerTitleRow}>
            <ThemedNetworkIcon size={20} />
            <Text style={styles.headerTitle}>Module Dependency & Architecture Graph</Text>
          </View>
          <ToolbarButton label="Close" onPress={onClose} style={styles.closeButton}>
            <ThemedCloseIcon size={18} />
          </ToolbarButton>
        </View>

        {isLoading ? (
          <View style={styles.detailsColumn}>
            <Text style={styles.emptyText}>Scanning workspace AST & imports...</Text>
          </View>
        ) : null}

        {!isLoading && !selectedNode ? (
          <View style={styles.detailsColumn}>
            <Text style={styles.emptyText}>No source modules detected in workspace.</Text>
          </View>
        ) : null}

        {!isLoading && selectedNode ? (
          <View style={styles.contentBody}>
            <ScrollView style={styles.nodesColumn}>
              <Text style={styles.columnTitle}>Workspace Modules ({nodes.length})</Text>
              {nodes.map((node) => (
                <ModuleNodeItem
                  key={node.id}
                  node={node}
                  isSelected={node.id === activeNodeId}
                  onSelect={setSelectedNodeId}
                />
              ))}
            </ScrollView>
            <NodeDetailsPanel node={selectedNode} onAddContext={handleAddGraphToContext} />
          </View>
        ) : null}
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
  sectionLabelSpaced: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
    marginBottom: 6,
    marginTop: 12,
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
