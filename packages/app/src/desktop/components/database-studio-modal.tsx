import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Modal, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Database, Play, Lock, Unlock, X, Table } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { PressHighlight } from "@/components/ui/press-highlight";
import { invokeTauri } from "@/utils/tauri-invoke";
import { useToast } from "@/contexts/toast-context";

const ThemedDatabaseIcon = withUnistyles(Database, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedTableIcon = withUnistyles(Table, (theme) => ({ color: theme.colors.foreground }));

interface DatabaseQueryResult {
  columns: string[];
  rows: string[][];
  rowCount: number;
  engine: string;
  rawOutput: string;
}

const DbFileChip = memo(function DbFileChip({
  db,
  isSelected,
  onSelect,
}: {
  db: string;
  isSelected: boolean;
  onSelect: (db: string) => void;
}) {
  const handlePress = useCallback(() => {
    onSelect(db);
  }, [db, onSelect]);

  return (
    <PressHighlight onPress={handlePress} style={[styles.chip, isSelected && styles.chipActive]}>
      <ThemedTableIcon size={11} />
      <Text style={styles.chipText}>{db.split("/").pop()}</Text>
    </PressHighlight>
  );
});

const QueryResultTable = memo(function QueryResultTable({
  result,
  columns,
}: {
  result: DatabaseQueryResult;
  columns: string[];
}) {
  const counts = new Map<string, number>();
  const colKeys = columns.map((c) => {
    const seen = (counts.get(c) ?? 0) + 1;
    counts.set(c, seen);
    return seen === 1 ? c : `${c}#${seen}`;
  });
  const keyedRows = result.rows.slice(0, 100).map((row) => ({
    rowKey: row.join("\u001f"),
    cells: colKeys.map((colKey, idx) => ({ colKey, value: row[idx] ?? "" })),
  }));

  return (
    <ScrollView horizontal style={styles.tableWrap}>
      <ScrollView>
        <View style={styles.tableRowHeader}>
          {colKeys.map((colKey) => (
            <Text key={colKey} style={styles.th}>
              {colKey}
            </Text>
          ))}
        </View>
        {keyedRows.map((entry) => (
          <View key={entry.rowKey} style={styles.tableRow}>
            {entry.cells.map((cell) => (
              <Text key={cell.colKey} style={styles.td} numberOfLines={2}>
                {cell.value}
              </Text>
            ))}
          </View>
        ))}
      </ScrollView>
    </ScrollView>
  );
});

export interface DatabaseStudioModalProps {
  visible: boolean;
  cwd: string;
  onClose: () => void;
}

export const DatabaseStudioModal = memo(function DatabaseStudioModal({
  visible,
  cwd,
  onClose,
}: DatabaseStudioModalProps) {
  const toast = useToast();
  const [databases, setDatabases] = useState<string[]>([]);
  const [selectedDb, setSelectedDb] = useState("");
  const [dbResetKey, setDbResetKey] = useState(0);
  const [sql, setSql] = useState(
    "SELECT name, type FROM sqlite_master WHERE type IN ('table','view');",
  );
  const [readOnly, setReadOnly] = useState(true);
  const [result, setResult] = useState<DatabaseQueryResult | null>(null);
  const [errorText, setErrorText] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(false);

  const loadDbs = useCallback(async () => {
    try {
      const found = (await invokeTauri<string[]>("discover_workspace_databases", { cwd })) ?? [];
      setDatabases(found);
      if (found.length > 0 && !selectedDb) {
        setSelectedDb(found[0]!);
        setDbResetKey((k) => k + 1);
      }
    } catch (err) {
      toast.show(`DB discovery failed: ${String(err)}`, { variant: "error" });
    }
  }, [cwd, selectedDb, toast]);

  useEffect(() => {
    if (visible) {
      void loadDbs();
    }
  }, [visible, loadDbs]);

  const handleToggleReadOnly = useCallback(() => {
    setReadOnly((v) => !v);
  }, []);

  const handleSelectChip = useCallback((db: string) => {
    setSelectedDb(db);
    setDbResetKey((k) => k + 1);
  }, []);

  const handleExecute = useCallback(async () => {
    if (!selectedDb.trim()) {
      setErrorText("Specify or select a SQLite .db/.sqlite file path first.");
      return;
    }
    setIsRunning(true);
    setErrorText(null);
    try {
      const res = await invokeTauri<DatabaseQueryResult>("execute_sqlite_query", {
        cwd,
        dbPath: selectedDb.trim(),
        sql,
        readOnly,
      });
      setResult(res);
    } catch (err) {
      setErrorText(String(err));
      setResult(null);
    } finally {
      setIsRunning(false);
    }
  }, [cwd, readOnly, selectedDb, sql]);

  const handleExecutePress = useCallback(() => {
    void handleExecute();
  }, [handleExecute]);

  if (!visible) return null;

  const renderResultView = () => {
    if (errorText) {
      return (
        <View style={styles.errBox}>
          <Text style={styles.errText}>{errorText}</Text>
        </View>
      );
    }
    if (result && result.columns.length > 0) {
      return <QueryResultTable result={result} columns={result.columns} />;
    }
    if (result) {
      return <Text style={styles.metaText}>Query returned 0 rows.</Text>;
    }
    return null;
  };

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop} testID="database-studio-modal">
        <View style={styles.card}>
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <ThemedDatabaseIcon size={18} />
              <Text style={styles.title}>Embedded Database Studio & SQL Console</Text>
            </View>
            <View style={styles.headerActions}>
              <PressHighlight
                onPress={handleToggleReadOnly}
                style={[styles.lockBtn, readOnly ? styles.lockSafe : styles.lockWrite]}
              >
                {readOnly ? (
                  <Lock size={12} color="#22c55e" />
                ) : (
                  <Unlock size={12} color="#f59e0b" />
                )}
                <Text style={styles.lockText}>{readOnly ? "Read-Only" : "Write Enabled"}</Text>
              </PressHighlight>
              <ToolbarButton label="Close" onPress={onClose}>
                <ThemedCloseIcon size={18} />
              </ToolbarButton>
            </View>
          </View>

          <View style={styles.body}>
            <View style={styles.dbBar}>
              <Text style={styles.label}>SQLite File:</Text>
              <AdaptiveTextInput
                initialValue={selectedDb}
                resetKey={dbResetKey}
                onChangeText={setSelectedDb}
                placeholder="path/to/database.sqlite"
                style={styles.dbInput}
              />
              {databases.slice(0, 4).map((db) => (
                <DbFileChip
                  key={db}
                  db={db}
                  isSelected={selectedDb === db}
                  onSelect={handleSelectChip}
                />
              ))}
            </View>

            <AdaptiveTextInput
              initialValue={sql}
              onChangeText={setSql}
              multiline
              style={styles.sqlInput}
              placeholder="SELECT * FROM users LIMIT 50;"
            />

            <View style={styles.runRow}>
              <Button size="sm" onPress={handleExecutePress} disabled={isRunning}>
                <Play size={13} color="#ffffff" />
                <Text style={styles.runText}>{isRunning ? "Running Query..." : "Run SQL"}</Text>
              </Button>
              {result && (
                <Text style={styles.metaText}>
                  {result.rowCount} rows via {result.engine}
                </Text>
              )}
            </View>

            {renderResultView()}
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
    maxWidth: 880,
    height: "84%",
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
  headerActions: { flexDirection: "row", alignItems: "center", gap: 10 },
  lockBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 5,
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 99,
    borderWidth: 1,
  },
  lockSafe: { borderColor: "rgba(34,197,94,0.4)", backgroundColor: "rgba(34,197,94,0.1)" },
  lockWrite: { borderColor: "rgba(245,158,11,0.4)", backgroundColor: "rgba(245,158,11,0.1)" },
  lockText: { fontSize: 11, fontWeight: "600", color: theme.colors.foreground },
  closeBtn: { padding: 4 },
  body: { flex: 1, padding: theme.spacing[4], gap: 10 },
  dbBar: { flexDirection: "row", alignItems: "center", gap: 8, flexWrap: "wrap" },
  label: { fontSize: 12, fontWeight: "600", color: theme.colors.foreground },
  dbInput: {
    flex: 1,
    minWidth: 200,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 6,
    paddingHorizontal: 10,
    paddingVertical: 6,
    color: theme.colors.foreground,
    fontFamily: "monospace",
    fontSize: 12,
  },
  chip: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: 6,
    backgroundColor: theme.colors.surface2,
  },
  chipActive: { borderWidth: 1, borderColor: theme.colors.accent },
  chipText: { fontSize: 11, color: theme.colors.foreground },
  sqlInput: {
    minHeight: 86,
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
  runRow: { flexDirection: "row", alignItems: "center", gap: 12 },
  runText: { color: "#ffffff", fontSize: 12, fontWeight: "600", marginLeft: 4 },
  metaText: { fontSize: 12, color: theme.colors.foregroundMuted },
  errBox: { padding: 10, borderRadius: 6, backgroundColor: "rgba(239,68,68,0.12)" },
  errText: { color: "#ef4444", fontSize: 12, fontFamily: "monospace" },
  tableWrap: {
    flex: 1,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 6,
    backgroundColor: theme.colors.surface0,
  },
  tableRowHeader: {
    flexDirection: "row",
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface2,
  },
  tableRow: {
    flexDirection: "row",
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  th: {
    width: 150,
    padding: 8,
    fontSize: 11,
    fontWeight: "700",
    color: theme.colors.foreground,
    fontFamily: "monospace",
  },
  td: {
    width: 150,
    padding: 8,
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    fontFamily: "monospace",
  },
}));
