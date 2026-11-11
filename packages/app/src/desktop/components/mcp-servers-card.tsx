import { memo, useState, useCallback, useMemo } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Server, Plus, Trash2, CheckCircle2 } from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { EditingTextInput } from "@/components/ui/text-input";
import { PressHighlight } from "@/components/ui/press-highlight";

export interface McpServerConfig {
  id: string;
  name: string;
  transport: "stdio" | "sse";
  command: string;
  args: string[];
  enabled: boolean;
}

const DEFAULT_MCP_SERVERS: McpServerConfig[] = [
  {
    id: "fs-mcp",
    name: "Filesystem MCP",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "."],
    enabled: true,
  },
  {
    id: "memory-mcp",
    name: "Memory Graph MCP",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-memory"],
    enabled: true,
  },
];

const ThemedServer = withUnistyles(Server);
const ThemedCheckCircle = withUnistyles(CheckCircle2);
const ThemedTrash = withUnistyles(Trash2);
const ThemedPlus = withUnistyles(Plus);

interface McpServerRowProps {
  server: McpServerConfig;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}

const McpServerRow = memo(function McpServerRow({ server, onToggle, onDelete }: McpServerRowProps) {
  const handleToggle = useCallback(() => {
    onToggle(server.id);
  }, [server.id, onToggle]);

  const handleDelete = useCallback(() => {
    onDelete(server.id);
  }, [server.id, onDelete]);

  return (
    <View style={styles.serverRow}>
      <View style={styles.serverInfo}>
        <View style={styles.nameRow}>
          <ThemedCheckCircle size={14} />
          <Text style={styles.serverName}>{server.name}</Text>
          <Text style={styles.badge}>{server.transport}</Text>
        </View>
        <Text style={styles.serverCmd}>
          {server.command} {server.args.join(" ")}
        </Text>
      </View>

      <View style={styles.actions}>
        <PressHighlight
          style={styles.toggleBtn}
          onPress={handleToggle}
          accessibilityRole="button"
          accessibilityLabel="Toggle Server"
        >
          <Text style={[styles.toggleText, server.enabled && styles.toggleTextActive]}>
            {server.enabled ? "Enabled" : "Disabled"}
          </Text>
        </PressHighlight>

        <PressHighlight
          style={styles.deleteBtn}
          onPress={handleDelete}
          accessibilityRole="button"
          accessibilityLabel="Delete Server"
        >
          <ThemedTrash size={14} />
        </PressHighlight>
      </View>
    </View>
  );
});

export const McpServersCard = memo(function McpServersCard() {
  const [servers, setServers] = useState<McpServerConfig[]>(DEFAULT_MCP_SERVERS);
  const [isAdding, setIsAdding] = useState(false);
  const [newName, setNewName] = useState("");
  const [newCommand, setNewCommand] = useState("");
  const [newArgs, setNewArgs] = useState("");

  const handleToggle = useCallback((id: string) => {
    setServers((prev) => {
      const next = [];
      for (const s of prev) {
        next.push(s.id === id ? { ...s, enabled: !s.enabled } : s);
      }
      return next;
    });
  }, []);

  const handleDelete = useCallback((id: string) => {
    setServers((prev) => {
      const next = [];
      for (const s of prev) {
        if (s.id !== id) next.push(s);
      }
      return next;
    });
  }, []);

  const toggleIsAdding = useCallback(() => {
    setIsAdding((prev) => !prev);
  }, []);

  const handleAddServer = useCallback(() => {
    if (!newName.trim() || !newCommand.trim()) return;
    const args = newArgs
      .split(" ")
      .map((a) => a.trim())
      .filter(Boolean);
    const newServer: McpServerConfig = {
      id: `mcp-${Date.now()}`,
      name: newName.trim(),
      transport: "stdio",
      command: newCommand.trim(),
      args,
      enabled: true,
    };
    setServers((prev) => [...prev, newServer]);
    setNewName("");
    setNewCommand("");
    setNewArgs("");
    setIsAdding(false);
  }, [newName, newCommand, newArgs]);

  const addIcon = useMemo(() => <ThemedPlus size={14} />, []);

  return (
    <View style={styles.container} testID="mcp-servers-card">
      <View style={styles.cardHeader}>
        <View style={styles.titleRow}>
          <ThemedServer size={18} />
          <View>
            <Text style={styles.titleText}>Model Context Protocol (MCP) Servers</Text>
            <Text style={styles.subtitleText}>
              Connect external MCP tool providers (PostgreSQL, GitHub, Puppeteer, Memory)
            </Text>
          </View>
        </View>
        <Button variant="outline" size="sm" leftIcon={addIcon} onPress={toggleIsAdding}>
          {isAdding ? "Cancel" : "Add Server"}
        </Button>
      </View>

      {isAdding ? (
        <View style={styles.addForm}>
          <Text style={styles.formTitle}>Add New MCP Server (stdio)</Text>
          <EditingTextInput
            placeholder="Server Name (e.g. GitHub MCP)"
            initialValue={newName}
            onChangeText={setNewName}
          />
          <EditingTextInput
            placeholder="Command / Executable (e.g. npx, uvx, docker)"
            initialValue={newCommand}
            onChangeText={setNewCommand}
          />
          <EditingTextInput
            placeholder="Arguments (e.g. -y @modelcontextprotocol/server-github)"
            initialValue={newArgs}
            onChangeText={setNewArgs}
          />
          <View style={styles.formActions}>
            <Button
              variant="default"
              size="sm"
              onPress={handleAddServer}
              disabled={!newName.trim() || !newCommand.trim()}
            >
              Save Server
            </Button>
          </View>
        </View>
      ) : null}

      <View style={styles.serverList}>
        {servers.map((server) => (
          <McpServerRow
            key={server.id}
            server={server}
            onToggle={handleToggle}
            onDelete={handleDelete}
          />
        ))}
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    backgroundColor: theme.colors.surface1,
    borderColor: theme.colors.border,
    borderWidth: 1,
    borderRadius: theme.borderRadius.lg,
    padding: theme.spacing[4],
    gap: theme.spacing[3],
    marginTop: theme.spacing[3],
  },
  cardHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  titleText: {
    fontSize: 14,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  subtitleText: {
    fontSize: 12,
    color: theme.colors.foregroundMuted,
    marginTop: 2,
  },
  addForm: {
    backgroundColor: theme.colors.surface0,
    padding: theme.spacing[3],
    borderRadius: theme.borderRadius.md,
    gap: theme.spacing[2],
    borderColor: theme.colors.border,
    borderWidth: 1,
  },
  formTitle: {
    fontSize: 12,
    fontWeight: "600",
    color: theme.colors.foreground,
    marginBottom: 2,
  },
  formActions: {
    flexDirection: "row",
    justifyContent: "flex-end",
    marginTop: 4,
  },
  serverList: {
    gap: theme.spacing[2],
  },
  serverRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    padding: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.md,
    borderColor: theme.colors.border,
    borderWidth: 1,
  },
  serverInfo: {
    flex: 1,
    gap: 3,
  },
  nameRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  serverName: {
    fontSize: 13,
    fontWeight: "500",
    color: theme.colors.foreground,
  },
  badge: {
    fontSize: 10,
    fontFamily: theme.fontFamily.mono,
    paddingHorizontal: 5,
    paddingVertical: 1,
    backgroundColor: theme.colors.surface2,
    borderRadius: theme.borderRadius.sm,
    color: theme.colors.foregroundMuted,
    textTransform: "uppercase",
  },
  serverCmd: {
    fontSize: 11,
    fontFamily: theme.fontFamily.mono,
    color: theme.colors.foregroundMuted,
  },
  actions: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  toggleBtn: {
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: theme.colors.surface2,
  },
  toggleText: {
    fontSize: 11,
    fontWeight: "500",
    color: theme.colors.foregroundMuted,
  },
  toggleTextActive: {
    color: theme.colors.palette.green[500],
  },
  deleteBtn: {
    padding: 6,
    borderRadius: theme.borderRadius.sm,
  },
}));
