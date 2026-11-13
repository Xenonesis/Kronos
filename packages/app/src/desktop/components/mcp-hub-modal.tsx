import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Plug, Play, X, Wrench, CheckCircle2 } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { PressHighlight } from "@/components/ui/press-highlight";
import { McpServersCard } from "./mcp-servers-card";
import { invokeTauri } from "@/utils/tauri-invoke";
import { useToast } from "@/contexts/toast-context";

const ThemedPlugIcon = withUnistyles(Plug, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedActiveWrenchIcon = withUnistyles(Wrench, (theme) => ({ color: theme.colors.accent }));
const ThemedMutedWrenchIcon = withUnistyles(Wrench, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

interface McpToolDef {
  name: string;
  description: string;
  inputSchema?: Record<string, unknown>;
}

const McpToolRow = memo(function McpToolRow({
  tool,
  active,
  onSelect,
}: {
  tool: McpToolDef;
  active: boolean;
  onSelect: (tool: McpToolDef) => void;
}) {
  const handlePress = useCallback(() => {
    onSelect(tool);
  }, [onSelect, tool]);

  return (
    <PressHighlight onPress={handlePress} style={[styles.toolRow, active && styles.toolRowActive]}>
      {active ? <ThemedActiveWrenchIcon size={13} /> : <ThemedMutedWrenchIcon size={13} />}
      <View style={styles.toolMeta}>
        <Text style={[styles.toolName, active && styles.toolNameActive]}>{tool.name}</Text>
        <Text style={styles.toolDesc} numberOfLines={2}>
          {tool.description}
        </Text>
      </View>
    </PressHighlight>
  );
});

const McpToolDetailPane = memo(function McpToolDetailPane({
  selectedTool,
  argsJson,
  isRunning,
  toolOutput,
  onChangeArgs,
  onRunTool,
}: {
  selectedTool: McpToolDef | null;
  argsJson: string;
  isRunning: boolean;
  toolOutput: string | null;
  onChangeArgs: (text: string) => void;
  onRunTool: () => void;
}) {
  if (!selectedTool) {
    return (
      <ScrollView style={styles.toolDetail}>
        <Text style={styles.toolDesc}>Select a registered MCP tool to inspect & test.</Text>
      </ScrollView>
    );
  }
  return (
    <ScrollView style={styles.toolDetail}>
      <View style={styles.detailHeader}>
        <CheckCircle2 size={14} color="#22c55e" />
        <Text style={styles.detailTitle}>{selectedTool.name}</Text>
      </View>
      <Text style={styles.detailDesc}>{selectedTool.description}</Text>

      <Text style={styles.sectionLabel}>JSON Input Arguments</Text>
      <AdaptiveTextInput
        initialValue={argsJson}
        onChangeText={onChangeArgs}
        multiline
        style={styles.jsonInput}
      />

      <View style={styles.runRow}>
        <Button size="sm" onPress={onRunTool} disabled={isRunning}>
          <Play size={13} color="#ffffff" />
          <Text style={styles.runBtnText}>{isRunning ? "Running..." : "Invoke MCP Tool"}</Text>
        </Button>
      </View>

      {toolOutput !== null && (
        <View style={styles.outputBox}>
          <Text style={styles.sectionLabel}>Response</Text>
          <Text style={styles.outputText} selectable>
            {toolOutput}
          </Text>
        </View>
      )}
    </ScrollView>
  );
});

const McpHubHeader = memo(function McpHubHeader({
  tab,
  toolCount,
  onSelectTools,
  onSelectServers,
  onClose,
}: {
  tab: "tools" | "servers";
  toolCount: number;
  onSelectTools: () => void;
  onSelectServers: () => void;
  onClose: () => void;
}) {
  return (
    <View style={styles.header}>
      <View style={styles.titleRow}>
        <ThemedPlugIcon size={18} />
        <Text style={styles.title}>Model Context Protocol (MCP) Hub</Text>
        <View style={styles.tabGroup}>
          <PressHighlight
            onPress={onSelectTools}
            style={[styles.tabBtn, tab === "tools" && styles.tabBtnActive]}
          >
            <Text style={[styles.tabText, tab === "tools" && styles.tabTextActive]}>
              Live Tools ({toolCount})
            </Text>
          </PressHighlight>
          <PressHighlight
            onPress={onSelectServers}
            style={[styles.tabBtn, tab === "servers" && styles.tabBtnActive]}
          >
            <Text style={[styles.tabText, tab === "servers" && styles.tabTextActive]}>
              Servers & Registry
            </Text>
          </PressHighlight>
        </View>
      </View>
      <ToolbarButton label="Close" onPress={onClose}>
        <ThemedCloseIcon size={18} />
      </ToolbarButton>
    </View>
  );
});

export interface McpHubModalProps {
  visible: boolean;
  onClose: () => void;
}

export const McpHubModal = memo(function McpHubModal({ visible, onClose }: McpHubModalProps) {
  const toast = useToast();
  const [tab, setTab] = useState<"tools" | "servers">("tools");
  const [tools, setTools] = useState<McpToolDef[]>([]);
  const [selectedTool, setSelectedTool] = useState<McpToolDef | null>(null);
  const [argsJson, setArgsJson] = useState("{}");
  const [toolOutput, setToolOutput] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(false);

  const loadTools = useCallback(async () => {
    try {
      const list = await invokeTauri<McpToolDef[]>("mcp_list_tools");
      if (list) {
        setTools(list);
        if (!selectedTool && list.length > 0) {
          setSelectedTool(list[0]!);
        }
      }
    } catch (err) {
      toast.show(`MCP list failed: ${String(err)}`, { variant: "error" });
    }
  }, [selectedTool, toast]);

  useEffect(() => {
    if (visible) {
      void loadTools();
    }
  }, [visible, loadTools]);

  const handleSelectToolsTab = useCallback(() => {
    setTab("tools");
  }, []);

  const handleSelectServersTab = useCallback(() => {
    setTab("servers");
  }, []);

  const handleSelectTool = useCallback((tool: McpToolDef) => {
    setSelectedTool(tool);
    setToolOutput(null);
  }, []);

  const handleRunTool = useCallback(async () => {
    if (!selectedTool) return;
    setIsRunning(true);
    setToolOutput(null);
    try {
      const parsed = JSON.parse(argsJson || "{}") as Record<string, unknown>;
      const result = await invokeTauri<string>("mcp_call_tool", {
        toolName: selectedTool.name,
        args: parsed,
      });
      setToolOutput(result ?? "Executed (no native host attached)");
    } catch (err) {
      setToolOutput(`Error: ${String(err)}`);
    } finally {
      setIsRunning(false);
    }
  }, [argsJson, selectedTool]);

  const handleRunToolPress = useCallback(() => {
    void handleRunTool();
  }, [handleRunTool]);

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop} testID="mcp-hub-modal">
        <View style={styles.card}>
          <McpHubHeader
            tab={tab}
            toolCount={tools.length}
            onSelectTools={handleSelectToolsTab}
            onSelectServers={handleSelectServersTab}
            onClose={onClose}
          />

          {tab === "servers" ? (
            <ScrollView style={styles.serversBody}>
              <McpServersCard onServersSynced={loadTools} />
            </ScrollView>
          ) : (
            <View style={styles.splitBody}>
              <ScrollView style={styles.toolSidebar}>
                {tools.map((tool) => (
                  <McpToolRow
                    key={tool.name}
                    tool={tool}
                    active={selectedTool?.name === tool.name}
                    onSelect={handleSelectTool}
                  />
                ))}
              </ScrollView>

              <McpToolDetailPane
                selectedTool={selectedTool}
                argsJson={argsJson}
                isRunning={isRunning}
                toolOutput={toolOutput}
                onChangeArgs={setArgsJson}
                onRunTool={handleRunToolPress}
              />
            </View>
          )}
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
    maxWidth: 880,
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
  titleRow: { flexDirection: "row", alignItems: "center", gap: 10 },
  title: { fontSize: theme.fontSize.base, fontWeight: "700", color: theme.colors.foreground },
  tabGroup: { flexDirection: "row", gap: 6, marginLeft: 12 },
  tabBtn: {
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 6,
    backgroundColor: theme.colors.surface2,
  },
  tabBtnActive: { backgroundColor: theme.colors.accent },
  tabText: { fontSize: 12, fontWeight: "600", color: theme.colors.foregroundMuted },
  tabTextActive: { color: "#ffffff" },
  closeBtn: { padding: 4 },
  serversBody: { flex: 1, padding: theme.spacing[4] },
  splitBody: { flex: 1, flexDirection: "row" },
  toolSidebar: {
    width: 280,
    borderRightWidth: 1,
    borderRightColor: theme.colors.border,
    padding: 8,
  },
  toolRow: {
    flexDirection: "row",
    gap: 8,
    padding: 10,
    borderRadius: 6,
    marginBottom: 4,
  },
  toolRowActive: { backgroundColor: theme.colors.surface2 },
  toolMeta: { flex: 1, gap: 2 },
  toolName: {
    fontSize: 12,
    fontWeight: "700",
    color: theme.colors.foreground,
    fontFamily: "monospace",
  },
  toolNameActive: { color: theme.colors.accent },
  toolDesc: { fontSize: 11, color: theme.colors.foregroundMuted },
  toolDetail: { flex: 1, padding: theme.spacing[4] },
  detailHeader: { flexDirection: "row", alignItems: "center", gap: 8, marginBottom: 6 },
  detailTitle: {
    fontSize: 15,
    fontWeight: "700",
    color: theme.colors.foreground,
    fontFamily: "monospace",
  },
  detailDesc: { fontSize: 13, color: theme.colors.foregroundMuted, marginBottom: 14 },
  sectionLabel: {
    fontSize: 11,
    fontWeight: "700",
    color: theme.colors.foregroundMuted,
    marginBottom: 6,
  },
  jsonInput: {
    minHeight: 90,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 6,
    padding: 10,
    color: theme.colors.foreground,
    fontFamily: "monospace",
    fontSize: 12,
    textAlignVertical: "top",
  },
  runRow: { marginTop: 10, flexDirection: "row" },
  runBtnText: { color: "#ffffff", fontSize: 12, fontWeight: "600", marginLeft: 4 },
  outputBox: {
    marginTop: 14,
    padding: 10,
    backgroundColor: theme.colors.surface0,
    borderRadius: 6,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  outputText: { fontSize: 12, fontFamily: "monospace", color: theme.colors.foreground },
}));
