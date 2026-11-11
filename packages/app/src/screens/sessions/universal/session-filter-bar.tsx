import React, { memo, useCallback } from "react";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { Check, RotateCw } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { SearchField } from "@/components/ui/search-field";
import type { HistoryScope, HistorySort } from "./session-types";

interface SessionFilterBarProps {
  scope: HistoryScope;
  onSelectScope: (scope: HistoryScope) => void;
  searchInput: string;
  onSearchChange: (text: string) => void;
  sort: HistorySort;
  sortLabel: string;
  onCycleSort: () => void;
  onRefresh: () => void;
  isLoading: boolean;
  scannedFiles: number;
  sessionCount: number;
  totalSessions: number;
}

const SCOPES: HistoryScope[] = ["workspace", "project", "all"];

interface ScopeTabItemProps {
  mode: HistoryScope;
  isActive: boolean;
  onSelect: (mode: HistoryScope) => void;
}

const ScopeTabItem = memo(function ScopeTabItem({ mode, isActive, onSelect }: ScopeTabItemProps) {
  const handlePress = useCallback(() => {
    onSelect(mode);
  }, [mode, onSelect]);

  return (
    <PressHighlight
      style={[styles.scopeTabItem, isActive && styles.scopeTabItemActive]}
      onPress={handlePress}
    >
      <Text style={[styles.scopeTabText, isActive && styles.scopeTabTextActive]}>
        {mode.charAt(0).toUpperCase() + mode.slice(1)}
      </Text>
    </PressHighlight>
  );
});

export const SessionFilterBar = memo(function SessionFilterBar({
  scope,
  onSelectScope,
  searchInput,
  onSearchChange,
  sortLabel,
  onCycleSort,
  onRefresh,
  isLoading,
  scannedFiles,
  sessionCount,
  totalSessions,
}: SessionFilterBarProps) {
  const refreshIconColor = isLoading ? "#38bdf8" : "#94a3b8";

  return (
    <View style={styles.header}>
      <View style={styles.headerTitleRow}>
        <View>
          <Text style={styles.headerTitle}>Agent Session History</Text>
          <Text style={styles.headerSubtitle}>Indexed history</Text>
        </View>
        <View style={styles.headerActions}>
          <PressHighlight style={styles.iconButton} onPress={onCycleSort} hitSlop={6}>
            <Text style={styles.sortToggleText}>{sortLabel} ∨</Text>
          </PressHighlight>
          <PressHighlight style={styles.iconButton} onPress={onRefresh} hitSlop={6}>
            <RotateCw size={14} color={refreshIconColor} />
          </PressHighlight>
        </View>
      </View>

      <View style={styles.scopeTabs}>
        {SCOPES.map((mode) => (
          <ScopeTabItem key={mode} mode={mode} isActive={scope === mode} onSelect={onSelectScope} />
        ))}
      </View>

      <SearchField
        value={searchInput}
        onChangeText={onSearchChange}
        placeholder="Search sessions"
        clearAccessibilityLabel="Clear search"
      />

      <View style={styles.telemetryRow}>
        <View style={styles.telemetryLeft}>
          <Check size={13} color="#22c55e" />
          <Text style={styles.telemetryText}>
            Scanned <Text style={styles.boldText}>{scannedFiles.toLocaleString()}</Text> files
          </Text>
        </View>
        <Text style={styles.telemetryRight}>Just now ∨</Text>
      </View>

      <View style={styles.sessionCountRow}>
        <Text style={styles.sessionCountText}>
          <Text style={styles.boldText}>{sessionCount}</Text> of{" "}
          <Text style={styles.boldText}>{totalSessions}</Text> sessions
        </Text>
        <PressHighlight onPress={onCycleSort}>
          <Text style={styles.sortSelectorText}>{sortLabel} ∨</Text>
        </PressHighlight>
      </View>
    </View>
  );
});

const styles = StyleSheet.create({
  header: {
    paddingHorizontal: 16,
    paddingTop: 14,
    paddingBottom: 10,
    borderBottomWidth: 1,
    borderBottomColor: "rgba(255, 255, 255, 0.06)",
    backgroundColor: "#0d0e12",
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: 12,
  },
  headerTitle: {
    fontSize: 15,
    fontWeight: "700",
    color: "#f8fafc",
    letterSpacing: -0.2,
  },
  headerSubtitle: {
    fontSize: 11,
    color: "#94a3b8",
    marginTop: 1,
  },
  headerActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  iconButton: {
    padding: 6,
    borderRadius: 6,
    backgroundColor: "rgba(255, 255, 255, 0.04)",
  },
  sortToggleText: {
    fontSize: 11,
    color: "#94a3b8",
  },
  scopeTabs: {
    flexDirection: "row",
    backgroundColor: "rgba(255, 255, 255, 0.04)",
    borderRadius: 8,
    padding: 3,
    marginBottom: 12,
  },
  scopeTabItem: {
    flex: 1,
    paddingVertical: 6,
    alignItems: "center",
    borderRadius: 6,
  },
  scopeTabItemActive: {
    backgroundColor: "rgba(255, 255, 255, 0.12)",
  },
  scopeTabText: {
    fontSize: 12,
    fontWeight: "500",
    color: "#94a3b8",
  },
  scopeTabTextActive: {
    color: "#ffffff",
    fontWeight: "600",
  },
  telemetryRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: 6,
  },
  telemetryLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  telemetryText: {
    fontSize: 11,
    color: "#cbd5e1",
  },
  boldText: {
    fontWeight: "700",
    color: "#f8fafc",
  },
  telemetryRight: {
    fontSize: 11,
    color: "#94a3b8",
  },
  sessionCountRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingTop: 4,
  },
  sessionCountText: {
    fontSize: 11,
    color: "#94a3b8",
  },
  sortSelectorText: {
    fontSize: 11,
    color: "#94a3b8",
  },
});
