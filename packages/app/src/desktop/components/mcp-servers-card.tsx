import { memo, useState, useCallback, useMemo, useEffect } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Server, Plus, Trash2, CheckCircle2, AlertCircle, RefreshCw } from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { EditingTextInput } from "@/components/ui/text-input";
import { PressHighlight } from "@/components/ui/press-highlight";
import { useToast } from "@/contexts/toast-context";
import { invokeTauri } from "@/utils/tauri-invoke";
import { McpRegistryCatalog } from "./mcp-registry-catalog";

export interface McpServerConfig {
  id: string;
  name: string;
  transport: "stdio" | "sse";
  command: string;
  args: string[];
  enabled: boolean;
}

export interface ExternalMcpServerStatus {
  id: string;
  name: string;
  status: "connected" | "disabled" | "error";
  toolCount: number;
  lastError?: string | null;
}

const ThemedServer = withUnistyles(Server);
const ThemedCheckCircle = withUnistyles(CheckCircle2);
const ThemedAlertCircle = withUnistyles(AlertCircle);
const ThemedTrash = withUnistyles(Trash2);
const ThemedPlus = withUnistyles(Plus);
const ThemedRefresh = withUnistyles(RefreshCw);

interface McpServerRowProps {
  server: McpServerConfig;
  status?: ExternalMcpServerStatus;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}

const McpServerRow = memo(function McpServerRow({
  server,
  status,
  onToggle,
  onDelete,
}: McpServerRowProps) {
  const handleToggle = useCallback(() => {
    onToggle(server.id);
  }, [server.id, onToggle]);

  const handleDelete = useCallback(() => {
    onDelete(server.id);
  }, [server.id, onDelete]);

  const statusText = useMemo(() => {
    if (!server.enabled) return "Disabled";
    if (!status) return "Pending";
    if (status.status === "connected") return `${status.toolCount} tools`;
    if (status.status === "error") return "Error";
    return status.status;
  }, [server.enabled, status]);

  return (
    <View style={styles.serverRow}>
      <View style={styles.serverInfo}>
        <View style={styles.nameRow}>
          {status?.status === "error" ? (
            <ThemedAlertCircle size={14} />
          ) : (
            <ThemedCheckCircle size={14} />
          )}
          <Text style={styles.serverName}>{server.name}</Text>
          <Text style={styles.badge}>{server.transport}</Text>
          <Text style={styles.badge}>{statusText}</Text>
        </View>
        <Text style={styles.serverCmd}>
          {server.command} {server.args.join(" ")}
        </Text>
        {status?.lastError ? (
          <Text style={styles.serverCmd}>{status.lastError}</Text>
        ) : null}
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

export interface McpServersCardProps {
  onServersSynced?: () => void;
}

export const McpServersCard = memo(function McpServersCard({
  onServersSynced,
}: McpServersCardProps) {
  const toast = useToast();
  const [servers, setServers] = useState<McpServerConfig[]>([]);
  const [statusesById, setStatusesById] = useState<Record<string, ExternalMcpServerStatus>>({});
  const [isSyncing, setIsSyncing] = useState(false);
  const [isAdding, setIsAdding] = useState(false);
  const [isCatalogOpen, setIsCatalogOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newCommand, setNewCommand] = useState("");
  const [newArgs, setNewArgs] = useState("");

  const syncAll = useCallback(async () => {
    setIsSyncing(true);
    try {
      const results = await invokeTauri<ExternalMcpServerStatus[]>("mcp_sync_all_servers");
      if (results) {
        const nextMap: Record<string, ExternalMcpServerStatus> = {};
        for (const r of results) {
          nextMap[r.id] = r;
        }
        setStatusesById(nextMap);
      }
      onServersSynced?.();
    } catch (err) {
      toast.show(`MCP sync failed: ${String(err)}`, { variant: "error" });
    } finally {
      setIsSyncing(false);
    }
  }, [onServersSynced, toast]);

  useEffect(() => {
    let cancelled = false;
    async function loadInitial() {
      try {
        const list = await invokeTauri<McpServerConfig[]>("mcp_list_servers");
        if (cancelled) return;
        setServers(list ?? []);
        const results = await invokeTauri<ExternalMcpServerStatus[]>("mcp_sync_all_servers");
        if (cancelled || !results) return;
        const nextMap: Record<string, ExternalMcpServerStatus> = {};
        for (const r of results) {
          nextMap[r.id] = r;
        }
        setStatusesById(nextMap);
      } catch {
        // Ignore when desktop host is not attached
      }
    }
    void loadInitial();
    return () => {
      cancelled = true;
    };
  }, []);

  const persistAndSync = useCallback(
    async (nextServers: McpServerConfig[], changedServer?: McpServerConfig) => {
      try {
        await invokeTauri("mcp_save_servers", { servers: nextServers });
        if (changedServer) {
          const st = await invokeTauri<ExternalMcpServerStatus>("mcp_probe_server", {
            server: changedServer,
          });
          if (st) {
            setStatusesById((prev) => ({ ...prev, [st.id]: st }));
          }
        }
        onServersSynced?.();
      } catch (err) {
        toast.show(`Failed to persist MCP config: ${String(err)}`, { variant: "error" });
      }
    },
    [onServersSynced, toast],
  );

  const handleToggle = useCallback(
    (id: string) => {
      let changed: McpServerConfig | undefined;
      const next: McpServerConfig[] = [];
      for (const s of servers) {
        if (s.id === id) {
          changed = { ...s, enabled: !s.enabled };
          next.push(changed);
        } else {
          next.push(s);
        }
      }
      setServers(next);
      void persistAndSync(next, changed);
    },
    [persistAndSync, servers],
  );

  const handleDelete = useCallback(
    (id: string) => {
      const next: McpServerConfig[] = [];
      for (const s of servers) {
        if (s.id !== id) next.push(s);
      }
      setServers(next);
      void persistAndSync(next);
    },
    [persistAndSync, servers],
  );

  const toggleIsAdding = useCallback(() => {
    setIsAdding((prev) => !prev);
  }, []);

  const handleOpenCatalog = useCallback(() => {
    setIsCatalogOpen(true);
  }, []);

  const handleCloseCatalog = useCallback(() => {
    setIsCatalogOpen(false);
  }, []);

  const handleSyncPress = useCallback(() => {
    void syncAll();
  }, [syncAll]);

  const handleInstallFromCatalog = useCallback(
    (newServer: McpServerConfig) => {
      const next: McpServerConfig[] = [];
      for (const s of servers) {
        if (s.id !== newServer.id) next.push(s);
      }
      next.push(newServer);
      setServers(next);
      setIsCatalogOpen(false);
      void persistAndSync(next, newServer);
    },
    [persistAndSync, servers],
  );

  const handleAddServer = useCallback(() => {
    if (!newName.trim() || !newCommand.trim()) return;
    const cmdText = newCommand.trim();
    const isHttp = /^https?:\/\//i.test(cmdText);
    const args = newArgs
      .split(" ")
      .map((a) => a.trim())
      .filter(Boolean);
    const newServer: McpServerConfig = {
      id: `mcp-${Date.now()}`,
      name: newName.trim(),
      transport: isHttp ? "sse" : "stdio",
      command: cmdText,
      args,
      enabled: true,
    };
    setServers((prev) => {
      const next = [...prev, newServer];
      void persistAndSync(next, newServer);
      return next;
    });
    setNewName("");
    setNewCommand("");
    setNewArgs("");
    setIsAdding(false);
  }, [newName, newCommand, newArgs, persistAndSync]);

  const addIcon = useMemo(() => <ThemedPlus size={14} />, []);
  const syncIcon = useMemo(() => <ThemedRefresh size={14} />, []);
  const installedIds = useMemo(() => servers.map((s) => s.id), [servers]);

  return (
    <View style={styles.container} testID="mcp-servers-card">
      <View style={styles.cardHeader}>
        <View style={styles.titleRow}>
          <ThemedServer size={18} />
          <View>
            <Text style={styles.titleText}>Model Context Protocol (MCP) Servers</Text>
            <Text style={styles.subtitleText}>
              Connect external MCP tool providers (stdio JSON-RPC & HTTP/SSE endpoints)
            </Text>
          </View>
        </View>
        <View style={styles.headerActions}>
          <Button
            variant="outline"
            size="sm"
            leftIcon={syncIcon}
            onPress={handleSyncPress}
            disabled={isSyncing}
          >
            {isSyncing ? "Syncing..." : "Sync All"}
          </Button>
          <Button variant="outline" size="sm" onPress={handleOpenCatalog}>
            Browse Registry
          </Button>
          <Button variant="outline" size="sm" leftIcon={addIcon} onPress={toggleIsAdding}>
            {isAdding ? "Cancel" : "Add Server"}
          </Button>
        </View>
      </View>

      <McpRegistryCatalog
        visible={isCatalogOpen}
        onClose={handleCloseCatalog}
        installedServerIds={installedIds}
        onInstall={handleInstallFromCatalog}
      />

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
            status={statusesById[server.id]}
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
  headerActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
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
